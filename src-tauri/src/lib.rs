use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::PathBuf,
    process::Stdio,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use tauri::{AppHandle, Emitter, State};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    process::Command,
    sync::{oneshot, Mutex},
};

const DOWNLOAD_EVENT: &str = "download-progress";

type DownloadTasks = Arc<DownloadState>;

struct DownloadState {
    next_id: AtomicU64,
    tasks: Mutex<HashMap<String, oneshot::Sender<()>>>,
}

impl Default for DownloadState {
    fn default() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            tasks: Mutex::new(HashMap::new()),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartDownloadRequest {
    url: String,
    download_dir: String,
    file_name: Option<String>,
    connections: Option<u8>,
    split: Option<u8>,
    min_split_size: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadStarted {
    id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadEventPayload {
    id: String,
    status: DownloadStatus,
    progress: f32,
    speed: Option<String>,
    eta: Option<String>,
    message: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum DownloadStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[tauri::command]
fn get_default_download_dir() -> Result<String, String> {
    dirs::download_dir()
        .or_else(|| std::env::current_dir().ok())
        .map(path_to_string)
        .ok_or_else(|| "无法确定默认下载目录".to_string())
}

#[tauri::command]
async fn start_download(
    request: StartDownloadRequest,
    app: AppHandle,
    state: State<'_, DownloadTasks>,
) -> Result<DownloadStarted, String> {
    let url = request.url.trim().to_string();
    if url.is_empty() {
        return Err("请输入下载链接".to_string());
    }

    let download_dir = request.download_dir.trim();
    if download_dir.is_empty() {
        return Err("请选择或填写下载目录".to_string());
    }

    let id = format!("dl-{}", state.next_id.fetch_add(1, Ordering::Relaxed));
    let connections = request.connections.unwrap_or(16).clamp(1, 16).to_string();
    let split = request.split.unwrap_or(16).clamp(1, 64).to_string();
    let min_split_size = sanitize_min_split_size(request.min_split_size.as_deref());

    let mut args = vec![
        "--continue=true".to_string(),
        "--allow-overwrite=false".to_string(),
        "--auto-file-renaming=true".to_string(),
        format!("--dir={download_dir}"),
        format!("--max-connection-per-server={connections}"),
        format!("--split={split}"),
        format!("--min-split-size={min_split_size}"),
        "--summary-interval=1".to_string(),
        "--show-console-readout=true".to_string(),
        "--console-log-level=warn".to_string(),
        "--download-result=hide".to_string(),
    ];

    if let Some(file_name) = request.file_name.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        args.push(format!("--out={file_name}"));
    }
    args.push(url);

    let mut child = Command::new("aria2c")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                "未找到 aria2c。请先安装 aria2（macOS 可执行：brew install aria2）。".to_string()
            } else {
                format!("启动 aria2c 失败：{error}")
            }
        })?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (cancel_tx, cancel_rx) = oneshot::channel();
    state.tasks.lock().await.insert(id.clone(), cancel_tx);

    emit_download_event(
        &app,
        DownloadEventPayload {
            id: id.clone(),
            status: DownloadStatus::Queued,
            progress: 0.0,
            speed: None,
            eta: None,
            message: Some("aria2 任务已创建".to_string()),
        },
    );

    let task_state = state.inner().clone();
    let task_app = app.clone();
    let task_id = id.clone();

    tauri::async_runtime::spawn(async move {
        let stdout_task = stdout.map(|stream| {
            let app = task_app.clone();
            let id = task_id.clone();
            tauri::async_runtime::spawn(async move { read_aria2_stdout(stream, app, id).await })
        });
        let stderr_task = stderr.map(|stream| {
            let app = task_app.clone();
            let id = task_id.clone();
            tauri::async_runtime::spawn(async move { read_aria2_stderr(stream, app, id).await })
        });

        let final_payload = tokio::select! {
            status = child.wait() => match status {
                Ok(exit_status) if exit_status.success() => DownloadEventPayload {
                    id: task_id.clone(),
                    status: DownloadStatus::Completed,
                    progress: 100.0,
                    speed: None,
                    eta: None,
                    message: Some("下载完成".to_string()),
                },
                Ok(exit_status) => DownloadEventPayload {
                    id: task_id.clone(),
                    status: DownloadStatus::Failed,
                    progress: 0.0,
                    speed: None,
                    eta: None,
                    message: Some(format!("aria2c 退出码：{exit_status}")),
                },
                Err(error) => DownloadEventPayload {
                    id: task_id.clone(),
                    status: DownloadStatus::Failed,
                    progress: 0.0,
                    speed: None,
                    eta: None,
                    message: Some(format!("等待 aria2c 结束失败：{error}")),
                },
            },
            _ = cancel_rx => {
                let _ = child.kill().await;
                DownloadEventPayload {
                    id: task_id.clone(),
                    status: DownloadStatus::Cancelled,
                    progress: 0.0,
                    speed: None,
                    eta: None,
                    message: Some("已取消下载任务".to_string()),
                }
            }
        };

        if let Some(handle) = stdout_task {
            let _ = handle.await;
        }
        if let Some(handle) = stderr_task {
            let _ = handle.await;
        }

        task_state.tasks.lock().await.remove(&task_id);
        emit_download_event(&task_app, final_payload);
    });

    Ok(DownloadStarted { id })
}

#[tauri::command]
async fn cancel_download(id: String, state: State<'_, DownloadTasks>) -> Result<(), String> {
    let mut tasks = state.tasks.lock().await;
    if let Some(cancel_tx) = tasks.remove(&id) {
        let _ = cancel_tx.send(());
        Ok(())
    } else {
        Err("未找到正在运行的下载任务".to_string())
    }
}

async fn read_aria2_stdout<R>(stream: R, app: AppHandle, id: String)
where
    R: AsyncRead + Unpin,
{
    let mut reader = BufReader::new(stream);
    let mut buffer = Vec::new();

    loop {
        buffer.clear();
        match reader.read_until(b'\r', &mut buffer).await {
            Ok(0) => break,
            Ok(_) => {
                let line = String::from_utf8_lossy(&buffer);
                if let Some(payload) = parse_progress_line(&id, &line) {
                    emit_download_event(&app, payload);
                }
            }
            Err(error) => {
                emit_download_event(
                    &app,
                    DownloadEventPayload {
                        id: id.clone(),
                        status: DownloadStatus::Failed,
                        progress: 0.0,
                        speed: None,
                        eta: None,
                        message: Some(format!("读取 aria2 输出失败：{error}")),
                    },
                );
                break;
            }
        }
    }
}

async fn read_aria2_stderr<R>(stream: R, app: AppHandle, id: String)
where
    R: AsyncRead + Unpin,
{
    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    while let Ok(bytes) = reader.read_line(&mut line).await {
        if bytes == 0 {
            break;
        }
        let message = line.trim();
        if !message.is_empty() {
            emit_download_event(
                &app,
                DownloadEventPayload {
                    id: id.clone(),
                    status: DownloadStatus::Running,
                    progress: 0.0,
                    speed: None,
                    eta: None,
                    message: Some(message.to_string()),
                },
            );
        }
        line.clear();
    }
}

fn parse_progress_line(id: &str, line: &str) -> Option<DownloadEventPayload> {
    if !line.contains("[#") || !line.contains('%') {
        return None;
    }

    let progress = Regex::new(r"\((\d+(?:\.\d+)?)%\)")
        .ok()
        .and_then(|regex| regex.captures(line))
        .and_then(|captures| captures.get(1))
        .and_then(|match_| match_.as_str().parse::<f32>().ok())?;

    Some(DownloadEventPayload {
        id: id.to_string(),
        status: DownloadStatus::Running,
        progress,
        speed: extract_token(line, "DL:"),
        eta: extract_token(line, "ETA:"),
        message: None,
    })
}

fn extract_token(line: &str, prefix: &str) -> Option<String> {
    let start = line.find(prefix)? + prefix.len();
    let rest = &line[start..];
    let token = rest
        .split(|character: char| character.is_whitespace() || character == ']')
        .next()
        .unwrap_or_default()
        .trim();

    (!token.is_empty()).then(|| token.to_string())
}

fn sanitize_min_split_size(value: Option<&str>) -> String {
    let candidate = value.unwrap_or("1M").trim();
    let valid = Regex::new(r"^\d+[KMG]?$")
        .map(|regex| regex.is_match(candidate))
        .unwrap_or(false);

    if valid {
        candidate.to_string()
    } else {
        "1M".to_string()
    }
}

fn emit_download_event(app: &AppHandle, payload: DownloadEventPayload) {
    let _ = app.emit(DOWNLOAD_EVENT, payload);
}

fn path_to_string(path: PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Arc::new(DownloadState::default()))
        .invoke_handler(tauri::generate_handler![
            get_default_download_dir,
            start_download,
            cancel_download
        ])
        .run(tauri::generate_context!())
        .expect("error while running AT Tool");
}
