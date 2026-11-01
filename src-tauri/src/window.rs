use crate::config::get;
use crate::config::set;
use crate::StringWrapper;
use crate::APP;
use log::{info, warn};
use tauri::Manager;
use tauri::Monitor;
use tauri::Window;
use tauri::WindowBuilder;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use window_shadows::set_shadow;

// Get daemon window instance
fn get_daemon_window() -> Window {
    let app_handle = APP.get().unwrap();
    match app_handle.get_window("daemon") {
        Some(v) => v,
        None => {
            warn!("Daemon window not found, create new daemon window!");
            WindowBuilder::new(
                app_handle,
                "daemon",
                tauri::WindowUrl::App("daemon.html".into()),
            )
            .title("Daemon")
            .additional_browser_args("--disable-web-security")
            .visible(false)
            .build()
            .unwrap()
        }
    }
}

// Get monitor where the mouse is currently located
fn get_current_monitor(x: i32, y: i32) -> Monitor {
    info!("Mouse position: {}, {}", x, y);
    let daemon_window = get_daemon_window();
    let monitors = daemon_window.available_monitors().unwrap();

    for m in monitors {
        let size = m.size();
        let position = m.position();

        if x >= position.x
            && x <= (position.x + size.width as i32)
            && y >= position.y
            && y <= (position.y + size.height as i32)
        {
            info!("Current Monitor: {:?}", m);
            return m;
        }
    }
    warn!("Current Monitor not found, using primary monitor");
    daemon_window.primary_monitor().unwrap().unwrap()
}

pub(crate) fn mouse_physical_position() -> (i32, i32) {
    use mouse_position::mouse_position::Mouse;

    match Mouse::get_mouse_position() {
        Mouse::Position { x, y } => (x, y),
        Mouse::Error => {
            warn!("Mouse position not found, using (0, 0) as default");
            (0, 0)
        }
    }
}

pub(crate) fn monitor_at_mouse() -> Monitor {
    let (x, y) = mouse_physical_position();
    get_current_monitor(x, y)
}

/// Borderless, transparent, always-on-top window used by the annotation editor and pins.
/// macOS transparency needs `macos-private-api` plus `macOSPrivateApi` in tauri.conf.json.
pub(crate) fn new_overlay_builder(
    label: impl Into<String>,
    title: impl Into<String>,
) -> WindowBuilder<'static, tauri::Wry> {
    let app_handle = APP.get().unwrap();
    let mut builder = WindowBuilder::new(
        app_handle,
        label,
        tauri::WindowUrl::App("index.html".into()),
    )
    .title(title)
    .visible(false)
    .focused(true)
    .resizable(false)
    .skip_taskbar(true)
    .always_on_top(true)
    .additional_browser_args("--disable-web-security");

    #[cfg(target_os = "macos")]
    {
        builder = builder
            .transparent(true)
            .decorations(false)
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .hidden_title(true)
            .accept_first_mouse(true);
    }
    #[cfg(not(target_os = "macos"))]
    {
        builder = builder.transparent(true).decorations(false);
    }
    builder
}

// Creating a window on the mouse monitor
fn build_window(label: &str, title: &str) -> (Window, bool) {
    use mouse_position::mouse_position::{Mouse, Position};

    let mouse_position = match Mouse::get_mouse_position() {
        Mouse::Position { x, y } => Position { x, y },
        Mouse::Error => {
            warn!("Mouse position not found, using (0, 0) as default");
            Position { x: 0, y: 0 }
        }
    };
    let current_monitor = get_current_monitor(mouse_position.x, mouse_position.y);
    let position = current_monitor.position();

    let app_handle = APP.get().unwrap();
    match app_handle.get_window(label) {
        Some(v) => {
            info!("Window existence: {}", label);
            v.set_focus().unwrap();
            (v, true)
        }
        None => {
            info!("Window not existence, Creating new window: {}", label);
            let mut builder = tauri::WindowBuilder::new(
                app_handle,
                label,
                tauri::WindowUrl::App("index.html".into()),
            )
            .position(position.x.into(), position.y.into())
            .additional_browser_args("--disable-web-security")
            .focused(true)
            .title(title)
            .visible(false);

            #[cfg(target_os = "macos")]
            {
                builder = builder
                    .title_bar_style(tauri::TitleBarStyle::Overlay)
                    .hidden_title(true);
            }
            #[cfg(not(target_os = "macos"))]
            {
                builder = builder.transparent(true).decorations(false);
            }
            let window = builder.build().unwrap();

            if label != "screenshot" {
                #[cfg(not(target_os = "linux"))]
                set_shadow(&window, true).unwrap_or_default();
            }
            let _ = window.current_monitor();
            (window, false)
        }
    }
}

pub fn config_window() {
    let (window, _exists) = build_window("config", "Snapshoot");
    window
        .set_min_size(Some(tauri::LogicalSize::new(800, 400)))
        .unwrap();
    window.set_size(tauri::LogicalSize::new(800, 600)).unwrap();
    window.center().unwrap();
}

fn translate_window() -> Window {
    use mouse_position::mouse_position::{Mouse, Position};
    // Mouse physical position
    let mut mouse_position = match Mouse::get_mouse_position() {
        Mouse::Position { x, y } => Position { x, y },
        Mouse::Error => {
            warn!("Mouse position not found, using (0, 0) as default");
            Position { x: 0, y: 0 }
        }
    };
    let (window, exists) = build_window("translate", "Snapshoot");
    if exists {
        return window;
    }
    window.set_skip_taskbar(true).unwrap();
    // Get Translate Window Size
    let width = match get("translate_window_width") {
        Some(v) => v.as_i64().unwrap(),
        None => {
            set("translate_window_width", 350);
            350
        }
    };
    let height = match get("translate_window_height") {
        Some(v) => v.as_i64().unwrap(),
        None => {
            set("translate_window_height", 420);
            420
        }
    };

    let monitor = window.current_monitor().unwrap().unwrap();
    let dpi = monitor.scale_factor();

    window
        .set_size(tauri::PhysicalSize::new(
            (width as f64) * dpi,
            (height as f64) * dpi,
        ))
        .unwrap();

    let position_type = match get("translate_window_position") {
        Some(v) => v.as_str().unwrap().to_string(),
        None => "mouse".to_string(),
    };

    match position_type.as_str() {
        "mouse" => {
            // Adjust window position
            let monitor_size = monitor.size();
            let monitor_size_width = monitor_size.width as f64;
            let monitor_size_height = monitor_size.height as f64;
            let monitor_position = monitor.position();
            let monitor_position_x = monitor_position.x as f64;
            let monitor_position_y = monitor_position.y as f64;

            if mouse_position.x as f64 + width as f64 * dpi
                > monitor_position_x + monitor_size_width
            {
                mouse_position.x -= (width as f64 * dpi) as i32;
                if (mouse_position.x as f64) < monitor_position_x {
                    mouse_position.x = monitor_position_x as i32;
                }
            }
            if mouse_position.y as f64 + height as f64 * dpi
                > monitor_position_y + monitor_size_height
            {
                mouse_position.y -= (height as f64 * dpi) as i32;
                if (mouse_position.y as f64) < monitor_position_y {
                    mouse_position.y = monitor_position_y as i32;
                }
            }

            window
                .set_position(tauri::PhysicalPosition::new(
                    mouse_position.x,
                    mouse_position.y,
                ))
                .unwrap();
        }
        _ => {
            let position_x = match get("translate_window_position_x") {
                Some(v) => v.as_i64().unwrap(),
                None => 0,
            };
            let position_y = match get("translate_window_position_y") {
                Some(v) => v.as_i64().unwrap(),
                None => 0,
            };
            window
                .set_position(tauri::PhysicalPosition::new(
                    (position_x as f64) * dpi,
                    (position_y as f64) * dpi,
                ))
                .unwrap();
        }
    }

    window
}

pub fn selection_translate() {
    use selection::get_text;
    // Get Selected Text
    let text = get_text();
    if !text.trim().is_empty() {
        let app_handle = APP.get().unwrap();
        // Write into State
        let state: tauri::State<StringWrapper> = app_handle.state();
        state.0.lock().unwrap().replace_range(.., &text);
    }

    let window = translate_window();
    window.emit("new_text", text).unwrap();
}

pub fn input_translate() {
    let app_handle = APP.get().unwrap();
    // Clear State
    let state: tauri::State<StringWrapper> = app_handle.state();
    state
        .0
        .lock()
        .unwrap()
        .replace_range(.., "[INPUT_TRANSLATE]");
    let window = translate_window();
    let position_type = match get("translate_window_position") {
        Some(v) => v.as_str().unwrap().to_string(),
        None => "mouse".to_string(),
    };
    if position_type == "mouse" {
        window.center().unwrap();
    }

    window.emit("new_text", "[INPUT_TRANSLATE]").unwrap();
}

pub fn text_translate(text: String) {
    let app_handle = APP.get().unwrap();
    // Clear State
    let state: tauri::State<StringWrapper> = app_handle.state();
    state.0.lock().unwrap().replace_range(.., &text);
    let window = translate_window();
    window.emit("new_text", text).unwrap();
}

pub fn image_translate() {
    let app_handle = APP.get().unwrap();
    let state: tauri::State<StringWrapper> = app_handle.state();
    state
        .0
        .lock()
        .unwrap()
        .replace_range(.., "[IMAGE_TRANSLATE]");
    let window = translate_window();
    window.emit("new_text", "[IMAGE_TRANSLATE]").unwrap();
}

pub fn recognize_window() {
    let (window, exists) = build_window("recognize", "Snapshoot");
    if exists {
        window.emit("new_image", "").unwrap();
        return;
    }
    let width = match get("recognize_window_width") {
        Some(v) => v.as_i64().unwrap(),
        None => {
            set("recognize_window_width", 800);
            800
        }
    };
    let height = match get("recognize_window_height") {
        Some(v) => v.as_i64().unwrap(),
        None => {
            set("recognize_window_height", 400);
            400
        }
    };
    let monitor = window.current_monitor().unwrap().unwrap();
    let dpi = monitor.scale_factor();
    window
        .set_size(tauri::PhysicalSize::new(
            (width as f64) * dpi,
            (height as f64) * dpi,
        ))
        .unwrap();
    window.center().unwrap();
    window.emit("new_image", "").unwrap();
}

fn place_screenshot_window(window: &Window) {
    #[cfg(target_os = "macos")]
    {
        crate::capture_macos::cover_monitor(window);
        return;
    }
    #[cfg(windows)]
    {
        if cover_monitor_win(window) {
            return;
        }
    }
    let monitor = monitor_at_mouse();
    let pos = monitor.position();
    let size = monitor.size();
    let _ = window.set_position(tauri::PhysicalPosition::new(pos.x, pos.y));
    let _ = window.set_size(tauri::PhysicalSize::new(size.width, size.height));
}

#[cfg(windows)]
fn cover_monitor_win(window: &Window) -> bool {
    let Ok(hwnd) = window.hwnd() else {
        return false;
    };
    unsafe {
        let raw = hwnd.0 as *mut std::ffi::c_void;
        let monitor = MonitorFromWindow(raw, 2);
        if monitor.is_null() {
            return false;
        }
        let mut info: MonitorInfo = std::mem::zeroed();
        info.cb_size = std::mem::size_of::<MonitorInfo>() as u32;
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return false;
        }
        let rect = info.rc_monitor;
        SetWindowPos(
            raw,
            (-1isize) as *mut std::ffi::c_void,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            0x0010 | 0x0020,
        );
    }
    true
}

#[cfg(windows)]
#[repr(C)]
struct WinRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[cfg(windows)]
#[repr(C)]
struct MonitorInfo {
    cb_size: u32,
    rc_monitor: WinRect,
    rc_work: WinRect,
    dw_flags: u32,
}

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn MonitorFromWindow(hwnd: *mut std::ffi::c_void, flags: u32) -> *mut std::ffi::c_void;
    fn GetMonitorInfoW(monitor: *mut std::ffi::c_void, info: *mut MonitorInfo) -> i32;
    fn SetWindowPos(
        hwnd: *mut std::ffi::c_void,
        insert_after: *mut std::ffi::c_void,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: u32,
    ) -> i32;
}

/// Capture is finished before this runs. The page shows itself after the
/// frozen image is decoded. A short fallback shows the window if that signal
/// never arrives (hidden webviews often do not run animation frames).
pub(crate) fn screenshot_window() -> Window {
    let app_handle = APP.get().unwrap();
    if let Some(window) = app_handle.get_window("screenshot") {
        place_screenshot_window(&window);
        let _ = window.set_always_on_top(true);
        let _ = window.emit("screenshot-reload", ());
        reveal_overlay_fallback(window.clone());
        return window;
    }

    let builder = WindowBuilder::new(
        app_handle,
        "screenshot",
        tauri::WindowUrl::App("index.html".into()),
    )
    .title("Snapshoot")
    .visible(false)
    .focused(false)
    .resizable(false)
    .skip_taskbar(true)
    .always_on_top(true)
    .decorations(false)
    .transparent(true)
    .additional_browser_args("--disable-web-security");

    #[cfg(target_os = "macos")]
    let builder = builder
        .hidden_title(true)
        .title_bar_style(tauri::TitleBarStyle::Overlay);

    let window = builder.build().expect("create screenshot window");
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            crate::long_screenshot::abandon_active();
        }
    });
    place_screenshot_window(&window);
    reveal_overlay_fallback(window.clone());
    window
}

fn reveal_overlay_fallback(window: Window) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(350));
        match window.is_visible() {
            Ok(true) => {}
            Ok(false) => {
                warn!("overlay still hidden after 350ms; showing it");
                let _ = window.show();
                let _ = window.set_focus();
                let _ = window.set_always_on_top(true);
            }
            Err(err) => warn!("overlay visibility check failed: {err}"),
        }
    });
}

#[tauri::command]
pub fn fit_overlay_window() {
    let Some(window) = APP.get().and_then(|app| app.get_window("screenshot")) else {
        return;
    };
    place_screenshot_window(&window);
    let _ = window.set_always_on_top(true);
    #[cfg(target_os = "macos")]
    crate::capture_macos::cover_monitor(&window);
}

pub(crate) fn open_frozen_overlay() {
    if crate::long_screenshot::is_active() {
        warn!("ignoring screenshot request while scroll capture is active");
        return;
    }
    use std::sync::atomic::{AtomicBool, Ordering};
    static BUSY: AtomicBool = AtomicBool::new(false);
    if BUSY.swap(true, Ordering::SeqCst) {
        return;
    }
    let was_visible = dismiss_screenshot_window();
    std::thread::spawn(move || {
        if was_visible {
            // Return to the event loop first so macOS can drop the overlay
            // from the framebuffer before the capture runs.
            std::thread::sleep(std::time::Duration::from_millis(80));
        }
        let result = crate::screenshot::capture_for_overlay();
        let queued = crate::image_io::spawn_main(move || {
            BUSY.store(false, Ordering::SeqCst);
            match result {
                Ok(()) => {
                    let _ = screenshot_window();
                }
                Err(err) => crate::screenshot::report_capture_failure(&err),
            }
        });
        if queued.is_err() {
            BUSY.store(false, Ordering::SeqCst);
        }
    });
}

/// Hide the frozen screenshot before the long-capture worker starts reading the
/// live desktop. Returns whether it had been visible.
pub(crate) fn dismiss_screenshot_window() -> bool {
    let Some(app) = APP.get() else {
        return false;
    };
    let Some(window) = app.get_window("screenshot") else {
        return false;
    };
    let visible = window.is_visible().unwrap_or(false);
    #[cfg(target_os = "macos")]
    {
        // AppKit window messages must be sent on the main thread. The long
        // capture command runs on Tokio's worker pool.
        let window_for_main = window.clone();
        let _ = crate::image_io::spawn_main(move || {
            crate::capture_macos::order_out(&window_for_main);
        });
    }
    let _ = window.hide();
    let _ = window.set_ignore_cursor_events(false);
    visible
}

use std::sync::atomic::{AtomicBool, Ordering};

/// CSS-pixel rectangles that must keep receiving clicks (the toolbar) while
/// the rest of the overlay lets the page underneath handle the wheel.
static LONG_CAPTURE_HIT_RECTS: std::sync::Mutex<Vec<(f64, f64, f64, f64)>> =
    std::sync::Mutex::new(Vec::new());
static LONG_CAPTURE_PASS_STOP: AtomicBool = AtomicBool::new(true);
static LONG_CAPTURE_POLLER: AtomicBool = AtomicBool::new(false);
static LONG_CAPTURE_POLL_QUEUED: AtomicBool = AtomicBool::new(false);
static LONG_CAPTURE_CLICK_THROUGH: AtomicBool = AtomicBool::new(false);

pub(crate) fn set_long_capture_hit_rects(rects: &[(f64, f64, f64, f64)]) {
    *LONG_CAPTURE_HIT_RECTS.lock().unwrap() = rects.to_vec();
}

pub(crate) fn long_capture_click_through() -> bool {
    LONG_CAPTURE_CLICK_THROUGH.load(Ordering::SeqCst)
}

fn screenshot_handle() -> Option<Window> {
    APP.get()?.get_window("screenshot")
}

fn apply_click_through(window: &Window, click_through: bool) {
    LONG_CAPTURE_CLICK_THROUGH.store(click_through, Ordering::SeqCst);
    // Tauri's Windows implementation applies this on a later turn of the UI
    // thread. Set the extended style here so a wheel replay in the same call
    // already falls through to the window underneath.
    #[cfg(windows)]
    set_click_through_style(window, click_through);
    let _ = window.set_ignore_cursor_events(click_through);
}

#[cfg(windows)]
fn set_click_through_style(window: &Window, click_through: bool) {
    #[link(name = "user32")]
    extern "system" {
        fn GetWindowLongPtrW(hwnd: *mut std::ffi::c_void, index: i32) -> isize;
        fn SetWindowLongPtrW(hwnd: *mut std::ffi::c_void, index: i32, value: isize) -> isize;
        fn SetWindowPos(
            hwnd: *mut std::ffi::c_void,
            insert_after: *mut std::ffi::c_void,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            flags: u32,
        ) -> i32;
    }
    const GWL_EXSTYLE: i32 = -20;
    const WS_EX_LAYERED: isize = 0x0008_0000;
    const WS_EX_TRANSPARENT: isize = 0x0000_0020;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_FRAMECHANGED: u32 = 0x0020;
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    let hwnd = hwnd.0 as *mut std::ffi::c_void;
    unsafe {
        let mut style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        if click_through {
            style |= WS_EX_LAYERED | WS_EX_TRANSPARENT;
        } else {
            style &= !WS_EX_TRANSPARENT;
        }
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style);
        let _ = SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
        );
    }
}

fn cursor_over_controls(window: &Window) -> bool {
    let rects = LONG_CAPTURE_HIT_RECTS.lock().unwrap().clone();
    if rects.is_empty() {
        return false;
    }
    let (cursor_x, cursor_y) = mouse_physical_position();
    let Ok(origin) = window.inner_position() else {
        return false;
    };
    let scale = window.scale_factor().unwrap_or(1.0).max(0.5);
    // `mouse_position` reports Quartz points on macOS and physical pixels on
    // Windows. The window origin and the CSS rects are converted into the
    // same space as the cursor.
    #[cfg(target_os = "macos")]
    let (cursor_x, cursor_y, origin_x, origin_y, scale) = (
        cursor_x as f64,
        cursor_y as f64,
        origin.x as f64 / scale,
        origin.y as f64 / scale,
        1.0,
    );
    #[cfg(not(target_os = "macos"))]
    let (cursor_x, cursor_y, origin_x, origin_y) = (
        cursor_x as f64,
        cursor_y as f64,
        origin.x as f64,
        origin.y as f64,
    );
    let pad = 12.0 * scale;
    rects.iter().any(|(x, y, width, height)| {
        let left = origin_x + x * scale - pad;
        let top = origin_y + y * scale - pad;
        let right = left + width * scale + pad * 2.0;
        let bottom = top + height * scale + pad * 2.0;
        cursor_x >= left && cursor_x <= right && cursor_y >= top && cursor_y <= bottom
    })
}

fn refresh_click_through(window: Window) {
    if LONG_CAPTURE_PASS_STOP.load(Ordering::SeqCst) {
        apply_click_through(&window, false);
        return;
    }
    let click_through = !cursor_over_controls(&window);
    if click_through != LONG_CAPTURE_CLICK_THROUGH.load(Ordering::SeqCst) {
        apply_click_through(&window, click_through);
    }
}

fn ensure_click_through_poller() {
    if LONG_CAPTURE_POLLER
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    std::thread::spawn(|| loop {
        let stopping = LONG_CAPTURE_PASS_STOP.load(Ordering::SeqCst);
        std::thread::sleep(std::time::Duration::from_millis(if stopping {
            40
        } else {
            16
        }));
        if LONG_CAPTURE_PASS_STOP.load(Ordering::SeqCst) {
            if LONG_CAPTURE_CLICK_THROUGH.load(Ordering::SeqCst) {
                if let Some(window) = screenshot_handle() {
                    let _ = window.run_on_main_thread(move || apply_click_through(&window, false));
                }
            }
            continue;
        }
        let Some(window) = screenshot_handle() else {
            continue;
        };
        if LONG_CAPTURE_POLL_QUEUED.swap(true, Ordering::SeqCst) {
            continue;
        }
        let queued = window.run_on_main_thread(move || {
            LONG_CAPTURE_POLL_QUEUED.store(false, Ordering::SeqCst);
            refresh_click_through(window);
        });
        if queued.is_err() {
            LONG_CAPTURE_POLL_QUEUED.store(false, Ordering::SeqCst);
        }
    });
}

/// Runs on the main thread (the start command is synchronous). Keep the
/// overlay on screen, drop it out of the hit test, and exclude it from grabs.
pub(crate) fn enter_long_screenshot_mode() -> Result<(), String> {
    let window =
        screenshot_handle().ok_or_else(|| "screenshot window is not available".to_string())?;
    #[cfg(target_os = "macos")]
    {
        crate::capture_macos::prepare_scroll_capture(&window)?;
    }
    #[cfg(windows)]
    set_excluded_from_capture(&window, true);
    LONG_CAPTURE_PASS_STOP.store(false, Ordering::SeqCst);
    apply_click_through(&window, true);
    ensure_click_through_poller();
    Ok(())
}

pub(crate) fn leave_long_screenshot_mode() {
    LONG_CAPTURE_PASS_STOP.store(true, Ordering::SeqCst);
    LONG_CAPTURE_HIT_RECTS.lock().unwrap().clear();
    let _ = crate::image_io::spawn_main(|| {
        #[cfg(target_os = "macos")]
        crate::capture_macos::clear_scroll_overlay_window();
        if let Some(window) = screenshot_handle() {
            #[cfg(windows)]
            set_excluded_from_capture(&window, false);
            apply_click_through(&window, false);
        }
    });
}

#[cfg(windows)]
fn set_excluded_from_capture(window: &Window, exclude: bool) {
    // Tauri's `hwnd()` is `windows 0.39`, which is a different type from the
    // `windows 0.58` dependency used here. The handle is a pointer either way.
    #[link(name = "user32")]
    extern "system" {
        fn SetWindowDisplayAffinity(hwnd: *mut std::ffi::c_void, affinity: u32) -> i32;
    }
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    let affinity = if exclude { 0x11 } else { 0 };
    unsafe {
        let _ = SetWindowDisplayAffinity(hwnd.0 as *mut std::ffi::c_void, affinity);
    }
}

/// This command is synchronous, so Tauri invokes it on the main thread. Keep
/// all NSWindow messages and event posting here to avoid AppKit thread crashes.
pub(crate) fn forward_long_capture_scroll(
    delta_y: i32,
    center_x: f64,
    center_y: f64,
) -> Result<(), String> {
    let window =
        screenshot_handle().ok_or_else(|| "screenshot window is not available".to_string())?;
    #[cfg(target_os = "macos")]
    {
        return crate::capture_macos::forward_scroll(&window, delta_y, center_x, center_y);
    }
    #[cfg(windows)]
    {
        let _ = (center_x, center_y);
        return forward_scroll_windows(&window, delta_y);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (window, delta_y, center_x, center_y);
        Err("long_screenshot.unsupported_platform".into())
    }
}

#[cfg(windows)]
fn forward_scroll_windows(window: &Window, delta_y: i32) -> Result<(), String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_WHEEL, MOUSEINPUT,
    };
    if delta_y == 0 {
        return Ok(());
    }
    // The overlay consumed this wheel event. Let the next hit test fall
    // through, then replay the same distance. DOM deltaY is positive when
    // content moves down; WM_MOUSEWHEEL is positive for the opposite direction.
    apply_click_through(window, true);
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: (-delta_y) as u32,
                dwFlags: MOUSEEVENTF_WHEEL,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let sent = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
    if sent == 0 {
        return Err("scroll input was not delivered".into());
    }
    Ok(())
}

pub fn ocr_recognize() {
    crate::screenshot::prepare_screen_capture("ocr");
    open_frozen_overlay();
}
pub fn ocr_translate() {
    crate::screenshot::prepare_screen_capture("translate");
    open_frozen_overlay();
}

#[tauri::command(async)]
pub fn updater_window() {
    let (window, _exists) = build_window("updater", "Snapshoot");
    window
        .set_min_size(Some(tauri::LogicalSize::new(600, 400)))
        .unwrap();
    window.set_size(tauri::LogicalSize::new(600, 400)).unwrap();
    window.center().unwrap();
}
