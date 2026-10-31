use crate::image_io::cache_file;
use crate::windows_list::{list_window_frames, WindowFrame};
use base64::{engine::general_purpose, Engine as _};
use image::GenericImageView;
use log::{info, warn};
use serde::Serialize;
use std::sync::Mutex;

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

#[tauri::command]
pub fn screenshot(x: i32, y: i32) -> Result<(), String> {
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
    let image = screen.capture().map_err(|err| err.to_string())?;
    let buffer = image
        .to_png(Compression::Fast)
        .map_err(|err| err.to_string())?;
    let path = full_screenshot_path();
    std::fs::write(&path, buffer).map_err(|err| format!("write screenshot: {err}"))?;
    Ok(())
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
