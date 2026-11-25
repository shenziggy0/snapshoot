use base64::{engine::general_purpose, Engine as _};
#[cfg(not(target_os = "macos"))]
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotNote {
    title: String,
    image: String,
    image_label: String,
    text: String,
    text_label: String,
    translation: String,
    translation_label: String,
    source: String,
    source_label: String,
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn note_html(note: &ScreenshotNote) -> Result<String, String> {
    let png = general_purpose::STANDARD
        .decode(&note.image)
        .map_err(|err| err.to_string())?;
    image::load_from_memory_with_format(&png, image::ImageFormat::Png)
        .map_err(|err| err.to_string())?;

    let title = escape_html(&note.title);
    let image_label = escape_html(&note.image_label);
    let mut html = format!(
        "<!doctype html>\n<html><head><meta charset=\"utf-8\">\
         <meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src data:; style-src 'unsafe-inline'\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{title}</title><style>\
         body{{max-width:54rem;margin:2rem auto;padding:0 1rem;font:1rem/1.6 system-ui,sans-serif}}\
         img{{display:block;max-width:100%;height:auto}}\
         pre{{font:inherit;white-space:pre-wrap;overflow-wrap:anywhere}}\
         </style></head><body><main><h1>{title}</h1>\
         <img alt=\"{image_label}\" src=\"data:image/png;base64,{}\">",
        note.image
    );
    for (label, content) in [
        (&note.text_label, &note.text),
        (&note.translation_label, &note.translation),
        (&note.source_label, &note.source),
    ] {
        if !content.trim().is_empty() {
            html.push_str(&format!(
                "<section><h2>{}</h2><pre>{}</pre></section>",
                escape_html(label),
                escape_html(content)
            ));
        }
    }
    html.push_str("</main></body></html>\n");
    Ok(html)
}

#[tauri::command]
pub async fn save_screenshot_note(
    window: tauri::Window,
    note: ScreenshotNote,
) -> Result<bool, String> {
    let html = note_html(&note)?;
    let Some(path) = tauri::api::dialog::blocking::FileDialogBuilder::new()
        .set_parent(&window)
        .set_file_name("snapshoot-note.html")
        .add_filter("HTML", &["html"])
        .save_file()
    else {
        return Ok(false);
    };
    if !path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("html"))
    {
        return Err("recognize.note_html_extension".into());
    }
    std::fs::write(path, html).map_err(|err| err.to_string())?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screenshot_note_embeds_png_and_escapes_visible_text() {
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(8, 8)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let image = general_purpose::STANDARD.encode(png.into_inner());
        let html = note_html(&ScreenshotNote {
            title: "截图笔记".into(),
            image: image.clone(),
            image_label: "原图".into(),
            text: "iOS <script>".into(),
            text_label: "原文".into(),
            translation: "示例".into(),
            translation_label: "译文".into(),
            source: "https://example.test/?a=1&b=2".into(),
            source_label: "来源".into(),
        })
        .unwrap();
        assert!(html.contains(&format!("data:image/png;base64,{image}")));
        assert!(html.contains("<pre>iOS &lt;script&gt;</pre>"));
        assert!(html.contains("<pre>示例</pre>"));
        assert!(html.contains("?a=1&amp;b=2"));
    }
}

#[tauri::command(async)]
pub fn system_ocr_layout(data: String, lang: Option<String>) -> Result<OcrLayout, String> {
    #[cfg(target_os = "macos")]
    {
        let payload = data
            .split_once(',')
            .map(|(_, rest)| rest)
            .unwrap_or(data.as_str());
        let bytes = general_purpose::STANDARD
            .decode(payload.trim())
            .map_err(|err| err.to_string())?;
        macos::recognize(&bytes, lang.as_deref().unwrap_or("auto"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (data, lang);
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
    use std::ffi::{c_char, c_int, CStr, CString};

    extern "C" {
        fn snapshoot_vision_ocr(
            bytes: *const u8,
            length: usize,
            language: *const c_char,
            status: *mut c_int,
        ) -> *mut c_char;
        fn snapshoot_vision_ocr_free(value: *mut c_char);
    }

    pub fn recognize(bytes: &[u8], lang: &str) -> Result<OcrLayout, String> {
        if bytes.is_empty() {
            return Err("OCR image is empty".into());
        }
        let language = CString::new(lang).map_err(|_| "recognize.language_unsupported")?;
        let mut status = 0;
        unsafe {
            let raw =
                snapshoot_vision_ocr(bytes.as_ptr(), bytes.len(), language.as_ptr(), &mut status);
            if raw.is_null() {
                if status == 1 {
                    return Err("recognize.language_unsupported".into());
                }
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
pub fn system_ocr(lang: &str) -> Result<String, String> {
    let bytes = std::fs::read(crate::image_io::cut_image_path()).map_err(|err| err.to_string())?;
    macos::recognize(&bytes, lang).map(|layout| layout.text)
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
