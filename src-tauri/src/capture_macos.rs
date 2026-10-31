use crate::screenshot::{rgba_is_blank, rgba_spread};
use core_graphics::display::CGDisplay;
use core_graphics::event::CGEvent;
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGRect;
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
    let pixel_width = display.pixels_wide().max(1);
    let pixel_height = display.pixels_high().max(1);
    let scale = pixel_width as f64 / bounds.size.width.max(1.0);
    CaptureGeometry {
        x: (bounds.origin.x * scale).round() as i32,
        y: (bounds.origin.y * scale).round() as i32,
        width: pixel_width as u32,
        height: pixel_height as u32,
    }
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
        let _: () = msg_send![ns_window, setContentMinSize: frame.size];
        let _: () = msg_send![ns_window, setContentMaxSize: frame.size];
        let _: () = msg_send![ns_window, setFrame:frame display:YES];
        let _: () = msg_send![ns_window, orderFrontRegardless];
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

fn ns_window(window: &tauri::Window) -> Option<*mut Object> {
    window.ns_window().ok().map(|raw| raw as *mut Object)
}

pub fn capture_to(path: &Path) -> Result<(), String> {
    let allowed = screen_recording_allowed();
    info!("screen recording preflight: {allowed}");
    let target = display_under_cursor()?;
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
    match capture_with_screencapture(&target, path) {
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
