use crate::config::get;
use crate::image_io::{
    cache_file, copy_to_clipboard, cut_image_path, ensure_png_extension, spawn_main, write_png,
};
use crate::window::{
    image_translate, monitor_at_mouse, mouse_physical_position, new_overlay_builder,
    recognize_window,
};
use crate::APP;
use base64::{engine::general_purpose, Engine as _};
use image::RgbaImage;
use log::{info, warn};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use tauri::api::notification::Notification;
use tauri::{PhysicalPosition, PhysicalSize, WindowEvent};

static NEXT_PIN: AtomicU64 = AtomicU64::new(1);
static CASCADE: AtomicI32 = AtomicI32::new(0);
static PINS: OnceLock<Mutex<HashMap<String, PinMeta>>> = OnceLock::new();

#[derive(Clone)]
struct PinMeta {
    path: PathBuf,
    width: u32,
    height: u32,
    zoom: f64,
}

#[derive(Serialize)]
pub struct PinInfo {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub zoom: f64,
}

fn pins() -> std::sync::MutexGuard<'static, HashMap<String, PinMeta>> {
    PINS.get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
}

pub fn cleanup_stale_pins() {
    let dir = crate::image_io::app_cache_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("pin_") && name.ends_with(".png") {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

pub fn pin_clipboard() {
    match read_clipboard_rgba() {
        Ok(image) => open_pin_rgba(image),
        Err(err) => {
            warn!("pin clipboard failed: {}", err);
            notify_no_image();
        }
    }
}

fn read_clipboard_rgba() -> Result<RgbaImage, String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|err| err.to_string())?;
    let image = clipboard.get_image().map_err(|err| err.to_string())?;
    let width = image.width as u32;
    let height = image.height as u32;
    if width == 0 || height == 0 {
        return Err("empty clipboard image".into());
    }
    let expected = (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(4);
    if image.bytes.len() < expected {
        return Err("clipboard image is truncated".into());
    }
    let buffer = image.bytes[..expected].to_vec();
    RgbaImage::from_raw(width, height, buffer)
        .ok_or_else(|| "clipboard image layout is invalid".into())
}

pub fn open_pin_dynamic(image: image::DynamicImage) {
    open_pin_rgba(image.to_rgba8());
}

fn open_pin_rgba(image: RgbaImage) {
    let width = image.width();
    let height = image.height();
    if width == 0 || height == 0 {
        notify_no_image();
        return;
    }
    let id = NEXT_PIN.fetch_add(1, Ordering::Relaxed);
    let label = format!("pin_{id}");
    let filename = format!("pin_{id}.png");
    let path = cache_file(&filename);
    if let Err(err) = write_png(&path, &image::DynamicImage::ImageRgba8(image)) {
        warn!("write pin image failed: {}", err);
        return;
    }

    let monitor = monitor_at_mouse();
    let scale = monitor.scale_factor().max(1.0);
    let mon_size = monitor.size();
    let max_w = mon_size.width as f64 * 0.85;
    let max_h = mon_size.height as f64 * 0.85;
    let mut zoom = (max_w / width as f64).min(max_h / height as f64).min(1.0);
    let min_phys = 96.0 * scale;
    let zoom_floor = (min_phys / width as f64).max(min_phys / height as f64);
    if zoom < zoom_floor {
        zoom = zoom_floor.min(4.0);
    }
    zoom = zoom.clamp(0.05, 8.0);

    let win_w = ((width as f64) * zoom).round().max(1.0) as u32;
    let win_h = ((height as f64) * zoom).round().max(1.0) as u32;

    let (mouse_x, mouse_y) = mouse_physical_position();
    let cascade = CASCADE.fetch_add(28, Ordering::Relaxed) % 224;
    let mon_pos = monitor.position();
    let mut x = mouse_x + cascade;
    let mut y = mouse_y + cascade;
    let max_x = mon_pos.x + mon_size.width as i32 - win_w as i32;
    let max_y = mon_pos.y + mon_size.height as i32 - win_h as i32;
    x = x.clamp(mon_pos.x, max_x.max(mon_pos.x));
    y = y.clamp(mon_pos.y, max_y.max(mon_pos.y));

    pins().insert(
        label.clone(),
        PinMeta {
            path: path.clone(),
            width,
            height,
            zoom,
        },
    );

    let builder = new_overlay_builder(&label, "Pin");
    let window = match builder.build() {
        Ok(window) => window,
        Err(err) => {
            warn!("create pin window failed: {}", err);
            pins().remove(&label);
            let _ = std::fs::remove_file(&path);
            return;
        }
    };
    let _ = window.set_position(PhysicalPosition::new(x, y));
    let _ = window.set_size(PhysicalSize::new(win_w, win_h));
    #[cfg(target_os = "macos")]
    {
        let _ = window.set_decorations(false);
        let _ = window.set_always_on_top(true);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window.set_always_on_top(true);
    }

    let label_for_close = label.clone();
    let path_for_close = path.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Destroyed = event {
            pins().remove(&label_for_close);
            let _ = std::fs::remove_file(&path_for_close);
        }
    });
    info!("Pin {} {}x{} zoom {:.2}", label, width, height, zoom);
}

fn meta_for(window: &tauri::Window) -> Result<PinMeta, String> {
    let label = window.label().to_string();
    pins()
        .get(&label)
        .cloned()
        .ok_or_else(|| format!("pin not found: {label}"))
}

fn copy_pin_file_to_cut(meta: &PinMeta) -> Result<(), String> {
    std::fs::copy(&meta.path, cut_image_path()).map_err(|err| err.to_string())?;
    Ok(())
}

fn notify_no_image() {
    let app = APP.get().unwrap();
    let (title, body) = match get("app_language")
        .and_then(|value| value.as_str().map(|s| s.to_string()))
        .as_deref()
    {
        Some("zh_cn") | Some("zh_tw") => ("无法贴图", "剪贴板中没有图片"),
        _ => ("Cannot pin", "No image on the clipboard"),
    };
    let _ = Notification::new(&app.config().tauri.bundle.identifier)
        .title(title)
        .body(body)
        .icon("pot")
        .show();
}

#[tauri::command]
pub fn pin_png_base64(data: String) -> Result<(), String> {
    let image = crate::image_io::decode_base64_image(&data).map_err(|err| err.to_string())?;
    spawn_main(move || open_pin_dynamic(image))
}

#[tauri::command]
pub fn pin_image_base64(window: tauri::Window) -> Result<String, String> {
    let meta = meta_for(&window)?;
    let bytes = std::fs::read(&meta.path).map_err(|err| format!("read pin image: {err}"))?;
    if bytes.is_empty() {
        return Err("pin image is empty".into());
    }
    Ok(general_purpose::STANDARD.encode(bytes))
}

#[tauri::command]
pub fn pin_window_info(window: tauri::Window) -> Result<PinInfo, String> {
    let meta = meta_for(&window)?;
    Ok(PinInfo {
        path: meta.path.to_string_lossy().to_string(),
        width: meta.width,
        height: meta.height,
        zoom: meta.zoom,
    })
}

#[tauri::command]
pub fn copy_pin_image(window: tauri::Window) -> Result<(), String> {
    let meta = meta_for(&window)?;
    crate::image_io::clipboard_on_main(move || {
        let image = image::open(&meta.path).map_err(|err| err.to_string())?;
        copy_to_clipboard(&image).map_err(|err| err.to_string())
    })
}

#[tauri::command]
pub fn save_pin_image(window: tauri::Window, path: String) -> Result<(), String> {
    let meta = meta_for(&window)?;
    let dest = ensure_png_extension(path);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    std::fs::copy(&meta.path, &dest).map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn ocr_pin_image(window: tauri::Window) -> Result<(), String> {
    let meta = meta_for(&window)?;
    copy_pin_file_to_cut(&meta)?;
    spawn_main(|| recognize_window())
}

#[tauri::command]
pub fn translate_pin_image(window: tauri::Window) -> Result<(), String> {
    let meta = meta_for(&window)?;
    copy_pin_file_to_cut(&meta)?;
    spawn_main(|| image_translate())
}
