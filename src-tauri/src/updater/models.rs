use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Phase {
    Idle,
    Checking,
    UpToDate,
    Available {
        info: ReleaseInfo,
    },
    Downloading {
        pct: u8,
        downloaded: u64,
        total: u64,
    },
    Verifying,
    Ready {
        version: String,
    },
    Applying,
    Error {
        message: String,
    },
}

impl Default for Phase {
    fn default() -> Self {
        Phase::Idle
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    pub version: String,
    pub notes: String,
    pub published_at: String,
    pub asset_name: String,
    pub asset_url: String,
    pub asset_size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub phase: Phase,
    pub current: String,
    pub auto_check: bool,
    pub last_check_at: Option<i64>,
    pub error: Option<String>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Snapshot {
            phase: Phase::Idle,
            current: env!("CARGO_PKG_VERSION").to_string(),
            auto_check: true,
            last_check_at: None,
            error: None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct GithubRelease {
    pub tag_name: String,
    // GitHub 有时会把 body / published_at 序列化成 null（release 未写描述、
    // 或 draft 未 publish 时），此时 String + #[serde(default)] 会解析失败。
    // 用 Option<String> 兼容缺失和 null 两种情况。
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub published_at: Option<String>,
    pub assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
pub struct GithubAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}
