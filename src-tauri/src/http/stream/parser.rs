pub const MAX_DATA_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct SseFrame {
    pub event: String,
    pub data: String,
    pub id: Option<String>,
    pub retry_ms: Option<u64>,
    pub truncated: bool,
}

pub struct SseParser {
    buf: String,
    cur_event: Option<String>,
    cur_data: String,
    cur_id: Option<String>,
    cur_retry: Option<u64>,
    cur_truncated: bool,
    cur_dirty: bool,
}

impl SseParser {
    pub fn new() -> Self {
        Self {
            buf: String::new(),
            cur_event: None,
            cur_data: String::new(),
            cur_id: None,
            cur_retry: None,
            cur_truncated: false,
            cur_dirty: false,
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) -> Vec<SseFrame> {
        self.buf.push_str(&String::from_utf8_lossy(bytes));
        let mut frames = Vec::new();
        loop {
            // 找到下一行结束（\n 或 \r\n）
            let nl = match self.buf.find('\n') {
                Some(i) => i,
                None => break,
            };
            // 提取行内容，处理 \r\n
            let raw_line_end = if nl > 0 && self.buf.as_bytes()[nl - 1] == b'\r' {
                nl - 1
            } else {
                nl
            };
            let line = self.buf[..raw_line_end].to_string();
            self.buf.drain(..=nl);

            if line.is_empty() {
                // 事件分隔符
                if self.cur_dirty {
                    frames.push(self.take_frame());
                }
                continue;
            }
            if line.starts_with(':') {
                // 注释
                continue;
            }

            let (field, value) = match line.find(':') {
                Some(i) => {
                    let (f, rest) = line.split_at(i);
                    let mut v = &rest[1..]; // 去掉 ':'
                    if v.starts_with(' ') {
                        v = &v[1..];
                    }
                    (f.to_string(), v.to_string())
                }
                None => (line.clone(), String::new()),
            };

            match field.as_str() {
                "event" => {
                    self.cur_event = Some(value);
                    self.cur_dirty = true;
                }
                "data" => {
                    if !self.cur_data.is_empty() {
                        self.cur_data.push('\n');
                    }
                    if self.cur_data.len() + value.len() > MAX_DATA_BYTES {
                        let room = MAX_DATA_BYTES.saturating_sub(self.cur_data.len());
                        self.cur_data.push_str(&value[..room.min(value.len())]);
                        self.cur_truncated = true;
                    } else {
                        self.cur_data.push_str(&value);
                    }
                    self.cur_dirty = true;
                }
                "id" => {
                    self.cur_id = Some(value);
                    self.cur_dirty = true;
                }
                "retry" => {
                    if let Ok(n) = value.parse::<u64>() {
                        self.cur_retry = Some(n);
                        self.cur_dirty = true;
                    }
                }
                _ => {
                    // unknown field: 规范要求忽略
                }
            }
        }
        frames
    }

    fn take_frame(&mut self) -> SseFrame {
        let frame = SseFrame {
            event: self.cur_event.take().unwrap_or_else(|| "message".to_string()),
            data: std::mem::take(&mut self.cur_data),
            id: self.cur_id.take(),
            retry_ms: self.cur_retry.take(),
            truncated: self.cur_truncated,
        };
        self.cur_truncated = false;
        self.cur_dirty = false;
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_event_default_type() {
        let mut p = SseParser::new();
        let out = p.feed(b"data: hello\n\n");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].event, "message");
        assert_eq!(out[0].data, "hello");
        assert!(out[0].id.is_none());
        assert!(out[0].retry_ms.is_none());
        assert!(!out[0].truncated);
    }

    #[test]
    fn multi_line_data_joined_with_newline() {
        let mut p = SseParser::new();
        let out = p.feed(b"data: a\ndata: b\ndata: c\n\n");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data, "a\nb\nc");
    }

    #[test]
    fn custom_event_and_id_and_retry() {
        let mut p = SseParser::new();
        let out = p.feed(b"event: ping\nid: 42\nretry: 3000\ndata: pong\n\n");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].event, "ping");
        assert_eq!(out[0].data, "pong");
        assert_eq!(out[0].id.as_deref(), Some("42"));
        assert_eq!(out[0].retry_ms, Some(3000));
    }

    #[test]
    fn crlf_line_endings_accepted() {
        let mut p = SseParser::new();
        let out = p.feed(b"data: r\r\ndata: n\r\n\r\n");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data, "r\nn");
    }

    #[test]
    fn chunk_boundary_in_the_middle_of_field() {
        let mut p = SseParser::new();
        let mut out = p.feed(b"da");
        assert!(out.is_empty());
        out.extend(p.feed(b"ta: hello\n\n"));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data, "hello");
    }

    #[test]
    fn leading_space_or_tab_after_colon_trimmed() {
        let mut p = SseParser::new();
        // 规范只 trim 一个 space。这里演示 "data:x" / "data: x" / "data:\tx" 三种。
        let out = p.feed(b"data:x\n\n");
        assert_eq!(out[0].data, "x");
        let mut p = SseParser::new();
        let out = p.feed(b"data: x\n\n");
        assert_eq!(out[0].data, "x");
        let mut p = SseParser::new();
        let out = p.feed(b"data:\tx\n\n");
        assert_eq!(out[0].data, "\tx"); // 只 trim 一个 space，其他保留
    }

    #[test]
    fn comment_lines_ignored() {
        let mut p = SseParser::new();
        let out = p.feed(b": this is a heartbeat\ndata: real\n\n");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data, "real");
    }

    #[test]
    fn oversized_data_gets_truncated() {
        let big = "x".repeat(MAX_DATA_BYTES + 100);
        let payload = format!("data: {}\n\n", big);
        let mut p = SseParser::new();
        let out = p.feed(payload.as_bytes());
        assert_eq!(out.len(), 1);
        assert!(out[0].truncated);
        assert!(out[0].data.len() <= MAX_DATA_BYTES);
    }
}
