use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use tauri::Emitter;
use tokio::sync::oneshot;

use super::buffer::SessionBuffer;
use super::parser::SseParser;
use super::session::HttpStreamState;
use crate::http::models::{SseSpec, StreamMessage};

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn emit<R: tauri::Runtime>(app: &tauri::AppHandle<R>, sid: &str, msg: &StreamMessage) {
    let _ = app.emit(&format!("http-stream-message-{sid}"), msg);
}

fn push_and_emit<R: tauri::Runtime>(
    buffer: &Arc<Mutex<SessionBuffer>>,
    app: &tauri::AppHandle<R>,
    sid: &str,
    msg: StreamMessage,
) {
    emit(app, sid, &msg);
    if let Ok(mut b) = buffer.lock() {
        b.push(msg);
        if let Some(meta) = b.take_truncated_meta(now_ms()) {
            emit(app, sid, &meta);
            b.push(meta);
        }
    }
}

pub async fn run_sse<R: tauri::Runtime>(
    session_id: String,
    spec: SseSpec,
    state: Arc<HttpStreamState>,
    app: tauri::AppHandle<R>,
    cancel_rx: oneshot::Receiver<()>,
    buffer: Arc<Mutex<SessionBuffer>>,
    generation: u64,
) {
    let sid = session_id.clone();

    // 构造 URL + Header
    let mut url = match reqwest::Url::parse(spec.url.trim()) {
        Ok(u) => u,
        Err(err) => {
            push_and_emit(
                &buffer,
                &app,
                &sid,
                StreamMessage::Error {
                    at_ms: now_ms(),
                    message: format!("URL 非法：{err}"),
                },
            );
            push_and_emit(
                &buffer,
                &app,
                &sid,
                StreamMessage::Closed {
                    at_ms: now_ms(),
                    code: None,
                    reason: "invalid url".into(),
                },
            );
            state.remove_if_generation(&sid, generation);
            return;
        }
    };
    for p in &spec.query_params {
        if !p.enabled || p.key.trim().is_empty() {
            continue;
        }
        url.query_pairs_mut().append_pair(&p.key, &p.value);
    }

    let mut headers = HeaderMap::new();
    headers.insert(
        reqwest::header::ACCEPT,
        HeaderValue::from_static("text/event-stream"),
    );
    headers.insert(
        reqwest::header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache"),
    );
    if let Some(last_id) = spec.last_event_id.as_deref().filter(|s| !s.is_empty()) {
        if let Ok(v) = HeaderValue::from_str(last_id) {
            headers.insert(HeaderName::from_static("last-event-id"), v);
        }
    }
    for h in &spec.headers {
        if !h.enabled || h.key.trim().is_empty() {
            continue;
        }
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_str(&h.key),
            HeaderValue::from_str(&h.value),
        ) {
            headers.insert(name, value);
        }
    }
    // Auth
    match spec.auth.r#type.as_str() {
        "bearer" if !spec.auth.bearer_token.is_empty() => {
            if let Ok(v) = HeaderValue::from_str(&format!("Bearer {}", spec.auth.bearer_token)) {
                headers.insert(reqwest::header::AUTHORIZATION, v);
            }
        }
        "basic" => {
            let raw = format!("{}:{}", spec.auth.basic_user, spec.auth.basic_pass);
            let encoded = base64::engine::general_purpose::STANDARD.encode(raw.as_bytes());
            if let Ok(v) = HeaderValue::from_str(&format!("Basic {encoded}")) {
                headers.insert(reqwest::header::AUTHORIZATION, v);
            }
        }
        _ => {}
    }

    let connect_timeout = Duration::from_secs(spec.timeout_seconds.unwrap_or(30).clamp(1, 300) as u64);
    let client = match reqwest::Client::builder()
        .connect_timeout(connect_timeout)
        .danger_accept_invalid_certs(!spec.verify_ssl)
        .build()
    {
        Ok(c) => c,
        Err(err) => {
            push_and_emit(
                &buffer,
                &app,
                &sid,
                StreamMessage::Error {
                    at_ms: now_ms(),
                    message: format!("构造 client 失败：{err}"),
                },
            );
            push_and_emit(
                &buffer,
                &app,
                &sid,
                StreamMessage::Closed {
                    at_ms: now_ms(),
                    code: None,
                    reason: "client build failed".into(),
                },
            );
            state.remove_if_generation(&sid, generation);
            return;
        }
    };

    let resp_fut = client.get(url).headers(headers).send();
    tokio::pin!(resp_fut);
    let mut cancel_rx = cancel_rx;

    let resp = tokio::select! {
        r = &mut resp_fut => r,
        _ = &mut cancel_rx => {
            push_and_emit(&buffer, &app, &sid, StreamMessage::Closed {
                at_ms: now_ms(), code: Some(1000), reason: "client".into(),
            });
            state.remove_if_generation(&sid, generation);
            return;
        }
    };

    let resp = match resp {
        Ok(r) => r,
        Err(err) => {
            push_and_emit(
                &buffer,
                &app,
                &sid,
                StreamMessage::Error {
                    at_ms: now_ms(),
                    message: format!("connect failed: {err}"),
                },
            );
            push_and_emit(
                &buffer,
                &app,
                &sid,
                StreamMessage::Closed {
                    at_ms: now_ms(),
                    code: None,
                    reason: err.to_string(),
                },
            );
            state.remove_if_generation(&sid, generation);
            return;
        }
    };

    let status = resp.status();
    let resp_headers: Vec<(String, String)> = resp
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("<binary>").to_string()))
        .collect();
    push_and_emit(
        &buffer,
        &app,
        &sid,
        StreamMessage::Open {
            at_ms: now_ms(),
            status: Some(status.as_u16()),
            headers: resp_headers,
        },
    );
    if !status.is_success() {
        push_and_emit(
            &buffer,
            &app,
            &sid,
            StreamMessage::Error {
                at_ms: now_ms(),
                message: format!("HTTP {}", status.as_u16()),
            },
        );
        push_and_emit(
            &buffer,
            &app,
            &sid,
            StreamMessage::Closed {
                at_ms: now_ms(),
                code: None,
                reason: format!("HTTP {}", status.as_u16()),
            },
        );
        state.remove_if_generation(&sid, generation);
        return;
    }

    let mut stream = resp.bytes_stream();
    let mut parser = SseParser::new();

    loop {
        tokio::select! {
            chunk = stream.next() => {
                match chunk {
                    Some(Ok(bytes)) => {
                        for frame in parser.feed(&bytes) {
                            push_and_emit(&buffer, &app, &sid, StreamMessage::SseEvent {
                                at_ms: now_ms(),
                                event: frame.event,
                                data: frame.data,
                                id: frame.id,
                                retry_ms: frame.retry_ms,
                                truncated: frame.truncated,
                            });
                        }
                    }
                    Some(Err(err)) => {
                        push_and_emit(&buffer, &app, &sid, StreamMessage::Error {
                            at_ms: now_ms(),
                            message: format!("read error: {err}"),
                        });
                        push_and_emit(&buffer, &app, &sid, StreamMessage::Closed {
                            at_ms: now_ms(), code: None, reason: err.to_string(),
                        });
                        break;
                    }
                    None => {
                        push_and_emit(&buffer, &app, &sid, StreamMessage::Closed {
                            at_ms: now_ms(), code: None, reason: "server closed stream".into(),
                        });
                        break;
                    }
                }
            }
            _ = &mut cancel_rx => {
                push_and_emit(&buffer, &app, &sid, StreamMessage::Closed {
                    at_ms: now_ms(), code: Some(1000), reason: "client".into(),
                });
                break;
            }
        }
    }

    state.remove_if_generation(&sid, generation);
}
