use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::{mpsc, oneshot};

use super::buffer::SessionBuffer;
use crate::http::models::StreamMessage;

pub struct SessionHandle {
    pub cancel_tx: oneshot::Sender<()>,
    pub send_tx: Option<mpsc::UnboundedSender<String>>,
    pub buffer: Arc<Mutex<SessionBuffer>>,
    pub generation: u64,
}

#[derive(Default)]
pub struct HttpStreamState {
    inner: Mutex<HashMap<String, SessionHandle>>,
    next_generation: AtomicU64,
}

impl HttpStreamState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a new monotonically-increasing generation id.
    pub fn next_generation(&self) -> u64 {
        self.next_generation.fetch_add(1, Ordering::SeqCst)
    }

    pub fn insert(&self, id: String, handle: SessionHandle) {
        if let Ok(mut map) = self.inner.lock() {
            map.insert(id, handle);
        }
    }

    /// Removes the session only if its stored generation matches `generation`.
    /// Returns true when the entry was removed, false when it was absent or owned by a newer session.
    pub fn remove_if_generation(&self, id: &str, generation: u64) -> bool {
        if let Ok(mut m) = self.inner.lock() {
            if let Some(h) = m.get(id) {
                if h.generation == generation {
                    m.remove(id);
                    return true;
                }
            }
        }
        false
    }

    /// 幂等：不存在返 false。存在则触发 cancel_tx 并把 handle 从表里移除。
    pub fn cancel(&self, id: &str) -> bool {
        let taken = self.inner.lock().ok().and_then(|mut m| m.remove(id));
        match taken {
            Some(h) => {
                let _ = h.cancel_tx.send(());
                true
            }
            None => false,
        }
    }

    pub fn send(&self, id: &str, text: String) -> Result<(), String> {
        let map = self
            .inner
            .lock()
            .map_err(|_| "stream state poisoned".to_string())?;
        let handle = map.get(id).ok_or_else(|| "session not open".to_string())?;
        let tx = handle
            .send_tx
            .as_ref()
            .ok_or_else(|| "session does not accept sends".to_string())?;
        tx.send(text)
            .map_err(|error| format!("send failed: {error}"))
    }

    pub fn snapshot(&self, id: &str) -> Vec<StreamMessage> {
        let buf = self.buffer_for(id);
        match buf {
            Some(b) => b
                .lock()
                .map(|g| g.snapshot())
                .unwrap_or_default(),
            None => Vec::new(),
        }
    }

    pub fn buffer_for(&self, id: &str) -> Option<Arc<Mutex<SessionBuffer>>> {
        self.inner
            .lock()
            .ok()
            .and_then(|m| m.get(id).map(|h| Arc::clone(&h.buffer)))
    }

    pub fn remove(&self, id: &str) {
        if let Ok(mut m) = self.inner.lock() {
            m.remove(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::models::Direction;

    fn make_handle() -> (SessionHandle, oneshot::Receiver<()>, mpsc::UnboundedReceiver<String>) {
        make_handle_with_gen(0)
    }

    fn make_handle_with_gen(generation: u64) -> (SessionHandle, oneshot::Receiver<()>, mpsc::UnboundedReceiver<String>) {
        let (cancel_tx, cancel_rx) = oneshot::channel();
        let (send_tx, send_rx) = mpsc::unbounded_channel();
        let handle = SessionHandle {
            cancel_tx,
            send_tx: Some(send_tx),
            buffer: Arc::new(Mutex::new(SessionBuffer::new())),
            generation,
        };
        (handle, cancel_rx, send_rx)
    }

    #[test]
    fn cancel_absent_returns_false() {
        let state = HttpStreamState::new();
        assert!(!state.cancel("missing"));
    }

    #[test]
    fn cancel_present_removes_and_signals() {
        let state = HttpStreamState::new();
        let (h, cancel_rx, _) = make_handle();
        state.insert("s1".into(), h);
        assert!(state.cancel("s1"));
        assert!(cancel_rx.blocking_recv().is_ok());
        assert!(!state.cancel("s1")); // 第二次幂等
    }

    #[test]
    fn send_delivers_to_send_rx() {
        let state = HttpStreamState::new();
        let (h, _cancel_rx, mut send_rx) = make_handle();
        state.insert("ws1".into(), h);
        state.send("ws1", "hi".into()).unwrap();
        assert_eq!(send_rx.blocking_recv().unwrap(), "hi");
    }

    #[test]
    fn send_on_missing_returns_err() {
        let state = HttpStreamState::new();
        let err = state.send("nope", "hi".into()).unwrap_err();
        assert!(err.contains("not open"));
    }

    #[test]
    fn snapshot_reflects_buffer() {
        let state = HttpStreamState::new();
        let (h, _c, _s) = make_handle();
        {
            let mut buf = h.buffer.lock().unwrap();
            buf.push(StreamMessage::WsText {
                at_ms: 1,
                direction: Direction::In,
                text: "a".into(),
                truncated: false,
            });
        }
        state.insert("s".into(), h);
        let snap = state.snapshot("s");
        assert_eq!(snap.len(), 1);
    }

    #[test]
    fn remove_if_generation_only_removes_own_generation() {
        let state = HttpStreamState::new();
        let (h, _c, _s) = make_handle_with_gen(1);
        state.insert("s".into(), h);

        // Wrong generation — session stays.
        assert!(!state.remove_if_generation("s", 999));
        assert!(state.buffer_for("s").is_some(), "session should still be present");

        // Correct generation — session removed.
        assert!(state.remove_if_generation("s", 1));
        assert!(state.buffer_for("s").is_none(), "session should be gone");
    }
}
