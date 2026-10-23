use serde::Serialize;
use std::path::Path;
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

async fn extract_video_with_program(
    program: &Path,
    shared_bridge: bool,
    js_runtime: Option<&Path>,
    url: &str,
    proxy: Option<&str>,
) -> Result<YoutubeVideoInfo, String> {
    let mut command = Command::new(program);
    if shared_bridge {
        command.arg("--youtube").arg(url);
        if let Some(js_runtime) = js_runtime {
            command.arg("--js-runtime").arg(js_runtime);
        }
    } else {
        command
            .arg("-j")
            .arg("--no-warnings")
            .arg("--skip-download")
            .arg("--no-playlist")
            .arg("--write-auto-subs")
            .arg("--sub-langs")
            .arg("en,zh-Hans,zh-Hant,zh")
            .arg(url);
    }
    if let Some(proxy) = proxy.map(str::trim).filter(|value| !value.is_empty()) {
        command.arg("--proxy").arg(proxy);
    }
    let output = command.output().await.map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            if shared_bridge {
                "AT Tool 内置运行时损坏（yt-dlp），请重新安装或更新 AT Tool。".to_string()
            } else {
                "未找到 yt-dlp。请先安装：`brew install yt-dlp` / `pipx install yt-dlp` / `apt install yt-dlp`。".to_string()
            }
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

    parse_video_json(&output.stdout)
}

pub async fn extract_video(url: &str, proxy: Option<&str>) -> Result<YoutubeVideoInfo, String> {
    #[cfg(not(debug_assertions))]
    {
        let bridge = crate::ecommerce::psd_bridge::runtime_bridge_path()?;
        let quickjs = crate::bundled_tools::quickjs()?;
        extract_video_with_program(&bridge, true, Some(&quickjs.entrypoint), url, proxy).await
    }

    #[cfg(debug_assertions)]
    {
        if let Some(program) = std::env::var_os("ATTOOL_YT_DLP_PATH") {
            return extract_video_with_program(Path::new(&program), false, None, url, proxy).await;
        }
        extract_video_with_program(Path::new("yt-dlp"), false, None, url, proxy).await
    }
}

fn parse_video_json(output: &[u8]) -> Result<YoutubeVideoInfo, String> {
    let json: serde_json::Value =
        serde_json::from_slice(output).map_err(|error| format!("解析 yt-dlp 输出失败：{error}"))?;

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
                if !matches!(
                    lang.as_str(),
                    "en" | "zh-Hans" | "zh-Hant" | "zh" | "zh-CN" | "zh-TW"
                ) {
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
                                    if source == "subtitles" {
                                        "官方"
                                    } else {
                                        "自动"
                                    }
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
        Some(
            "未能拿到合并 mp4 直链；YouTube 高清通常是 DASH 分离流，需 yt-dlp 直接下载并本地合流"
                .to_string(),
        )
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

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    use super::extract_video_with_program;

    #[tokio::test]
    async fn shared_runtime_extracts_youtube_without_a_system_yt_dlp() {
        let directory = tempfile::tempdir().unwrap();
        let program = directory.path().join("runtime-bridge");
        std::fs::write(
            &program,
            r#"#!/bin/sh
[ "$1" = "--youtube" ] || exit 41
[ "$2" = "https://youtu.be/example" ] || exit 42
[ "$3" = "--js-runtime" ] || exit 43
[ "$4" = "/runtime/qjs" ] || exit 44
[ "$5" = "--proxy" ] || exit 45
[ "$6" = "http://127.0.0.1:7890" ] || exit 46
printf '%s' '{"title":"Embedded runtime","uploader":"AT","thumbnail":"https://example/cover.jpg","duration":12,"url":"https://example/video.mp4","subtitles":{}}'
"#,
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&program).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&program, permissions).unwrap();

        let result = extract_video_with_program(
            &program,
            true,
            Some(Path::new("/runtime/qjs")),
            "https://youtu.be/example",
            Some("http://127.0.0.1:7890"),
        )
        .await
        .unwrap();

        assert_eq!(result.title, "Embedded runtime");
        assert_eq!(
            result.video_url.as_deref(),
            Some("https://example/video.mp4")
        );
    }
}
