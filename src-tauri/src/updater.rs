/// Startup update checks are disabled so this fork can be packaged without the
/// upstream updater private key. The tray "Check Update" item still opens the
/// updater window, which reports that no signed update feed is configured.
pub fn check_update(_app_handle: tauri::AppHandle) {}
