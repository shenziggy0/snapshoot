use crate::image_io::cache_file;
use crate::long_screenshot::CaptureRegion;
use crate::windows_list::{list_window_frames, WindowFrame};
use crate::APP;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;
use tauri::Manager;

static RECORD_TARGET: Mutex<Option<CaptureTarget>> = Mutex::new(None);

pub(crate) fn record_capture_target() -> Option<CaptureTarget> {
    RECORD_TARGET.lock().unwrap().clone()
}

static INTENT: Mutex<String> = Mutex::new(String::new());
static EDIT_IMAGE: Mutex<String> = Mutex::new(String::new());
static PREFILLED_SELECTION: Mutex<Option<CaptureRegion>> = Mutex::new(None);

pub fn prepare_screen_capture(intent: &str) {
    if crate::screen_record::is_active() {
        return;
    }
    *INTENT.lock().unwrap() = intent.to_string();
    EDIT_IMAGE.lock().unwrap().clear();
    *PREFILLED_SELECTION.lock().unwrap() = None;
    *RECORD_TARGET.lock().unwrap() = None;
}

pub fn prepare_edit_image(path: &std::path::Path) {
    *INTENT.lock().unwrap() = "edit".to_string();
    *EDIT_IMAGE.lock().unwrap() = path.to_string_lossy().to_string();
    *PREFILLED_SELECTION.lock().unwrap() = None;
    *RECORD_TARGET.lock().unwrap() = None;
}

fn full_screenshot_path() -> std::path::PathBuf {
    cache_file("pot_screenshot.png")
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayBoot {
    pub request_id: u64,
    pub ready: bool,
    pub intent: String,
    pub image_path: Option<String>,
    pub error: Option<String>,
    pub display: Option<CaptureDisplay>,
    pub selection: Option<CaptureRegion>,
}

#[derive(Clone, Debug)]
pub struct CaptureTarget {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    #[cfg(target_os = "macos")]
    pub display_id: u32,
    pub scale: f64,
    identity: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CaptureDisplay {
    id: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    scale: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SavedCaptureRegion {
    display: CaptureDisplay,
    pub pixels: CaptureRegion,
}

impl SavedCaptureRegion {
    pub fn valid_for_image(&self, width: u32, height: u32) -> bool {
        let rect = self.pixels;
        width == rect.width
            && height == rect.height
            && width > 0
            && height > 0
            && rect
                .x
                .checked_add(width)
                .is_some_and(|x| x <= self.display.width)
            && rect
                .y
                .checked_add(height)
                .is_some_and(|y| y <= self.display.height)
            && self.display.scale.is_finite()
            && self.display.scale > 0.0
            && !self.display.id.is_empty()
    }
}

impl CaptureTarget {
    fn display(&self) -> Option<CaptureDisplay> {
        Some(CaptureDisplay {
            id: self.identity.clone()?,
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
            scale: self.scale,
        })
    }
}

#[cfg(not(target_os = "macos"))]
fn capture_target_from_monitor(monitor: &tauri::Monitor) -> CaptureTarget {
    let position = monitor.position();
    let size = monitor.size();
    CaptureTarget {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
        scale: monitor.scale_factor(),
        identity: monitor
            .name()
            .and_then(|name| platform_display_identity(name)),
    }
}

#[cfg(windows)]
fn platform_display_identity(name: &str) -> Option<String> {
    use windows::core::PCWSTR;
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayDevicesW, DISPLAY_DEVICEW, DISPLAY_DEVICE_ACTIVE,
    };
    use windows::Win32::UI::WindowsAndMessaging::EDD_GET_DEVICE_INTERFACE_NAME;

    let wide_name = name.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let mut identities = Vec::new();
    for index in 0.. {
        let mut device = DISPLAY_DEVICEW {
            cb: std::mem::size_of::<DISPLAY_DEVICEW>() as u32,
            ..Default::default()
        };
        if !unsafe {
            EnumDisplayDevicesW(
                PCWSTR(wide_name.as_ptr()),
                index,
                &mut device,
                EDD_GET_DEVICE_INTERFACE_NAME,
            )
        }
        .as_bool()
        {
            break;
        }
        if device.StateFlags & DISPLAY_DEVICE_ACTIVE != 0 {
            let length = device
                .DeviceID
                .iter()
                .position(|value| *value == 0)
                .unwrap_or(device.DeviceID.len());
            if length > 0 {
                identities.push(String::from_utf16_lossy(&device.DeviceID[..length]));
            }
        }
    }
    // Display numbers can be reused when the hardware changes. An ambiguous
    // mirror setup therefore has no reusable same-region coordinates.
    if identities.len() == 1 {
        identities.pop()
    } else {
        None
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
fn platform_display_identity(name: &str) -> Option<String> {
    Some(name.to_string())
}

#[cfg(target_os = "macos")]
impl From<crate::capture_macos::DisplayCaptureTarget> for CaptureTarget {
    fn from(target: crate::capture_macos::DisplayCaptureTarget) -> Self {
        Self {
            x: target.x,
            y: target.y,
            width: target.width,
            height: target.height,
            display_id: target.id,
            scale: target.scale,
            identity: crate::capture_macos::display_identity(target.id),
        }
    }
}

pub fn capture_target_at_mouse() -> Result<CaptureTarget, String> {
    #[cfg(target_os = "macos")]
    {
        crate::capture_macos::capture_target_at_mouse().map(Into::into)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(capture_target_from_monitor(
            &crate::window::monitor_at_mouse(),
        ))
    }
}

pub(crate) fn resolve_saved_region(region: &SavedCaptureRegion) -> Result<CaptureTarget, String> {
    let unavailable = || "quick_access.recapture_unavailable".to_string();
    #[cfg(target_os = "macos")]
    let targets = crate::capture_macos::capture_targets()
        .map_err(|_| unavailable())?
        .into_iter()
        .map(CaptureTarget::from);
    #[cfg(not(target_os = "macos"))]
    let targets = APP
        .get()
        .and_then(|app| {
            app.get_window("screenshot")
                .or_else(|| app.get_window("config"))
        })
        .ok_or_else(unavailable)?
        .available_monitors()
        .map_err(|_| unavailable())?
        .into_iter()
        .map(|monitor| capture_target_from_monitor(&monitor));
    let mut matches = targets.filter(|target| target.display().as_ref() == Some(&region.display));
    let target = matches.next().ok_or_else(unavailable)?;
    if matches.next().is_some() {
        return Err(unavailable());
    }
    Ok(target)
}

/// Grab one monitor into memory. Long capture calls this many times a second,
/// so it must not encode a PNG on the way through.
pub fn capture_target_region(
    target: &CaptureTarget,
    full_width: u32,
    full_height: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<image::RgbaImage, String> {
    #[cfg(target_os = "macos")]
    {
        crate::capture_macos::capture_region_rgba(
            target.display_id,
            full_width,
            full_height,
            x,
            y,
            width,
            height,
        )
    }
    #[cfg(not(target_os = "macos"))]
    {
        let image = capture_monitor_rgba(target.x, target.y)?;
        if image.width() != full_width || image.height() != full_height {
            return Err("size_changed".into());
        }
        if x.saturating_add(width) > image.width() || y.saturating_add(height) > image.height() {
            return Err("selected region is outside the captured screen".into());
        }
        Ok(image::imageops::crop_imm(&image, x, y, width, height).to_image())
    }
}

#[tauri::command]
pub fn screenshot(x: i32, y: i32) -> Result<(), String> {
    capture_screen_to(x, y, &full_screenshot_path())
}

#[cfg(not(target_os = "macos"))]
fn capture_monitor_rgba(x: i32, y: i32) -> Result<image::RgbaImage, String> {
    use screenshots::Screen;
    let screens = Screen::all().map_err(|err| err.to_string())?;
    let screen = screens.into_iter().find(|screen| {
        let info = screen.display_info;
        info.x == x && info.y == y
    });
    let Some(screen) = screen else {
        return Err(format!("No display at {x},{y}"));
    };
    // Same DPI adjustment as `capture_screen_to`: the process is per-monitor
    // aware, and `screenshots` would otherwise scale the bitmap a second time.
    let image = {
        #[cfg(windows)]
        {
            let mut info = screen.display_info;
            let scale = info.scale_factor;
            if scale > 1.01 {
                info.width = ((info.width as f32) / scale).round().max(1.0) as u32;
                info.height = ((info.height as f32) / scale).round().max(1.0) as u32;
            }
            screenshots::Screen::new(&info)
                .capture()
                .map_err(|err| err.to_string())?
        }
        #[cfg(not(windows))]
        {
            screen.capture().map_err(|err| err.to_string())?
        }
    };
    image::RgbaImage::from_raw(image.width(), image.height(), image.rgba().clone())
        .ok_or_else(|| "rgba buffer".to_string())
}

fn capture_screen_to(x: i32, y: i32, path: &Path) -> Result<(), String> {
    use screenshots::{Compression, Screen};
    info!("Screenshot screen with position: x={}, y={}", x, y);
    let screens = Screen::all().map_err(|err| err.to_string())?;
    let screen = screens.into_iter().find(|screen| {
        let info = screen.display_info;
        info.x == x && info.y == y
    });
    let Some(screen) = screen else {
        return Err(format!("No display at {x},{y}"));
    };
    // The process is per-monitor DPI aware, so this rect is already in physical
    // pixels. screenshots multiplies by scale_factor again and the bitmap grows
    // past the monitor, leaving a black margin in the overlay.
    let image = {
        #[cfg(windows)]
        {
            let mut info = screen.display_info;
            let scale = info.scale_factor;
            if scale > 1.01 {
                info.width = ((info.width as f32) / scale).round().max(1.0) as u32;
                info.height = ((info.height as f32) / scale).round().max(1.0) as u32;
            }
            screenshots::Screen::new(&info)
                .capture()
                .map_err(|err| err.to_string())?
        }
        #[cfg(not(windows))]
        {
            screen.capture().map_err(|err| err.to_string())?
        }
    };
    let buffer = image
        .to_png(Compression::Fast)
        .map_err(|err| err.to_string())?;
    std::fs::write(path, buffer).map_err(|err| format!("write screenshot: {err}"))?;
    Ok(())
}

/// Grab the monitor under the cursor. The overlay window must already be
/// ordered out; this does not create or show one.
pub fn capture_for_overlay() -> Result<(), String> {
    capture_overlay_region(None)
}

pub(crate) fn capture_overlay_region(region: Option<SavedCaptureRegion>) -> Result<(), String> {
    let target = match region.as_ref() {
        Some(saved) => resolve_saved_region(saved)?,
        None => capture_target_at_mouse()?,
    };
    let path = full_screenshot_path();
    let _ = std::fs::remove_file(&path);
    #[cfg(target_os = "macos")]
    crate::capture_macos::capture_to(target.display_id, &path)?;
    #[cfg(not(target_os = "macos"))]
    {
        screenshot(target.x, target.y)?;
        reject_blank(&path)?;
    }
    if let Some(saved) = region {
        if image::image_dimensions(&path).ok() != Some((target.width, target.height)) {
            return Err("quick_access.recapture_unavailable".into());
        }
        *PREFILLED_SELECTION.lock().unwrap() = Some(saved.pixels);
    }
    *RECORD_TARGET.lock().unwrap() = Some(target);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn reject_blank(path: &Path) -> Result<(), String> {
    let image = image::open(path).map_err(|err| format!("decode capture: {err}"))?;
    let rgba = image.to_rgba8();
    if rgba_is_blank(rgba.as_raw(), rgba.width(), rgba.height()) {
        let _ = std::fs::remove_file(path);
        return Err("capture is black".into());
    }
    Ok(())
}

fn sample_offset(rgba: &[u8], width: u32, height: u32, gx: usize, gy: usize) -> Option<usize> {
    if width == 0 || height == 0 {
        return None;
    }
    let needed = (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(4);
    if rgba.len() < needed {
        return None;
    }
    let x = if width <= 1 {
        0
    } else {
        (gx as u64 * u64::from(width - 1) / 7) as usize
    };
    let y = if height <= 1 {
        0
    } else {
        (gy as u64 * u64::from(height - 1) / 7) as usize
    };
    Some((y * width as usize + x) * 4)
}

pub(crate) fn rgba_is_blank(rgba: &[u8], width: u32, height: u32) -> bool {
    for gy in 0..8 {
        for gx in 0..8 {
            let Some(offset) = sample_offset(rgba, width, height, gx, gy) else {
                return true;
            };
            if rgba[offset] > 12 || rgba[offset + 1] > 12 || rgba[offset + 2] > 12 {
                return false;
            }
        }
    }
    true
}

/// Difference between the brightest and darkest sampled channel.
/// A photo of the desktop is well above a flat window-background fill.
pub(crate) fn rgba_spread(rgba: &[u8], width: u32, height: u32) -> u8 {
    let mut low = 255u8;
    let mut high = 0u8;
    for gy in 0..8 {
        for gx in 0..8 {
            let Some(offset) = sample_offset(rgba, width, height, gx, gy) else {
                return 0;
            };
            for channel in 0..3 {
                low = low.min(rgba[offset + channel]);
                high = high.max(rgba[offset + channel]);
            }
        }
    }
    high.saturating_sub(low)
}

pub fn report_capture_failure(detail: &str) {
    warn!("screen capture failed: {detail}");
    let app = APP.get().unwrap();
    let parent = app.get_window("config").or_else(|| app.get_window("main"));
    let text = if detail == "quick_access.recapture_unavailable" {
        "显示器或缩放已变化，请重新框选截图。".to_string()
    } else if cfg!(target_os = "macos") {
        "没有获取到桌面画面。请打开「系统设置 → 隐私与安全性 → 屏幕录制」，允许 Snapshoot，然后完全退出并重新打开。"
            .to_string()
    } else {
        format!("截图失败：{detail}")
    };
    tauri::api::dialog::message(parent.as_ref(), "无法截取屏幕", text);
}

/// One small handshake replaces the old intent + whole-PNG-over-IPC pair.
/// The page loads the PNG directly through the asset protocol, so there is no
/// Base64 copy, no extra image decode on the Rust main thread, and no large
/// JSON payload in the latency-sensitive path.
#[tauri::command]
pub fn screenshot_boot() -> OverlayBoot {
    let (request_id, ready) = crate::window::overlay_boot_state();
    let intent = {
        let current = INTENT.lock().unwrap().clone();
        if current.is_empty() {
            "annotate".to_string()
        } else {
            current
        }
    };
    if request_id == 0 || !ready {
        return OverlayBoot {
            request_id,
            ready: false,
            intent,
            image_path: None,
            error: None,
            display: None,
            selection: None,
        };
    }

    let edit = EDIT_IMAGE.lock().unwrap().clone();
    let path = if edit.is_empty() {
        full_screenshot_path()
    } else {
        std::path::PathBuf::from(edit)
    };
    let error = match std::fs::metadata(&path) {
        Ok(metadata) if metadata.len() >= 32 => None,
        Ok(_) => Some("capture file is empty".to_string()),
        Err(err) => Some(format!("read capture {}: {err}", path.display())),
    };
    OverlayBoot {
        request_id,
        ready: true,
        intent,
        image_path: error.is_none().then(|| path.to_string_lossy().to_string()),
        error,
        display: record_capture_target().and_then(|target| target.display()),
        selection: *PREFILLED_SELECTION.lock().unwrap(),
    }
}

#[tauri::command]
pub fn list_capture_windows() -> Vec<WindowFrame> {
    let frames = list_window_frames();
    if frames.is_empty() {
        warn!("No top-level windows reported for capture hover");
    }
    frames
}

#[cfg(test)]
mod tests {
    use super::rgba_is_blank;

    #[test]
    fn blank_frame_is_rejected_and_a_gray_pixel_is_kept() {
        assert!(rgba_is_blank(&[0, 0, 0, 255], 1, 1));
        assert!(!rgba_is_blank(&[45, 45, 49, 255], 1, 1));
        assert!(super::rgba_spread(&[45, 45, 49, 255], 1, 1) < 8);
        assert!(super::rgba_spread(&[0, 0, 0, 255, 200, 200, 200, 255], 2, 1) >= 8);
    }
}
