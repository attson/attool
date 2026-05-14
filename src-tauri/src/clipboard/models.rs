use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardHistoryItem {
    pub id: String,
    pub kind: ClipboardItemKind,
    pub preview: String,
    pub content_text: String,
    pub file_paths: Vec<String>,
    pub asset_path: Option<String>,
    pub asset_url: Option<String>,
    pub is_pinned: bool,
    pub created_at: String,
    pub last_copied_at: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ClipboardItemKind {
    Text,
    Image,
    Files,
}

impl ClipboardItemKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Image => "image",
            Self::Files => "files",
        }
    }

    pub fn from_db(value: &str) -> Result<Self, String> {
        match value {
            "text" => Ok(Self::Text),
            "image" => Ok(Self::Image),
            "files" => Ok(Self::Files),
            other => Err(format!("未知剪贴板类型：{other}")),
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardHistorySettings {
    pub capture_enabled: bool,
    pub retention_limit: usize,
    pub shortcut: String,
}

impl Default for ClipboardHistorySettings {
    fn default() -> Self {
        Self {
            capture_enabled: true,
            retention_limit: 500,
            shortcut: "CommandOrControl+Shift+V".to_string(),
        }
    }
}
