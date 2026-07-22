use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

use super::models::Phase;

const CHUNK_EMIT_INTERVAL: Duration = Duration::from_millis(150);
const DEFAULT_DOWNLOAD_MIRRORS: &[&str] = &["https://gh-proxy.com/", "https://github.akams.cn/"];

pub struct DownloadResult {
    pub staged_path: PathBuf,
}

pub async fn fetch_text(url: &str) -> Result<String, String> {
    let client = build_client()?;
    let mut errors = Vec::new();
    for candidate in download_url_candidates(url) {
        match fetch_text_once(&client, &candidate).await {
            Ok(text) => return Ok(text),
            Err(error) => errors.push(error),
        }
    }
    Err(format!("下载 {url} 失败：{}", errors.join("；")))
}

async fn fetch_text_once(client: &reqwest::Client, url: &str) -> Result<String, String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("下载 {url} 失败：{error}"))?
        .error_for_status()
        .map_err(|error| format!("{url} 返回错误：{error}"))?;
    resp.text()
        .await
        .map_err(|error| format!("读取 {url} 响应失败：{error}"))
}

pub async fn download_to_stage(
    app: &AppHandle,
    stage_dir: &Path,
    filename: &str,
    url: &str,
    expected_sha256: &str,
    cancel: Arc<AtomicBool>,
) -> Result<DownloadResult, String> {
    std::fs::create_dir_all(stage_dir).map_err(|error| format!("创建 stage 目录失败：{error}"))?;
    let staged = stage_dir.join(filename);

    if staged.exists() {
        let bytes =
            std::fs::read(&staged).map_err(|error| format!("读取已缓存文件失败：{error}"))?;
        let got = super::verify::compute_sha256_hex(&bytes);
        if got == expected_sha256 {
            emit_phase(
                app,
                Phase::Downloading {
                    pct: 100,
                    downloaded: bytes.len() as u64,
                    total: bytes.len() as u64,
                },
            );
            return Ok(DownloadResult {
                staged_path: staged,
            });
        }
        std::fs::remove_file(&staged).ok();
    }

    let client = build_client()?;
    let mut errors = Vec::new();
    for candidate in download_url_candidates(url) {
        match download_url_to_stage(
            app,
            &client,
            &staged,
            &candidate,
            expected_sha256,
            cancel.clone(),
        )
        .await
        {
            Ok(()) => {
                return Ok(DownloadResult {
                    staged_path: staged,
                })
            }
            Err(error) => {
                std::fs::remove_file(&staged).ok();
                errors.push(error);
            }
        }
    }
    Err(format!("下载 {url} 失败：{}", errors.join("；")))
}

async fn download_url_to_stage(
    app: &AppHandle,
    client: &reqwest::Client,
    staged: &Path,
    url: &str,
    expected_sha256: &str,
    cancel: Arc<AtomicBool>,
) -> Result<(), String> {
    let mut resp = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("请求下载失败：{error}"))?
        .error_for_status()
        .map_err(|error| format!("下载 URL 返回错误：{error}"))?;

    let total = resp.content_length().unwrap_or(0);
    let mut file = tokio::fs::File::create(&staged)
        .await
        .map_err(|error| format!("创建 stage 文件失败：{error}"))?;
    let mut hasher = Sha256::new();
    let mut downloaded: u64 = 0;
    let mut last_emit = Instant::now();

    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|error| format!("读取下载块失败：{error}"))?
    {
        if cancel.load(Ordering::SeqCst) {
            drop(file);
            std::fs::remove_file(&staged).ok();
            return Err("下载已取消".to_string());
        }
        file.write_all(&chunk)
            .await
            .map_err(|error| format!("写入 stage 文件失败：{error}"))?;
        hasher.update(&chunk);
        downloaded += chunk.len() as u64;
        if last_emit.elapsed() >= CHUNK_EMIT_INTERVAL {
            let pct = if total > 0 {
                ((downloaded as f64 / total as f64) * 100.0)
                    .round()
                    .clamp(0.0, 100.0) as u8
            } else {
                0
            };
            emit_phase(
                app,
                Phase::Downloading {
                    pct,
                    downloaded,
                    total,
                },
            );
            last_emit = Instant::now();
        }
    }
    file.flush()
        .await
        .map_err(|error| format!("flush stage 文件失败：{error}"))?;

    let got = hex::encode(hasher.finalize());
    if got != expected_sha256 {
        return Err(format!(
            "SHA256 校验失败：期望 {expected_sha256}，实际 {got}"
        ));
    }
    Ok(())
}

fn download_url_candidates(url: &str) -> Vec<String> {
    if !is_github_download_url(url) {
        return vec![url.to_string()];
    }

    let mut urls: Vec<String> = configured_download_mirrors()
        .into_iter()
        .map(|mirror| mirror_url(&mirror, url))
        .filter(|candidate| candidate != url)
        .collect();
    urls.push(url.to_string());
    urls.dedup();
    urls
}

fn configured_download_mirrors() -> Vec<String> {
    let from_env = std::env::var("ATTOOL_UPDATE_DOWNLOAD_MIRRORS")
        .ok()
        .map(|raw| {
            raw.split([',', ';', '\n'])
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|values| !values.is_empty());

    from_env.unwrap_or_else(|| {
        DEFAULT_DOWNLOAD_MIRRORS
            .iter()
            .map(|value| value.to_string())
            .collect()
    })
}

fn is_github_download_url(url: &str) -> bool {
    url.starts_with("https://github.com/") || url.starts_with("https://www.github.com/")
}

fn mirror_url(mirror: &str, original: &str) -> String {
    let mirror = mirror.trim();
    if mirror.contains("{url}") {
        return mirror.replace("{url}", original);
    }
    format!("{}/{}", mirror.trim_end_matches('/'), original)
}

fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(format!("attool/{}", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|error| format!("构造 HTTP 客户端失败：{error}"))
}

fn emit_phase(app: &AppHandle, phase: Phase) {
    let _ = app.emit("updater://phase", phase);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_url_candidates_use_mirrors_before_origin() {
        let original = "https://github.com/attson/attool/releases/download/v0.8.13/AT.Tool_0.8.13_arm64.app.tar.gz";

        let urls = download_url_candidates(original);

        assert!(urls.len() > 1);
        assert_eq!(urls.last().unwrap(), original);
        assert!(urls[0].ends_with(original));
        assert_ne!(urls[0], original);
    }

    #[test]
    fn download_url_candidates_keep_non_github_url_as_is() {
        let original = "https://example.com/file.tar.gz";

        let urls = download_url_candidates(original);

        assert_eq!(urls, vec![original.to_string()]);
    }

    #[test]
    fn mirror_url_trims_slashes_and_prefixes_original_url() {
        let original = "https://github.com/attson/attool/releases/download/v0.8.13/file.zip";

        assert_eq!(
            mirror_url("https://gh-proxy.com/", original),
            "https://gh-proxy.com/https://github.com/attson/attool/releases/download/v0.8.13/file.zip"
        );
    }
}
