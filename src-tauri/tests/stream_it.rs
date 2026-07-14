//! Integration tests：跑一个本地 tokio TCP server 模拟 SSE / WS。

use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use attool_lib::http::models::{HttpAuth, KeyValue, SseSpec, StreamMessage};
use attool_lib::http::stream::buffer::SessionBuffer;
use attool_lib::http::stream::session::HttpStreamState;
use attool_lib::http::stream::sse::run_sse;

async fn bind() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    (listener, format!("http://{}", addr))
}

fn empty_auth() -> HttpAuth {
    HttpAuth {
        r#type: "none".into(),
        bearer_token: String::new(),
        basic_user: String::new(),
        basic_pass: String::new(),
    }
}

fn make_spec(url: String) -> SseSpec {
    SseSpec {
        url,
        headers: vec![],
        query_params: vec![],
        auth: empty_auth(),
        timeout_seconds: Some(5),
        verify_ssl: true,
        last_event_id: None,
    }
}

// tauri::AppHandle 无法在测试里造，用 mock：直接跳过 emit（buffer 是主源）
// run_sse 是泛型 fn<R: Runtime>，所以 MockRuntime 可以直接传入。
fn mock_app() -> tauri::AppHandle<tauri::test::MockRuntime> {
    tauri::test::mock_app().handle().clone()
}

#[tokio::test]
async fn sse_receives_events_and_closes() {
    let (listener, base) = bind().await;

    tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut req = [0u8; 1024];
        let _ = sock.read(&mut req).await.unwrap();
        let response = concat!(
            "HTTP/1.1 200 OK\r\n",
            "Content-Type: text/event-stream\r\n",
            "Cache-Control: no-cache\r\n",
            "\r\n",
            "data: hello\n\n",
            "event: tick\ndata: 1\n\n",
        );
        sock.write_all(response.as_bytes()).await.unwrap();
        // 主动 close
        drop(sock);
    });

    let state = Arc::new(HttpStreamState::new());
    let buffer = Arc::new(StdMutex::new(SessionBuffer::new()));
    let (_cancel_tx, cancel_rx) = oneshot::channel();

    let task = tokio::spawn(run_sse(
        "s1".to_string(),
        make_spec(base),
        Arc::clone(&state),
        mock_app(),
        cancel_rx,
        Arc::clone(&buffer),
    ));

    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .expect("run_sse timed out")
        .expect("run_sse panicked");

    let snap: Vec<StreamMessage> = buffer.lock().unwrap().snapshot();
    // 期望：Open + 2 SseEvent + Closed
    assert!(matches!(snap[0], StreamMessage::Open { .. }));
    let sse_events: Vec<_> = snap
        .iter()
        .filter(|m| matches!(m, StreamMessage::SseEvent { .. }))
        .collect();
    assert_eq!(sse_events.len(), 2);
    assert!(matches!(snap.last(), Some(StreamMessage::Closed { .. })));
}

#[tokio::test]
async fn sse_401_becomes_error_and_closed() {
    let (listener, base) = bind().await;

    tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut req = [0u8; 1024];
        let _ = sock.read(&mut req).await.unwrap();
        let response = "HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n";
        sock.write_all(response.as_bytes()).await.unwrap();
    });

    let state = Arc::new(HttpStreamState::new());
    let buffer = Arc::new(StdMutex::new(SessionBuffer::new()));
    let (_cancel_tx, cancel_rx) = oneshot::channel();

    let task = tokio::spawn(run_sse(
        "s2".to_string(),
        make_spec(base),
        Arc::clone(&state),
        mock_app(),
        cancel_rx,
        Arc::clone(&buffer),
    ));

    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .expect("timeout")
        .expect("panic");

    let snap = buffer.lock().unwrap().snapshot();
    let has_error = snap
        .iter()
        .any(|m| matches!(m, StreamMessage::Error { message, .. } if message.contains("401")));
    assert!(has_error, "expected 401 in error, got {:?}", snap);
    assert!(matches!(snap.last(), Some(StreamMessage::Closed { .. })));
}

use attool_lib::http::stream::ws::run_ws;
use attool_lib::http::models::{Direction, WsSpec};
use tokio::sync::mpsc;

fn make_ws_spec(url: String) -> WsSpec {
    WsSpec {
        url,
        headers: vec![],
        query_params: vec![],
        auth: empty_auth(),
        verify_ssl: true,
        subprotocols: vec![],
        ping_interval_seconds: None,
        templates: vec![],
    }
}

async fn echo_ws_once() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (sock, _) = listener.accept().await.unwrap();
        let ws = tokio_tungstenite::accept_async(sock).await.unwrap();
        let (mut tx, mut rx) = ws.split();
        use futures_util::{SinkExt, StreamExt};
        while let Some(Ok(msg)) = rx.next().await {
            if msg.is_text() {
                tx.send(msg).await.unwrap();
            } else if msg.is_close() {
                break;
            }
        }
    });
    format!("ws://{}", addr)
}

#[tokio::test]
async fn ws_echoes_text_and_closes_on_client_close() {
    let base = echo_ws_once().await;

    let state = Arc::new(HttpStreamState::new());
    let buffer = Arc::new(StdMutex::new(SessionBuffer::new()));
    let (cancel_tx, cancel_rx) = oneshot::channel();
    let (send_tx, send_rx) = mpsc::unbounded_channel();

    let task = tokio::spawn(run_ws(
        "w1".to_string(),
        make_ws_spec(base),
        Arc::clone(&state),
        mock_app(),
        cancel_rx,
        send_rx,
        Arc::clone(&buffer),
    ));

    // Wait for Open
    for _ in 0..50 {
        if buffer.lock().unwrap().snapshot().iter().any(|m| matches!(m, StreamMessage::Open { .. })) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    send_tx.send("hi".to_string()).unwrap();

    // Wait for echo to come back
    for _ in 0..50 {
        let snap = buffer.lock().unwrap().snapshot();
        let has_in = snap.iter().any(|m| matches!(
            m, StreamMessage::WsText { direction: Direction::In, text, .. } if text == "hi"
        ));
        if has_in {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    // Client closes
    let _ = cancel_tx.send(());
    tokio::time::timeout(Duration::from_secs(3), task).await.unwrap().unwrap();

    let snap = buffer.lock().unwrap().snapshot();
    let has_in = snap.iter().any(|m| matches!(m, StreamMessage::WsText { direction: Direction::In, .. }));
    let has_out = snap.iter().any(|m| matches!(m, StreamMessage::WsText { direction: Direction::Out, .. }));
    assert!(has_in && has_out, "in/out missing: {:?}", snap);
    assert!(matches!(snap.last(), Some(StreamMessage::Closed { .. })));
}
