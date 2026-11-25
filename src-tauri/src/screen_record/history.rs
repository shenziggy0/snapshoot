use super::{platform, RecordResult};
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::Manager;

const LIMIT: usize = 60;
const WRITE_FAILED: &str = "record_history.write_failed";
static ACCESS: Mutex<()> = Mutex::new(());

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Recording {
    id: String,
    path: String,
    created_at: i64,
    width: u32,
    height: u32,
    fps: u32,
    duration_ms: u64,
    size_bytes: u64,
}

impl Recording {
    fn exists(&self) -> bool {
        Path::new(&self.path).is_file()
    }
}

#[derive(Serialize)]
pub struct RecordingStatus {
    #[serde(flatten)]
    record: Recording,
    exists: bool,
}

fn index_path() -> Result<PathBuf, String> {
    crate::APP
        .get()
        .and_then(|app| app.path_resolver().app_data_dir())
        .map(|directory| directory.join("record_history.json"))
        .ok_or_else(|| WRITE_FAILED.into())
}

fn read(path: &Path) -> io::Result<Vec<Recording>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

fn write(path: &Path, entries: &[Recording]) -> io::Result<()> {
    let directory = path.parent().ok_or(io::ErrorKind::InvalidInput)?;
    std::fs::create_dir_all(directory)?;
    let mut file = tempfile::NamedTempFile::new_in(directory)?;
    serde_json::to_writer(&mut file, entries)?;
    file.flush()?;
    file.as_file().sync_all()?;
    file.persist(path)?;
    Ok(())
}

fn add_to(index: &Path, result: &RecordResult) -> io::Result<()> {
    let mut entries = read(index)?;
    let now = chrono::Utc::now();
    entries.retain(|entry| entry.path != result.path);
    entries.insert(
        0,
        Recording {
            id: format!("{}-{}", now.timestamp_micros(), std::process::id()),
            path: result.path.clone(),
            created_at: now.timestamp_millis(),
            width: result.width,
            height: result.height,
            fps: result.fps,
            duration_ms: result.duration_ms,
            size_bytes: result.size_bytes,
        },
    );
    entries.truncate(LIMIT);
    write(index, &entries)
}

fn forget_from(index: &Path, matches: impl Fn(&Recording) -> bool) -> io::Result<()> {
    let mut entries = read(index)?;
    let old_count = entries.len();
    entries.retain(|entry| !matches(entry));
    if old_count != entries.len() {
        write(index, &entries)?;
    }
    Ok(())
}

fn index_error(error: io::Error) -> String {
    log::warn!("recording history: {error}");
    WRITE_FAILED.into()
}

pub(super) fn add(result: &RecordResult) -> Result<(), String> {
    let _guard = ACCESS.lock().unwrap();
    add_to(&index_path()?, result).map_err(index_error)
}

pub(super) fn forget_file(path: &str) -> Result<(), String> {
    let _guard = ACCESS.lock().unwrap();
    forget_from(&index_path()?, |entry| entry.path == path).map_err(index_error)
}

#[tauri::command(async)]
pub fn record_history_list() -> Result<Vec<RecordingStatus>, String> {
    let _guard = ACCESS.lock().unwrap();
    Ok(read(&index_path()?)
        .map_err(index_error)?
        .into_iter()
        .map(|record| RecordingStatus {
            exists: record.exists(),
            record,
        })
        .collect())
}

#[tauri::command(async)]
pub fn record_history_action(
    app: tauri::AppHandle,
    id: String,
    action: String,
) -> Result<(), String> {
    let index = index_path()?;
    let guard = ACCESS.lock().unwrap();
    if action == "forget" {
        return forget_from(&index, |record| record.id == id).map_err(index_error);
    }
    let record = read(&index)
        .map_err(index_error)?
        .into_iter()
        .find(|record| record.id == id && record.exists())
        .ok_or("record_history.file_missing")?;
    drop(guard);
    let path = Path::new(&record.path);
    match action.as_str() {
        "copy" => {
            crate::image_io::clipboard_on_main(move || platform::copy_file(Path::new(&record.path)))
        }
        "reveal" => platform::reveal(path),
        "open" => tauri::api::shell::open(&app.shell_scope(), record.path, None).map_err(|error| {
            log::warn!("open recording: {error}");
            "record_history.open_failed".into()
        }),
        _ => Err("record_history.invalid_action".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recordings_survive_reload_and_forgetting_leaves_the_file_alone() {
        let directory = tempfile::tempdir().unwrap();
        let index = directory.path().join("record_history.json");
        let video = directory.path().join("Demo.mp4");
        std::fs::write(&video, b"test file").unwrap();
        let record = RecordResult {
            path: video.to_string_lossy().into_owned(),
            width: 320,
            height: 240,
            fps: 30,
            duration_ms: 3000,
            size_bytes: 9,
            clipboard_copied: false,
        };

        add_to(&index, &record).unwrap();
        let entry = read(&index).unwrap().pop().unwrap();
        assert_eq!(entry.duration_ms, 3000);
        assert!(entry.exists());
        forget_from(&index, |candidate| candidate.id == entry.id).unwrap();
        assert!(read(&index).unwrap().is_empty());
        assert!(video.is_file());

        add_to(&index, &record).unwrap();
        std::fs::rename(&video, directory.path().join("Moved.mp4")).unwrap();
        assert!(!read(&index).unwrap()[0].exists());
    }
}
