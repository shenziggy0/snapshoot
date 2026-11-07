use crate::image_io::spawn_main;
use crate::pin::open_pin_dynamic;
use crate::screenshot::{capture_for_overlay, rgba_spread};
use crate::window::screenshot_window;
use crate::ALLOW_EXIT;
use crate::APP;
use base64::{engine::general_purpose, Engine as _};
use image::{DynamicImage, Rgba, RgbaImage};
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tauri::Manager;

pub fn enabled() -> bool {
    std::env::var("SNAPSHOOT_SMOKE_TEST").ok().as_deref() == Some("1")
}

pub fn capture_bench_count() -> Option<usize> {
    std::env::var("SNAPSHOOT_CAPTURE_BENCH")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|count| *count > 0)
}

fn smoke_dir() -> PathBuf {
    let path = std::env::var("SNAPSHOOT_SMOKE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("snapshoot-smoke"));
    let _ = fs::create_dir_all(&path);
    path
}

fn log_line(dir: &Path, line: &str) {
    let path = dir.join("smoke.log");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
        let _ = file.flush();
    }
    eprintln!("smoke: {line}");
}

fn on_main<T: Send + 'static>(task: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    let (tx, rx) = mpsc::sync_channel(1);
    spawn_main(move || {
        let _ = tx.send(task());
    })?;
    rx.recv_timeout(Duration::from_secs(30))
        .map_err(|err| format!("main thread: {err}"))
}

fn window_visible(label: &str) -> Result<bool, String> {
    let label = label.to_string();
    on_main(move || {
        APP.get()
            .and_then(|app| app.get_window(&label))
            .and_then(|window| window.is_visible().ok())
            .unwrap_or(false)
    })
}

fn wait_visible(label: &str, timeout: Duration) -> Result<(), String> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if window_visible(label)? {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Err(format!("{label} did not become visible"))
}

fn close_window(label: &str) -> Result<(), String> {
    let label = label.to_string();
    on_main(move || {
        if let Some(window) = APP.get().and_then(|app| app.get_window(&label)) {
            let _ = window.close();
        }
    })
}

fn window_rect(label: &str) -> Result<(i32, i32, u32, u32), String> {
    let label = label.to_string();
    on_main(move || {
        APP.get()
            .and_then(|app| app.get_window(&label))
            .and_then(|window| {
                let position = window.outer_position().ok()?;
                let size = window.outer_size().ok()?;
                let scale = window
                    .current_monitor()
                    .ok()
                    .flatten()
                    .map(|monitor| monitor.scale_factor().max(1.0))
                    .unwrap_or(1.0);
                Some((
                    (f64::from(position.x) / scale).round() as i32,
                    (f64::from(position.y) / scale).round() as i32,
                    (f64::from(size.width) / scale).round() as u32,
                    (f64::from(size.height) / scale).round() as u32,
                ))
            })
            .ok_or_else(|| format!("window rect unavailable: {label}"))
    })?
}

fn test_pattern() -> DynamicImage {
    let width = 640u32;
    let height = 400u32;
    let mut image = RgbaImage::new(width, height);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        *pixel = if y < 48 {
            Rgba([240, 210, 20, 255])
        } else if x < width / 2 {
            Rgba([220, 24, 24, 255])
        } else {
            Rgba([24, 48, 220, 255])
        };
    }
    DynamicImage::ImageRgba8(image)
}

fn describe_png(path: &Path) -> String {
    match image::open(path) {
        Ok(image) => {
            let rgba = image.to_rgba8();
            format!(
                "{}x{} spread {}",
                rgba.width(),
                rgba.height(),
                rgba_spread(rgba.as_raw(), rgba.width(), rgba.height())
            )
        }
        Err(err) => format!("unreadable: {err}"),
    }
}

fn translate_hello(dir: &Path) {
    let outcome = match tauri::async_runtime::block_on(request_translation()) {
        Ok(text) if contains_cjk(&text) => json!({"status": "ok", "text": text}),
        Ok(text) => json!({"status": "empty", "text": text}),
        Err(err) => json!({"status": "network", "detail": err}),
    };
    let path = dir.join("translate-result.json");
    let _ = fs::write(&path, outcome.to_string());
    log_line(dir, &format!("translate {outcome}"));
}

async fn request_translation() -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|err| err.to_string())?;
    let response = client
        .post("https://translate.volcengine.com/crx/translate/v1/")
        .json(&json!({
            "source_language": "en",
            "target_language": "zh",
            "text": "hello"
        }))
        .send()
        .await
        .map_err(|err| err.to_string())?;
    let status = response.status();
    let body = response.text().await.map_err(|err| err.to_string())?;
    if !status.is_success() {
        return Err(format!("http {status}: {body}"));
    }
    let parsed: serde_json::Value = serde_json::from_str(&body).map_err(|err| err.to_string())?;
    let code = parsed
        .pointer("/base_resp/status_code")
        .and_then(|value| value.as_i64());
    if code.is_some() && code != Some(0) {
        return Err(format!("api {parsed}"));
    }
    parsed
        .get("translation")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("empty translation: {body}"))
}

fn contains_cjk(text: &str) -> bool {
    text.chars()
        .any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch))
}

fn finish(code: i32) -> ! {
    let _ = on_main(move || {
        ALLOW_EXIT.store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(app) = APP.get() {
            app.exit(code);
        }
    });
    std::thread::sleep(Duration::from_secs(2));
    std::process::exit(code);
}

pub fn run() {
    let dir = smoke_dir();
    if let Err(err) = run_steps(&dir) {
        log_line(&dir, &format!("FAIL {err}"));
        let _ = fs::write(dir.join("smoke-error"), &err);
        finish(1);
    }
}

fn percentile(sorted: &[u128], percent: usize) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    let index = ((sorted.len() - 1) * percent).div_ceil(100);
    sorted[index.min(sorted.len() - 1)]
}

pub fn run_capture_bench(count: usize) {
    let dir = smoke_dir();
    std::thread::sleep(Duration::from_secs(2));
    let mut samples = Vec::with_capacity(count);
    let mut errors = Vec::new();
    for index in 0..count {
        let started = Instant::now();
        match capture_for_overlay() {
            Ok(()) => {
                let elapsed = started.elapsed().as_millis();
                samples.push(elapsed);
                log_line(&dir, &format!("capture {}: {elapsed}ms", index + 1));
            }
            Err(err) => {
                errors.push(err.clone());
                log_line(&dir, &format!("capture {} failed: {err}", index + 1));
            }
        }
        std::thread::sleep(Duration::from_millis(180));
    }
    if samples.is_empty() {
        let _ = fs::write(dir.join("capture-bench-error"), errors.join("\n"));
        finish(1);
    }
    samples.sort_unstable();
    let total: u128 = samples.iter().sum();
    let result = json!({
        "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "samples": count,
        "successful": samples.len(),
        "min_ms": samples.first().copied().unwrap_or(0),
        "p50_ms": percentile(&samples, 50),
        "p95_ms": percentile(&samples, 95),
        "max_ms": samples.last().copied().unwrap_or(0),
        "avg_ms": total / samples.len() as u128,
        "errors": errors,
    });
    let path = dir.join("capture-bench.json");
    let _ = fs::write(
        &path,
        serde_json::to_string_pretty(&result).unwrap_or_default(),
    );
    log_line(&dir, &format!("capture bench {}", result));
    finish(0);
}

fn run_steps(dir: &Path) -> Result<(), String> {
    log_line(dir, "smoke test started");
    std::thread::sleep(Duration::from_secs(2));

    if let Ok(path) = std::env::var("SNAPSHOOT_OCR_FIXTURE") {
        let bytes = fs::read(&path).map_err(|err| format!("OCR fixture: {err}"))?;
        let layout = crate::system_ocr::system_ocr_layout(general_purpose::STANDARD.encode(bytes))?;
        if layout.lines.is_empty() {
            return Err("Vision OCR returned no lines".into());
        }
        log_line(
            dir,
            &format!(
                "vision OCR {}x{} lines={} text={:?}",
                layout.width,
                layout.height,
                layout.lines.len(),
                layout.text
            ),
        );
    }
    let record_support = crate::screen_record::screen_record_support();
    log_line(
        dir,
        &format!(
            "record support system_audio={} microphone={}",
            record_support.system_audio, record_support.microphone
        ),
    );

    let pattern = test_pattern();
    pattern
        .save(dir.join("pin-source.png"))
        .map_err(|err| format!("save pattern: {err}"))?;
    on_main({
        let pattern = pattern.clone();
        move || open_pin_dynamic(pattern)
    })?;
    wait_visible("pin_1", Duration::from_secs(90))?;
    std::thread::sleep(Duration::from_secs(3));
    log_line(dir, "pattern pin is visible");

    capture_for_overlay()?;
    let frozen_src = crate::image_io::cache_file("pot_screenshot.png");
    let frozen = dir.join("frozen-capture.png");
    fs::copy(&frozen_src, &frozen).map_err(|err| format!("copy capture: {err}"))?;
    log_line(dir, &format!("frozen capture {}", describe_png(&frozen)));

    close_window("pin_1")?;
    std::thread::sleep(Duration::from_millis(600));

    on_main(|| {
        let _ = screenshot_window();
    })?;
    wait_visible("screenshot", Duration::from_secs(90))?;
    std::thread::sleep(Duration::from_secs(4));
    if crate::window::overlay_watchdog_used() {
        return Err("screenshot overlay needed the 2.5s watchdog".into());
    }
    if !crate::window::last_overlay_frame_ready() {
        return Err("screenshot overlay did not load a frozen frame".into());
    }
    fs::write(dir.join("overlay-ready"), "ok").map_err(|err| err.to_string())?;
    log_line(dir, "overlay ready");

    let bytes = fs::read(&frozen).map_err(|err| err.to_string())?;
    crate::quick_access::capture_history_add(
        general_purpose::STANDARD.encode(bytes),
        "smoke".to_string(),
        Some(true),
    )?;
    if crate::quick_access::capture_history_list().is_empty() {
        return Err("capture history is empty".into());
    }
    on_main(crate::window::hide_screenshot_window)?;
    wait_visible("quick_access", Duration::from_secs(10))?;
    std::thread::sleep(Duration::from_secs(1));
    if let Ok((x, y, width, height)) = window_rect("quick_access") {
        let region = format!("-R{x},{y},{width},{height}");
        let _ = Command::new("/usr/sbin/screencapture")
            .arg("-x")
            .arg(region)
            .arg(dir.join("quick-access.png"))
            .status();
    }
    on_main(crate::quick_access::hide_quick_access_window)?;
    log_line(dir, "quick access ready");
    std::thread::sleep(Duration::from_secs(25));

    close_window("screenshot")?;
    std::thread::sleep(Duration::from_millis(600));

    let pattern = test_pattern();
    on_main(move || open_pin_dynamic(pattern))?;
    wait_visible("pin_2", Duration::from_secs(90))?;
    std::thread::sleep(Duration::from_secs(3));
    fs::write(dir.join("pin-ready"), "ok").map_err(|err| err.to_string())?;
    log_line(dir, "pin ready");

    on_main(|| crate::pin::open_pin_text("Snapshoot text pin\nsmoke test".to_string()))?;
    wait_visible("pin_text_3", Duration::from_secs(10))?;
    close_window("pin_text_3")?;
    log_line(dir, "text pin ready");

    translate_hello(dir);
    std::thread::sleep(Duration::from_secs(20));
    fs::write(dir.join("smoke-done"), "ok").map_err(|err| err.to_string())?;
    log_line(dir, "quitting");
    finish(0);
}
