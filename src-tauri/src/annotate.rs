use crate::image_io::{
    clipboard_on_main, copy_to_clipboard, cut_image_path, decode_base64_image,
    ensure_png_extension, spawn_main, write_png,
};
use crate::window::{image_translate, recognize_window};
use base64::{engine::general_purpose, Engine as _};
use image::GenericImageView;
use serde::Serialize;

pub fn screenshot_annotate() {
    crate::window::open_frozen_overlay("annotate");
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
    spawn_main(recognize_window)
}

#[tauri::command]
pub fn open_translate_from_png_base64(data: String) -> Result<(), String> {
    if !data.trim().is_empty() {
        write_cut_from_base64(&data)?;
    }
    spawn_main(image_translate)
}
