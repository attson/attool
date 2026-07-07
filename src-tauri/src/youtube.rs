use serde::Serialize;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YoutubeVideoInfo {
    pub title: String,
    pub uploader: Option<String>,
    pub cover: Option<String>,
    pub duration: Option<i64>,
    pub video_url: Option<String>,
    pub subtitle_urls: Vec<SubtitleTrack>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleTrack {
    pub language: String,
    pub name: String,
    pub url: String,
}

pub async fn extract_video(url: &str, proxy: Option<&str>) -> Result<YoutubeVideoInfo, String> {
    let mut cmd = Command::new("yt-dlp");
    cmd.arg("-j")
        .arg("--no-warnings")
        .arg("--skip-download")
        .arg("--no-playlist")
        .arg("--write-auto-subs")
        .arg("--sub-langs")
        .arg("en,zh-Hans,zh-Hant,zh")
        .arg(url);
    if let Some(p) = proxy.map(str::trim).filter(|v| !v.is_empty()) {
        cmd.arg("--proxy").arg(p);
    }
    let output = cmd.output().await.map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            "未找到 yt-dlp。请先安装：`brew install yt-dlp` / `pipx install yt-dlp` / `apt install yt-dlp`。".to_string()
        } else {
            format!("启动 yt-dlp 失败：{error}")
        }
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let hint = if stderr.contains("Sign in") || stderr.contains("age restricted") {
            "（可能是年龄限制或需要登录，考虑传入 --cookies-from-browser 参数）"
        } else if stderr.contains("HTTP Error 403") || stderr.contains("blocked") {
            "（IP 可能被墙，试试设置代理）"
        } else {
            ""
        };
        return Err(format!("yt-dlp 执行失败{hint}：{stderr}"));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("解析 yt-dlp 输出失败：{error}"))?;

    let title = json
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let uploader = json
        .get("uploader")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let cover = json
        .get("thumbnail")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let duration = json.get("duration").and_then(|v| v.as_i64());

    // Pick best mp4 (video+audio combined). yt-dlp -j gives `url` for best merged when possible.
    let video_url = json
        .get("url")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            // Fallback: iterate formats for best mp4 with audio
            json.get("formats")
                .and_then(|arr| arr.as_array())
                .and_then(|formats| {
                    formats.iter().rev().find_map(|f| {
                        let ext = f.get("ext").and_then(|v| v.as_str()).unwrap_or("");
                        let has_audio = f
                            .get("acodec")
                            .and_then(|v| v.as_str())
                            .map(|s| s != "none")
                            .unwrap_or(false);
                        let has_video = f
                            .get("vcodec")
                            .and_then(|v| v.as_str())
                            .map(|s| s != "none")
                            .unwrap_or(false);
                        if ext == "mp4" && has_audio && has_video {
                            f.get("url").and_then(|v| v.as_str()).map(str::to_string)
                        } else {
                            None
                        }
                    })
                })
        });

    // Subtitles: prefer manual, fall back to auto
    let mut subs: Vec<SubtitleTrack> = Vec::new();
    for source in ["subtitles", "automatic_captions"] {
        if let Some(map) = json.get(source).and_then(|v| v.as_object()) {
            for (lang, tracks) in map {
                if !matches!(lang.as_str(), "en" | "zh-Hans" | "zh-Hant" | "zh" | "zh-CN" | "zh-TW") {
                    continue;
                }
                if let Some(arr) = tracks.as_array() {
                    // Prefer vtt or srt over json3
                    if let Some(track) = arr.iter().find(|t| {
                        matches!(
                            t.get("ext").and_then(|v| v.as_str()).unwrap_or(""),
                            "vtt" | "srt"
                        )
                    }) {
                        if let Some(url) = track.get("url").and_then(|v| v.as_str()) {
                            subs.push(SubtitleTrack {
                                language: lang.clone(),
                                name: format!(
                                    "{lang}（{}）",
                                    if source == "subtitles" { "官方" } else { "自动" }
                                ),
                                url: url.to_string(),
                            });
                        }
                    }
                }
            }
        }
        if !subs.is_empty() {
            break;
        }
    }

    let notes = if video_url.is_none() {
        Some("未能拿到合并 mp4 直链；YouTube 高清通常是 DASH 分离流，需 yt-dlp 直接下载并本地合流".to_string())
    } else {
        None
    };

    Ok(YoutubeVideoInfo {
        title,
        uploader,
        cover,
        duration,
        video_url,
        subtitle_urls: subs,
        notes,
    })
}
