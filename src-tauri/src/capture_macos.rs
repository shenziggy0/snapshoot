use crate::screenshot::{rgba_is_blank, rgba_spread};
use core_graphics::display::CGDisplay;
use core_graphics::event::{CGEvent, CGEventTapLocation, EventField, ScrollEventUnit};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use core_graphics::image::CGImage;
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
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

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

/// Window number of the capture overlay, so frame grabs can omit it.
static SCROLL_OVERLAY_WINDOW: AtomicU32 = AtomicU32::new(0);
/// Drops the echo of a scroll event we just posted.
static SUPPRESS_FORWARD_UNTIL_MS: AtomicU64 = AtomicU64::new(0);

pub fn clear_scroll_overlay_window() {
    SCROLL_OVERLAY_WINDOW.store(0, Ordering::SeqCst);
}

pub fn prepare_scroll_capture(window: &tauri::Window) -> Result<(), String> {
    exclude_from_capture(window);
    // Stay visible, but leave the hit test to the window underneath so a
    // trackpad gesture scrolls that window directly. Re-injecting the gesture
    // as a few mouse-wheel lines was both late and coarse.
    let Some(ns_window) = ns_window(window) else {
        return Ok(());
    };
    unsafe {
        let number: isize = msg_send![ns_window, windowNumber];
        if number > 0 {
            SCROLL_OVERLAY_WINDOW.store(number as u32, Ordering::SeqCst);
        }
        let _: () = msg_send![ns_window, orderFrontRegardless];
        let _: () = msg_send![ns_window, setIgnoresMouseEvents: YES];
        let _: () = msg_send![ns_window, setHidesOnDeactivate: NO];
    }
    Ok(())
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

/// Replay a wheel gesture the overlay swallowed (pointer over the toolbar).
/// The selection itself is click-through, so this is not on the path that
/// scrolls the page during a normal long capture.
///
/// The delta is in CSS pixels, the same unit as a DOM `wheel` event, and is
/// posted as a continuous pixel scroll to the process under the pointer.
/// Posting at the HID tap while our window is still hit-testable feeds the
/// event back into the page and multiplies the delta.
pub fn forward_scroll(
    _window: &tauri::Window,
    delta_y: i32,
    _center_x: f64,
    _center_y: f64,
) -> Result<(), String> {
    if delta_y == 0 {
        return Ok(());
    }
    let now = now_ms();
    if now < SUPPRESS_FORWARD_UNTIL_MS.load(Ordering::SeqCst) {
        return Ok(());
    }
    if !macos_accessibility_client::accessibility::application_is_trusted() {
        return Err("long_screenshot.accessibility".into());
    }
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .map_err(|_| "scroll event source".to_string())?;
    let pointer = CGEvent::new(source.clone())
        .map_err(|_| "mouse event".to_string())?
        .location();
    let event = CGEvent::new_scroll_event(source, ScrollEventUnit::PIXEL, 1, delta_y, 0, 0)
        .map_err(|_| "create scroll event".to_string())?;
    event.set_integer_value_field(EventField::SCROLL_WHEEL_EVENT_IS_CONTINUOUS, 1);
    event.set_integer_value_field(
        EventField::SCROLL_WHEEL_EVENT_POINT_DELTA_AXIS_1,
        i64::from(delta_y),
    );
    event.set_double_value_field(
        EventField::SCROLL_WHEEL_EVENT_FIXED_POINT_DELTA_AXIS_1,
        f64::from(delta_y),
    );
    unsafe {
        CGEventSetLocation(event.as_ptr(), pointer);
    }
    let skip = SCROLL_OVERLAY_WINDOW.load(Ordering::SeqCst);
    if let Some(pid) = pid_under_point(pointer, skip) {
        unsafe {
            CGEventPostToPid(pid, event.as_ptr());
        }
    } else {
        // No target process: post into the session and ignore the echo.
        SUPPRESS_FORWARD_UNTIL_MS.store(now.saturating_add(40), Ordering::SeqCst);
        event.post(CGEventTapLocation::Session);
    }
    info!(
        "forwarded scroll delta={delta_y} at pointer {:.0},{:.0}",
        pointer.x, pointer.y
    );
    Ok(())
}

fn pid_under_point(point: CGPoint, skip_window: u32) -> Option<i32> {
    use std::ffi::CString;
    use std::os::raw::{c_char, c_void};

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFArrayGetCount(array: *const c_void) -> isize;
        fn CFArrayGetValueAtIndex(array: *const c_void, index: isize) -> *const c_void;
        fn CFDictionaryGetValue(dict: *const c_void, key: *const c_void) -> *const c_void;
        fn CFRelease(cf: *const c_void);
        fn CFStringCreateWithCString(
            alloc: *const c_void,
            c_str: *const c_char,
            encoding: u32,
        ) -> *const c_void;
        fn CFNumberGetValue(number: *const c_void, the_type: i32, value: *mut c_void) -> u8;
    }
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGWindowListCopyWindowInfo(option: u32, relative: u32) -> *const c_void;
        static kCGWindowOwnerPID: *const c_void;
        static kCGWindowNumber: *const c_void;
        static kCGWindowBounds: *const c_void;
    }

    const UTF8: u32 = 0x0800_0100;
    const SINT32: i32 = 3;
    const SINT64: i32 = 4;
    const FLOAT64: i32 = 6;
    let our_pid = i64::from(std::process::id());

    unsafe {
        let cf_key = |name: &str| {
            let c_string = CString::new(name).unwrap_or_else(|_| CString::new("").unwrap());
            CFStringCreateWithCString(std::ptr::null(), c_string.as_ptr(), UTF8)
        };
        let cf_f64 = |dict: *const c_void, key: *const c_void| {
            let number = CFDictionaryGetValue(dict, key);
            if number.is_null() {
                return 0.0;
            }
            let mut value = 0.0f64;
            CFNumberGetValue(number, FLOAT64, &mut value as *mut f64 as *mut c_void);
            value
        };
        let cf_i64 = |dict: *const c_void, key: *const c_void| {
            let number = CFDictionaryGetValue(dict, key);
            if number.is_null() {
                return 0;
            }
            let mut value = 0i64;
            if CFNumberGetValue(number, SINT64, &mut value as *mut i64 as *mut c_void) == 0 {
                let mut value32 = 0i32;
                CFNumberGetValue(number, SINT32, &mut value32 as *mut i32 as *mut c_void);
                return i64::from(value32);
            }
            value
        };
        // kCGWindowListOptionOnScreenOnly. Front to back.
        let array = CGWindowListCopyWindowInfo(1, 0);
        if array.is_null() {
            return None;
        }
        let x_key = cf_key("X");
        let y_key = cf_key("Y");
        let w_key = cf_key("Width");
        let h_key = cf_key("Height");
        let mut found = None;
        let count = CFArrayGetCount(array);
        for index in 0..count {
            let dict = CFArrayGetValueAtIndex(array, index);
            if dict.is_null() {
                continue;
            }
            let window_number = cf_i64(dict, kCGWindowNumber);
            if window_number <= 0 || window_number as u32 == skip_window {
                continue;
            }
            let bounds = CFDictionaryGetValue(dict, kCGWindowBounds);
            if bounds.is_null() {
                continue;
            }
            let x = cf_f64(bounds, x_key);
            let y = cf_f64(bounds, y_key);
            let width = cf_f64(bounds, w_key);
            let height = cf_f64(bounds, h_key);
            let inside =
                point.x >= x && point.y >= y && point.x < x + width && point.y < y + height;
            if !inside {
                continue;
            }
            let pid = cf_i64(dict, kCGWindowOwnerPID);
            // Never post back into this process. That echo is what made a
            // small trackpad tick turn into a runaway scroll.
            if pid > 0 && pid != our_pid {
                found = Some(pid as i32);
                break;
            }
        }
        CFRelease(x_key);
        CFRelease(y_key);
        CFRelease(w_key);
        CFRelease(h_key);
        CFRelease(array);
        found
    }
}

extern "C" {
    fn CGEventSetLocation(event: core_graphics::sys::CGEventRef, location: CGPoint);
    fn CGEventPostToPid(pid: i32, event: core_graphics::sys::CGEventRef);
    fn CGDisplayPixelsWide(display: u32) -> usize;
    fn CGDisplayPixelsHigh(display: u32) -> usize;
}

/// Capture the fixed selection without a PNG round trip. The image is the
/// display below the overlay when that grab has the same pixel size as the
/// frozen screenshot; otherwise the full display image is cropped the same way.
pub fn capture_region_rgba(
    display_id: u32,
    full_width: u32,
    full_height: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<RgbaImage, String> {
    if let Some(image) = capture_below_overlay(display_id) {
        if image.width() as u32 == full_width && image.height() as u32 == full_height {
            if let Ok(region) = crop_cgimage(&image, x, y, width, height) {
                return Ok(region);
            }
        }
    }
    let image = CGDisplay::new(display_id)
        .image()
        .ok_or_else(|| "CGDisplayCreateImage returned nothing".to_string())?;
    if image.width() as u32 != full_width || image.height() as u32 != full_height {
        return Err("size_changed".into());
    }
    crop_cgimage(&image, x, y, width, height)
}

fn capture_below_overlay(display_id: u32) -> Option<CGImage> {
    use core_graphics::display::{
        kCGWindowImageBestResolution, kCGWindowImageBoundsIgnoreFraming,
        kCGWindowListOptionOnScreenBelowWindow,
    };
    let window_id = SCROLL_OVERLAY_WINDOW.load(Ordering::SeqCst);
    if window_id == 0 {
        return None;
    }
    let bounds = CGDisplay::new(display_id).bounds();
    CGDisplay::screenshot(
        bounds,
        kCGWindowListOptionOnScreenBelowWindow,
        window_id,
        kCGWindowImageBoundsIgnoreFraming | kCGWindowImageBestResolution,
    )
}

fn crop_cgimage(
    image: &CGImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<RgbaImage, String> {
    let full_w = image.width() as u32;
    let full_h = image.height() as u32;
    if width == 0
        || height == 0
        || x.saturating_add(width) > full_w
        || y.saturating_add(height) > full_h
    {
        return Err("selected region is outside the captured screen".into());
    }
    let rect = CGRect::new(
        &CGPoint::new(f64::from(x), f64::from(y)),
        &CGSize::new(f64::from(width), f64::from(height)),
    );
    let cropped = image
        .cropped(rect)
        .ok_or_else(|| "crop capture".to_string())?;
    cgimage_to_rgba(&cropped)
}

fn cgimage_to_rgba(image: &CGImage) -> Result<RgbaImage, String> {
    if image.bits_per_pixel() != 32 {
        return Err(format!("unexpected {} bpp", image.bits_per_pixel()));
    }
    let width = image.width() as u32;
    let height = image.height() as u32;
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
    RgbaImage::from_raw(width, height, rgba).ok_or_else(|| "rgba buffer".to_string())
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
    crate::image_io::diag_log(
        "capture_display_image",
        &format!("CGDisplayCreateImage => {width}x{height}"),
    );
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
    info!("screencapture wrote {}x{}", rgba.width(), rgba.height());
    Ok(())
}

pub(crate) fn screen_recording_allowed() -> bool {
    unsafe {
        let symbol = b"CGPreflightScreenCaptureAccess\0";
        let ptr = libc::dlsym(libc::RTLD_DEFAULT, symbol.as_ptr() as *const libc::c_char);
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
        let ptr = libc::dlsym(libc::RTLD_DEFAULT, symbol.as_ptr() as *const libc::c_char);
        if ptr.is_null() {
            return;
        }
        let request: unsafe extern "C" fn() -> u8 = std::mem::transmute(ptr);
        let _ = request();
    }
}
