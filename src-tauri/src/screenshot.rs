use crate::image_io::cache_file;
use crate::windows_list::{list_window_frames, WindowFrame};
use crate::APP;
use base64::{engine::general_purpose, Engine as _};
use image::GenericImageView;
use log::{info, warn};
use serde::Serialize;
use std::path::Path;
use std::sync::Mutex;
use tauri::Manager;

static INTENT: Mutex<String> = Mutex::new(String::new());
static EDIT_IMAGE: Mutex<String> = Mutex::new(String::new());

pub fn prepare_screen_capture(intent: &str) {
    *INTENT.lock().unwrap() = intent.to_string();
    EDIT_IMAGE.lock().unwrap().clear();
}

pub fn prepare_edit_image(path: &std::path::Path) {
    *INTENT.lock().unwrap() = "edit".to_string();
    *EDIT_IMAGE.lock().unwrap() = path.to_string_lossy().to_string();
}

fn full_screenshot_path() -> std::path::PathBuf {
    cache_file("pot_screenshot.png")
}

#[derive(Serialize)]
pub struct CapturedImage {
    pub data: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug)]
pub struct CaptureTarget {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    #[cfg(target_os = "macos")]
    pub display_id: u32,
    #[cfg(target_os = "macos")]
    pub scale: f64,
}

pub fn capture_target_at_mouse() -> Result<CaptureTarget, String> {
    #[cfg(target_os = "macos")]
    {
        let target = crate::capture_macos::capture_target_at_mouse()?;
        return Ok(CaptureTarget {
            x: target.x,
            y: target.y,
            width: target.width,
            height: target.height,
            display_id: target.id,
            scale: target.scale,
        });
    }
    #[cfg(not(target_os = "macos"))]
    {
        let monitor = crate::window::monitor_at_mouse();
        let position = monitor.position();
        let size = monitor.size();
        Ok(CaptureTarget {
            x: position.x,
            y: position.y,
            width: size.width,
            height: size.height,
        })
    }
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
        return crate::capture_macos::capture_region_rgba(
            target.display_id,
            full_width,
            full_height,
            x,
            y,
            width,
            height,
        );
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

pub fn capture_target_to(target: &CaptureTarget, path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        crate::capture_macos::capture_id_to(target.display_id, path)?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        capture_screen_to(target.x, target.y, path)?;
        reject_blank(path)
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
    let path = full_screenshot_path();
    let _ = std::fs::remove_file(&path);
    #[cfg(target_os = "macos")]
    {
        return crate::capture_macos::capture_to(&path);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let monitor = crate::window::monitor_at_mouse();
        let pos = *monitor.position();
        screenshot(pos.x, pos.y)?;
        reject_blank(&path)
    }
}

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
    let text = if cfg!(target_os = "macos") {
        "没有获取到桌面画面。请打开「系统设置 → 隐私与安全性 → 屏幕录制」，允许 Snapshoot，然后完全退出并重新打开。"
            .to_string()
    } else {
        format!("截图失败：{detail}")
    };
    tauri::api::dialog::message(parent.as_ref(), "无法截取屏幕", text);
}

#[tauri::command]
pub fn screenshot_intent() -> String {
    let intent = INTENT.lock().unwrap().clone();
    if intent.is_empty() {
        "annotate".to_string()
    } else {
        intent
    }
}

#[tauri::command]
pub fn read_capture_image() -> Result<CapturedImage, String> {
    let edit = EDIT_IMAGE.lock().unwrap().clone();
    let path = if edit.is_empty() {
        full_screenshot_path()
    } else {
        std::path::PathBuf::from(edit)
    };
    let bytes =
        std::fs::read(&path).map_err(|err| format!("read capture {}: {err}", path.display()))?;
    if bytes.len() < 32 {
        return Err("capture file is empty".into());
    }
    let image = image::load_from_memory(&bytes).map_err(|err| format!("decode capture: {err}"))?;
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return Err("capture has no pixels".into());
    }
    Ok(CapturedImage {
        data: general_purpose::STANDARD.encode(bytes),
        width,
        height,
    })
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
