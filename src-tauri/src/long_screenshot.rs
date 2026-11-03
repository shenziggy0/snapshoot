use crate::image_io::{
    cache_file, clipboard_on_main, copy_to_clipboard, cut_image_path, write_png,
};
use crate::screenshot::{capture_target_at_mouse, CaptureTarget};
use crate::APP;
use image::{imageops, RgbaImage};
use log::{info, warn};
use long_stitch::{integrate, AppendResult, StitchedImage};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

const MIN_REGION_WIDTH: u32 = 16;
const MIN_REGION_HEIGHT: u32 = 32;
// Capture while the gesture is still moving. Waiting until the wheel goes
// quiet keeps only the first and last frame, and a flick between them no
// longer overlaps.
const CAPTURE_INTERVAL: Duration = Duration::from_millis(32);

const MSG_CAPTURE_FAILED: &str = "long_screenshot.capture_failed";
const MSG_SIZE_CHANGED: &str = "long_screenshot.size_changed";
const MSG_NO_OVERLAP: &str = "long_screenshot.no_overlap";
const MSG_AMBIGUOUS: &str = "long_screenshot.ambiguous";
const MSG_LIMIT: &str = "long_screenshot.limit";

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct CaptureRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LongCaptureState {
    Capturing,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LongCaptureStatus {
    pub state: LongCaptureState,
    pub width: u32,
    pub height: u32,
    pub message: Option<String>,
}

impl LongCaptureStatus {
    fn capturing(width: u32, height: u32) -> Self {
        Self {
            state: LongCaptureState::Capturing,
            width,
            height,
            message: None,
        }
    }
}

struct SharedSession {
    target: CaptureTarget,
    region: CaptureRegion,
    full_width: u32,
    full_height: u32,
    stop: AtomicBool,
    capture_requested: Mutex<u64>,
    capture_signal: Condvar,
    output: Mutex<StitchedImage>,
    status: Mutex<LongCaptureStatus>,
}

struct ActiveSession {
    shared: Arc<SharedSession>,
    worker: JoinHandle<()>,
}

#[derive(Default)]
pub struct LongCaptureManager {
    active: Mutex<Option<ActiveSession>>,
}

pub fn is_active() -> bool {
    APP.get()
        .map(|app| {
            app.state::<LongCaptureManager>()
                .active
                .lock()
                .unwrap()
                .is_some()
        })
        .unwrap_or(false)
}

fn crop_region(image: &RgbaImage, region: CaptureRegion) -> Result<RgbaImage, String> {
    if region.width < MIN_REGION_WIDTH || region.height < MIN_REGION_HEIGHT {
        return Err("selected region is too small".into());
    }
    let (width, height) = image.dimensions();
    let right = region.x.checked_add(region.width);
    let bottom = region.y.checked_add(region.height);
    if right.map_or(true, |value| value > width) || bottom.map_or(true, |value| value > height) {
        return Err("selected region is outside the captured screen".into());
    }
    Ok(imageops::crop_imm(image, region.x, region.y, region.width, region.height).to_image())
}

fn publish_status(app: &AppHandle, shared: &SharedSession, next: LongCaptureStatus) {
    let mut status = shared.status.lock().unwrap();
    if *status == next {
        return;
    }
    *status = next.clone();
    drop(status);
    if let Err(err) = app.emit_all("long-capture-status", next) {
        warn!("long capture status emit failed: {err}");
    }
}

fn error_status(shared: &SharedSession, message: &str) -> LongCaptureStatus {
    let status = shared.status.lock().unwrap();
    LongCaptureStatus {
        state: LongCaptureState::Error,
        width: status.width,
        height: status.height,
        message: Some(message.into()),
    }
}

fn process_frame(app: &AppHandle, shared: &Arc<SharedSession>) {
    let region = shared.region;
    let captured = crate::screenshot::capture_target_region(
        &shared.target,
        shared.full_width,
        shared.full_height,
        region.x,
        region.y,
        region.width,
        region.height,
    );
    let frame = match captured {
        Ok(frame) => frame,
        Err(err) => {
            if err == "size_changed" {
                publish_status(app, shared, error_status(shared, MSG_SIZE_CHANGED));
            } else {
                warn!("long capture frame failed: {err}");
                publish_status(app, shared, error_status(shared, MSG_CAPTURE_FAILED));
            }
            return;
        }
    };
    if frame.width() != region.width || frame.height() != region.height {
        publish_status(app, shared, error_status(shared, MSG_SIZE_CHANGED));
        return;
    }

    let (result, width, height) = {
        let mut output = shared.output.lock().unwrap();
        let result = integrate(&mut output, &frame);
        (result, output.width, output.height)
    };
    match result {
        AppendResult::Appended(_) => {
            publish_status(app, shared, LongCaptureStatus::capturing(width, height));
        }
        AppendResult::Unchanged => {
            // A single missed grab should not leave the hint stuck on an error
            // once frames are matching again.
            let status = shared.status.lock().unwrap().clone();
            if status.state == LongCaptureState::Error
                && status.message.as_deref() == Some(MSG_CAPTURE_FAILED)
            {
                publish_status(app, shared, LongCaptureStatus::capturing(width, height));
            }
        }
        // A frame captured during a compositor transition can be unrelated
        // for one cycle. Keep the current stitched image usable and wait for
        // the next settled frame instead of poisoning the whole session.
        AppendResult::NoMatch | AppendResult::Ambiguous => {}
        AppendResult::Limit => {
            publish_status(
                app,
                shared,
                LongCaptureStatus {
                    state: LongCaptureState::Error,
                    width,
                    height,
                    message: Some(MSG_LIMIT.into()),
                },
            );
        }
    }
}

fn remove_frame_cache() {
    let path = cache_file("pot_long_screenshot_frame.png");
    let _ = std::fs::remove_file(path);
}

fn stop_worker(shared: &SharedSession) {
    shared.stop.store(true, Ordering::SeqCst);
    shared.capture_signal.notify_all();
}

fn spawn_worker(app: AppHandle, shared: Arc<SharedSession>) -> JoinHandle<()> {
    thread::spawn(move || {
        // Grab the live page continuously. Scroll events only wake the loop
        // early; they are not required, because a click-through overlay lets
        // the page move without an IPC round trip.
        while !shared.stop.load(Ordering::SeqCst) {
            let started = std::time::Instant::now();
            process_frame(&app, &shared);
            if shared.stop.load(Ordering::SeqCst) {
                break;
            }
            let pause = CAPTURE_INTERVAL.saturating_sub(started.elapsed());
            if pause.is_zero() {
                continue;
            }
            let guard = shared.capture_requested.lock().unwrap();
            let (guard, _) = shared.capture_signal.wait_timeout(guard, pause).unwrap();
            drop(guard);
        }
    })
}

/// Drop the running session without waiting for the worker. Used when the
/// screenshot window disappears behind our back or scroll forwarding fails on
/// the main thread, where joining could otherwise block the UI.
pub fn abandon_active() {
    let Some(app) = APP.get() else {
        return;
    };
    let manager = app.state::<LongCaptureManager>();
    let active = manager.active.lock().unwrap().take();
    if let Some(active) = active {
        stop_worker(&active.shared);
        crate::window::leave_long_screenshot_mode();
        thread::spawn(move || {
            let _ = active.worker.join();
            remove_frame_cache();
        });
    }
}

impl LongCaptureManager {
    pub fn start(&self, region: CaptureRegion) -> Result<LongCaptureStatus, String> {
        if self.active.lock().unwrap().is_some() {
            return Err("a long capture is already running".into());
        }
        let source_path = cache_file("pot_screenshot.png");
        remove_frame_cache();
        let source = image::open(&source_path)
            .map_err(|err| format!("open initial screenshot: {err}"))?
            .to_rgba8();
        let first = crop_region(&source, region)?;
        let target = capture_target_at_mouse()?;
        let status = LongCaptureStatus::capturing(first.width(), first.height());
        let shared = Arc::new(SharedSession {
            target,
            region,
            full_width: source.width(),
            full_height: source.height(),
            stop: AtomicBool::new(false),
            capture_requested: Mutex::new(0),
            capture_signal: Condvar::new(),
            output: Mutex::new(StitchedImage::from_frame(&first)?),
            status: Mutex::new(status.clone()),
        });

        // Keep the original overlay visible, exclude it from later grabs, and
        // let wheel events fall through to the window under the selection.
        crate::window::enter_long_screenshot_mode()?;
        let app = APP.get().ok_or_else(|| "app is not ready".to_string())?;
        let worker = spawn_worker(app.clone(), shared.clone());
        info!(
            "long capture started: monitor {},{} {}x{}, region {},{} {}x{}",
            shared.target.x,
            shared.target.y,
            shared.target.width,
            shared.target.height,
            region.x,
            region.y,
            region.width,
            region.height
        );
        let mut active = self.active.lock().unwrap();
        if active.is_some() {
            drop(active);
            stop_worker(&shared);
            crate::window::leave_long_screenshot_mode();
            let _ = worker.join();
            remove_frame_cache();
            return Err("a long capture is already running".into());
        }
        *active = Some(ActiveSession { shared, worker });
        Ok(status)
    }

    pub fn status(&self) -> Result<LongCaptureStatus, String> {
        let active = self.active.lock().unwrap();
        let session = active
            .as_ref()
            .ok_or_else(|| "no active long capture".to_string())?;
        let status = session.shared.status.lock().unwrap().clone();
        Ok(status)
    }

    pub fn scroll(&self, delta_y: i32) -> Result<(), String> {
        let shared = {
            let active = self.active.lock().unwrap();
            active
                .as_ref()
                .ok_or_else(|| "no active long capture".to_string())?
                .shared
                .clone()
        };
        if delta_y != 0 {
            // The selection is normally click-through, so the page already
            // received this gesture and forwarding it again would double the
            // distance. Only the events the overlay itself swallowed (the
            // pointer is over the toolbar) are replayed, at the original
            // pixel distance.
            if !crate::window::long_capture_click_through() {
                #[cfg(target_os = "macos")]
                let scale = shared.target.scale.max(0.5);
                #[cfg(not(target_os = "macos"))]
                let scale = 1.0;
                let center_x = (i64::from(shared.target.x)
                    + i64::from(shared.region.x)
                    + i64::from(shared.region.width) / 2) as f64
                    / scale;
                let center_y = (i64::from(shared.target.y)
                    + i64::from(shared.region.y)
                    + i64::from(shared.region.height) / 2) as f64
                    / scale;
                if let Err(err) =
                    crate::window::forward_long_capture_scroll(delta_y, center_x, center_y)
                {
                    warn!("long capture scroll forward failed: {err}");
                }
            }
        }
        let mut requested = shared.capture_requested.lock().unwrap();
        *requested = requested.saturating_add(1);
        drop(requested);
        shared.capture_signal.notify_one();
        Ok(())
    }

    pub fn finish_copy(&self) -> Result<(), String> {
        let shared = self.take_active()?;
        let (width, height, pixels) = {
            let output = shared.output.lock().unwrap();
            (output.width, output.height, output.pixels.clone())
        };
        let image = RgbaImage::from_raw(width, height, pixels)
            .ok_or_else(|| "long capture buffer is invalid".to_string())?;
        let image = image::DynamicImage::ImageRgba8(image);
        write_png(&cut_image_path(), &image).map_err(|err| err.to_string())?;
        clipboard_on_main(move || copy_to_clipboard(&image).map_err(|err| err.to_string()))?;
        remove_frame_cache();
        crate::window::leave_long_screenshot_mode();
        info!("long capture copied: {}x{}", width, height);
        Ok(())
    }

    pub fn cancel(&self) -> Result<(), String> {
        let active = self.active.lock().unwrap().take();
        if let Some(active) = active {
            stop_worker(&active.shared);
            let _ = active.worker.join();
        }
        remove_frame_cache();
        crate::window::leave_long_screenshot_mode();
        Ok(())
    }

    fn take_active(&self) -> Result<Arc<SharedSession>, String> {
        let active = self
            .active
            .lock()
            .unwrap()
            .take()
            .ok_or_else(|| "no active long capture".to_string())?;
        stop_worker(&active.shared);
        active
            .worker
            .join()
            .map_err(|_| "long capture worker stopped unexpectedly".to_string())?;
        Ok(active.shared)
    }
}

/// Runs on the main thread: `enter_long_screenshot_mode` performs AppKit
/// calls (exclude from capture) that must not happen on a worker thread.
#[tauri::command]
pub fn long_capture_start(
    region: CaptureRegion,
    manager: State<'_, LongCaptureManager>,
) -> Result<LongCaptureStatus, String> {
    crate::image_io::diag_log(
        "start",
        &format!(
            "region {},{} {}x{}",
            region.x, region.y, region.width, region.height
        ),
    );
    let status = manager.start(region).map_err(|e| {
        crate::image_io::diag_log("start_err", &e);
        e
    })?;
    crate::image_io::diag_log("start_ok", &format!("{}x{}", status.width, status.height));
    Ok(status)
}

#[tauri::command]
pub fn long_capture_status(
    manager: State<'_, LongCaptureManager>,
) -> Result<LongCaptureStatus, String> {
    manager.status()
}

#[tauri::command]
pub fn long_capture_scroll(
    delta_y: i32,
    manager: State<'_, LongCaptureManager>,
) -> Result<(), String> {
    manager.scroll(delta_y)
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct OverlayHitRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// CSS-pixel rectangles (toolbar, hint) that must stay clickable while the
/// rest of the overlay lets scroll events through.
#[tauri::command]
pub fn overlay_set_hit_rects(rects: Vec<OverlayHitRect>) {
    let rects = rects
        .into_iter()
        .map(|rect| (rect.x, rect.y, rect.width, rect.height))
        .collect::<Vec<_>>();
    crate::window::set_overlay_hit_rects(&rects);
}

#[tauri::command(async)]
pub fn long_capture_finish_copy(manager: State<'_, LongCaptureManager>) -> Result<(), String> {
    manager.finish_copy()
}

#[tauri::command(async)]
pub fn long_capture_cancel(manager: State<'_, LongCaptureManager>) -> Result<(), String> {
    manager.cancel()
}
