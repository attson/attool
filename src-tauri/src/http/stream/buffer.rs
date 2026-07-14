use std::collections::VecDeque;

pub const MAX_MESSAGES: usize = 2000;
pub const MAX_BYTES: usize = 5 * 1024 * 1024;

// 占位 —— Task 3 会用真实 enum 替代
#[cfg(test)]
#[derive(Debug, Clone)]
pub struct StreamMessage {
    pub size: usize,
    pub tag: String,
}

#[cfg(test)]
impl StreamMessage {
    pub fn approx_size(&self) -> usize {
        self.size
    }
}

#[cfg(not(test))]
pub use crate::http::models::StreamMessage;

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

    pub fn take_truncated_meta(&mut self, _at_ms: u64) -> Option<StreamMessage> {
        // Task 3 会返回真实 StreamMessage::BufferTruncated；测试环境返回占位
        if self.dropped_since_last_meta == 0 {
            return None;
        }
        let dropped = self.dropped_since_last_meta;
        self.dropped_since_last_meta = 0;
        #[cfg(test)]
        {
            Some(StreamMessage {
                size: 32,
                tag: format!("truncated:{dropped}"),
            })
        }
        #[cfg(not(test))]
        {
            let _ = dropped;
            unreachable!("real impl provided in Task 3")
        }
    }

    pub fn snapshot(&self) -> Vec<StreamMessage> {
        self.deque.iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(size: usize, tag: &str) -> StreamMessage {
        StreamMessage {
            size,
            tag: tag.to_string(),
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
            b.push(m(1, &format!("{i}")));
        }
        let snap = b.snapshot();
        assert_eq!(snap.len(), MAX_MESSAGES);
        assert_eq!(snap.first().unwrap().tag, "5"); // 前 5 条被丢
    }

    #[test]
    fn drops_oldest_when_byte_size_exceeded() {
        let mut b = SessionBuffer::new();
        let big = 1024 * 1024; // 1 MB per msg
        for i in 0..8 {
            b.push(m(big, &format!("{i}")));
        }
        let snap = b.snapshot();
        // 8 MB pushed, cap is 5 MB => 保留 5 条最新，丢弃 3 条
        assert_eq!(snap.len(), 5);
        assert_eq!(snap.first().unwrap().tag, "3");
    }

    #[test]
    fn take_truncated_meta_after_drops() {
        let mut b = SessionBuffer::new();
        for i in 0..(MAX_MESSAGES + 3) {
            b.push(m(1, &format!("{i}")));
        }
        let meta = b.take_truncated_meta(0).expect("expected meta");
        assert_eq!(meta.tag, "truncated:3");
        // 第二次调用应为 None
        assert!(b.take_truncated_meta(0).is_none());
    }

    #[test]
    fn snapshot_reflects_current_content() {
        let mut b = SessionBuffer::new();
        b.push(m(1, "a"));
        b.push(m(1, "b"));
        b.push(m(1, "c"));
        let snap = b.snapshot();
        assert_eq!(snap.len(), 3);
        assert_eq!(snap[0].tag, "a");
        assert_eq!(snap[2].tag, "c");
    }
}
