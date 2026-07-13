use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

use super::models::Phase;

const CHUNK_EMIT_INTERVAL: Duration = Duration::from_millis(150);

pub struct DownloadResult {
    pub staged_path: PathBuf,
}

pub async fn fetch_text(url: &str) -> Result<String, String> {
    let client = build_client()?;
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
        let bytes = std::fs::read(&staged)
            .map_err(|error| format!("读取已缓存文件失败：{error}"))?;
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
            return Ok(DownloadResult { staged_path: staged });
        }
        std::fs::remove_file(&staged).ok();
    }

    let client = build_client()?;
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
                ((downloaded as f64 / total as f64) * 100.0).round().clamp(0.0, 100.0) as u8
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
        std::fs::remove_file(&staged).ok();
        return Err(format!(
            "SHA256 校验失败：期望 {expected_sha256}，实际 {got}"
        ));
    }
    Ok(DownloadResult { staged_path: staged })
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
