// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod annotate;
mod backup;
#[cfg(target_os = "macos")]
mod capture_macos;
mod clipboard;
mod cmd;
mod config;
mod error;
mod hotkey;
mod image_io;
mod lang_detect;
mod long_screenshot;
#[cfg(target_os = "macos")]
mod macos_permissions;
mod pin;
mod quick_access;
mod screen_record;
mod screenshot;
mod server;
mod smoke;
mod system_ocr;
mod tray;
mod updater;
mod window;
mod windows_list;

use backup::*;
use clipboard::*;
use cmd::*;
use config::*;
use hotkey::*;
use lang_detect::*;
use log::info;
use once_cell::sync::OnceCell;
use server::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use system_ocr::*;
use tauri::api::notification::Notification;
use tauri::Manager;
use tauri_plugin_log::LogTarget;
use tray::*;
use updater::check_update;
use window::config_window;
use window::updater_window;

// Global AppHandle
pub static APP: OnceCell<tauri::AppHandle> = OnceCell::new();
pub static ALLOW_EXIT: AtomicBool = AtomicBool::new(false);

// Text to be translated
pub struct StringWrapper(pub Mutex<String>);

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, cwd| {
            Notification::new(&app.config().tauri.bundle.identifier)
                .title("Snapshoot is already running. Please do not start it again!")
                .body(cwd)
                .icon("snapshoot")
                .show()
                .ok();
        }))
        .plugin(
            tauri_plugin_log::Builder::default()
                .targets([LogTarget::LogDir, LogTarget::Stdout])
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .plugin(tauri_plugin_sql::Builder::default().build())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_fs_watch::init())
        .system_tray(tauri::SystemTray::new())
        .setup(|app| {
            info!("============== Start App ==============");
            #[cfg(target_os = "macos")]
            if smoke::enabled() {
                app.set_activation_policy(tauri::ActivationPolicy::Regular);
            } else {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }
            // Global AppHandle
            APP.get_or_init(|| app.handle());
            // Init Config
            info!("Init Config Store");
            init_config(app);
            #[cfg(target_os = "macos")]
            if !smoke::enabled() && !macos_permissions::prepare(&app.handle(), !is_first_run()) {
                let trusted =
                    macos_accessibility_client::accessibility::application_is_trusted_with_prompt();
                info!("MacOS Accessibility Trusted: {}", trusted);
            }
            // Check First Run
            if is_first_run() && !smoke::enabled() {
                // Open Config Window
                info!("First Run, opening config window");
                config_window();
            }
            app.manage(StringWrapper(Mutex::new("".to_string())));
            app.manage(long_screenshot::LongCaptureManager::default());
            app.manage(screen_record::ScreenRecordManager::default());
            // Update Tray Menu
            update_tray(app.app_handle(), "".to_string(), "".to_string());
            // Start http server
            start_server();
            // Register Global Shortcut
            match register_shortcut("all") {
                Ok(()) => {}
                Err(e) => Notification::new(app.config().tauri.bundle.identifier.clone())
                    .title("Failed to register global shortcut")
                    .body(&e)
                    .icon("snapshoot")
                    .show()
                    .unwrap(),
            }
            if let Some(v) = get("proxy_enable") {
                let enabled = v.as_bool().unwrap_or(false);
                let has_host = get("proxy_host")
                    .and_then(|host| host.as_str().map(|value| !value.is_empty()))
                    .unwrap_or(false);
                if enabled && has_host {
                    let _ = set_proxy();
                }
            }
            // Check Update
            check_update(app.handle());
            if let Some(engine) = get("translate_detect_engine") {
                if engine.as_str().unwrap() == "local" {
                    init_lang_detect();
                }
            }
            let clipboard_monitor = match get("clipboard_monitor") {
                Some(v) => v.as_bool().unwrap(),
                None => {
                    set("clipboard_monitor", false);
                    false
                }
            };
            app.manage(ClipboardMonitorEnableWrapper(Mutex::new(
                clipboard_monitor.to_string(),
            )));
            start_clipboard_monitor(app.handle());
            pin::cleanup_stale_pins();
            if let Some(count) = smoke::capture_bench_count() {
                std::thread::spawn(move || smoke::run_capture_bench(count));
            } else if smoke::enabled() {
                std::thread::spawn(smoke::run);
            } else {
                // Keep the latency-sensitive overlay out of the first shortcut.
                // The hidden WebView reports request id 0 until a capture begins.
                std::thread::spawn(|| {
                    std::thread::sleep(std::time::Duration::from_millis(1500));
                    let _ = crate::image_io::spawn_main(|| {
                        crate::window::prewarm_screenshot_window();
                        crate::quick_access::prewarm_quick_access();
                    });
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            reload_store,
            get_text,
            cut_image,
            get_base64,
            copy_img,
            system_ocr,
            system_ocr_layout,
            set_proxy,
            unset_proxy,
            run_binary,
            open_devtools,
            register_shortcut_by_frontend,
            update_tray,
            updater_window,
            updater::check_latest_release,
            screenshot::screenshot,
            screenshot::screenshot_boot,
            screenshot::list_capture_windows,
            long_screenshot::long_capture_start,
            long_screenshot::long_capture_status,
            long_screenshot::long_capture_scroll,
            long_screenshot::overlay_set_hit_rects,
            screen_record::screen_record_support,
            screen_record::screen_record_start,
            screen_record::screen_record_status,
            screen_record::screen_record_pause,
            screen_record::screen_record_resume,
            screen_record::screen_record_stop,
            screen_record::screen_record_cancel,
            screen_record::screen_record_last_result,
            screen_record::screen_record_reveal_last,
            screen_record::screen_record_delete_last,
            long_screenshot::long_capture_finish_copy,
            long_screenshot::long_capture_cancel,
            window::prepare_overlay_display,
            window::reveal_overlay,
            window::hide_screenshot_window,
            lang_detect,
            webdav,
            local,
            install_plugin,
            font_list,
            aliyun,
            annotate::annotate_source_image,
            annotate::copy_png_base64,
            annotate::save_png_base64,
            annotate::open_ocr_from_png_base64,
            annotate::open_translate_from_png_base64,
            quick_access::capture_history_add,
            quick_access::capture_history_list,
            quick_access::capture_history_copy,
            quick_access::capture_history_share,
            quick_access::capture_history_save,
            quick_access::capture_history_pin,
            quick_access::capture_history_edit,
            quick_access::capture_history_ocr,
            quick_access::capture_history_delete,
            quick_access::capture_history_clear,
            quick_access::show_quick_access,
            quick_access::hide_quick_access_window,
            pin::annotate_pin,
            pin::pin_png_base64,
            pin::pin_image_base64,
            pin::pin_window_info,
            pin::pin_text_content,
            pin::copy_pin_image,
            pin::save_pin_image,
            pin::ocr_pin_image,
            pin::translate_pin_image
        ])
        .on_system_tray_event(tray_event_handler)
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        // 窗口关闭不退出
        .run(|_app_handle, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if !ALLOW_EXIT.load(Ordering::SeqCst) {
                    api.prevent_exit();
                }
            }
        });
}
