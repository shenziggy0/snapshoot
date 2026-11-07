use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct WindowFrame {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

pub fn list_window_frames() -> Vec<WindowFrame> {
    #[cfg(windows)]
    {
        return list_windows_win();
    }
    #[cfg(target_os = "macos")]
    {
        list_windows_mac()
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Vec::new()
    }
}

#[cfg(windows)]
fn list_windows_win() -> Vec<WindowFrame> {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowLongPtrW, GetWindowRect, IsIconic, IsWindowVisible, GWL_EXSTYLE,
        WS_EX_TOOLWINDOW,
    };

    let mut frames = Vec::new();
    unsafe {
        let _ = EnumWindows(
            Some(enum_window),
            LPARAM(&mut frames as *mut Vec<WindowFrame> as isize),
        );
    }

    unsafe extern "system" fn enum_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let frames = &mut *(lparam.0 as *mut Vec<WindowFrame>);
        if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return true.into();
        }
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        if ex_style & WS_EX_TOOLWINDOW.0 != 0 {
            return true.into();
        }
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return true.into();
        }
        let width = rect.right.saturating_sub(rect.left);
        let height = rect.bottom.saturating_sub(rect.top);
        if width < 48 || height < 32 {
            return true.into();
        }
        frames.push(WindowFrame {
            x: rect.left,
            y: rect.top,
            width: width as u32,
            height: height as u32,
        });
        true.into()
    }

    frames
}

#[cfg(target_os = "macos")]
fn list_windows_mac() -> Vec<WindowFrame> {
    use std::ffi::CString;
    use std::os::raw::{c_char, c_void};

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFArrayGetCount(array: *const c_void) -> isize;
        fn CFArrayGetValueAtIndex(array: *const c_void, idx: isize) -> *const c_void;
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
        static kCGWindowBounds: *const c_void;
        static kCGWindowLayer: *const c_void;
    }
    #[link(name = "objc", kind = "dylib")]
    extern "C" {
        fn objc_getClass(name: *const c_char) -> *const c_void;
        fn sel_registerName(name: *const c_char) -> *const c_void;
        fn objc_msgSend();
    }

    const UTF8: u32 = 0x0800_0100;
    const FLOAT64: i32 = 6;
    const SINT64: i32 = 4;

    unsafe fn cf_key(name: &str) -> *const c_void {
        let c = CString::new(name).unwrap();
        CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), UTF8)
    }
    unsafe fn cf_f64(dict: *const c_void, key: *const c_void) -> f64 {
        let number = CFDictionaryGetValue(dict, key);
        if number.is_null() {
            return 0.0;
        }
        let mut value = 0.0f64;
        CFNumberGetValue(number, FLOAT64, &mut value as *mut f64 as *mut c_void);
        value
    }
    unsafe fn backing_scale() -> f64 {
        let class = objc_getClass(c"NSScreen".as_ptr());
        let main_sel = sel_registerName(c"mainScreen".as_ptr());
        let scale_sel = sel_registerName(c"backingScaleFactor".as_ptr());
        let msg_id: unsafe extern "C" fn(*const c_void, *const c_void) -> *const c_void =
            std::mem::transmute(objc_msgSend as *const ());
        let screen = msg_id(class, main_sel);
        if screen.is_null() {
            return 1.0;
        }
        let msg_f64: unsafe extern "C" fn(*const c_void, *const c_void) -> f64 =
            std::mem::transmute(objc_msgSend as *const ());
        let scale = msg_f64(screen, scale_sel);
        if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        }
    }

    unsafe {
        let array = CGWindowListCopyWindowInfo(1 | (1 << 4), 0);
        if array.is_null() {
            return Vec::new();
        }
        let scale = backing_scale();
        let xk = cf_key("X");
        let yk = cf_key("Y");
        let wk = cf_key("Width");
        let hk = cf_key("Height");
        let mut frames = Vec::new();
        let count = CFArrayGetCount(array);
        for index in 0..count {
            let dict = CFArrayGetValueAtIndex(array, index);
            if dict.is_null() {
                continue;
            }
            let layer_ref = CFDictionaryGetValue(dict, kCGWindowLayer);
            let mut layer = 0i64;
            if !layer_ref.is_null() {
                CFNumberGetValue(layer_ref, SINT64, &mut layer as *mut i64 as *mut c_void);
            }
            if layer != 0 {
                continue;
            }
            let bounds = CFDictionaryGetValue(dict, kCGWindowBounds);
            if bounds.is_null() {
                continue;
            }
            let x = cf_f64(bounds, xk) * scale;
            let y = cf_f64(bounds, yk) * scale;
            let width = cf_f64(bounds, wk) * scale;
            let height = cf_f64(bounds, hk) * scale;
            if width < 48.0 || height < 32.0 {
                continue;
            }
            frames.push(WindowFrame {
                x: x.round() as i32,
                y: y.round() as i32,
                width: width.round() as u32,
                height: height.round() as u32,
            });
        }
        CFRelease(xk);
        CFRelease(yk);
        CFRelease(wk);
        CFRelease(hk);
        CFRelease(array);
        frames
    }
}
