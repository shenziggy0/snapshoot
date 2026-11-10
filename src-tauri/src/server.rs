use crate::config::{get, set};
use crate::window::*;
use log::{info, warn};
use std::io::Read;
use std::thread;
use tiny_http::{Request, Response, Server};

const MAX_BODY: u64 = 1024 * 1024;

pub fn start_server() {
    let port = match get("server_port") {
        Some(v) => v.as_i64().unwrap_or(60828),
        None => {
            set("server_port", 60828);
            60828
        }
    };
    let _ = api_token();
    thread::spawn(move || {
        let server = match Server::http(format!("127.0.0.1:{port}")) {
            Ok(v) => v,
            Err(e) => {
                // Do not raise a system notification here. When another instance
                // (or an installed build) already owns the port, the dev binary
                // has no bundle to attach the notification to and macOS aborts
                // the whole process inside mac-notification-sys.
                warn!(
                    "Server start failed on port {port}: {e}. Another instance may be running; \
                     change server_port in the config if this is unexpected."
                );
                return;
            }
        };
        for request in server.incoming_requests() {
            http_handle(request);
        }
    });
}

/// Stable local secret. Callers must send it as `X-Snapshoot-Token` or `?token=`.
pub fn api_token() -> String {
    if let Some(existing) = get("server_token").and_then(|v| v.as_str().map(str::to_owned)) {
        if existing.len() >= 32 {
            return existing;
        }
    }
    let mut bytes = [0u8; 32];
    if getrandom::getrandom(&mut bytes).is_err() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        for (i, slot) in bytes.iter_mut().enumerate() {
            *slot = (nanos.rotate_left((i as u32) * 3) & 0xff) as u8;
        }
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut token = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        token.push(HEX[(byte >> 4) as usize] as char);
        token.push(HEX[(byte & 0xf) as usize] as char);
    }
    set("server_token", token.clone());
    token
}

fn token_matches(presented: &str, expected: &str) -> bool {
    let presented = presented.as_bytes();
    let expected = expected.as_bytes();
    if presented.len() != expected.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in presented.iter().zip(expected.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

fn presented_token(request: &Request) -> Option<String> {
    for header in request.headers() {
        if header.field.equiv("X-Snapshoot-Token") {
            return Some(header.value.as_str().trim().to_string());
        }
    }
    let query = request.url().split_once('?')?.1;
    for pair in query.split('&') {
        if let Some(value) = pair.strip_prefix("token=") {
            return Some(value.to_string());
        }
    }
    None
}

fn authorized(request: &Request) -> bool {
    presented_token(request)
        .map(|value| token_matches(&value, &api_token()))
        .unwrap_or(false)
}

fn http_handle(request: Request) {
    if !authorized(&request) {
        respond(request, 401, "unauthorized");
        return;
    }
    let url = request.url().to_string();
    let path = url.split('?').next().unwrap_or(url.as_str());
    info!("Handle {path} request");
    match path {
        "/" | "/translate" => handle_translate(request),
        "/config" => handle_config(request),
        "/selection_translate" => handle_selection_translate(request),
        "/input_translate" => handle_input_translate(request),
        "/ocr_recognize" => handle_ocr_recognize(request, &url),
        "/ocr_translate" => handle_ocr_translate(request, &url),
        _ => respond(request, 404, "not found"),
    }
}

fn handle_config(request: Request) {
    config_window();
    respond(request, 200, "ok");
}

fn handle_translate(mut request: Request) {
    let content = match read_body(&mut request) {
        Ok(content) => content,
        Err(()) => {
            respond(request, 400, "bad request");
            return;
        }
    };
    text_translate(content);
    respond(request, 200, "ok");
}

fn handle_selection_translate(request: Request) {
    selection_translate();
    respond(request, 200, "ok");
}

fn handle_input_translate(request: Request) {
    input_translate();
    respond(request, 200, "ok");
}

fn handle_ocr_recognize(request: Request, url: &str) {
    if url.contains("screenshot=false") {
        recognize_window();
    } else {
        ocr_recognize();
    }
    respond(request, 200, "ok");
}

fn handle_ocr_translate(request: Request, url: &str) {
    if url.contains("screenshot=false") {
        image_translate();
    } else {
        ocr_translate();
    }
    respond(request, 200, "ok");
}

fn read_body(request: &mut Request) -> Result<String, ()> {
    let mut content = String::new();
    request
        .as_reader()
        .take(MAX_BODY)
        .read_to_string(&mut content)
        .map_err(|err| {
            warn!("http body read failed: {err}");
        })?;
    Ok(content)
}

fn respond(request: Request, status: u16, body: &str) {
    let response = Response::from_string(body).with_status_code(status);
    if let Err(err) = request.respond(response) {
        warn!("http respond failed: {err}");
    }
}
