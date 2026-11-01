use crate::image_io::{cache_file, clipboard_on_main, copy_to_clipboard, cut_image_path, write_png};
use crate::screenshot::{capture_target_at_mouse, capture_target_to, CaptureTarget};
use crate::APP;
use image::{imageops, RgbaImage};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

const MIN_REGION_WIDTH: u32 = 16;
const MIN_REGION_HEIGHT: u32 = 32;
const MIN_APPEND_ROWS: u32 = 4;
const MAX_OUTPUT_HEIGHT: u32 = 30_000;
const MAX_OUTPUT_PIXELS: u64 = 24_000_000;
// Screen captures across a scroll can differ slightly due to compositor
// antialiasing and subpixel text rasterization. A tighter threshold drops
// valid overlaps and leaves the final image incomplete.
const MATCH_THRESHOLD: f64 = 12.0;
const MATCH_MARGIN: f64 = 3.0;
// Merge high-frequency wheel events into a short capture cadence. New scroll
// events arrive while this delay or the capture itself is in progress are
// coalesced into the next frame instead of creating one capture per IPC call.
const CAPTURE_DEBOUNCE: Duration = Duration::from_millis(120);
const SCROLL_AMPLIFICATION: i32 = 4;
const MAX_LINE_SCROLL_DELTA: i32 = 3;

const MSG_CAPTURE_FAILED: &str = "long_screenshot.capture_failed";
const MSG_SIZE_CHANGED: &str = "long_screenshot.size_changed";
const MSG_NO_OVERLAP: &str = "long_screenshot.no_overlap";
const MSG_AMBIGUOUS: &str = "long_screenshot.ambiguous";
const MSG_LIMIT: &str = "long_screenshot.limit";

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct CaptureRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LongCaptureState {
    Capturing,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LongCaptureStatus {
    pub state: LongCaptureState,
    pub width: u32,
    pub height: u32,
    pub message: Option<String>,
}

impl LongCaptureStatus {
    fn capturing(width: u32, height: u32) -> Self {
        Self {
            state: LongCaptureState::Capturing,
            width,
            height,
            message: None,
        }
    }
}

struct StitchedImage {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl StitchedImage {
    fn from_frame(frame: &RgbaImage) -> Result<Self, String> {
        let (width, height) = frame.dimensions();
        if width == 0
            || height == 0
            || height > MAX_OUTPUT_HEIGHT
            || u64::from(width) * u64::from(height) > MAX_OUTPUT_PIXELS
        {
            return Err("selected region is too large".into());
        }
        Ok(Self {
            width,
            height,
            pixels: frame.as_raw().clone(),
        })
    }

    fn can_append(&self, rows: u32) -> bool {
        let height = self.height.saturating_add(rows);
        height <= MAX_OUTPUT_HEIGHT
            && u64::from(self.width) * u64::from(height) <= MAX_OUTPUT_PIXELS
    }

    fn append(&mut self, frame: &RgbaImage, new_rows: u32) {
        let row_bytes = self.width as usize * 4;
        let start_row = frame.height().saturating_sub(new_rows);
        let start = start_row as usize * row_bytes;
        self.pixels.extend_from_slice(&frame.as_raw()[start..]);
        self.height += new_rows;
    }
}

struct SharedSession {
    target: CaptureTarget,
    region: CaptureRegion,
    full_width: u32,
    full_height: u32,
    stop: AtomicBool,
    capture_requested: Mutex<u64>,
    capture_signal: Condvar,
    output: Mutex<StitchedImage>,
    status: Mutex<LongCaptureStatus>,
}

struct ActiveSession {
    shared: Arc<SharedSession>,
    worker: JoinHandle<()>,
}

#[derive(Default)]
pub struct LongCaptureManager {
    active: Mutex<Option<ActiveSession>>,
}

pub fn is_active() -> bool {
    APP.get()
        .map(|app| {
            app.state::<LongCaptureManager>()
                .active
                .lock()
                .unwrap()
                .is_some()
        })
        .unwrap_or(false)
}

#[derive(Debug, PartialEq)]
enum AppendResult {
    Appended(u32),
    Unchanged,
    NoMatch,
    Ambiguous,
    Limit,
}

fn append_tag(result: &AppendResult) -> String {
    match result {
        AppendResult::Appended(rows) => format!("appended({rows})"),
        AppendResult::Unchanged => "unchanged".into(),
        AppendResult::NoMatch => "no_match".into(),
        AppendResult::Ambiguous => "ambiguous".into(),
        AppendResult::Limit => "limit".into(),
    }
}

fn crop_region(image: &RgbaImage, region: CaptureRegion) -> Result<RgbaImage, String> {
    if region.width < MIN_REGION_WIDTH || region.height < MIN_REGION_HEIGHT {
        return Err("selected region is too small".into());
    }
    let (width, height) = image.dimensions();
    let right = region.x.checked_add(region.width);
    let bottom = region.y.checked_add(region.height);
    if right.map_or(true, |value| value > width) || bottom.map_or(true, |value| value > height) {
        return Err("selected region is outside the captured screen".into());
    }
    Ok(imageops::crop_imm(image, region.x, region.y, region.width, region.height).to_image())
}

fn find_append(previous: &[u8], next: &RgbaImage) -> AppendResult {
    let width = next.width();
    let height = next.height();
    let frame_bytes = width as usize * height as usize * 4;
    if previous.len() < frame_bytes || width == 0 || height == 0 {
        return AppendResult::NoMatch;
    }
    let previous = &previous[..frame_bytes];
    if previous == next.as_raw().as_slice() {
        return AppendResult::Unchanged;
    }
    // Allow a larger scroll jump while retaining a small overlap to match.
    // Trackpad bursts can move close to a full viewport before the debounced
    // capture runs, so a 20% minimum overlap rejected otherwise valid frames.
    // A single line-scroll event should never append most of a viewport.
    // Repetitive pages can otherwise produce a false near-full-frame match
    // and duplicate large sections of the screenshot.
    let min_overlap = (height / 2).max(8);
    let max_append = height.saturating_sub(min_overlap);
    if max_append < MIN_APPEND_ROWS {
        return AppendResult::NoMatch;
    }

    let mut best: Option<(u32, f64)> = None;
    for append in MIN_APPEND_ROWS..=max_append {
        let overlap = height - append;
        let score = overlap_score(previous, next.as_raw(), width, overlap);
        if best.map_or(true, |(_, value)| score < value) {
            best = Some((append, score));
        }
    }
    let Some((best_append, best_score)) = best else {
        return AppendResult::NoMatch;
    };
    if best_score > MATCH_THRESHOLD {
        return AppendResult::NoMatch;
    }

    let mut second_score = f64::MAX;
    for append in MIN_APPEND_ROWS..=max_append {
        if append.abs_diff(best_append) <= 3 {
            continue;
        }
        let overlap = height - append;
        let score = overlap_score(previous, next.as_raw(), width, overlap);
        second_score = second_score.min(score);
    }
    if second_score < best_score + MATCH_MARGIN {
        // Repetitive pages often have several nearly identical candidate
        // overlaps. The best candidate is still more useful than aborting
        // the whole session; keep stitching and let later frames refine the
        // result.
        return AppendResult::Appended(best_append);
    }
    AppendResult::Appended(best_append)
}

/// Average grayscale difference over the overlapping rows. Sampling keeps the
/// matcher fast; the accepted frame is still appended at full resolution.
fn overlap_score(previous: &[u8], next: &[u8], width: u32, overlap: u32) -> f64 {
    if width == 0 || overlap == 0 {
        return f64::MAX;
    }
    let row_bytes = width as usize * 4;
    let margin = (width as usize / 20).max(1);
    let usable = (width as usize).saturating_sub(margin * 2).max(1);
    let cols = usable.min(48);
    let rows = (overlap as usize).min(48);
    let previous_height = previous.len() / row_bytes;
    let first_previous_row = previous_height - overlap as usize;
    let mut total = 0u64;
    let mut count = 0u64;

    for row in 0..rows {
        let sample_row = if rows <= 1 {
            0
        } else {
            row * (overlap as usize - 1) / (rows - 1)
        };
        let previous_start = (first_previous_row + sample_row) * row_bytes;
        let next_start = sample_row * row_bytes;
        for col in 0..cols {
            let x = margin
                + if cols <= 1 {
                    0
                } else {
                    col * (usable - 1) / (cols - 1)
                };
            let offset = x * 4;
            let previous_pixel = &previous[previous_start + offset..previous_start + offset + 4];
            let next_pixel = &next[next_start + offset..next_start + offset + 4];
            let previous_luma = (u16::from(previous_pixel[0])
                + u16::from(previous_pixel[1])
                + u16::from(previous_pixel[2]))
                / 3;
            let next_luma =
                (u16::from(next_pixel[0]) + u16::from(next_pixel[1]) + u16::from(next_pixel[2]))
                    / 3;
            total += u64::from(previous_luma.abs_diff(next_luma));
            count += 1;
        }
    }
    if count == 0 {
        f64::MAX
    } else {
        total as f64 / count as f64
    }
}

fn overlap_score_reverse(previous: &[u8], next: &[u8], width: u32, overlap: u32) -> f64 {
    if width == 0 || overlap == 0 {
        return f64::MAX;
    }
    let row_bytes = width as usize * 4;
    let margin = (width as usize / 20).max(1);
    let usable = (width as usize).saturating_sub(margin * 2).max(1);
    let cols = usable.min(48);
    let rows = (overlap as usize).min(48);
    let previous_height = previous.len() / row_bytes;
    let next_height = next.len() / row_bytes;
    let new_rows = next_height.saturating_sub(overlap as usize);
    let mut total = 0u64;
    let mut count = 0u64;
    for row in 0..rows {
        let sample_row = if rows <= 1 { 0 } else { row * (overlap as usize - 1) / (rows - 1) };
        let previous_start = sample_row * row_bytes;
        let next_start = (new_rows + sample_row) * row_bytes;
        for col in 0..cols {
            let x = margin + if cols <= 1 { 0 } else { col * (usable - 1) / (cols - 1) };
            let offset = x * 4;
            let a = &previous[previous_start + offset..previous_start + offset + 4];
            let b = &next[next_start + offset..next_start + offset + 4];
            let al = (u16::from(a[0]) + u16::from(a[1]) + u16::from(a[2])) / 3;
            let bl = (u16::from(b[0]) + u16::from(b[1]) + u16::from(b[2])) / 3;
            total += u64::from(al.abs_diff(bl));
            count += 1;
        }
    }
    if count == 0 { f64::MAX } else { total as f64 / count as f64 }
}

fn find_prepend(previous: &[u8], next: &RgbaImage) -> AppendResult {
    let width = next.width();
    let height = next.height();
    let frame_bytes = width as usize * height as usize * 4;
    if previous.len() < frame_bytes || width == 0 || height == 0 { return AppendResult::NoMatch; }
    let min_overlap = (height / 2).max(8);
    let max_append = height.saturating_sub(min_overlap);
    let mut best = None;
    for append in MIN_APPEND_ROWS..=max_append {
        let score = overlap_score_reverse(previous, next.as_raw(), width, height - append);
        if best.map_or(true, |(_, value)| score < value) { best = Some((append, score)); }
    }
    let Some((rows, score)) = best else { return AppendResult::NoMatch; };
    if score > MATCH_THRESHOLD { AppendResult::NoMatch } else { AppendResult::Appended(rows) }
}

fn append_to_output(output: &mut StitchedImage, frame: &RgbaImage) -> AppendResult {
    let frame_height = frame.height();
    if output.height < frame_height {
        return AppendResult::NoMatch;
    }
    let row_bytes = output.width as usize * 4;
    let start = (output.height - frame_height) as usize * row_bytes;
    let result = find_append(&output.pixels[start..], frame);
    if matches!(result, AppendResult::NoMatch) {
        let reverse = find_prepend(&output.pixels[..frame_height as usize * row_bytes], frame);
        if let AppendResult::Appended(rows) = reverse {
            if !output.can_append(rows) { return AppendResult::Limit; }
            let prefix_len = rows as usize * row_bytes;
            let mut pixels = Vec::with_capacity(output.pixels.len() + prefix_len);
            pixels.extend_from_slice(&frame.as_raw()[..prefix_len]);
            pixels.extend_from_slice(&output.pixels);
            output.pixels = pixels;
            output.height += rows;
            return reverse;
        }
    }
    match result {
        AppendResult::Appended(rows) => {
            if !output.can_append(rows) {
                return AppendResult::Limit;
            }
            output.append(frame, rows);
            result
        }
        other => other,
    }
}

fn publish_status(app: &AppHandle, shared: &SharedSession, next: LongCaptureStatus) {
    let mut status = shared.status.lock().unwrap();
    if *status == next {
        return;
    }
    *status = next.clone();
    drop(status);
    if let Err(err) = app.emit_all("long-capture-status", next) {
        warn!("long capture status emit failed: {err}");
    }
}

fn error_status(shared: &SharedSession, message: &str) -> LongCaptureStatus {
    let status = shared.status.lock().unwrap();
    LongCaptureStatus {
        state: LongCaptureState::Error,
        width: status.width,
        height: status.height,
        message: Some(message.into()),
    }
}

fn process_frame(app: &AppHandle, shared: &Arc<SharedSession>) {
    crate::image_io::diag_log("worker_capture", "process_frame entered");
    let path = cache_file("pot_long_screenshot_frame.png");
    let captured = capture_target_to(&shared.target, &path);
    if let Err(err) = captured {
        crate::image_io::diag_log("worker_capture_err", &err);
        warn!("long capture frame failed: {err}");
        publish_status(app, shared, error_status(shared, MSG_CAPTURE_FAILED));
        return;
    }

    let frame = match image::open(&path) {
        Ok(image) => image.to_rgba8(),
        Err(err) => {
            warn!("long capture frame decode failed: {err}");
            publish_status(app, shared, error_status(shared, MSG_CAPTURE_FAILED));
            return;
        }
    };
    crate::image_io::diag_log(
        "worker_frame_size",
        &format!(
            "frame={}x{} full={}x{} region={},{} {}x{}",
            frame.width(),
            frame.height(),
            shared.full_width,
            shared.full_height,
            shared.region.x,
            shared.region.y,
            shared.region.width,
            shared.region.height
        ),
    );
    if frame.width() != shared.full_width || frame.height() != shared.full_height {
        crate::image_io::diag_log("worker_size_mismatch", MSG_SIZE_CHANGED);
        publish_status(app, shared, error_status(shared, MSG_SIZE_CHANGED));
        return;
    }
    let region = match crop_region(&frame, shared.region) {
        Ok(region) => region,
        Err(err) => {
            warn!("long capture crop failed: {err}");
            return;
        }
    };

    let (result, width, height) = {
        let mut output = shared.output.lock().unwrap();
        let result = append_to_output(&mut output, &region);
        (result, output.width, output.height)
    };
    crate::image_io::diag_log(
        "append_result",
        &format!("result={:?} out={}x{}", append_tag(&result), width, height),
    );
    match result {
        AppendResult::Appended(_) => {
            publish_status(app, shared, LongCaptureStatus::capturing(width, height));
        }
        AppendResult::Unchanged => {}
        // A frame captured during a compositor transition can be unrelated
        // for one cycle. Keep the current stitched image usable and wait for
        // the next settled frame instead of poisoning the whole session.
        AppendResult::NoMatch | AppendResult::Ambiguous => {
            crate::image_io::diag_log("append_skip", &format!("result={:?} out={}x{}", result, width, height));
        }
        AppendResult::Limit => {
            publish_status(
                app,
                shared,
                LongCaptureStatus {
                    state: LongCaptureState::Error,
                    width,
                    height,
                    message: Some(MSG_LIMIT.into()),
                },
            );
        }
    }
}

fn remove_frame_cache() {
    let path = cache_file("pot_long_screenshot_frame.png");
    let _ = std::fs::remove_file(path);
}

fn stop_worker(shared: &SharedSession) {
    shared.stop.store(true, Ordering::SeqCst);
    shared.capture_signal.notify_all();
}

fn spawn_worker(app: AppHandle, shared: Arc<SharedSession>) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut handled = 0u64;
        loop {
            let mut requested = shared.capture_requested.lock().unwrap();
            while *requested == handled && !shared.stop.load(Ordering::SeqCst) {
                requested = shared.capture_signal.wait(requested).unwrap();
            }
            if shared.stop.load(Ordering::SeqCst) {
                break;
            }
            drop(requested);

            crate::image_io::diag_log("worker_wake", "woken; waiting for scroll quiet");
            // Debounce from the *last* wheel event. A fixed sleep from the
            // first event captures the page while a trackpad gesture is still
            // animating, producing an intermediate frame that cannot overlap
            // reliably with the previous frame. Keep extending the quiet
            // window whenever another scroll request arrives.
            let mut quiet_for = Duration::ZERO;
            let mut observed = *shared.capture_requested.lock().unwrap();
            while quiet_for < CAPTURE_DEBOUNCE {
                thread::sleep(Duration::from_millis(20));
                if shared.stop.load(Ordering::SeqCst) {
                    break;
                }
                let current = *shared.capture_requested.lock().unwrap();
                if current != observed {
                    observed = current;
                    quiet_for = Duration::ZERO;
                } else {
                    quiet_for += Duration::from_millis(20);
                }
            }
            if shared.stop.load(Ordering::SeqCst) {
                break;
            }
            handled = observed;
            process_frame(&app, &shared);
        }
    })
}

/// Drop the running session without waiting for the worker. Used when the
/// screenshot window disappears behind our back or scroll forwarding fails on
/// the main thread, where joining could otherwise block the UI.
pub fn abandon_active() {
    let Some(app) = APP.get() else {
        return;
    };
    let manager = app.state::<LongCaptureManager>();
    let active = manager.active.lock().unwrap().take();
    if let Some(active) = active {
        stop_worker(&active.shared);
        thread::spawn(move || {
            let _ = active.worker.join();
            remove_frame_cache();
        });
    }
}

impl LongCaptureManager {
    pub fn start(&self, region: CaptureRegion) -> Result<LongCaptureStatus, String> {
        if self.active.lock().unwrap().is_some() {
            return Err("a long capture is already running".into());
        }
        let source_path = cache_file("pot_screenshot.png");
        remove_frame_cache();
        let source = image::open(&source_path)
            .map_err(|err| format!("open initial screenshot: {err}"))?
            .to_rgba8();
        let first = crop_region(&source, region)?;
        let target = capture_target_at_mouse()?;
        let status = LongCaptureStatus::capturing(first.width(), first.height());
        let shared = Arc::new(SharedSession {
            target,
            region,
            full_width: source.width(),
            full_height: source.height(),
            stop: AtomicBool::new(false),
            capture_requested: Mutex::new(0),
            capture_signal: Condvar::new(),
            output: Mutex::new(StitchedImage::from_frame(&first)?),
            status: Mutex::new(status.clone()),
        });

        // Keep the original overlay visible, but exclude it from subsequent
        // display captures and verify event-forwarding permission up front.
        crate::window::enter_long_screenshot_mode()?;
        let app = APP.get().ok_or_else(|| "app is not ready".to_string())?;
        let worker = spawn_worker(app.clone(), shared.clone());
        info!(
            "long capture started: monitor {},{} {}x{}, region {},{} {}x{}",
            shared.target.x,
            shared.target.y,
            shared.target.width,
            shared.target.height,
            region.x,
            region.y,
            region.width,
            region.height
        );
        let mut active = self.active.lock().unwrap();
        if active.is_some() {
            drop(active);
            stop_worker(&shared);
            let _ = worker.join();
            remove_frame_cache();
            return Err("a long capture is already running".into());
        }
        *active = Some(ActiveSession { shared, worker });
        Ok(status)
    }

    pub fn status(&self) -> Result<LongCaptureStatus, String> {
        let active = self.active.lock().unwrap();
        let session = active
            .as_ref()
            .ok_or_else(|| "no active long capture".to_string())?;
        let status = session.shared.status.lock().unwrap().clone();
        Ok(status)
    }

    pub fn scroll(&self, delta_y: i32) -> Result<(), String> {
        if delta_y == 0 {
            return Ok(());
        }
        let shared = {
            let active = self.active.lock().unwrap();
            active
                .as_ref()
                .ok_or_else(|| "no active long capture".to_string())?
                .shared
                .clone()
        };
        // Route the synthetic wheel event to the center of the selection, in
        // global display coordinates, so macOS delivers the scroll to the
        // window underneath the frozen overlay.
        //
        // `target.x/y` and `region` are both in physical pixels, but
        // `CGEventSetLocation` expects global *points*. On a Retina display
        // (scale = 2) posting pixel coordinates would land off-screen, so the
        // underlying window would never receive the scroll.
        #[cfg(target_os = "macos")]
        let scale = shared.target.scale.max(0.5);
        #[cfg(not(target_os = "macos"))]
        let scale = 1.0;
        let center_x = (i64::from(shared.target.x)
            + i64::from(shared.region.x)
            + i64::from(shared.region.width) / 2) as f64
            / scale;
        let center_y = (i64::from(shared.target.y)
            + i64::from(shared.region.y)
            + i64::from(shared.region.height) / 2) as f64
            / scale;
        crate::image_io::diag_log(
            "scroll_center",
            &format!(
                "delta={delta_y} scale={scale} target=({},{}) region=({},{}) center=({center_x:.1},{center_y:.1})",
                shared.target.x,
                shared.target.y,
                shared.region.x,
                shared.region.y
            ),
        );
        // The stitcher can only match frames whose scroll offset leaves at
        // least `min_overlap` rows in common (region_height / 2). A trackpad
        // fling can report deltas of hundreds of points per event; forwarding
        // them unfiltered makes the page jump past the matchable range and
        // every capture ends in `NoMatch`. Clamp the forwarded delta so a
        // single scroll never exceeds 60% of the region height in physical
        // pixels — the user simply keeps scrolling for more distance.
        let max_delta = MAX_LINE_SCROLL_DELTA;
        let forward = delta_y
            .saturating_mul(SCROLL_AMPLIFICATION)
            .clamp(-max_delta, max_delta);
        crate::image_io::diag_log(
            "scroll_clamp",
            &format!("input={delta_y} amplified={SCROLL_AMPLIFICATION} forward={forward} max_delta={max_delta}"),
        );
        crate::window::forward_long_capture_scroll(forward, center_x, center_y)?;
        let mut requested = shared.capture_requested.lock().unwrap();
        *requested = requested.saturating_add(1);
        drop(requested);
        shared.capture_signal.notify_one();
        Ok(())
    }

    pub fn finish_copy(&self) -> Result<(), String> {
        let shared = self.take_active()?;
        let (width, height, pixels) = {
            let output = shared.output.lock().unwrap();
            (output.width, output.height, output.pixels.clone())
        };
        let image = RgbaImage::from_raw(width, height, pixels)
            .ok_or_else(|| "long capture buffer is invalid".to_string())?;
        let image = image::DynamicImage::ImageRgba8(image);
        write_png(&cut_image_path(), &image).map_err(|err| err.to_string())?;
        clipboard_on_main(move || copy_to_clipboard(&image).map_err(|err| err.to_string()))?;
        remove_frame_cache();
        info!("long capture copied: {}x{}", width, height);
        Ok(())
    }

    pub fn cancel(&self) -> Result<(), String> {
        let active = self.active.lock().unwrap().take();
        if let Some(active) = active {
            stop_worker(&active.shared);
            let _ = active.worker.join();
        }
        remove_frame_cache();
        Ok(())
    }

    fn take_active(&self) -> Result<Arc<SharedSession>, String> {
        let active = self
            .active
            .lock()
            .unwrap()
            .take()
            .ok_or_else(|| "no active long capture".to_string())?;
        stop_worker(&active.shared);
        active
            .worker
            .join()
            .map_err(|_| "long capture worker stopped unexpectedly".to_string())?;
        Ok(active.shared)
    }
}

/// Runs on the main thread: `enter_long_screenshot_mode` performs AppKit
/// calls (exclude from capture) that must not happen on a worker thread.
#[tauri::command]
pub fn long_capture_start(
    region: CaptureRegion,
    manager: State<'_, LongCaptureManager>,
) -> Result<LongCaptureStatus, String> {
    crate::image_io::diag_log("start", &format!("region {},{} {}x{}", region.x, region.y, region.width, region.height));
    let status = manager.start(region).map_err(|e| {
        crate::image_io::diag_log("start_err", &e);
        e
    })?;
    crate::image_io::diag_log("start_ok", &format!("{}x{}", status.width, status.height));
    Ok(status)
}

#[tauri::command]
pub fn long_capture_status(
    manager: State<'_, LongCaptureManager>,
) -> Result<LongCaptureStatus, String> {
    manager.status()
}

#[tauri::command]
pub fn long_capture_scroll(
    delta_y: i32,
    manager: State<'_, LongCaptureManager>,
) -> Result<(), String> {
    crate::image_io::diag_log("scroll", &format!("delta_y={delta_y}"));
    let result = manager.scroll(delta_y);
    if let Err(err) = &result {
        crate::image_io::diag_log("scroll_err", err);
    }
    result
}

#[tauri::command(async)]
pub fn long_capture_finish_copy(manager: State<'_, LongCaptureManager>) -> Result<(), String> {
    manager.finish_copy()
}

#[tauri::command(async)]
pub fn long_capture_cancel(manager: State<'_, LongCaptureManager>) -> Result<(), String> {
    manager.cancel()
}

#[cfg(test)]
mod tests {
    use super::{find_append, AppendResult, StitchedImage};
    use image::{Rgba, RgbaImage};

    fn test_frame(width: u32, height: u32, offset: u32) -> RgbaImage {
        RgbaImage::from_fn(width, height, |x, y| {
            let absolute_y = y + offset;
            Rgba([
                ((absolute_y * 37 + x * 11) % 251) as u8,
                ((absolute_y * 73 + x * 19) % 241) as u8,
                ((absolute_y * 13 + x * 43) % 239) as u8,
                255,
            ])
        })
    }

    #[test]
    fn overlapping_frames_append_exact_new_rows() {
        let previous = test_frame(160, 120, 0);
        let next = test_frame(160, 120, 42);
        assert_eq!(
            find_append(previous.as_raw(), &next),
            AppendResult::Appended(42)
        );

        let mut output = StitchedImage::from_frame(&previous).unwrap();
        super::append_to_output(&mut output, &next);
        assert_eq!(output.height, 162);
        let expected = test_frame(160, 162, 0);
        assert_eq!(&output.pixels, expected.as_raw());
    }

    #[test]
    fn unchanged_and_unrelated_frames_do_not_append() {
        let previous = test_frame(160, 120, 0);
        assert_eq!(
            find_append(previous.as_raw(), &previous),
            AppendResult::Unchanged
        );
        let unrelated = RgbaImage::from_fn(160, 120, |x, y| {
            Rgba([(x * 3) as u8, (y * 5) as u8, ((x + y) * 7) as u8, 255])
        });
        assert_eq!(
            find_append(previous.as_raw(), &unrelated),
            AppendResult::NoMatch
        );
    }
}
