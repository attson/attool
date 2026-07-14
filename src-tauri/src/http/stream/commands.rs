use std::sync::{Arc, Mutex};

use tauri::{AppHandle, State};
use tokio::sync::{mpsc, oneshot};

use super::buffer::SessionBuffer;
use super::session::{HttpStreamState, SessionHandle};
use super::sse::run_sse;
use super::ws::run_ws;
use crate::http::models::{SseSpec, StreamMessage, WsSpec};

#[tauri::command]
pub async fn open_stream(
    session_id: String,
    kind: String,
    spec: serde_json::Value,
    state: State<'_, Arc<HttpStreamState>>,
    app: AppHandle,
) -> Result<(), String> {
    // 已有 session -> 先内部 cancel
    let _ = state.cancel(&session_id);

    let generation = state.next_generation();
    let buffer = Arc::new(Mutex::new(SessionBuffer::new()));
    let (cancel_tx, cancel_rx) = oneshot::channel();

    match kind.as_str() {
        "sse" => {
            let spec: SseSpec = serde_json::from_value(spec)
                .map_err(|error| format!("SSE spec 非法：{error}"))?;
            {
                let parsed = url::Url::parse(spec.url.trim())
                    .map_err(|e| format!("SSE URL 非法：{e}"))?;
                let scheme = parsed.scheme();
                if !matches!(scheme, "http" | "https") {
                    return Err(format!(
                        "SSE 需要 http/https URL，收到：{scheme}"
                    ));
                }
            }
            state.insert(
                session_id.clone(),
                SessionHandle {
                    cancel_tx,
                    send_tx: None,
                    buffer: Arc::clone(&buffer),
                    generation,
                },
            );
            let state_inner = state.inner().clone();
            let app_inner = app.clone();
            let sid = session_id.clone();
            tokio::spawn(async move {
                run_sse(sid, spec, state_inner, app_inner, cancel_rx, buffer, generation).await;
            });
            Ok(())
        }
        "ws" => {
            let spec: WsSpec = serde_json::from_value(spec)
                .map_err(|error| format!("WS spec 非法：{error}"))?;
            {
                let parsed = url::Url::parse(spec.url.trim())
                    .map_err(|e| format!("WS URL 非法：{e}"))?;
                let scheme = parsed.scheme();
                if !matches!(scheme, "ws" | "wss") {
                    return Err(format!(
                        "WS 需要 ws/wss URL，收到：{scheme}"
                    ));
                }
            }
            let (send_tx, send_rx) = mpsc::unbounded_channel();
            state.insert(
                session_id.clone(),
                SessionHandle {
                    cancel_tx,
                    send_tx: Some(send_tx),
                    buffer: Arc::clone(&buffer),
                    generation,
                },
            );
            let state_inner = state.inner().clone();
            let app_inner = app.clone();
            let sid = session_id.clone();
            tokio::spawn(async move {
                run_ws(sid, spec, state_inner, app_inner, cancel_rx, send_rx, buffer, generation).await;
            });
            Ok(())
        }
        other => Err(format!("unsupported stream kind: {other}")),
    }
}

#[tauri::command]
pub fn close_stream(
    session_id: String,
    state: State<'_, Arc<HttpStreamState>>,
) -> Result<(), String> {
    // 幂等：不存在也 Ok
    state.cancel(&session_id);
    Ok(())
}

#[tauri::command]
pub fn send_ws_message(
    session_id: String,
    text: String,
    state: State<'_, Arc<HttpStreamState>>,
) -> Result<(), String> {
    state.send(&session_id, text)
}

#[tauri::command]
pub fn list_stream_messages(
    session_id: String,
    state: State<'_, Arc<HttpStreamState>>,
) -> Result<Vec<StreamMessage>, String> {
    Ok(state.snapshot(&session_id))
}
