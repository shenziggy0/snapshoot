use crate::config::get;
use crate::image_io::{
    app_cache_dir, clipboard_on_main, cut_image_path, decode_base64_image, write_png,
};
use crate::screenshot::{resolve_saved_region, SavedCaptureRegion};
use crate::window::open_edit_overlay;
use crate::APP;
use image::GenericImageView;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Manager, PhysicalPosition, Window, WindowBuilder};

const HISTORY_LIMIT: usize = 60;
const QUICK_WIDTH: f64 = 380.0;
const QUICK_HEIGHT: f64 = 246.0;

static QUICK_PENDING: AtomicBool = AtomicBool::new(false);
static CAPTURE_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static HISTORY: OnceLock<Mutex<Vec<CaptureHistoryItem>>> = OnceLock::new();

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureHistoryItem {
    pub id: String,
    pub path: String,
    pub thumb_path: String,
    pub width: u32,
    pub height: u32,
    pub created_at: u64,
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<SavedCaptureRegion>,
}

fn captures_dir() -> std::path::PathBuf {
    let dir = app_cache_dir().join("capture_history");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn index_path() -> std::path::PathBuf {
    captures_dir().join("index.json")
}

fn load_history() -> Vec<CaptureHistoryItem> {
    std::fs::read_to_string(index_path())
        .ok()
        .and_then(|data| serde_json::from_str::<Vec<CaptureHistoryItem>>(&data).ok())
        .unwrap_or_default()
}

fn history() -> MutexGuard<'static, Vec<CaptureHistoryItem>> {
    HISTORY
        .get_or_init(|| Mutex::new(load_history()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn save_index(items: &[CaptureHistoryItem]) -> Result<(), String> {
    let data = serde_json::to_vec_pretty(items).map_err(|err| err.to_string())?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(captures_dir()).map_err(|err| err.to_string())?;
    temporary.write_all(&data).map_err(|err| err.to_string())?;
    temporary
        .persist(index_path())
        .map_err(|err| err.to_string())?;
    Ok(())
}

fn emit_history_changed() {
    if let Some(app) = APP.get() {
        let _ = app.emit_all("capture-history-changed", ());
    }
}

fn new_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or_default();
    let sequence = CAPTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("capture_{millis}_{sequence}")
}

#[tauri::command(async)]
pub fn capture_history_add(
    data: String,
    action: String,
    show_quick: Option<bool>,
    region: Option<SavedCaptureRegion>,
) -> Result<CaptureHistoryItem, String> {
    let image = decode_base64_image(&data).map_err(|err| err.to_string())?;
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return Err("capture history image is empty".into());
    }
    let id = new_id();
    let dir = captures_dir();
    let path = dir.join(format!("{id}.png"));
    let thumb_path = dir.join(format!("{id}_thumb.png"));
    write_png(&path, &image).map_err(|err| err.to_string())?;
    let thumb =
        image::DynamicImage::ImageRgba8(image::imageops::thumbnail(&image.to_rgba8(), 360, 240));
    if let Err(err) = write_png(&thumb_path, &thumb) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(thumb_path);
        return Err(err.to_string());
    }

    let item = CaptureHistoryItem {
        id,
        path: path.to_string_lossy().to_string(),
        thumb_path: thumb_path.to_string_lossy().to_string(),
        width,
        height,
        created_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_millis() as u64)
            .unwrap_or_default(),
        action,
        region: region.filter(|region| region.valid_for_image(width, height)),
    };

    let mut items = history();
    let mut next = items.clone();
    next.insert(0, item.clone());
    let removed = next.split_off(next.len().min(HISTORY_LIMIT));
    if let Err(err) = save_index(&next) {
        let _ = std::fs::remove_file(&item.path);
        let _ = std::fs::remove_file(&item.thumb_path);
        return Err(err);
    }
    *items = next;
    drop(items);
    for old in removed {
        let _ = std::fs::remove_file(old.path);
        let _ = std::fs::remove_file(old.thumb_path);
    }
    emit_history_changed();
    if show_quick.unwrap_or(true) {
        QUICK_PENDING.store(true, Ordering::SeqCst);
        let _ = crate::image_io::spawn_main(|| {
            if screenshot_hidden() {
                show_pending();
            }
        });
    }
    Ok(item)
}

fn screenshot_hidden() -> bool {
    APP.get()
        .and_then(|app| app.get_window("screenshot"))
        .and_then(|window| window.is_visible().ok())
        .map(|visible| !visible)
        .unwrap_or(true)
}

#[tauri::command]
pub fn capture_history_list() -> Vec<CaptureHistoryItem> {
    history().clone()
}

fn item_by_id(id: &str) -> Option<CaptureHistoryItem> {
    history().iter().find(|item| item.id == id).cloned()
}

#[tauri::command]
pub fn recapture_region(id: String) -> Result<(), String> {
    let item = item_by_id(&id).ok_or("quick_access.recapture_unavailable")?;
    let region = item
        .region
        .filter(|saved| saved.valid_for_image(item.width, item.height))
        .ok_or("quick_access.recapture_unavailable")?;
    resolve_saved_region(&region)?;
    crate::window::open_frozen_overlay_with_region("annotate", Some(region))
}

#[tauri::command]
pub fn capture_history_drag(window: Window, id: String) -> Result<(), String> {
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = (window, id);
        Err("quick_access.drag_unsupported".into())
    }
    #[cfg(any(target_os = "macos", windows))]
    {
        let item = item_by_id(&id).ok_or("quick_access.drag_failed")?;
        let path = stage_drag_file(&item).map_err(|err| {
            warn!("prepare screenshot drag: {err}");
            "quick_access.drag_failed"
        })?;
        let started = start_file_drag(&window, &path);
        if !started {
            let _ = std::fs::remove_file(path);
            return Err("quick_access.drag_failed".into());
        }
        Ok(())
    }
}

#[cfg(any(target_os = "macos", windows))]
fn stage_drag_file(item: &CaptureHistoryItem) -> std::io::Result<PathBuf> {
    let base = captures_dir();
    let capture_dir = base.canonicalize()?;
    let expected = capture_dir.join(format!("{}.png", item.id));
    let source = Path::new(&item.path).canonicalize()?;
    if source != expected || !source.is_file() || source.parent() != Some(capture_dir.as_path()) {
        return Err(std::io::Error::other("invalid screenshot source"));
    }
    let drag_dir = base.join("drag");
    std::fs::create_dir_all(&drag_dir)?;
    // Leave the file available for the receiving app to open after drop.
    for entry in std::fs::read_dir(&drag_dir)?.flatten() {
        let old = entry
            .metadata()
            .ok()
            .filter(|metadata| metadata.is_file())
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|date| date.elapsed().ok())
            .is_some_and(|age| age > std::time::Duration::from_secs(3600));
        if old {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    let path = drag_dir.join(format!("Snapshoot_{}.png", new_id()));
    if let Err(error) = std::fs::copy(source, &path) {
        let _ = std::fs::remove_file(path);
        return Err(error);
    }
    Ok(path)
}

#[cfg(target_os = "macos")]
fn start_file_drag(window: &Window, path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    extern "C" {
        fn snapshoot_drag_file(window: *mut std::ffi::c_void, path: *const std::ffi::c_char)
            -> i32;
    }
    let Ok(window) = window.ns_window() else {
        return false;
    };
    let Ok(path) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    unsafe { snapshoot_drag_file(window, path.as_ptr()) != 0 }
}

#[cfg(windows)]
fn start_file_drag(_window: &Window, path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    extern "C" {
        fn snapshoot_drag_file(path: *const u16) -> i32;
    }
    let path = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    unsafe { snapshoot_drag_file(path.as_ptr()) != 0 }
}

#[tauri::command(async)]
pub fn capture_history_copy(id: String) -> Result<(), String> {
    let item = item_by_id(&id).ok_or_else(|| "capture history item not found".to_string())?;
    let image = image::open(&item.path).map_err(|err| err.to_string())?;
    clipboard_on_main(move || {
        crate::image_io::copy_to_clipboard(&image).map_err(|err| err.to_string())
    })
}

#[tauri::command(async)]
pub async fn capture_history_share(id: String) -> Result<String, String> {
    use reqwest_dav::{Auth, ClientBuilder};
    let item = item_by_id(&id).ok_or_else(|| "capture history item not found".to_string())?;
    let base = get("webdav_url")
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default();
    if base.trim().is_empty() {
        return Err("quick_access.share_needs_webdav".into());
    }
    let username = get("webdav_username")
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default();
    let password = get("webdav_password")
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default();
    let client = ClientBuilder::new()
        .set_host(base.trim_end_matches('/').to_string())
        .set_auth(Auth::Basic(username, password))
        .build()
        .map_err(|err| err.to_string())?;
    let _ = client.mkcol("/snapshoot").await;
    let _ = client.mkcol("/snapshoot/captures").await;
    let filename = format!("{}.png", item.id);
    let remote = format!("/snapshoot/captures/{filename}");
    let bytes = std::fs::read(&item.path).map_err(|err| err.to_string())?;
    client
        .put(&remote, bytes)
        .await
        .map_err(|err| err.to_string())?;
    Ok(format!(
        "{}/snapshoot/captures/{filename}",
        base.trim_end_matches('/')
    ))
}

#[tauri::command]
pub fn capture_history_save(id: String, path: String) -> Result<(), String> {
    let item = item_by_id(&id).ok_or_else(|| "capture history item not found".to_string())?;
    let destination = crate::image_io::ensure_png_extension(path);
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    std::fs::copy(&item.path, destination).map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn capture_history_pin(id: String) -> Result<(), String> {
    let item = item_by_id(&id).ok_or_else(|| "capture history item not found".to_string())?;
    hide_quick_access_window();
    let image = image::open(&item.path).map_err(|err| err.to_string())?;
    crate::image_io::spawn_main(move || crate::pin::open_pin_dynamic(image))
}

#[tauri::command]
pub fn capture_history_edit(id: String) -> Result<(), String> {
    let item = item_by_id(&id).ok_or_else(|| "capture history item not found".to_string())?;
    hide_quick_access_window();
    open_edit_overlay(Path::new(&item.path))
        .map(|_| ())
        .ok_or_else(|| "screenshot overlay is busy".to_string())
}

#[tauri::command]
pub fn capture_history_ocr(id: String) -> Result<(), String> {
    let item = item_by_id(&id).ok_or_else(|| "capture history item not found".to_string())?;
    std::fs::copy(&item.path, cut_image_path()).map_err(|err| err.to_string())?;
    hide_quick_access_window();
    crate::image_io::spawn_main(crate::window::recognize_window)
}

#[tauri::command]
pub fn capture_history_delete(id: String) -> Result<(), String> {
    let mut items = history();
    let Some(index) = items.iter().position(|item| item.id == id) else {
        return Ok(());
    };
    let mut next = items.clone();
    let item = next.remove(index);
    save_index(&next)?;
    *items = next;
    drop(items);
    let _ = std::fs::remove_file(item.path);
    let _ = std::fs::remove_file(item.thumb_path);
    emit_history_changed();
    Ok(())
}

#[tauri::command]
pub fn capture_history_clear() -> Result<(), String> {
    let mut items = history();
    save_index(&[])?;
    let old = std::mem::take(&mut *items);
    drop(items);
    for item in old {
        let _ = std::fs::remove_file(item.path);
        let _ = std::fs::remove_file(item.thumb_path);
    }
    emit_history_changed();
    Ok(())
}

#[tauri::command]
pub fn hide_quick_access_window() {
    if let Some(window) = APP.get().and_then(|app| app.get_window("quick_access")) {
        let _ = window.hide();
    }
}

fn position_quick_access(window: &Window) {
    let monitor = crate::window::monitor_at_mouse();
    let scale = monitor.scale_factor().max(1.0);
    let position = monitor.position();
    let size = monitor.size();
    let width = (QUICK_WIDTH * scale).round() as i32;
    let height = (QUICK_HEIGHT * scale).round() as i32;
    let margin = (24.0 * scale).round() as i32;
    let bottom = (76.0 * scale).round() as i32;
    let x = position.x + size.width as i32 - width - margin;
    let y = position.y + size.height as i32 - height - bottom;
    let _ = window.set_size(tauri::LogicalSize::new(QUICK_WIDTH, QUICK_HEIGHT));
    let _ = window.set_position(PhysicalPosition::new(x.max(position.x), y.max(position.y)));
}

fn build_quick_access_window() -> Result<Window, String> {
    let builder: WindowBuilder<'static, tauri::Wry> =
        crate::window::new_overlay_builder("quick_access", "Snapshoot")
            .visible(false)
            .focused(false)
            .inner_size(QUICK_WIDTH, QUICK_HEIGHT);
    let window = builder.build().map_err(|err| err.to_string())?;
    position_quick_access(&window);
    Ok(window)
}

fn window_for_quick_access() -> Result<Window, String> {
    if let Some(window) = APP.get().and_then(|app| app.get_window("quick_access")) {
        return Ok(window);
    }
    build_quick_access_window()
}

fn reveal_quick_access() -> Result<(), String> {
    let window = window_for_quick_access()?;
    position_quick_access(&window);
    let _ = window.set_always_on_top(true);
    window.show().map_err(|err| err.to_string())?;
    let latest = history().first().cloned();
    if let Some(item) = latest {
        let _ = window.emit("quick-access-update", item);
    }
    Ok(())
}

pub(crate) fn show_pending() {
    if !QUICK_PENDING.swap(false, Ordering::SeqCst) {
        return;
    }
    let enabled = get("quick_access_enabled")
        .and_then(|value| value.as_bool())
        .unwrap_or(true);
    if !enabled {
        return;
    }
    if let Err(err) = reveal_quick_access() {
        warn!("show quick access failed: {err}");
    }
}

#[tauri::command]
pub fn show_quick_access() -> Result<(), String> {
    reveal_quick_access()
}

pub(crate) fn hide_for_capture() {
    QUICK_PENDING.store(false, Ordering::SeqCst);
    if let Some(window) = APP.get().and_then(|app| app.get_window("quick_access")) {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        }
    }
}

pub(crate) fn prewarm_quick_access() {
    if APP
        .get()
        .and_then(|app| app.get_window("quick_access"))
        .is_some()
    {
        return;
    }
    match build_quick_access_window() {
        Ok(_) => info!("quick access window prewarmed"),
        Err(err) => warn!("prewarm quick access failed: {err}"),
    }
}

#[cfg(test)]
mod tests {
    use super::CaptureHistoryItem;
    use serde_json::json;

    #[test]
    fn capture_history_retains_pixel_region_and_reads_older_entries() {
        let mut value = json!({
            "id": "capture_1_1", "path": "capture_1_1.png", "thumbPath": "capture_1_1_thumb.png",
            "width": 800, "height": 500, "createdAt": 1, "action": "copy"
        });
        let legacy: CaptureHistoryItem = serde_json::from_value(value.clone()).unwrap();
        assert!(legacy.region.is_none());

        value["region"] = json!({
            "display": { "id": "display-example", "x": 0, "y": 0, "width": 2940, "height": 1912, "scale": 2.0 },
            "pixels": { "x": 130, "y": 288, "width": 800, "height": 500 }
        });
        let current: CaptureHistoryItem = serde_json::from_value(value.clone()).unwrap();
        let region = current.region.as_ref().unwrap();
        assert!(region.valid_for_image(800, 500));
        assert!(!region.valid_for_image(800, 499));
        assert_eq!(serde_json::to_value(current).unwrap(), value);
    }
}
