use crate::annotate::screenshot_annotate;
use crate::config::{get, set};
use crate::pin::pin_clipboard;
use crate::screen_record::screen_record_hotkey;
use crate::window::{input_translate, ocr_recognize, ocr_translate, selection_translate};
use crate::APP;
use log::{info, warn};
use tauri::{AppHandle, GlobalShortcutManager};

fn default_hotkey(name: &str) -> &'static str {
    match name {
        // F-keys are the Windows/Linux defaults. macOS treats bare F-keys as media keys,
        // so the first-run defaults there are chord shortcuts the user can change.
        "hotkey_screenshot_annotate" => {
            if cfg!(target_os = "macos") {
                "Command+Shift+A"
            } else {
                "F1"
            }
        }
        "hotkey_pin_clipboard" => {
            if cfg!(target_os = "macos") {
                "Command+Shift+P"
            } else {
                "F3"
            }
        }
        "hotkey_screen_record" => {
            if cfg!(target_os = "windows") {
                "F2"
            } else {
                ""
            }
        }
        _ => "",
    }
}

fn read_hotkey(name: &str, explicit: &str) -> String {
    if !explicit.is_empty() {
        set(name, explicit);
        return explicit.to_string();
    }
    match get(name) {
        Some(v) => v.as_str().unwrap_or("").to_string(),
        None => {
            let value = default_hotkey(name).to_string();
            set(name, &value);
            value
        }
    }
}

fn register<F>(app_handle: &AppHandle, name: &str, handler: F, key: &str) -> Result<(), String>
where
    F: Fn() + Send + 'static,
{
    let hotkey = read_hotkey(name, key);

    if !hotkey.is_empty() {
        match app_handle
            .global_shortcut_manager()
            .register(hotkey.as_str(), handler)
        {
            Ok(()) => {
                info!("Registered global shortcut: {} for {}", hotkey, name);
            }
            Err(e) => {
                warn!("Failed to register global shortcut: {} {:?}", hotkey, e);
                return Err(e.to_string());
            }
        };
    }
    Ok(())
}

// Register global shortcuts
pub fn register_shortcut(shortcut: &str) -> Result<(), String> {
    let app_handle = APP.get().unwrap();
    match shortcut {
        "hotkey_selection_translate" => register(
            app_handle,
            "hotkey_selection_translate",
            selection_translate,
            "",
        )?,
        "hotkey_input_translate" => {
            register(app_handle, "hotkey_input_translate", input_translate, "")?
        }
        "hotkey_ocr_recognize" => register(app_handle, "hotkey_ocr_recognize", ocr_recognize, "")?,
        "hotkey_ocr_translate" => register(app_handle, "hotkey_ocr_translate", ocr_translate, "")?,
        "hotkey_screenshot_annotate" => register(
            app_handle,
            "hotkey_screenshot_annotate",
            screenshot_annotate,
            "",
        )?,
        "hotkey_pin_clipboard" => register(app_handle, "hotkey_pin_clipboard", pin_clipboard, "")?,
        "hotkey_screen_record" => {
            register(app_handle, "hotkey_screen_record", screen_record_hotkey, "")?
        }
        "all" => {
            register(
                app_handle,
                "hotkey_selection_translate",
                selection_translate,
                "",
            )?;
            register(app_handle, "hotkey_input_translate", input_translate, "")?;
            register(app_handle, "hotkey_ocr_recognize", ocr_recognize, "")?;
            register(app_handle, "hotkey_ocr_translate", ocr_translate, "")?;
            // A failure here should not unregister the shortcuts above.
            let mut shortcut_errors = Vec::new();
            if let Err(err) = register(
                app_handle,
                "hotkey_screenshot_annotate",
                screenshot_annotate,
                "",
            ) {
                shortcut_errors.push(err);
            }
            if let Err(err) = register(app_handle, "hotkey_pin_clipboard", pin_clipboard, "") {
                shortcut_errors.push(err);
            }
            if let Err(err) = register(app_handle, "hotkey_screen_record", screen_record_hotkey, "")
            {
                shortcut_errors.push(err);
            }
            if !shortcut_errors.is_empty() {
                return Err(shortcut_errors.join("; "));
            }
        }
        _ => {}
    }
    Ok(())
}

#[tauri::command]
pub fn register_shortcut_by_frontend(name: &str, shortcut: &str) -> Result<(), String> {
    let app_handle = APP.get().unwrap();
    match name {
        "hotkey_selection_translate" => register(
            app_handle,
            "hotkey_selection_translate",
            selection_translate,
            shortcut,
        )?,
        "hotkey_input_translate" => register(
            app_handle,
            "hotkey_input_translate",
            input_translate,
            shortcut,
        )?,
        "hotkey_ocr_recognize" => {
            register(app_handle, "hotkey_ocr_recognize", ocr_recognize, shortcut)?
        }
        "hotkey_ocr_translate" => {
            register(app_handle, "hotkey_ocr_translate", ocr_translate, shortcut)?
        }
        "hotkey_screenshot_annotate" => register(
            app_handle,
            "hotkey_screenshot_annotate",
            screenshot_annotate,
            shortcut,
        )?,
        "hotkey_pin_clipboard" => {
            register(app_handle, "hotkey_pin_clipboard", pin_clipboard, shortcut)?
        }
        "hotkey_screen_record" => register(
            app_handle,
            "hotkey_screen_record",
            screen_record_hotkey,
            shortcut,
        )?,
        _ => {}
    }
    Ok(())
}
