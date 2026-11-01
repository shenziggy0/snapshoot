use crate::screenshot::{rgba_is_blank, rgba_spread};
use core_graphics::display::CGDisplay;
use core_graphics::event::{CGEvent, CGEventTapLocation, ScrollEventUnit};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::{CGRect, CGPoint};
use foreign_types::ForeignType;
use image::RgbaImage;
use log::{info, warn};
use objc::runtime::{Object, NO, YES};
use objc::{class, msg_send, sel, sel_impl};

#[repr(C)]
#[derive(Clone, Copy)]
struct NsPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct NsSize {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct NsRect {
    origin: NsPoint,
    size: NsSize,
}
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub struct CaptureGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

struct DisplayTarget {
    id: u32,
    bounds: CGRect,
    geometry: CaptureGeometry,
}

/// Display under the cursor, in the same point space `CGDisplayBounds` uses.
/// Tauri monitor origins are physical pixels, so they must not be compared
/// with these point values on a Retina display.
fn display_under_cursor() -> Result<DisplayTarget, String> {
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .map_err(|_| "mouse event source".to_string())?;
    let event = CGEvent::new(source).map_err(|_| "mouse event".to_string())?;
    let point = event.location();
    let ids = CGDisplay::active_displays().map_err(|err| format!("displays: {err}"))?;
    let mut fallback = None;
    for id in ids {
        let display = CGDisplay::new(id);
        let bounds = display.bounds();
        let target = DisplayTarget {
            id,
            bounds,
            geometry: geometry_for(&display, bounds),
        };
        if fallback.is_none() && display.is_main() {
            fallback = Some(target_clone(&target));
        }
        if point.x >= bounds.origin.x
            && point.y >= bounds.origin.y
            && point.x < bounds.origin.x + bounds.size.width
            && point.y < bounds.origin.y + bounds.size.height
        {
            return Ok(target);
        }
        if fallback.is_none() {
            fallback = Some(target);
        }
    }
    fallback.ok_or_else(|| "no display".to_string())
}

fn target_clone(target: &DisplayTarget) -> DisplayTarget {
    DisplayTarget {
        id: target.id,
        bounds: target.bounds,
        geometry: CaptureGeometry {
            x: target.geometry.x,
            y: target.geometry.y,
            width: target.geometry.width,
            height: target.geometry.height,
        },
    }
}

fn geometry_for(display: &CGDisplay, bounds: CGRect) -> CaptureGeometry {
    // `CGDisplay::pixels_wide()` reports the *point* width on some macOS
    // versions (1470 on a 2940-pixel Retina panel), so it cannot be trusted to
    // derive the backing scale. Use the real backing pixel dimensions instead.
    let pixel_width = display_pixels_wide(display.id).max(1);
    let pixel_height = display_pixels_high(display.id).max(1);
    let scale = pixel_width as f64 / bounds.size.width.max(1.0);
    crate::image_io::diag_log(
        "geometry_for",
        &format!(
            "id={} pixels_wide={} pixels_high={} bounds_w={:.1} scale={scale}",
            display.id, pixel_width, pixel_height, bounds.size.width
        ),
    );
    CaptureGeometry {
        x: (bounds.origin.x * scale).round() as i32,
        y: (bounds.origin.y * scale).round() as i32,
        width: pixel_width as u32,
        height: pixel_height as u32,
    }
}

fn display_pixels_wide(id: u32) -> u32 {
    unsafe { CGDisplayPixelsWide(id) as u32 }
}

fn display_pixels_high(id: u32) -> u32 {
    unsafe { CGDisplayPixelsHigh(id) as u32 }
}

pub fn overlay_geometry() -> Option<CaptureGeometry> {
    display_under_cursor().ok().map(|target| target.geometry)
}

pub fn exclude_from_capture(window: &tauri::Window) {
    let Some(ns_window) = ns_window(window) else {
        return;
    };
    unsafe {
        // NSWindowSharingNone. CGWindowListCreateImage skips this window.
        let _: () = msg_send![ns_window, setSharingType: 0isize];
    }
}

/// Borderless window covering the whole monitor, above the menu bar and Dock,
/// on every Space including other apps' fullscreen spaces.
pub fn cover_monitor(window: &tauri::Window) {
    let Some(ns_window) = ns_window(window) else {
        return;
    };
    unsafe {
        let mouse: NsPoint = msg_send![class!(NSEvent), mouseLocation];
        let screens: *mut Object = msg_send![class!(NSScreen), screens];
        if screens.is_null() {
            return;
        }
        let count: usize = msg_send![screens, count];
        if count == 0 {
            return;
        }
        let mut screen: *mut Object = msg_send![screens, objectAtIndex:0usize];
        for index in 0..count {
            let candidate: *mut Object = msg_send![screens, objectAtIndex:index];
            let frame: NsRect = msg_send![candidate, frame];
            let inside = mouse.x >= frame.origin.x
                && mouse.y >= frame.origin.y
                && mouse.x < frame.origin.x + frame.size.width
                && mouse.y < frame.origin.y + frame.size.height;
            if inside {
                screen = candidate;
                break;
            }
        }
        let frame: NsRect = msg_send![screen, frame];
        // NSWindowStyleMaskBorderless
        let _: () = msg_send![ns_window, setStyleMask: 0usize];
        // NSScreenSaverWindowLevel: above the menu bar and Dock.
        let _: () = msg_send![ns_window, setLevel: 1000isize];
        // All Spaces, auxiliary to fullscreen apps, not listed in the window cycle.
        let behavior: usize = 1 | (1 << 4) | (1 << 6) | (1 << 8);
        let _: () = msg_send![ns_window, setCollectionBehavior: behavior];
        let _: () = msg_send![ns_window, setHidesOnDeactivate: NO];
        let _: () = msg_send![ns_window, setMovable: NO];
        let clear: *mut Object = msg_send![class!(NSColor), clearColor];
        let _: () = msg_send![ns_window, setOpaque: NO];
        let _: () = msg_send![ns_window, setBackgroundColor: clear];
        let _: () = msg_send![ns_window, setContentMinSize: frame.size];
        let _: () = msg_send![ns_window, setContentMaxSize: frame.size];
        // Keep the window ordered out until the frozen frame is painted.
        let _: () = msg_send![ns_window, setFrame:frame display:NO];
        info!(
            "overlay frame {:.0},{:.0} {:.0}x{:.0}",
            frame.origin.x, frame.origin.y, frame.size.width, frame.size.height
        );
    }
}

pub fn order_out(window: &tauri::Window) {
    exclude_from_capture(window);
    let Some(ns_window) = ns_window(window) else {
        return;
    };
    unsafe {
        let _: () = msg_send![ns_window, orderOut: std::ptr::null::<Object>()];
    }
}

pub fn prepare_scroll_capture(window: &tauri::Window) -> Result<(), String> {
    if !macos_accessibility_client::accessibility::application_is_trusted() {
        return Err("long_screenshot.accessibility".into());
    }
    exclude_from_capture(window);
    // Make the overlay the key window so macOS routes wheel events to its
    // WKWebView instead of the application underneath it. Without this the
    // transparent overlay may never see the user's scroll gestures.
    let Some(ns_window) = ns_window(window) else {
        return Ok(());
    };
    unsafe {
        let _: () = msg_send![ns_window, makeKeyAndOrderFront: std::ptr::null::<Object>()];
        let _: () = msg_send![ns_window, setAcceptsMouseMovedEvents: YES];
    }
    let _ = window.set_focus();
    Ok(())
}

/// Tracks whether the overlay is currently ordered out for scroll forwarding,
/// and the timestamp of the most recent forwarded scroll. While the user keeps
/// scrolling the overlay stays hidden (no flicker); a background watcher
/// restores it once scrolling has been quiet for a moment.
static OVERLAY_HIDDEN: AtomicBool = AtomicBool::new(false);
static LAST_SCROLL_MS: AtomicU64 = AtomicU64::new(0);

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

/// Forward the overlay's wheel gesture to the application underneath it. The
/// caller runs on the main thread.
///
/// `center_x`/`center_y` are the selection center in global display *points*.
/// A scroll event created by `CGEventCreateScrollWheelEvent2` carries no
/// location (defaults to (0,0)), so posting it would route it to the menu bar.
/// We stamp the point-space location onto the event and inject it at the HID
/// tap so the WindowServer delivers it to whatever window is under that point.
///
/// To guarantee the injected event never lands back on our own overlay (which
/// would re-enter the frontend wheel handler and create a feedback loop of
/// ever-growing synthetic deltas), the overlay is ordered out while the user
/// is actively scrolling. While it is ordered out the WindowServer hit-tests
/// straight through to the application underneath. A background watcher brings
/// it back once scrolling has been quiet for 150ms, so a scroll burst causes
/// a single hide/show transition instead of one per wheel tick.
pub fn forward_scroll(
    window: &tauri::Window,
    delta_y: i32,
    _center_x: f64,
    _center_y: f64,
) -> Result<(), String> {
    if !macos_accessibility_client::accessibility::application_is_trusted() {
        return Err("long_screenshot.accessibility".into());
    }
    crate::image_io::diag_log(
        "forward_enter",
        &format!("delta={delta_y} at current pointer"),
    );
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .map_err(|_| "scroll event source".to_string())?;
    // Keep the user's pointer as the event location. Do not teleport the
    // pointer to the selection center: the selection may be in screenshot
    // pixels while the WindowServer expects screen points, and injecting an
    // off-screen location makes scrolling appear broken and causes the
    // pointer to jump. The user is already pointing at the content to scroll.
    let pointer = CGEvent::new(source.clone())
        .map_err(|_| "mouse event".to_string())?
        .location();
    // The line event direction is aligned with the DOM delta used by the
    // frontend so a downward gesture produces a downward capture sequence.
    // Line-based events are handled consistently by browsers and native
    // scroll views. Pixel/continuous events are frequently coalesced or
    // ignored when injected through the HID tap, which leaves the captured
    // frames unchanged until the user swipes very hard.
    let event = CGEvent::new_scroll_event(
        source,
        ScrollEventUnit::LINE,
        1,
        delta_y,
        0,
        0,
    )
    .map_err(|_| "create scroll event".to_string())?;
    unsafe {
        CGEventSetLocation(event.as_ptr(), pointer);
    }

    // Keep the overlay hidden while scrolling is active; the watcher thread
    // below restores it after the burst ends.
    LAST_SCROLL_MS.store(now_ms(), Ordering::SeqCst);
    if !OVERLAY_HIDDEN.swap(true, Ordering::SeqCst) {
        if let Some(ns_window) = ns_window(window) {
            unsafe {
                let _: () = msg_send![ns_window, orderOut: std::ptr::null::<Object>()];
            }
        }
    }
    event.post(CGEventTapLocation::HID);

    let watcher = window.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(60));
        let last = LAST_SCROLL_MS.load(Ordering::SeqCst);
        if now_ms().saturating_sub(last) < QUIET_RESTORE_MS {
            continue;
        }
        let restore_window = watcher.clone();
        let _ = watcher.run_on_main_thread(move || {
            if OVERLAY_HIDDEN.swap(false, Ordering::SeqCst) {
                if let Some(ns_window) = ns_window(&restore_window) {
                    unsafe {
                        let _: () =
                            msg_send![ns_window, makeKeyAndOrderFront: std::ptr::null::<Object>()];
                    }
                }
            }
        });
        break;
    });
    crate::image_io::diag_log("forward_posted", &format!("delta={delta_y}"));
    info!("forwarded scroll delta={delta_y} at pointer {:.0},{:.0}", pointer.x, pointer.y);
    Ok(())
}

const QUIET_RESTORE_MS: u64 = 150;

extern "C" {
    fn CGEventSetLocation(event: core_graphics::sys::CGEventRef, location: CGPoint);
    fn CGDisplayPixelsWide(display: u32) -> usize;
    fn CGDisplayPixelsHigh(display: u32) -> usize;
}

fn ns_window(window: &tauri::Window) -> Option<*mut Object> {
    window.ns_window().ok().map(|raw| raw as *mut Object)
}

pub fn capture_to(path: &Path) -> Result<(), String> {
    let target = display_under_cursor()?;
    capture_resolved(&target, path)
}

#[derive(Clone, Copy)]
pub struct DisplayCaptureTarget {
    pub id: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// Physical pixels per point (Retina = 2.0). Used to convert the
    /// selection's pixel coordinates back to the global point space that
    /// CGEventSetLocation expects.
    pub scale: f64,
}

pub fn capture_target_at_mouse() -> Result<DisplayCaptureTarget, String> {
    let target = display_under_cursor()?;
    // The region sent by the frontend is expressed in the same pixel space
    // as the image returned by CGDisplayCreateImage. On some Retina setups
    // that image is already downsampled to logical pixels (for example,
    // 1470x956), while NSScreen.backingScaleFactor still reports 2. Using
    // the latter would halve the event location and post scrolls to the
    // wrong part of the display. Derive the conversion from the actual
    // capture geometry instead.
    let scale = target.geometry.width as f64 / target.bounds.size.width.max(1.0);
    crate::image_io::diag_log(
        "event_scale",
        &format!(
            "id={} image={}x{} bounds={:.1}x{:.1} scale={scale:.3}",
            target.id,
            target.geometry.width,
            target.geometry.height,
            target.bounds.size.width,
            target.bounds.size.height
        ),
    );
    Ok(DisplayCaptureTarget {
        id: target.id,
        x: target.geometry.x,
        y: target.geometry.y,
        width: target.geometry.width,
        height: target.geometry.height,
        scale,
    })
}

pub fn capture_id_to(id: u32, path: &Path) -> Result<(), String> {
    let display = CGDisplay::new(id);
    let bounds = display.bounds();
    let target = DisplayTarget {
        id,
        bounds,
        geometry: geometry_for(&display, bounds),
    };
    capture_resolved(&target, path)
}

fn capture_resolved(target: &DisplayTarget, path: &Path) -> Result<(), String> {
    let allowed = screen_recording_allowed();
    info!("screen recording preflight: {allowed}");
    info!(
        "capture display {} points {:.0},{:.0} {:.0}x{:.0} pixels {}x{}",
        target.id,
        target.bounds.origin.x,
        target.bounds.origin.y,
        target.bounds.size.width,
        target.bounds.size.height,
        target.geometry.width,
        target.geometry.height
    );
    if allowed {
        match capture_display_image(target.id, path) {
            Ok(()) => return Ok(()),
            Err(err) => warn!("CGDisplayCreateImage unusable: {err}"),
        }
    }
    match capture_with_screencapture(target, path) {
        Ok(()) => Ok(()),
        Err(err) => {
            // The system prompt blocks a CI session where nobody can click it.
            if !allowed && std::env::var("SNAPSHOOT_SMOKE_TEST").ok().as_deref() != Some("1") {
                request_screen_recording();
            }
            Err(err)
        }
    }
}

fn capture_display_image(id: u32, path: &Path) -> Result<(), String> {
    let image = CGDisplay::new(id)
        .image()
        .ok_or_else(|| "CGDisplayCreateImage returned nothing".to_string())?;
    if image.bits_per_pixel() != 32 {
        return Err(format!("unexpected {} bpp", image.bits_per_pixel()));
    }
    let width = image.width() as u32;
    let height = image.height() as u32;
    crate::image_io::diag_log("capture_display_image", &format!("CGDisplayCreateImage => {width}x{height}"));
    if width == 0 || height == 0 {
        return Err("empty display image".into());
    }
    let row = image.bytes_per_row();
    let bgra = image.data();
    let src = bgra.bytes();
    let mut rgba = vec![0u8; (width as usize) * (height as usize) * 4];
    for y in 0..height as usize {
        for x in 0..width as usize {
            let i = y * row + x * 4;
            let o = (y * width as usize + x) * 4;
            if i + 3 >= src.len() {
                return Err("display image buffer is short".into());
            }
            rgba[o] = src[i + 2];
            rgba[o + 1] = src[i + 1];
            rgba[o + 2] = src[i];
            rgba[o + 3] = 255;
        }
    }
    if rgba_is_blank(&rgba, width, height) || rgba_spread(&rgba, width, height) < 8 {
        return Err("display image has no desktop detail".into());
    }
    RgbaImage::from_raw(width, height, rgba)
        .ok_or_else(|| "rgba buffer".to_string())?
        .save(path)
        .map_err(|err| format!("save capture: {err}"))?;
    Ok(())
}

fn capture_with_screencapture(target: &DisplayTarget, path: &Path) -> Result<(), String> {
    let x = target.bounds.origin.x.round() as i32;
    let y = target.bounds.origin.y.round() as i32;
    let w = target.bounds.size.width.round().max(1.0) as i32;
    let h = target.bounds.size.height.round().max(1.0) as i32;
    // One argv so a negative display origin is not parsed as another flag.
    let region = format!("-R{x},{y},{w},{h}");
    let output = Command::new("/usr/sbin/screencapture")
        .arg("-x")
        .arg(region)
        .arg(path)
        .output()
        .map_err(|err| format!("screencapture: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("screencapture exited {}: {stderr}", output.status));
    }
    let image = image::open(path).map_err(|err| format!("decode screencapture: {err}"))?;
    let rgba = image.to_rgba8();
    if rgba_is_blank(rgba.as_raw(), rgba.width(), rgba.height()) {
        let _ = std::fs::remove_file(path);
        return Err("screencapture image is black".into());
    }
    info!(
        "screencapture wrote {}x{}",
        rgba.width(),
        rgba.height()
    );
    Ok(())
}

fn screen_recording_allowed() -> bool {
    unsafe {
        let symbol = b"CGPreflightScreenCaptureAccess\0";
        let ptr = libc::dlsym(
            libc::RTLD_DEFAULT,
            symbol.as_ptr() as *const libc::c_char,
        );
        if ptr.is_null() {
            return true;
        }
        let check: unsafe extern "C" fn() -> u8 = std::mem::transmute(ptr);
        check() != 0
    }
}

fn request_screen_recording() {
    unsafe {
        let symbol = b"CGRequestScreenCaptureAccess\0";
        let ptr = libc::dlsym(
            libc::RTLD_DEFAULT,
            symbol.as_ptr() as *const libc::c_char,
        );
        if ptr.is_null() {
            return;
        }
        let request: unsafe extern "C" fn() -> u8 = std::mem::transmute(ptr);
        let _ = request();
    }
}
