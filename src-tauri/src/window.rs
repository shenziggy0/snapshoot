use crate::config::get;
use crate::config::set;
use crate::StringWrapper;
use crate::APP;
use log::{info, warn};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};
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
            // set_focus alone leaves a minimized window minimized.
            let _ = v.unminimize();
            let _ = v.show();
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
    }
    #[cfg(not(target_os = "macos"))]
    {
        #[cfg(windows)]
        if cover_monitor_win(window) {
            return;
        }
        let monitor = monitor_at_mouse();
        let pos = monitor.position();
        let size = monitor.size();
        let _ = window.set_position(tauri::PhysicalPosition::new(pos.x, pos.y));
        let _ = window.set_size(tauri::PhysicalSize::new(size.width, size.height));
    }
}

#[cfg(windows)]
fn cover_monitor_win(window: &Window) -> bool {
    let generation = OVERLAY_PLACEMENT_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let Ok(hwnd) = window.hwnd() else {
        return false;
    };
    unsafe {
        let raw = hwnd.0 as *mut std::ffi::c_void;
        let Some(rect) = monitor_rect_at_cursor_win() else {
            return false;
        };
        if !fit_monitor_rect_win(raw, rect) {
            return false;
        }
        // WM_DPICHANGED can apply its suggested size after the initial move.
        // Recheck on the event loop after DPI/layout messages have settled.
        // A newer placement invalidates pending checks for the previous screen.
        let window = window.clone();
        std::thread::spawn(move || {
            for delay in [40, 160] {
                std::thread::sleep(std::time::Duration::from_millis(delay));
                let posted = window.clone();
                let _ = window.run_on_main_thread(move || {
                    if OVERLAY_PLACEMENT_GENERATION.load(Ordering::SeqCst) != generation {
                        return;
                    }
                    if let Ok(hwnd) = posted.hwnd() {
                        let raw = hwnd.0 as *mut std::ffi::c_void;
                        let mut actual: WinRect = std::mem::zeroed();
                        if GetWindowRect(raw, &mut actual) != 0 && actual != rect {
                            if !fit_monitor_rect_win(raw, rect) {
                                warn!("overlay monitor rect did not settle: expected {rect:?}");
                            }
                        }
                    }
                });
            }
        });
    }
    true
}

#[cfg(windows)]
unsafe fn monitor_rect_at_cursor_win() -> Option<WinRect> {
    // A new overlay starts on the primary display; select by cursor instead.
    let mut cursor = WinPoint { x: 0, y: 0 };
    if GetCursorPos(&mut cursor) == 0 {
        return None;
    }
    let monitor = MonitorFromPoint(cursor, 2); // MONITOR_DEFAULTTONEAREST
    if monitor.is_null() {
        return None;
    }
    let mut info: MonitorInfo = std::mem::zeroed();
    info.cb_size = std::mem::size_of::<MonitorInfo>() as u32;
    if GetMonitorInfoW(monitor, &mut info) == 0 {
        return None;
    }
    Some(info.rc_monitor)
}

#[cfg(windows)]
static OVERLAY_PLACEMENT_GENERATION: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

#[cfg(windows)]
unsafe fn fit_monitor_rect_win(raw: *mut std::ffi::c_void, rect: WinRect) -> bool {
    for _ in 0..2 {
        if SetWindowPos(
            raw,
            (-1isize) as *mut std::ffi::c_void,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            0x0010 | 0x0020,
        ) == 0
        {
            return false;
        }
        let mut actual: WinRect = std::mem::zeroed();
        if GetWindowRect(raw, &mut actual) != 0 && actual == rect {
            return true;
        }
    }
    false
}

#[cfg(windows)]
#[repr(C)]
struct WinPoint {
    x: i32,
    y: i32,
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    fn GetCursorPos(point: *mut WinPoint) -> i32;
    fn MonitorFromPoint(point: WinPoint, flags: u32) -> *mut std::ffi::c_void;
    fn GetWindowRect(hwnd: *mut std::ffi::c_void, rect: *mut WinRect) -> i32;
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

#[derive(Debug)]
struct OverlayGate {
    current: u64,
    busy: bool,
    ready: bool,
    revealed: bool,
    started: Option<std::time::Instant>,
}

impl OverlayGate {
    const fn new() -> Self {
        Self {
            current: 0,
            busy: false,
            ready: false,
            revealed: true,
            started: None,
        }
    }

    fn begin(&mut self) -> Option<u64> {
        if self.busy {
            return None;
        }
        self.current = self.current.wrapping_add(1);
        self.busy = true;
        self.ready = false;
        self.revealed = false;
        self.started = Some(std::time::Instant::now());
        Some(self.current)
    }

    fn mark_ready(&mut self, request_id: u64) -> bool {
        if self.current != request_id || !self.busy {
            return false;
        }
        self.ready = true;
        true
    }

    fn can_prepare(&self, request_id: u64) -> bool {
        self.current == request_id && self.ready && self.busy && !self.revealed
    }

    fn can_reveal(&self, request_id: u64) -> bool {
        self.current == request_id && self.ready
    }

    fn complete(&mut self, request_id: u64) -> bool {
        if self.current != request_id || !self.ready {
            return false;
        }
        self.revealed = true;
        self.busy = false;
        true
    }

    fn fallback(&mut self, request_id: u64) -> bool {
        if self.current != request_id || !self.ready || self.revealed {
            return false;
        }
        self.revealed = true;
        self.busy = false;
        true
    }

    fn abort(&mut self, request_id: u64) {
        if self.current == request_id {
            self.busy = false;
            self.ready = false;
            self.revealed = true;
            self.started = None;
        }
    }

    fn cancel(&mut self) {
        self.current = self.current.wrapping_add(1);
        self.busy = false;
        self.ready = false;
        self.revealed = true;
        self.started = None;
    }

    fn snapshot(&self) -> (u64, bool) {
        (self.current, self.ready)
    }

    fn elapsed_ms(&self, request_id: u64) -> Option<u128> {
        if self.current != request_id {
            return None;
        }
        self.started.map(|started| started.elapsed().as_millis())
    }
}

static OVERLAY_GATE: Mutex<OverlayGate> = Mutex::new(OverlayGate::new());
static OVERLAY_FRAME_READY: AtomicBool = AtomicBool::new(false);
static OVERLAY_WATCHDOG_USED: AtomicBool = AtomicBool::new(false);

fn overlay_gate() -> MutexGuard<'static, OverlayGate> {
    OVERLAY_GATE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub(crate) fn overlay_boot_state() -> (u64, bool) {
    overlay_gate().snapshot()
}

pub(crate) fn last_overlay_frame_ready() -> bool {
    OVERLAY_FRAME_READY.load(Ordering::SeqCst)
}

pub(crate) fn overlay_watchdog_used() -> bool {
    OVERLAY_WATCHDOG_USED.load(Ordering::SeqCst)
}

fn begin_overlay_request() -> Option<u64> {
    let request_id = overlay_gate().begin();
    if request_id.is_some() {
        OVERLAY_FRAME_READY.store(false, Ordering::SeqCst);
        OVERLAY_WATCHDOG_USED.store(false, Ordering::SeqCst);
    }
    request_id
}

fn build_screenshot_window() -> Result<Window, tauri::Error> {
    let app_handle = APP.get().unwrap();
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

    let window = builder.build()?;
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            crate::long_screenshot::abandon_active();
            crate::screen_record::abandon_active();
            overlay_gate().cancel();
        }
    });
    place_screenshot_window(&window);
    Ok(window)
}

/// Create the hidden screenshot WebView during idle time. The first shortcut
/// then only reloads a warm page instead of paying WebView startup in the
/// latency-sensitive capture path.
pub(crate) fn prewarm_screenshot_window() {
    let Some(app) = APP.get() else {
        return;
    };
    if app.get_window("screenshot").is_some() {
        return;
    }
    match build_screenshot_window() {
        Ok(window) => {
            #[cfg(target_os = "macos")]
            crate::capture_macos::set_window_alpha(&window, 0.0);
            info!("screenshot overlay prewarmed");
        }
        Err(err) => warn!("prewarm screenshot overlay failed: {err}"),
    }
}

/// Capture is finished before this runs. The page owns the reveal: it first
/// shows a transparent window, waits for the canvas frame, then asks Rust to
/// make that same window visible. A slow watchdog exists only for a broken
/// page, never as a second normal display path.
fn screenshot_window_for_request(request_id: u64) -> Option<Window> {
    if !overlay_gate().mark_ready(request_id) {
        return None;
    }

    let app_handle = APP.get()?;
    if let Some(window) = app_handle.get_window("screenshot") {
        place_screenshot_window(&window);
        #[cfg(target_os = "macos")]
        crate::capture_macos::set_window_alpha(&window, 0.0);
        let _ = window.set_always_on_top(true);
        let _ = window.emit("screenshot-reload", request_id);
        arm_overlay_reveal_watchdog(request_id, window.clone());
        return Some(window);
    }

    let window = match build_screenshot_window() {
        Ok(window) => window,
        Err(err) => {
            warn!("create screenshot window failed: {err}");
            overlay_gate().abort(request_id);
            return None;
        }
    };
    #[cfg(target_os = "macos")]
    crate::capture_macos::set_window_alpha(&window, 0.0);
    arm_overlay_reveal_watchdog(request_id, window.clone());
    Some(window)
}

fn arm_overlay_reveal_watchdog(request_id: u64, window: Window) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(2500));
        if !overlay_gate().fallback(request_id) {
            return;
        }
        let elapsed = overlay_gate()
            .elapsed_ms(request_id)
            .map(|value| format!("{value}ms"))
            .unwrap_or_else(|| "unknown".to_string());
        OVERLAY_FRAME_READY.store(false, Ordering::SeqCst);
        OVERLAY_WATCHDOG_USED.store(true, Ordering::SeqCst);
        warn!("overlay first frame was not reported after 2500ms; revealing with watchdog ({elapsed})");
        let shown = window.clone();
        let _ = crate::image_io::spawn_main(move || {
            let _ = shown.show();
            #[cfg(target_os = "macos")]
            crate::capture_macos::set_window_alpha(&shown, 1.0);
            let _ = shown.set_focus();
            let _ = shown.set_always_on_top(true);
            let _ = shown.set_ignore_cursor_events(false);
        });
    });
}

#[tauri::command]
pub fn prepare_overlay_display(request_id: u64) -> bool {
    let ready = {
        let gate = overlay_gate();
        if gate.current == request_id && gate.ready && gate.revealed {
            return true;
        }
        gate.can_prepare(request_id)
    };
    if !ready {
        return false;
    }
    let Some(window) = APP.get().and_then(|app| app.get_window("screenshot")) else {
        return false;
    };
    #[cfg(target_os = "macos")]
    crate::capture_macos::set_window_alpha(&window, 0.0);
    let _ = window.set_ignore_cursor_events(true);
    window.show().is_ok()
}

#[tauri::command]
pub fn reveal_overlay(request_id: u64, image_loaded: bool) -> bool {
    if !overlay_gate().can_reveal(request_id) {
        return false;
    }
    let elapsed = overlay_gate()
        .elapsed_ms(request_id)
        .map(|value| format!("{value}ms"))
        .unwrap_or_else(|| "unknown".to_string());
    let Some(window) = APP.get().and_then(|app| app.get_window("screenshot")) else {
        overlay_gate().abort(request_id);
        return false;
    };
    let _ = window.show();
    #[cfg(target_os = "macos")]
    crate::capture_macos::set_window_alpha(&window, 1.0);
    let _ = window.set_ignore_cursor_events(false);
    let _ = window.set_focus();
    let _ = window.set_always_on_top(true);
    OVERLAY_FRAME_READY.store(image_loaded, Ordering::SeqCst);
    let completed = overlay_gate().complete(request_id);
    if completed {
        info!("overlay revealed request={request_id} image={image_loaded} after {elapsed}");
    }
    completed
}

#[tauri::command]
pub fn hide_screenshot_window() {
    overlay_gate().cancel();
    OVERLAY_FRAME_READY.store(false, Ordering::SeqCst);
    OVERLAY_WATCHDOG_USED.store(false, Ordering::SeqCst);
    crate::long_screenshot::abandon_active();
    crate::screen_record::abandon_active();
    if let Some(window) = APP.get().and_then(|app| app.get_window("screenshot")) {
        let _ = window.hide();
    }
    crate::quick_access::show_pending();
}

pub(crate) fn open_edit_overlay(path: &std::path::Path) -> Option<Window> {
    let request_id = begin_overlay_request()?;
    crate::screenshot::prepare_edit_image(path);
    screenshot_window_for_request(request_id)
}

/// Used by the smoke test; the screenshot hotkey path below allocates its
/// request before the background capture.
pub(crate) fn screenshot_window() -> Option<Window> {
    let request_id = begin_overlay_request()?;
    screenshot_window_for_request(request_id)
}

pub(crate) fn open_frozen_overlay(intent: &str) {
    if crate::long_screenshot::is_active() || crate::screen_record::is_active() {
        warn!("ignoring screenshot request while scroll capture is active");
        return;
    }
    crate::quick_access::hide_for_capture();
    let Some(request_id) = begin_overlay_request() else {
        warn!("ignoring screenshot request while the previous overlay is still loading");
        return;
    };
    // Snapshot the intent only after this request owns the gate. A duplicate
    // shortcut must not overwrite the capture that is already in flight.
    crate::screenshot::prepare_screen_capture(intent);
    let was_visible = dismiss_screenshot_window();
    std::thread::spawn(move || {
        let capture_started = std::time::Instant::now();
        if was_visible {
            // Give WindowServer two frames to composite the removed overlay.
            // The window normally stays hidden between captures, so this path
            // only runs when a new capture replaces a visible frozen frame.
            std::thread::sleep(std::time::Duration::from_millis(32));
        }
        let result = crate::screenshot::capture_for_overlay();
        crate::image_io::diag_log(
            "overlay_capture",
            &format!(
                "request={request_id} visible_retry={was_visible} elapsed={}ms ok={}",
                capture_started.elapsed().as_millis(),
                result.is_ok()
            ),
        );
        let queued = crate::image_io::spawn_main(move || match result {
            Ok(()) => {
                if screenshot_window_for_request(request_id).is_none() {
                    warn!("dropping stale screenshot request {request_id}");
                }
            }
            Err(err) => {
                dismiss_screenshot_window();
                overlay_gate().abort(request_id);
                crate::screenshot::report_capture_failure(&err);
            }
        });
        if queued.is_err() {
            overlay_gate().abort(request_id);
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

/// CSS-pixel rectangles that must keep receiving clicks (the toolbar) while
/// the rest of the overlay lets the page underneath handle the wheel.
static OVERLAY_HIT_RECTS: std::sync::Mutex<Vec<(f64, f64, f64, f64)>> =
    std::sync::Mutex::new(Vec::new());
static OVERLAY_PASS_STOP: AtomicBool = AtomicBool::new(true);
static OVERLAY_POLLER: AtomicBool = AtomicBool::new(false);
static OVERLAY_POLL_QUEUED: AtomicBool = AtomicBool::new(false);
static OVERLAY_CLICK_THROUGH: AtomicBool = AtomicBool::new(false);

pub(crate) fn set_overlay_hit_rects(rects: &[(f64, f64, f64, f64)]) {
    *OVERLAY_HIT_RECTS.lock().unwrap() = rects.to_vec();
}

pub(crate) fn long_capture_click_through() -> bool {
    OVERLAY_CLICK_THROUGH.load(Ordering::SeqCst)
}

fn screenshot_handle() -> Option<Window> {
    APP.get()?.get_window("screenshot")
}

fn apply_click_through(window: &Window, click_through: bool) {
    OVERLAY_CLICK_THROUGH.store(click_through, Ordering::SeqCst);
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
    let rects = OVERLAY_HIT_RECTS.lock().unwrap().clone();
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
    if OVERLAY_PASS_STOP.load(Ordering::SeqCst) {
        apply_click_through(&window, false);
        return;
    }
    let click_through = !cursor_over_controls(&window);
    if click_through != OVERLAY_CLICK_THROUGH.load(Ordering::SeqCst) {
        apply_click_through(&window, click_through);
    }
}

fn ensure_click_through_poller() {
    if OVERLAY_POLLER
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    std::thread::spawn(|| loop {
        let stopping = OVERLAY_PASS_STOP.load(Ordering::SeqCst);
        std::thread::sleep(std::time::Duration::from_millis(if stopping {
            40
        } else {
            16
        }));
        if OVERLAY_PASS_STOP.load(Ordering::SeqCst) {
            if OVERLAY_CLICK_THROUGH.load(Ordering::SeqCst) {
                if let Some(window) = screenshot_handle() {
                    let posted = window.clone();
                    let _ = posted.run_on_main_thread(move || apply_click_through(&window, false));
                }
            }
            continue;
        }
        let Some(window) = screenshot_handle() else {
            continue;
        };
        if OVERLAY_POLL_QUEUED.swap(true, Ordering::SeqCst) {
            continue;
        }
        let queued = window.clone().run_on_main_thread(move || {
            OVERLAY_POLL_QUEUED.store(false, Ordering::SeqCst);
            refresh_click_through(window);
        });
        if queued.is_err() {
            OVERLAY_POLL_QUEUED.store(false, Ordering::SeqCst);
        }
    });
}

/// Runs on the main thread (the start command is synchronous). Keep the
/// overlay on screen, drop it out of the hit test, and exclude it from grabs.
pub(crate) fn enter_long_screenshot_mode() -> Result<(), String> {
    if crate::screen_record::is_active() {
        return Err("screen_record.already_running".into());
    }
    let window =
        screenshot_handle().ok_or_else(|| "screenshot window is not available".to_string())?;
    #[cfg(target_os = "macos")]
    {
        crate::capture_macos::prepare_scroll_capture(&window)?;
    }
    #[cfg(windows)]
    let _ = set_excluded_from_capture(&window, true);
    OVERLAY_PASS_STOP.store(false, Ordering::SeqCst);
    apply_click_through(&window, true);
    ensure_click_through_poller();
    Ok(())
}

pub(crate) fn leave_long_screenshot_mode() {
    OVERLAY_PASS_STOP.store(true, Ordering::SeqCst);
    OVERLAY_HIT_RECTS.lock().unwrap().clear();
    let _ = crate::image_io::spawn_main(|| {
        #[cfg(target_os = "macos")]
        crate::capture_macos::clear_scroll_overlay_window();
        if let Some(window) = screenshot_handle() {
            #[cfg(windows)]
            let _ = set_excluded_from_capture(&window, false);
            apply_click_through(&window, false);
        }
    });
}

pub(crate) fn enter_record_mode() -> Result<(), String> {
    let window = screenshot_handle().ok_or("screen_record.encoder_failed")?;
    #[cfg(windows)]
    set_excluded_from_capture(&window, true)?;
    #[cfg(target_os = "macos")]
    crate::capture_macos::exclude_from_capture(&window);
    OVERLAY_PASS_STOP.store(false, Ordering::SeqCst);
    apply_click_through(&window, true);
    ensure_click_through_poller();
    Ok(())
}

pub(crate) fn leave_record_mode(close: bool) {
    OVERLAY_PASS_STOP.store(true, Ordering::SeqCst);
    OVERLAY_HIT_RECTS.lock().unwrap().clear();
    let _ = crate::image_io::spawn_main(move || {
        if let Some(window) = screenshot_handle() {
            #[cfg(windows)]
            let _ = set_excluded_from_capture(&window, false);
            apply_click_through(&window, false);
            if close {
                let _ = window.close();
            }
        }
        crate::screen_record::finish_overlay_transition();
    });
}

pub(crate) fn record_result_window(clipboard_ok: bool) {
    let (window, _) = build_window("record_result", "Snapshoot");
    let monitor = monitor_at_mouse();
    let scale = monitor.scale_factor();
    let width = 380.0;
    let height = 220.0;
    let _ = window.set_size(tauri::LogicalSize::new(width, height));
    let _ = window.set_resizable(false);
    let _ = window.set_always_on_top(true);
    let _ = window.set_skip_taskbar(true);
    let _ = window.set_position(tauri::PhysicalPosition::new(
        monitor.position().x + monitor.size().width as i32 - ((width + 24.0) * scale) as i32,
        monitor.position().y + monitor.size().height as i32 - ((height + 64.0) * scale) as i32,
    ));
    let _ = window.show();
    // Store notification state in the URL-independent window event. The result
    // page also reads the Rust-owned result on mount, avoiding event races.
    let _ = window.emit("record-result-ready", clipboard_ok);
}

#[cfg(windows)]
fn set_excluded_from_capture(window: &Window, exclude: bool) -> Result<(), String> {
    // Tauri's `hwnd()` is `windows 0.39`, which is a different type from the
    // `windows 0.58` dependency used here. The handle is a pointer either way.
    #[link(name = "user32")]
    extern "system" {
        fn SetWindowDisplayAffinity(hwnd: *mut std::ffi::c_void, affinity: u32) -> i32;
    }
    let Ok(hwnd) = window.hwnd() else {
        return Err("window handle is not available".into());
    };
    let affinity = if exclude { 0x11 } else { 0 };
    unsafe {
        if SetWindowDisplayAffinity(hwnd.0 as *mut std::ffi::c_void, affinity) == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
    }
    Ok(())
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
        crate::capture_macos::forward_scroll(&window, delta_y, center_x, center_y)
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
    open_frozen_overlay("ocr");
}
pub fn ocr_translate() {
    open_frozen_overlay("translate");
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

#[cfg(test)]
mod overlay_gate_tests {
    use super::OverlayGate;

    #[test]
    fn accepts_only_one_capture_until_the_first_frame_is_revealed() {
        let mut gate = OverlayGate::new();
        let first = gate.begin().expect("first request");
        assert!(
            gate.begin().is_none(),
            "capture must not re-enter while busy"
        );
        assert!(!gate.can_prepare(first), "capture file is not ready yet");
        assert!(gate.mark_ready(first));
        assert!(gate.can_prepare(first));
        assert!(gate.complete(first));
        assert!(gate.begin().is_some(), "next shortcut after reveal");
    }

    #[test]
    fn stale_page_cannot_reveal_or_hide_a_newer_capture() {
        let mut gate = OverlayGate::new();
        let first = gate.begin().expect("first request");
        assert!(gate.mark_ready(first));
        assert!(gate.complete(first));

        let second = gate.begin().expect("second request");
        assert!(gate.mark_ready(second));
        assert!(!gate.can_reveal(first));
        assert!(!gate.complete(first));
        assert!(gate.can_reveal(second));
        assert!(gate.complete(second));
    }

    #[test]
    fn watchdog_reveals_once_and_releases_the_busy_gate() {
        let mut gate = OverlayGate::new();
        let request = gate.begin().expect("request");
        assert!(gate.mark_ready(request));
        assert!(gate.fallback(request));
        assert!(!gate.fallback(request), "watchdog must be idempotent");
        assert!(!gate.can_prepare(request));
        assert!(gate.begin().is_some());
    }

    #[test]
    fn hiding_an_overlay_invalidates_its_watchdog() {
        let mut gate = OverlayGate::new();
        let request = gate.begin().expect("request");
        assert!(gate.mark_ready(request));
        gate.cancel();
        assert!(!gate.fallback(request));
        assert!(gate.begin().is_some());
    }
}
