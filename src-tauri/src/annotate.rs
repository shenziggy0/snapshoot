use crate::image_io::{
    clear_capture_origin, clipboard_on_main, copy_to_clipboard, cut_image_path,
    decode_base64_image, ensure_png_extension, spawn_main, take_capture_origin, write_png,
};
use crate::window::{image_translate, monitor_at_mouse, new_overlay_builder, recognize_window};
use crate::APP;
use base64::{engine::general_purpose, Engine as _};
use image::GenericImageView;
use log::{info, warn};
use serde::Serialize;
use tauri::{Manager, PhysicalPosition, PhysicalSize};

/// CSS pixels reserved under the image for the annotation toolbar.
/// The editor page uses the same value.
const TOOLBAR_CSS_PX: f64 = 108.0;

pub fn screenshot_annotate() {
    clear_capture_origin();
    crate::screenshot::prepare_screen_capture("annotate");
    crate::window::open_frozen_overlay();
}

pub fn open_annotate_window() {
    let path = cut_image_path();
    let (img_w, img_h) = match image::open(&path) {
        Ok(image) => image.dimensions(),
        Err(err) => {
            warn!("annotate image missing: {}", err);
            return;
        }
    };

    let monitor = monitor_at_mouse();
    let scale = monitor.scale_factor().max(1.0);
    let mon_pos = monitor.position();
    let mon_size = monitor.size();
    let max_w = mon_size.width as f64 * 0.96;
    let max_h = mon_size.height as f64 * 0.96;
    let min_w = (560.0 * scale).min(max_w);
    let min_h = (220.0 * scale).min(max_h);
    let toolbar_phys = TOOLBAR_CSS_PX * scale;

    let win_w = (img_w as f64).max(min_w).min(max_w).round().max(1.0);
    let win_h = (img_h as f64 + toolbar_phys)
        .max(min_h)
        .min(max_h)
        .round()
        .max(1.0);

    let origin = take_capture_origin();
    let (mut x, mut y) = match origin {
        Some((ox, oy)) => {
            let extra_w = (win_w - img_w as f64).max(0.0);
            (ox - (extra_w / 2.0) as i32, oy)
        }
        None => (
            mon_pos.x + ((mon_size.width as f64 - win_w) / 2.0) as i32,
            mon_pos.y + ((mon_size.height as f64 - win_h) / 2.0) as i32,
        ),
    };
    let max_x = mon_pos.x + mon_size.width as i32 - win_w as i32;
    let max_y = mon_pos.y + mon_size.height as i32 - win_h as i32;
    x = x.clamp(mon_pos.x, max_x.max(mon_pos.x));
    y = y.clamp(mon_pos.y, max_y.max(mon_pos.y));

    let app = APP.get().unwrap();
    if let Some(window) = app.get_window("annotate") {
        let _ = window.set_position(PhysicalPosition::new(x, y));
        let _ = window.set_size(PhysicalSize::new(win_w as u32, win_h as u32));
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.emit("annotate-reload", ());
        return;
    }

    let builder = new_overlay_builder("annotate", "Snapshoot");
    let window = match builder.build() {
        Ok(window) => window,
        Err(err) => {
            warn!("create annotate window failed: {}", err);
            return;
        }
    };
    let _ = window.set_position(PhysicalPosition::new(x, y));
    let _ = window.set_size(PhysicalSize::new(win_w as u32, win_h as u32));
    #[cfg(target_os = "macos")]
    {
        let _ = window.set_decorations(false);
        let _ = window.set_always_on_top(true);
    }
    info!(
        "Annotate window at ({}, {}) size {}x{} image {}x{}",
        x, y, win_w, win_h, img_w, img_h
    );
}

#[derive(Serialize)]
pub struct SourceImage {
    pub data: String,
    pub width: u32,
    pub height: u32,
}

#[tauri::command]
pub fn annotate_source_image() -> Result<SourceImage, String> {
    let path = cut_image_path();
    let bytes =
        std::fs::read(&path).map_err(|err| format!("read screenshot {}: {err}", path.display()))?;
    if bytes.len() < 32 {
        return Err("screenshot file is empty".into());
    }
    let image =
        image::load_from_memory(&bytes).map_err(|err| format!("decode screenshot: {err}"))?;
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return Err("screenshot has no pixels".into());
    }
    Ok(SourceImage {
        data: general_purpose::STANDARD.encode(bytes),
        width,
        height,
    })
}

fn write_cut_from_base64(data: &str) -> Result<(), String> {
    let image = decode_base64_image(data).map_err(|err| err.to_string())?;
    write_png(&cut_image_path(), &image).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn copy_png_base64(data: String) -> Result<(), String> {
    clipboard_on_main(move || {
        let image = decode_base64_image(&data).map_err(|err| err.to_string())?;
        copy_to_clipboard(&image).map_err(|err| err.to_string())
    })
}

#[tauri::command]
pub fn save_png_base64(path: String, data: String) -> Result<(), String> {
    let image = decode_base64_image(&data).map_err(|err| err.to_string())?;
    let path = ensure_png_extension(path);
    write_png(&path, &image).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn open_ocr_from_png_base64(data: String) -> Result<(), String> {
    if !data.trim().is_empty() {
        write_cut_from_base64(&data)?;
    }
    spawn_main(|| recognize_window())
}

#[tauri::command]
pub fn open_translate_from_png_base64(data: String) -> Result<(), String> {
    if !data.trim().is_empty() {
        write_cut_from_base64(&data)?;
    }
    spawn_main(|| image_translate())
}
