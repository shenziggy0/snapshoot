use super::{RecordOptions, RecordResult, RecordState, RecordStatus, DISK_FAILED, ENCODER_FAILED};
use std::ffi::{c_char, c_void, CString};
use std::path::Path;
use std::ptr::NonNull;

extern "C" {
    fn sr_supported() -> i32;
    fn sr_audio_supported() -> i32;
    fn sr_microphone_supported() -> i32;
    fn sr_start(
        display: u32,
        scale: f64,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        output_width: u32,
        output_height: u32,
        fps: u32,
        bitrate: u32,
        cursor: i32,
        audio: i32,
        microphone: i32,
        path: *const c_char,
        handle: *mut *mut c_void,
    ) -> i32;
    fn sr_status(
        handle: *mut c_void,
        state: *mut u32,
        elapsed_ms: *mut u64,
        error: *mut i32,
    ) -> i32;
    fn sr_pause(handle: *mut c_void, paused: i32) -> i32;
    fn sr_finish(handle: *mut c_void, cancel: i32, elapsed_ms: *mut u64) -> i32;
    fn sr_release(handle: *mut c_void);
    fn sr_copy_file(path: *const c_char) -> i32;
    fn sr_reveal(path: *const c_char) -> i32;
}

fn error_key(code: i32) -> &'static str {
    match code {
        1 => "screen_record.unsupported",
        2 => "screen_record.permission_denied",
        4 => DISK_FAILED,
        _ => ENCODER_FAILED,
    }
}

fn check(code: i32) -> Result<(), String> {
    if code == 0 {
        Ok(())
    } else {
        Err(error_key(code).into())
    }
}

fn file_name(path: &Path) -> Result<CString, String> {
    CString::new(path.to_str().ok_or(DISK_FAILED)?).map_err(|_| DISK_FAILED.into())
}

pub fn supported() -> bool {
    unsafe { sr_supported() != 0 }
}
pub fn audio_supported() -> bool {
    unsafe { sr_audio_supported() != 0 }
}
pub fn microphone_supported() -> bool {
    unsafe { sr_microphone_supported() != 0 }
}

pub struct Recorder {
    handle: Option<NonNull<c_void>>,
    options: RecordOptions,
}

// The opaque object owns a serial dispatch queue. Its C entry points marshal
// capture/writer/state access to that queue; no AppKit objects live here.
unsafe impl Send for Recorder {}

impl Recorder {
    pub fn start(options: RecordOptions) -> Result<Self, String> {
        if !supported() {
            return Err("screen_record.unsupported".into());
        }
        let path = file_name(&options.path)?;
        // The manager reserves an empty file. AVAssetWriter requires a path
        // which does not exist; never overwrite a nonempty recording.
        match std::fs::metadata(&options.path) {
            Ok(metadata) if metadata.len() == 0 => {
                std::fs::remove_file(&options.path).map_err(|_| DISK_FAILED)?
            }
            Ok(_) => return Err(DISK_FAILED.into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(DISK_FAILED.into()),
        }
        let mut raw = std::ptr::null_mut();
        let r = options.region;
        check(unsafe {
            sr_start(
                options.target.display_id,
                options.target.scale,
                r.x,
                r.y,
                r.width,
                r.height,
                options.width,
                options.height,
                options.fps,
                options.bitrate,
                i32::from(options.cursor),
                i32::from(options.audio),
                i32::from(options.microphone),
                path.as_ptr(),
                &mut raw,
            )
        })?;
        let handle = NonNull::new(raw).ok_or(ENCODER_FAILED)?;
        Ok(Self {
            handle: Some(handle),
            options,
        })
    }

    pub fn status(&self) -> RecordStatus {
        let mut state = 3;
        let mut elapsed_ms = 0;
        let mut error = 0;
        let result = self
            .handle
            .map(|h| unsafe { sr_status(h.as_ptr(), &mut state, &mut elapsed_ms, &mut error) })
            .unwrap_or(3);
        if result != 0 {
            error = result;
            state = 3;
        }
        RecordStatus {
            state: match state {
                0 => RecordState::Recording,
                1 => RecordState::Paused,
                2 => RecordState::Finalizing,
                _ => RecordState::Error,
            },
            elapsed_ms,
            width: self.options.width,
            height: self.options.height,
            message: if error == 0 {
                None
            } else {
                Some(error_key(error).into())
            },
        }
    }

    pub fn pause(&mut self) -> Result<(), String> {
        self.set_paused(true)
    }
    pub fn resume(&mut self) -> Result<(), String> {
        self.set_paused(false)
    }
    fn set_paused(&self, paused: bool) -> Result<(), String> {
        let handle = self.handle.ok_or(ENCODER_FAILED)?;
        check(unsafe { sr_pause(handle.as_ptr(), i32::from(paused)) })
    }

    pub fn stop(&mut self) -> Result<RecordResult, String> {
        let handle = self.handle.take().ok_or(ENCODER_FAILED)?;
        let mut duration_ms = 0;
        let code = unsafe { sr_finish(handle.as_ptr(), 0, &mut duration_ms) };
        unsafe {
            sr_release(handle.as_ptr());
        }
        if let Err(err) = check(code) {
            let _ = std::fs::remove_file(&self.options.path);
            return Err(err);
        }
        Ok(RecordResult {
            path: self.options.path.to_string_lossy().into_owned(),
            width: self.options.width,
            height: self.options.height,
            fps: self.options.fps,
            duration_ms,
            size_bytes: std::fs::metadata(&self.options.path)
                .map_err(|_| DISK_FAILED)?
                .len(),
            clipboard_copied: false,
        })
    }

    pub fn cancel(&mut self) -> Result<(), String> {
        let mut native_result = Ok(());
        if let Some(handle) = self.handle.take() {
            let mut duration_ms = 0;
            let code = unsafe { sr_finish(handle.as_ptr(), 1, &mut duration_ms) };
            unsafe {
                sr_release(handle.as_ptr());
            }
            native_result = check(code);
        }
        // Even a native timeout must not skip unlinking our partial output.
        match std::fs::remove_file(&self.options.path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(DISK_FAILED.into()),
        }
        native_result
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        // Drop can run on the UI thread: release schedules cancellation without
        // waiting for ScreenCaptureKit or its completion callbacks.
        if let Some(handle) = self.handle.take() {
            unsafe {
                sr_release(handle.as_ptr());
            }
        }
    }
}

pub fn copy_file(path: &Path) -> Result<(), String> {
    let path = file_name(path)?;
    crate::image_io::clipboard_on_main(move || check(unsafe { sr_copy_file(path.as_ptr()) }))
}

pub fn reveal(path: &Path) -> Result<(), String> {
    // The reveal command is synchronous and already runs on the main thread.
    let path = file_name(path)?;
    check(unsafe { sr_reveal(path.as_ptr()) })
}
