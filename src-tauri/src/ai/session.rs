use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use tokio::sync::oneshot;

/// 单个进行中 AI 回复的取消句柄：`tx` 用于唤醒 provider 的 `stream_chat`
/// select 循环，`cancelled` 供主任务在收尾时区分「被取消」还是「正常出错」。
pub struct AiCancelHandle {
    pub tx: oneshot::Sender<()>,
    pub cancelled: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct AiSessionState {
    inner: Mutex<HashMap<String, AiCancelHandle>>,
}

impl AiSessionState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, message_id: String, handle: AiCancelHandle) {
        if let Ok(mut m) = self.inner.lock() {
            if let Some(prev) = m.insert(message_id, handle) {
                // 同一条消息不该有两个并发流；防御性地把旧的也取消掉。
                let _ = prev.tx.send(());
                prev.cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }
    }

    pub fn cancel(&self, message_id: &str) -> bool {
        let taken = self.inner.lock().ok().and_then(|mut m| m.remove(message_id));
        match taken {
            Some(h) => {
                h.cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
                let _ = h.tx.send(());
                true
            }
            None => false,
        }
    }

    pub fn remove(&self, message_id: &str) {
        if let Ok(mut m) = self.inner.lock() {
            m.remove(message_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn session_state_cancel_marks_flag() {
        let s = AiSessionState::new();
        let flag = Arc::new(AtomicBool::new(false));
        let (tx, _rx) = oneshot::channel();
        s.insert(
            "m1".into(),
            AiCancelHandle {
                tx,
                cancelled: flag.clone(),
            },
        );
        assert!(s.cancel("m1"));
        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn cancel_unknown_message_returns_false() {
        let s = AiSessionState::new();
        assert!(!s.cancel("missing"));
    }

    #[test]
    fn remove_drops_handle_without_cancelling() {
        let s = AiSessionState::new();
        let flag = Arc::new(AtomicBool::new(false));
        let (tx, _rx) = oneshot::channel();
        s.insert(
            "m1".into(),
            AiCancelHandle {
                tx,
                cancelled: flag.clone(),
            },
        );
        s.remove("m1");
        assert!(!flag.load(Ordering::SeqCst));
        assert!(!s.cancel("m1"));
    }
}
