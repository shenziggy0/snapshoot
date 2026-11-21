use reqwest::{redirect::Policy, Url};
use serde::Serialize;
use serde_json::{Map, Value};
use std::time::Duration;
use tauri::Manager;

const INVALID_CONFIG: &str = "translate.stream_config_invalid";
const INVALID_STREAM: &str = "translate.stream_response_invalid";
const UPSTREAM_FAILED: &str = "translate.stream_upstream_failed";
const TIMED_OUT: &str = "translate.stream_timeout";
const MAX_EVENT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy)]
enum Provider {
    OpenAi,
    Gemini,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StreamChunk<'a> {
    request_id: &'a str,
    text: &'a str,
}

fn request_error(error: reqwest::Error) -> String {
    if error.is_timeout() {
        TIMED_OUT
    } else {
        UPSTREAM_FAILED
    }
    .into()
}

fn configured_instance(window: &tauri::Window, key: &str) -> Result<Value, String> {
    // 与前端使用同一个 Store，在一次加锁期间核对服务列表和实例配置。
    let app = window.app_handle();
    let path = app
        .path_resolver()
        .app_config_dir()
        .ok_or(INVALID_CONFIG)?
        .join("config.json");
    tauri_plugin_store::with_store(app.clone(), app.state(), path, |store| {
        let enabled = store
            .get("translate_service_list")
            .and_then(Value::as_array)
            .is_some_and(|list| list.iter().any(|instance| instance.as_str() == Some(key)));
        Ok(store
            .get(key)
            .filter(|config| {
                enabled
                    && config["enable"].as_bool() != Some(false)
                    && config["stream"].as_bool() == Some(true)
            })
            .cloned())
    })
    .map_err(|_| INVALID_CONFIG)?
    .ok_or_else(|| INVALID_CONFIG.into())
}

#[tauri::command]
pub async fn translate_stream(
    window: tauri::Window,
    request_id: String,
    instance_key: String,
    mut body: Map<String, Value>,
) -> Result<String, String> {
    let provider = match instance_key.split('@').next() {
        Some("openai") => Provider::OpenAi,
        Some("geminipro") => Provider::Gemini,
        _ => return Err(INVALID_CONFIG.into()),
    };
    let config = configured_instance(&window, &instance_key)?;
    let address = config["requestPath"]
        .as_str()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .or(match provider {
            Provider::Gemini => {
                Some("https://generativelanguage.googleapis.com/v1beta/models/gemini-pro")
            }
            Provider::OpenAi => None,
        })
        .ok_or(INVALID_CONFIG)?;
    let address = if address.contains("://") {
        address.to_owned()
    } else {
        format!("https://{address}")
    };
    let mut url = Url::parse(&address).map_err(|_| INVALID_CONFIG)?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(INVALID_CONFIG.into());
    }
    let key = config["apiKey"].as_str().ok_or(INVALID_CONFIG)?;
    let client = reqwest::Client::builder()
        .redirect(Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(request_error)?;

    let request = match provider {
        Provider::OpenAi => {
            let service = config["service"].as_str().unwrap_or("openai");
            body.insert("stream".into(), Value::Bool(true));
            match service {
                "openai" => {
                    let path = url.path().trim_end_matches('/');
                    if !path.ends_with("/chat/completions") {
                        let suffix = if path.ends_with("/v1") {
                            "chat/completions"
                        } else {
                            "v1/chat/completions"
                        };
                        url.set_path(&format!("{path}/{suffix}"));
                    }
                    body.insert("model".into(), config["model"].clone());
                    client.post(url).bearer_auth(key)
                }
                "azure" => client.post(url).header("api-key", key),
                _ => return Err(INVALID_CONFIG.into()),
            }
        }
        Provider::Gemini => {
            let path = url.path().trim_end_matches('/');
            let path = path
                .strip_suffix(":generateContent")
                .or_else(|| path.strip_suffix(":streamGenerateContent"))
                .unwrap_or(path);
            url.set_path(&format!("{path}:streamGenerateContent"));
            let params: Vec<_> = url
                .query_pairs()
                .into_owned()
                .filter(|(name, _)| name != "alt" && name != "key")
                .collect();
            url.query_pairs_mut()
                .clear()
                .extend_pairs(params)
                .append_pair("alt", "sse");
            client.post(url).header("x-goog-api-key", key)
        }
    };
    let mut response = request
        .header("accept", "text/event-stream")
        .json(&body)
        .send()
        .await
        .map_err(request_error)?;
    if !response.status().is_success() {
        return Err(format!(
            "{UPSTREAM_FAILED} ({})",
            response.status().as_u16()
        ));
    }

    read_stream(&mut response, provider, |text| {
        window
            .emit_to(
                window.label(),
                "translate-stream-chunk",
                StreamChunk {
                    request_id: &request_id,
                    text,
                },
            )
            .map_err(|_| UPSTREAM_FAILED.to_owned())
    })
    .await
}

async fn read_stream(
    response: &mut reqwest::Response,
    provider: Provider,
    mut emit: impl FnMut(&str) -> Result<(), String>,
) -> Result<String, String> {
    let mut pending = Vec::new();
    let mut data = Vec::new();
    let mut result = String::new();
    let mut finished = false;
    let mut done = false;
    while let Some(chunk) = response.chunk().await.map_err(request_error)? {
        pending.extend_from_slice(&chunk);
        let mut consumed = 0;
        for line in pending.split_inclusive(|byte| *byte == b'\n') {
            if !line.ends_with(b"\n") {
                break;
            }
            consumed += line.len();
            let line = line.strip_suffix(b"\n").unwrap();
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            if line.is_empty() && !data.is_empty() {
                done = data == b"[DONE]";
                let (delta, terminal) = parse_event(&data, provider)?;
                data.clear();
                finished |= terminal;
                if !delta.is_empty() {
                    if result.len() + delta.len() > MAX_EVENT_BYTES {
                        return Err(INVALID_STREAM.into());
                    }
                    result.push_str(&delta);
                    emit(&delta)?;
                }
                if done {
                    break;
                }
            } else if let Some(value) = line.strip_prefix(b"data:") {
                if !data.is_empty() {
                    data.push(b'\n');
                }
                data.extend_from_slice(value.strip_prefix(b" ").unwrap_or(value));
                if data.len() > MAX_EVENT_BYTES {
                    return Err(INVALID_STREAM.into());
                }
            }
        }
        pending.drain(..consumed);
        if pending.len() > MAX_EVENT_BYTES {
            return Err(INVALID_STREAM.into());
        }
        if done || (finished && matches!(provider, Provider::Gemini)) {
            break;
        }
    }
    if !finished || result.trim().is_empty() {
        return Err(INVALID_STREAM.into());
    }
    Ok(result.trim().to_owned())
}

fn parse_event(data: &[u8], provider: Provider) -> Result<(String, bool), String> {
    if data == b"[DONE]" {
        return Ok((String::new(), true));
    }
    let value: Value = serde_json::from_slice(data).map_err(|_| INVALID_STREAM)?;
    if !value["error"].is_null() {
        return Err(UPSTREAM_FAILED.into());
    }
    match provider {
        Provider::OpenAi => {
            let choice = &value["choices"][0];
            let reason = choice["finish_reason"].as_str();
            if reason.is_some_and(|reason| reason != "stop") {
                return Err(INVALID_STREAM.into());
            }
            Ok((
                choice["delta"]["content"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                reason == Some("stop"),
            ))
        }
        Provider::Gemini => {
            let candidate = &value["candidates"][0];
            let reason = candidate["finishReason"].as_str();
            if reason.is_some_and(|reason| reason != "STOP") {
                return Err(INVALID_STREAM.into());
            }
            let parts = candidate["content"]["parts"].as_array();
            let text = parts
                .into_iter()
                .flatten()
                .filter(|part| part["thought"].as_bool() != Some(true))
                .filter_map(|part| part["text"].as_str())
                .collect();
            Ok((text, reason == Some("STOP")))
        }
    }
}
