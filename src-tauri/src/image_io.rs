use crate::error::Error;
use crate::APP;
use base64::{engine::general_purpose, Engine as _};
use image::DynamicImage;
use std::path::{Path, PathBuf};

/// Append a diagnostic line to the app cache. Capture timing is also logged
/// through the normal logger so a missing cache directory does not drop it.
pub fn diag_log(tag: &str, message: &str) {
    log::debug!("[{tag}] {message}");
    let path = app_cache_dir().join("diag.log");
    let line = format!("[{tag}] {message}\n");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        use std::io::Write;
        let _ = file.write_all(line.as_bytes());
    }
}

pub fn app_cache_dir() -> PathBuf {
    let app = APP.get().unwrap();
    let mut path = dirs::cache_dir().expect("Get Cache Dir Failed");
    path.push(&app.config().tauri.bundle.identifier);
    if !path.exists() {
        std::fs::create_dir_all(&path).expect("Create Cache Dir Failed");
    }
    path
}

pub fn cache_file(name: &str) -> PathBuf {
    let mut path = app_cache_dir();
    path.push(name);
    path
}

pub fn cut_image_path() -> PathBuf {
    cache_file("pot_screenshot_cut.png")
}

pub fn decode_base64_image(data: &str) -> Result<DynamicImage, Error> {
    let trimmed = data.trim();
    if trimmed.is_empty() {
        return Err(Error::Error("empty image".into()));
    }
    let payload = trimmed
        .split_once(',')
        .map(|(_, rest)| rest)
        .unwrap_or(trimmed);
    let bytes = general_purpose::STANDARD
        .decode(payload.trim())
        .map_err(|err| Error::Error(err.to_string().into()))?;
    let image = image::load_from_memory(&bytes)?;
    Ok(image)
}

pub fn write_png(path: &Path, image: &DynamicImage) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    image.save(path)?;
    Ok(())
}

pub fn copy_to_clipboard(image: &DynamicImage) -> Result<(), Error> {
    use arboard::{Clipboard, ImageData};
    use std::borrow::Cow;

    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    let img = ImageData {
        width: width as usize,
        height: height as usize,
        bytes: Cow::Borrowed(rgba.as_raw()),
    };
    Clipboard::new()?.set_image(img)?;
    Ok(())
}

pub fn ensure_png_extension(path: String) -> PathBuf {
    let path = PathBuf::from(path);
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("png") => path,
        _ => path.with_extension("png"),
    }
}

pub fn spawn_main<F>(task: F) -> Result<(), String>
where
    F: FnOnce() + Send + 'static,
{
    APP.get()
        .unwrap()
        .run_on_main_thread(task)
        .map_err(|err| err.to_string())
}

/// Clipboard and window calls on macOS must run on the main thread.
/// Invoke handlers run off the main thread, so wait for the queued task.
pub fn clipboard_on_main<F>(task: F) -> Result<(), String>
where
    F: FnOnce() -> Result<(), String> + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    spawn_main(move || {
        let _ = tx.send(task());
    })?;
    rx.recv_timeout(std::time::Duration::from_secs(8))
        .map_err(|err| err.to_string())?
}
