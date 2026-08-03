pub mod anthropic;
pub mod openai;
pub mod ollama;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelInfo {
    pub id: String,
    pub display_name: String,
}

#[derive(Debug, Clone)]
pub enum ContentPart {
    Text(String),
    Image { path: String, mime: String },
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: Vec<ContentPart>,
}

pub struct ChatRequest {
    pub model_id: String,
    pub messages: Vec<ChatMessage>,
    pub system_prompt: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i64>,
}

pub enum ChatDelta {
    Text(String),
}

#[derive(Debug, Default)]
pub struct ChatFinal {
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
}

#[derive(Debug)]
pub enum ProviderError {
    Http { status: u16, body: String },
    Network(String),
    Parse(String),
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http { status, body } => {
                write!(f, "HTTP {status}: {}", body.chars().take(500).collect::<String>())
            }
            Self::Network(m) => write!(f, "网络错误：{m}"),
            Self::Parse(m) => write!(f, "解析错误：{m}"),
        }
    }
}

impl std::error::Error for ProviderError {}

/// OpenAI shape: full `data:{mime};base64,{data}` URL.
pub(crate) async fn read_image_data_url(path: &str, mime: &str) -> Result<String, ProviderError> {
    let base64 = read_image_base64(path).await?;
    Ok(format!("data:{mime};base64,{base64}"))
}

/// Anthropic / Ollama shape: bare base64 payload.
pub(crate) async fn read_image_base64(path: &str) -> Result<String, ProviderError> {
    use base64::Engine;
    let clean = path.trim_start_matches("file://");
    let bytes = tokio::fs::read(clean)
        .await
        .map_err(|err| ProviderError::Network(format!("读取附件失败：{err}")))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// Find the first occurrence of `needle` in `hay`. Used to locate frame
/// delimiters in a byte buffer without assuming UTF-8 validity of the
/// buffer as a whole (a multi-byte char may still be split mid-buffer).
pub(crate) fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Drain complete `delim`-terminated frames out of `buf`, decoding each as
/// UTF-8 only once it is whole. This is what prevents a multi-byte
/// character split across two network chunks from being decoded as two
/// halves (each becoming U+FFFD) — bytes for an incomplete frame stay in
/// `buf` untouched until the rest arrives. Malformed (non-UTF-8) frames are
/// dropped rather than surfaced, matching the previous lossy-decode
/// behavior of "skip what we can't parse".
pub(crate) fn assemble_utf8_frames(buf: &mut Vec<u8>, delim: &[u8]) -> Vec<String> {
    let mut frames = Vec::new();
    while let Some(pos) = find_subslice(buf, delim) {
        let take = pos + delim.len();
        let frame_bytes: Vec<u8> = buf.drain(..take).collect();
        if let Ok(frame) = std::str::from_utf8(&frame_bytes) {
            frames.push(frame.to_string());
        }
    }
    frames
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_subslice_locates_needle() {
        assert_eq!(find_subslice(b"abc\n\ndef", b"\n\n"), Some(3));
        assert_eq!(find_subslice(b"abcdef", b"\n\n"), None);
        assert_eq!(find_subslice(b"a\nb", b"\n"), Some(1));
    }

    #[test]
    fn assemble_utf8_frames_waits_for_split_multibyte_char() {
        // "你好" is 6 bytes in UTF-8; split the chunk at byte 4, which is
        // mid-character for '好' (bytes 3..6 of the string).
        let full = "data: 你好\n\n".as_bytes().to_vec();
        let (first, second) = full.split_at(4);

        let mut buf: Vec<u8> = Vec::new();
        buf.extend_from_slice(first);
        // No complete frame yet; nothing should be emitted (and nothing
        // should panic trying to decode a partial multi-byte char).
        assert!(assemble_utf8_frames(&mut buf, b"\n\n").is_empty());

        buf.extend_from_slice(second);
        let frames = assemble_utf8_frames(&mut buf, b"\n\n");
        assert_eq!(frames, vec!["data: 你好\n\n".to_string()]);
        assert!(!frames[0].contains('\u{FFFD}'));
    }

    #[test]
    fn assemble_utf8_frames_skips_malformed_frame() {
        let mut buf: Vec<u8> = vec![0xFF, 0xFE, b'\n', b'\n'];
        assert!(assemble_utf8_frames(&mut buf, b"\n\n").is_empty());
        assert!(buf.is_empty());
    }

    #[test]
    fn assemble_utf8_frames_handles_single_byte_delim() {
        let mut buf: Vec<u8> = b"line one\nline two\n".to_vec();
        let frames = assemble_utf8_frames(&mut buf, b"\n");
        assert_eq!(frames, vec!["line one\n".to_string(), "line two\n".to_string()]);
    }
}
