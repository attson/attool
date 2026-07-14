use std::str::FromStr;
use std::sync::{Arc, Mutex};

use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use tauri::Emitter;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::handshake::client::Request;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::Message;

use super::buffer::SessionBuffer;
use super::session::HttpStreamState;
use crate::http::models::{Direction, StreamMessage, WsSpec};

const MAX_TEXT_BYTES: usize = 1024 * 1024;
const BINARY_PREVIEW_BYTES: usize = 4 * 1024;

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

fn floor_char_boundary(s: &str, n: usize) -> usize {
    if n >= s.len() {
        return s.len();
    }
    let bytes = s.as_bytes();
    let mut i = n;
    while i > 0 && (bytes[i] & 0b1100_0000) == 0b1000_0000 {
        i -= 1;
    }
    i
}

fn truncate_text(text: String) -> (String, bool) {
    if text.len() <= MAX_TEXT_BYTES {
        (text, false)
    } else {
        let cutoff = floor_char_boundary(&text, MAX_TEXT_BYTES);
        let mut trimmed = text;
        trimmed.truncate(cutoff);
        (trimmed, true)
    }
}

pub async fn run_ws<R: tauri::Runtime>(
    session_id: String,
    spec: WsSpec,
    state: Arc<HttpStreamState>,
    app: tauri::AppHandle<R>,
    cancel_rx: oneshot::Receiver<()>,
    mut send_rx: mpsc::UnboundedReceiver<String>,
    buffer: Arc<Mutex<SessionBuffer>>,
) {
    let sid = session_id.clone();

    // Build URL with query params
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
            state.remove(&sid);
            return;
        }
    };
    if !matches!(url.scheme(), "ws" | "wss") {
        push_and_emit(
            &buffer,
            &app,
            &sid,
            StreamMessage::Error {
                at_ms: now_ms(),
                message: "scheme must be ws / wss".into(),
            },
        );
        push_and_emit(
            &buffer,
            &app,
            &sid,
            StreamMessage::Closed {
                at_ms: now_ms(),
                code: None,
                reason: "bad scheme".into(),
            },
        );
        state.remove(&sid);
        return;
    }
    for p in &spec.query_params {
        if !p.enabled || p.key.trim().is_empty() {
            continue;
        }
        url.query_pairs_mut().append_pair(&p.key, &p.value);
    }

    let mut req: Request = match url.as_str().into_client_request() {
        Ok(r) => r,
        Err(err) => {
            push_and_emit(
                &buffer,
                &app,
                &sid,
                StreamMessage::Error {
                    at_ms: now_ms(),
                    message: format!("bad request: {err}"),
                },
            );
            push_and_emit(
                &buffer,
                &app,
                &sid,
                StreamMessage::Closed {
                    at_ms: now_ms(),
                    code: None,
                    reason: "bad request".into(),
                },
            );
            state.remove(&sid);
            return;
        }
    };
    let headers = req.headers_mut();
    for h in &spec.headers {
        if !h.enabled || h.key.trim().is_empty() {
            continue;
        }
        if let (Ok(name), Ok(value)) = (
            tokio_tungstenite::tungstenite::http::HeaderName::from_str(&h.key),
            HeaderValue::from_str(&h.value),
        ) {
            headers.insert(name, value);
        }
    }
    // Auth
    match spec.auth.r#type.as_str() {
        "bearer" if !spec.auth.bearer_token.is_empty() => {
            if let Ok(v) = HeaderValue::from_str(&format!("Bearer {}", spec.auth.bearer_token)) {
                headers.insert("authorization", v);
            }
        }
        "basic" => {
            let raw = format!("{}:{}", spec.auth.basic_user, spec.auth.basic_pass);
            let encoded = base64::engine::general_purpose::STANDARD.encode(raw.as_bytes());
            if let Ok(v) = HeaderValue::from_str(&format!("Basic {encoded}")) {
                headers.insert("authorization", v);
            }
        }
        _ => {}
    }
    if !spec.subprotocols.is_empty() {
        if let Ok(v) = HeaderValue::from_str(&spec.subprotocols.join(", ")) {
            headers.insert("sec-websocket-protocol", v);
        }
    }

    let connect_fut = tokio_tungstenite::connect_async(req);
    tokio::pin!(connect_fut);
    let mut cancel_rx = cancel_rx;

    let (ws_stream, resp) = tokio::select! {
        r = &mut connect_fut => match r {
            Ok(pair) => pair,
            Err(err) => {
                push_and_emit(&buffer, &app, &sid, StreamMessage::Error {
                    at_ms: now_ms(),
                    message: format!("connect failed: {err}"),
                });
                push_and_emit(&buffer, &app, &sid, StreamMessage::Closed {
                    at_ms: now_ms(), code: None, reason: err.to_string(),
                });
                state.remove(&sid);
                return;
            }
        },
        _ = &mut cancel_rx => {
            push_and_emit(&buffer, &app, &sid, StreamMessage::Closed {
                at_ms: now_ms(), code: Some(1000), reason: "client".into(),
            });
            state.remove(&sid);
            return;
        }
    };

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
            status: None,
            headers: resp_headers,
        },
    );

    let (mut sink, mut stream) = ws_stream.split();

    loop {
        tokio::select! {
            incoming = stream.next() => {
                match incoming {
                    Some(Ok(Message::Text(t))) => {
                        let (text, truncated) = truncate_text(t.to_string());
                        push_and_emit(&buffer, &app, &sid, StreamMessage::WsText {
                            at_ms: now_ms(), direction: Direction::In, text, truncated,
                        });
                    }
                    Some(Ok(Message::Binary(b))) => {
                        let preview = &b[..b.len().min(BINARY_PREVIEW_BYTES)];
                        let preview_b64 = base64::engine::general_purpose::STANDARD.encode(preview);
                        push_and_emit(&buffer, &app, &sid, StreamMessage::WsBinary {
                            at_ms: now_ms(),
                            direction: Direction::In,
                            bytes_len: b.len(),
                            preview_b64,
                        });
                    }
                    Some(Ok(Message::Close(frame))) => {
                        let (code, reason) = match frame {
                            Some(CloseFrame { code, reason }) => (Some(u16::from(code)), reason.to_string()),
                            None => (None, String::new()),
                        };
                        push_and_emit(&buffer, &app, &sid, StreamMessage::Closed {
                            at_ms: now_ms(), code, reason,
                        });
                        break;
                    }
                    Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) | Some(Ok(Message::Frame(_))) => {}
                    Some(Err(err)) => {
                        push_and_emit(&buffer, &app, &sid, StreamMessage::Error {
                            at_ms: now_ms(), message: format!("read error: {err}"),
                        });
                        push_and_emit(&buffer, &app, &sid, StreamMessage::Closed {
                            at_ms: now_ms(), code: None, reason: err.to_string(),
                        });
                        break;
                    }
                    None => {
                        push_and_emit(&buffer, &app, &sid, StreamMessage::Closed {
                            at_ms: now_ms(), code: None, reason: "peer closed".into(),
                        });
                        break;
                    }
                }
            }
            out_text = send_rx.recv() => {
                match out_text {
                    Some(text) => {
                        let display = text.clone();
                        if let Err(err) = sink.send(Message::Text(text)).await {
                            push_and_emit(&buffer, &app, &sid, StreamMessage::Error {
                                at_ms: now_ms(), message: format!("send failed: {err}"),
                            });
                            push_and_emit(&buffer, &app, &sid, StreamMessage::Closed {
                                at_ms: now_ms(), code: None, reason: err.to_string(),
                            });
                            break;
                        }
                        let (text, truncated) = truncate_text(display);
                        push_and_emit(&buffer, &app, &sid, StreamMessage::WsText {
                            at_ms: now_ms(), direction: Direction::Out, text, truncated,
                        });
                    }
                    None => {
                        // send channel was dropped (session removed), treat as cancel
                        let _ = sink.send(Message::Close(Some(CloseFrame {
                            code: 1000u16.into(),
                            reason: "client".into(),
                        }))).await;
                        push_and_emit(&buffer, &app, &sid, StreamMessage::Closed {
                            at_ms: now_ms(), code: Some(1000), reason: "client".into(),
                        });
                        break;
                    }
                }
            }
            _ = &mut cancel_rx => {
                let _ = sink.send(Message::Close(Some(CloseFrame {
                    code: 1000u16.into(),
                    reason: "client".into(),
                }))).await;
                push_and_emit(&buffer, &app, &sid, StreamMessage::Closed {
                    at_ms: now_ms(), code: Some(1000), reason: "client".into(),
                });
                break;
            }
        }
    }

    state.remove(&sid);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_text_ascii_at_exact_max() {
        let input = "x".repeat(MAX_TEXT_BYTES + 100);
        let (output, truncated) = truncate_text(input);
        assert!(truncated);
        assert_eq!(output.len(), MAX_TEXT_BYTES);
    }

    #[test]
    fn truncate_text_multibyte_respects_char_boundary() {
        // "中" is 3 bytes in UTF-8; feed well over MAX_TEXT_BYTES worth.
        let input = "中".repeat(MAX_TEXT_BYTES); // >> MAX_TEXT_BYTES bytes
        let (output, truncated) = truncate_text(input);
        // Must not panic, must set truncated, length within budget, and on a 3-byte boundary.
        assert!(truncated);
        assert!(output.len() <= MAX_TEXT_BYTES);
        assert_eq!(output.len() % 3, 0);
    }
}
