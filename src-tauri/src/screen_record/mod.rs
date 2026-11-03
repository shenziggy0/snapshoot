use crate::long_screenshot::CaptureRegion;
use crate::screenshot::CaptureTarget;
use crate::APP;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tauri::{Manager, State};

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows as platform;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as platform;

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod platform {
    use super::{RecordOptions, RecordResult, RecordStatus};
    use std::path::Path;
    pub struct Recorder;
    pub fn supported() -> bool { false }
    fn unsupported<T>() -> Result<T, String> { Err("screen_record.unsupported".into()) }
    impl Recorder {
        pub fn start(_: RecordOptions) -> Result<Self, String> { unsupported() }
        pub fn status(&self) -> RecordStatus { unreachable!("unsupported recorder") }
        pub fn pause(&mut self) -> Result<(), String> { unsupported() }
        pub fn resume(&mut self) -> Result<(), String> { unsupported() }
        pub fn stop(&mut self) -> Result<RecordResult, String> { unsupported() }
        pub fn cancel(&mut self) -> Result<(), String> { unsupported() }
    }
    pub fn copy_file(_: &Path) -> Result<(), String> { unsupported() }
    pub fn reveal(_: &Path) -> Result<(), String> { unsupported() }
}

pub(super) const ENCODER_FAILED: &str = "screen_record.encoder_failed";
pub(super) const DISK_FAILED: &str = "screen_record.disk_write_failed";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RecordState { Recording, Paused, Finalizing, Error }

#[derive(Clone, Debug, Serialize)]
pub struct RecordStatus {
    pub state: RecordState,
    pub elapsed_ms: u64,
    pub width: u32,
    pub height: u32,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RecordResult {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub duration_ms: u64,
    pub size_bytes: u64,
    pub clipboard_copied: bool,
}

#[derive(Serialize)]
pub struct RecordSupport { pub supported: bool, pub system_audio: bool }

#[derive(Clone)]
pub(super) struct RecordOptions {
    pub target: CaptureTarget,
    pub region: CaptureRegion,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate: u32,
    pub cursor: bool,
    pub audio: bool,
    pub path: PathBuf,
}

#[derive(Default)]
pub struct ScreenRecordManager {
    active: Mutex<Option<platform::Recorder>>,
    last: Mutex<Option<RecordResult>>,
    generation: AtomicU64,
    closing: AtomicBool,
}

pub fn is_active() -> bool {
    APP.get().map(|app| {
        let manager = app.state::<ScreenRecordManager>();
        manager.closing.load(Ordering::SeqCst) || manager.active.lock().unwrap().is_some()
    }).unwrap_or(false)
}

pub(crate) fn finish_overlay_transition() {
    if let Some(app) = APP.get() {
        app.state::<ScreenRecordManager>().closing.store(false, Ordering::SeqCst);
    }
}

// Destruction callbacks must never join a capture worker on the event loop.
pub fn abandon_active() {
    if let Some(app) = APP.get() {
        let generation = app.state::<ScreenRecordManager>().generation.load(Ordering::SeqCst);
        let app = app.clone();
        std::thread::spawn(move || { let _ = app.state::<ScreenRecordManager>().cancel_generation(Some(generation)); });
    }
}

fn config_string(key: &str, default: &str) -> String {
    crate::config::get(key).and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_else(|| default.into())
}

impl ScreenRecordManager {
    fn start(&self, region: CaptureRegion) -> Result<RecordStatus, String> {
        let mut active = self.active.lock().unwrap();
        if self.closing.load(Ordering::SeqCst) || active.is_some() || crate::long_screenshot::is_active() {
            return Err("screen_record.already_running".into());
        }
        if !platform::supported() { return Err("screen_record.unsupported".into()); }
        let target = crate::screenshot::record_capture_target().ok_or(ENCODER_FAILED)?;
        if region.x.checked_add(region.width).map_or(true, |v| v > target.width)
            || region.y.checked_add(region.height).map_or(true, |v| v > target.height) {
            return Err("screen_record.invalid_region".into());
        }
        let (width, height) = compute_output_size(region.width, region.height, &config_string("record_resolution", "original"));
        if width == 0 || height == 0 { return Err("screen_record.invalid_region".into()); }
        let fps = crate::config::get("record_fps").and_then(|v| v.as_u64()).filter(|v| [15, 30, 60].contains(v)).unwrap_or(30) as u32;
        let directory = config_string("record_save_dir", "");
        let directory = if directory.is_empty() {
            dirs::video_dir().ok_or(DISK_FAILED)?.join("Snapshoot")
        } else { PathBuf::from(directory) };
        std::fs::create_dir_all(&directory).map_err(|_| DISK_FAILED)?;
        let stem = format!("Snapshoot_{}", chrono::Local::now().format("%Y%m%d_%H%M%S"));
        let mut path = directory.join(format!("{stem}.mp4"));
        // Reserve the name: rapid recordings must not overwrite an earlier result.
        for suffix in 0..1000 {
            if suffix > 0 { path = directory.join(format!("{stem}_{suffix}.mp4")); }
            match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(_) => break,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && suffix < 999 => continue,
                Err(_) => return Err(DISK_FAILED.into()),
            }
        }
        let options = RecordOptions {
            target, region, width, height, fps,
            bitrate: compute_bitrate(width, height, fps, &config_string("record_quality", "standard")),
            cursor: crate::config::get("record_cursor").and_then(|v| v.as_bool()).unwrap_or(true),
            audio: crate::config::get("record_system_audio").and_then(|v| v.as_bool()).unwrap_or(false),
            path: path.clone(),
        };
        if let Err(err) = crate::window::enter_record_mode() {
            let _ = std::fs::remove_file(&path);
            log::warn!("record overlay: {err}");
            return Err(ENCODER_FAILED.into());
        }
        match platform::Recorder::start(options) {
            Ok(recorder) => {
                let status = recorder.status();
                self.generation.fetch_add(1, Ordering::SeqCst);
                *active = Some(recorder);
                // Close the previous result so its delayed timer or delete
                // action cannot race the result of this new recording.
                if let Some(window) = APP.get().and_then(|app| app.get_window("record_result")) {
                    let _ = window.close();
                }
                crate::tray::set_recording(true);
                Ok(status)
            }
            Err(err) => {
                self.closing.store(true, Ordering::SeqCst);
                crate::window::leave_record_mode(false);
                let _ = std::fs::remove_file(&path);
                Err(err)
            }
        }
    }

    fn status(&self) -> Result<RecordStatus, String> {
        self.active.lock().unwrap().as_ref().map(|r| r.status()).ok_or_else(|| "screen_record.not_running".into())
    }

    fn pause(&self, paused: bool) -> Result<RecordStatus, String> {
        let mut active = self.active.lock().unwrap();
        let recorder = active.as_mut().ok_or("screen_record.not_running")?;
        if paused { recorder.pause()?; } else { recorder.resume()?; }
        Ok(recorder.status())
    }

    fn stop(&self) -> Result<RecordResult, String> {
        let mut active = self.active.lock().unwrap();
        let recorder = active.as_mut().ok_or("screen_record.not_running")?;
        let result = recorder.stop();
        self.closing.store(true, Ordering::SeqCst);
        *active = None;
        drop(active);
        crate::tray::set_recording(false);
        let mut result = match result {
            Ok(result) => { crate::window::leave_record_mode(true); result }
            Err(err) => { crate::window::leave_record_mode(false); return Err(err); }
        };
        // Saving succeeds independently of a clipboard locked by another process.
        let copied = platform::copy_file(std::path::Path::new(&result.path));
        if let Err(err) = &copied { log::warn!("record clipboard: {err}"); }
        let clipboard_ok = copied.is_ok();
        result.clipboard_copied = clipboard_ok;
        *self.last.lock().unwrap() = Some(result.clone());
        let _ = crate::image_io::spawn_main(move || crate::window::record_result_window(clipboard_ok));
        Ok(result)
    }

    fn cancel(&self) -> Result<(), String> {
        self.cancel_generation(None)
    }

    fn cancel_generation(&self, generation: Option<u64>) -> Result<(), String> {
        let mut active = self.active.lock().unwrap();
        if generation.is_some_and(|g| g != self.generation.load(Ordering::SeqCst)) { return Ok(()); }
        if let Some(mut recorder) = active.take() {
            let result = recorder.cancel();
            self.closing.store(true, Ordering::SeqCst);
            drop(active);
            crate::tray::set_recording(false);
            crate::window::leave_record_mode(true);
            return result;
        }
        Ok(())
    }
}

pub fn screen_record_hotkey() {
    if APP.get().is_some_and(|app| app.state::<ScreenRecordManager>().closing.load(Ordering::SeqCst)) { return; }
    if is_active() {
        std::thread::spawn(|| {
            if let Some(app) = APP.get() {
                if let Err(err) = app.state::<ScreenRecordManager>().stop() {
                    log::warn!("record hotkey stop: {err}");
                    let _ = app.emit_all("screen-record-error", err);
                }
            }
        });
    } else if platform::supported() && !crate::long_screenshot::is_active() {
        crate::screenshot::prepare_screen_capture("record");
        crate::window::open_frozen_overlay();
    }
}

#[tauri::command]
pub fn screen_record_support() -> RecordSupport {
    let supported = platform::supported();
    #[cfg(target_os = "macos")]
    let system_audio = supported && macos::audio_supported();
    #[cfg(not(target_os = "macos"))]
    let system_audio = supported;
    RecordSupport { supported, system_audio }
}

#[tauri::command]
pub fn screen_record_start(region: CaptureRegion, manager: State<'_, ScreenRecordManager>) -> Result<RecordStatus, String> { manager.start(region) }
#[tauri::command(async)]
pub fn screen_record_status(manager: State<'_, ScreenRecordManager>) -> Result<RecordStatus, String> { manager.status() }
#[tauri::command(async)]
pub fn screen_record_pause(manager: State<'_, ScreenRecordManager>) -> Result<RecordStatus, String> { manager.pause(true) }
#[tauri::command(async)]
pub fn screen_record_resume(manager: State<'_, ScreenRecordManager>) -> Result<RecordStatus, String> { manager.pause(false) }
#[tauri::command(async)]
pub fn screen_record_stop(manager: State<'_, ScreenRecordManager>) -> Result<RecordResult, String> { manager.stop() }
#[tauri::command(async)]
pub fn screen_record_cancel(manager: State<'_, ScreenRecordManager>) -> Result<(), String> { manager.cancel() }
#[tauri::command]
pub fn screen_record_last_result(manager: State<'_, ScreenRecordManager>) -> Option<RecordResult> { manager.last.lock().unwrap().clone() }
#[tauri::command]
pub fn screen_record_reveal_last(manager: State<'_, ScreenRecordManager>) -> Result<(), String> {
    let last = manager.last.lock().unwrap();
    let result = last.as_ref().ok_or("screen_record.no_result")?;
    platform::reveal(std::path::Path::new(&result.path))
}
#[tauri::command]
pub fn screen_record_delete_last(manager: State<'_, ScreenRecordManager>) -> Result<(), String> {
    let mut last = manager.last.lock().unwrap();
    if let Some(result) = last.as_ref() {
        match std::fs::remove_file(&result.path) {
            Ok(()) => {},
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
            Err(_) => return Err(DISK_FAILED.into()),
        }
    }
    *last = None;
    Ok(())
}

pub fn compute_output_size(width: u32, height: u32, resolution: &str) -> (u32, u32) {
    if width < 2 || height < 2 {
        return (0, 0);
    }
    let (long, short) = match resolution {
        "1080p" => (1920.0, 1080.0),
        "720p" => (1280.0, 720.0),
        _ => (4096.0, 4096.0),
    };
    let scale = (long / width.max(height) as f64)
        .min(short / width.min(height) as f64).min(1.0);
    let result = ((width as f64 * scale) as u32 & !1, (height as f64 * scale) as u32 & !1);
    if result.0 < 2 || result.1 < 2 { (0, 0) } else { result }
}

pub fn compute_bitrate(width: u32, height: u32, fps: u32, quality: &str) -> u32 {
    let percent = match quality { "high" => 15, "small" => 4, _ => 8 };
    (u64::from(width) * u64::from(height) * u64::from(fps) * percent / 100)
        .min(u64::from(u32::MAX)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_output_size_cases() {
        assert_eq!(compute_output_size(3001, 1689, "1080p"), (1918, 1080));
        assert_eq!(compute_output_size(5120, 2880, "original"), (4096, 2304));
        assert_eq!(compute_output_size(1689, 3001, "1080p"), (1080, 1918));
        assert_eq!(compute_output_size(800, 500, "720p"), (800, 500));
        assert_eq!(compute_output_size(801, 501, "original"), (800, 500));
        assert_eq!(compute_output_size(0, 500, "original"), (0, 0));
        assert_eq!(compute_output_size(1, 500, "original"), (0, 0));
    }

    #[test]
    fn compute_bitrate_cases() {
        assert_eq!(compute_bitrate(1920, 1080, 30, "standard"), 4_976_640);
        assert_eq!(compute_bitrate(1920, 1080, 30, "high"), 9_331_200);
        assert_eq!(compute_bitrate(1920, 1080, 30, "small"), 2_488_320);
        assert_eq!(compute_bitrate(800, 500, 30, "standard"), 960_000);
    }
}
