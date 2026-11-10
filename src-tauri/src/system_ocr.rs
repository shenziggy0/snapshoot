#[cfg(target_os = "macos")]
use base64::{engine::general_purpose, Engine as _};
use dirs::cache_dir;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrLine {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub confidence: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrBarcode {
    pub text: String,
    #[serde(default)]
    pub symbology: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrLayout {
    pub width: u32,
    pub height: u32,
    pub text: String,
    #[serde(default)]
    pub lines: Vec<OcrLine>,
    #[serde(default)]
    pub barcodes: Vec<OcrBarcode>,
}

#[tauri::command(async)]
pub fn system_ocr_layout(data: String) -> Result<OcrLayout, String> {
    #[cfg(target_os = "macos")]
    {
        let payload = data
            .split_once(',')
            .map(|(_, rest)| rest)
            .unwrap_or(data.as_str());
        let bytes = general_purpose::STANDARD
            .decode(payload.trim())
            .map_err(|err| err.to_string())?;
        macos::recognize(&bytes)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = data;
        Err("system_ocr_layout is currently available on macOS".into())
    }
}

#[tauri::command(async)]
#[cfg(target_os = "windows")]
pub fn system_ocr(app_handle: tauri::AppHandle, lang: &str) -> Result<String, String> {
    use windows::core::HSTRING;
    use windows::Globalization::Language;
    use windows::Graphics::Imaging::BitmapDecoder;
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::{FileAccessMode, StorageFile};

    fn win_err(err: impl ToString) -> String {
        err.to_string()
    }

    let mut app_cache_dir_path = cache_dir().ok_or_else(|| "Get Cache Dir Failed".to_string())?;
    app_cache_dir_path.push(&app_handle.config().tauri.bundle.identifier);
    app_cache_dir_path.push("pot_screenshot_cut.png");
    if !app_cache_dir_path.exists() {
        return Err("OCR image is missing".into());
    }

    let path = app_cache_dir_path.to_string_lossy().replace("\\\\?\\", "");

    let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(path))
        .map_err(win_err)?
        .get()
        .map_err(win_err)?;

    let decoder_id = BitmapDecoder::PngDecoderId().map_err(win_err)?;
    let stream = file
        .OpenAsync(FileAccessMode::Read)
        .map_err(win_err)?
        .get()
        .map_err(win_err)?;
    let bitmap = BitmapDecoder::CreateWithIdAsync(decoder_id, &stream)
        .map_err(win_err)?
        .get()
        .map_err(win_err)?;

    let bitmap = bitmap
        .GetSoftwareBitmapAsync()
        .map_err(win_err)?
        .get()
        .map_err(win_err)?;

    let engine = match lang {
        "auto" => OcrEngine::TryCreateFromUserProfileLanguages(),
        _ => {
            let language = Language::CreateLanguage(&HSTRING::from(lang)).map_err(win_err)?;
            OcrEngine::TryCreateFromLanguage(&language)
        }
    };

    match engine {
        Ok(engine) => {
            let result = engine
                .RecognizeAsync(&bitmap)
                .map_err(win_err)?
                .get()
                .map_err(win_err)?;
            Ok(result.Text().map_err(win_err)?.to_string_lossy())
        }
        Err(e) => {
            if e.to_string().contains("0x00000000") {
                Err("Language package not installed!\n\nSee: https://learn.microsoft.com/zh-cn/windows/powertoys/text-extractor#supported-languages".to_string())
            } else {
                Err(e.to_string())
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::OcrLayout;
    use std::ffi::{c_char, CStr};

    extern "C" {
        fn snapshoot_vision_ocr(bytes: *const u8, length: usize) -> *mut c_char;
        fn snapshoot_vision_ocr_free(value: *mut c_char);
    }

    pub fn recognize(bytes: &[u8]) -> Result<OcrLayout, String> {
        if bytes.is_empty() {
            return Err("OCR image is empty".into());
        }
        unsafe {
            let raw = snapshoot_vision_ocr(bytes.as_ptr(), bytes.len());
            if raw.is_null() {
                return Err("Apple Vision OCR failed".into());
            }
            let json = CStr::from_ptr(raw).to_string_lossy().to_string();
            snapshoot_vision_ocr_free(raw);
            serde_json::from_str(&json).map_err(|err| format!("decode Vision OCR result: {err}"))
        }
    }
}

#[tauri::command(async)]
#[cfg(target_os = "macos")]
pub fn system_ocr(app_handle: tauri::AppHandle, _lang: &str) -> Result<String, String> {
    let mut app_cache_dir_path = cache_dir().expect("Get Cache Dir Failed");
    app_cache_dir_path.push(&app_handle.config().tauri.bundle.identifier);
    app_cache_dir_path.push("pot_screenshot_cut.png");
    let bytes = std::fs::read(&app_cache_dir_path).map_err(|err| err.to_string())?;
    macos::recognize(&bytes).map(|layout| layout.text)
}

#[tauri::command(async)]
#[cfg(target_os = "linux")]
pub fn system_ocr(app_handle: tauri::AppHandle, lang: &str) -> Result<String, String> {
    let mut app_cache_dir_path = cache_dir().expect("Get Cache Dir Failed");
    app_cache_dir_path.push(&app_handle.config().tauri.bundle.identifier);
    app_cache_dir_path.push("pot_screenshot_cut.png");
    let mut args = ["", ""];
    if lang != "auto" {
        args = ["-l", lang];
    }

    let output = match std::process::Command::new("tesseract")
        .arg(app_cache_dir_path.to_str().unwrap())
        .arg("stdout")
        .args(args)
        .output()
    {
        Ok(v) => v,
        Err(e) => {
            if e.to_string().contains("os error 2") {
                return Err("Tesseract not installed!".to_string());
            }
            return Err(e.to_string());
        }
    };
    if output.status.success() {
        let content = String::from_utf8(output.stdout).unwrap_or_default();
        Ok(content)
    } else {
        let content = String::from_utf8(output.stderr).unwrap_or_default();

        if content.contains("data") {
            if lang == "auto" {
                return Err(
                    "Language data not installed!\nPlease try install tesseract-ocr-eng"
                        .to_string(),
                );
            } else {
                return Err(format!(
                    "Language data not installed!\nPlease try install tesseract-ocr-{lang}"
                ));
            }
        }
        Err(content)
    }
}
