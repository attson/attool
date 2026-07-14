use std::collections::VecDeque;

use crate::http::models::StreamMessage;

pub const MAX_MESSAGES: usize = 2000;
pub const MAX_BYTES: usize = 5 * 1024 * 1024;

pub struct SessionBuffer {
    deque: VecDeque<StreamMessage>,
    bytes: usize,
    dropped_since_last_meta: usize,
}

impl SessionBuffer {
    pub fn new() -> Self {
        Self {
            deque: VecDeque::new(),
            bytes: 0,
            dropped_since_last_meta: 0,
        }
    }

    pub fn push(&mut self, msg: StreamMessage) {
        let sz = msg.approx_size();
        self.deque.push_back(msg);
        self.bytes += sz;
        while self.deque.len() > MAX_MESSAGES || self.bytes > MAX_BYTES {
            if let Some(dropped) = self.deque.pop_front() {
                self.bytes = self.bytes.saturating_sub(dropped.approx_size());
                self.dropped_since_last_meta += 1;
            } else {
                break;
            }
        }
    }

    pub fn take_truncated_meta(&mut self, at_ms: u64) -> Option<StreamMessage> {
        if self.dropped_since_last_meta == 0 {
            return None;
        }
        let dropped = self.dropped_since_last_meta;
        self.dropped_since_last_meta = 0;
        Some(StreamMessage::BufferTruncated { at_ms, dropped })
    }

    pub fn snapshot(&self) -> Vec<StreamMessage> {
        self.deque.iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::models::{Direction, StreamMessage};

    fn text(text: &str) -> StreamMessage {
        StreamMessage::WsText {
            at_ms: 0,
            direction: Direction::In,
            text: text.to_string(),
            truncated: false,
        }
    }

    fn big(payload: &str) -> StreamMessage {
        StreamMessage::WsText {
            at_ms: 0,
            direction: Direction::In,
            text: payload.to_string(),
            truncated: false,
        }
    }

    #[test]
    fn empty_starts_zero() {
        let b = SessionBuffer::new();
        assert!(b.snapshot().is_empty());
    }

    #[test]
    fn drops_oldest_when_msg_count_exceeded() {
        let mut b = SessionBuffer::new();
        for i in 0..(MAX_MESSAGES + 5) {
            b.push(text(&format!("m{i}")));
        }
        assert_eq!(b.snapshot().len(), MAX_MESSAGES);
        if let StreamMessage::WsText { text, .. } = &b.snapshot()[0] {
            assert_eq!(text, "m5");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn drops_oldest_when_byte_size_exceeded() {
        let mut b = SessionBuffer::new();
        // approx_size for WsText = 24 + text.len(); use (1 MB - 24) so each msg = exactly 1 MB
        let payload = "x".repeat(1024 * 1024 - 24);
        for i in 0..8 {
            b.push(big(&payload));
            let _ = i;
        }
        // 8 MB pushed, cap 5 MB => 保留 5 条最新，丢弃 3 条
        assert_eq!(b.snapshot().len(), 5);
    }

    #[test]
    fn take_truncated_meta_after_drops() {
        let mut b = SessionBuffer::new();
        for i in 0..(MAX_MESSAGES + 3) {
            b.push(text(&format!("m{i}")));
        }
        let meta = b.take_truncated_meta(42).expect("expected meta");
        match meta {
            StreamMessage::BufferTruncated { at_ms, dropped } => {
                assert_eq!(at_ms, 42);
                assert_eq!(dropped, 3);
            }
            _ => panic!("wrong variant"),
        }
        assert!(b.take_truncated_meta(0).is_none());
    }

    #[test]
    fn snapshot_reflects_current_content() {
        let mut b = SessionBuffer::new();
        b.push(text("a"));
        b.push(text("b"));
        let snap = b.snapshot();
        assert_eq!(snap.len(), 2);
    }
}
