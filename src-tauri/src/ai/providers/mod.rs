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

pub(crate) fn read_and_encode_image(path: &str, mime: &str) -> std::io::Result<String> {
    use base64::Engine;
    let clean = path.trim_start_matches("file://");
    let bytes = std::fs::read(clean)?;
    Ok(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}
