mod bilibili;
mod clipboard;
mod douyin;
pub mod ecommerce;
pub mod imaging;
mod xhs;
mod youtube;

use ecommerce::EcommerceStore;
use regex::Regex;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
#[cfg(target_os = "macos")]
use tauri::ActivationPolicy;
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    process::Command,
    sync::{oneshot, Mutex},
};

const DOWNLOAD_EVENT: &str = "download-progress";

type DownloadTasks = Arc<DownloadState>;

struct DownloadState {
    next_id: AtomicU64,
    db_path: PathBuf,
    tasks: Mutex<HashMap<String, oneshot::Sender<()>>>,
}

impl DownloadState {
    fn new(db_path: PathBuf, next_id: u64) -> Self {
        Self {
            next_id: AtomicU64::new(next_id),
            db_path,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BatchLogoRequest {
    image_paths: Vec<String>,
    logo_path: String,
    output_dir: String,
    position: LogoPosition,
    margin: Option<u32>,
    logo_width_percent: Option<f32>,
    logo_x_percent: Option<f32>,
    logo_y_percent: Option<f32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveLogoPresetRequest {
    name: String,
    logo_path: String,
    output_dir: String,
    logo_x_percent: f32,
    logo_y_percent: f32,
    logo_width_percent: f32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogoPresetRecord {
    id: i64,
    name: String,
    logo_path: String,
    output_dir: String,
    logo_x_percent: f32,
    logo_y_percent: f32,
    logo_width_percent: f32,
    updated_at: String,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
enum LogoPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchLogoResult {
    total: usize,
    succeeded: usize,
    outputs: Vec<String>,
    failed: Vec<ImageProcessFailure>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImageProcessFailure {
    path: String,
    message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadStarted {
    id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadTaskRecord {
    id: String,
    url: String,
    download_dir: String,
    file_name: Option<String>,
    status: DownloadStatus,
    progress: f32,
    speed: Option<String>,
    eta: Option<String>,
    message: Option<String>,
    created_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
    local_path: Option<String>,
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
    started_at: Option<String>,
    finished_at: Option<String>,
    local_path: Option<String>,
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

impl DownloadStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    fn from_str(value: &str) -> Self {
        match value {
            "queued" => Self::Queued,
            "running" => Self::Running,
            "completed" => Self::Completed,
            "cancelled" => Self::Cancelled,
            _ => Self::Failed,
        }
    }
}

#[tauri::command]
fn get_default_download_dir() -> Result<String, String> {
    dirs::download_dir()
        .or_else(|| std::env::current_dir().ok())
        .map(path_to_string)
        .ok_or_else(|| "无法确定默认下载目录".to_string())
}

#[tauri::command]
async fn list_download_tasks(
    state: State<'_, DownloadTasks>,
) -> Result<Vec<DownloadTaskRecord>, String> {
    let db_path = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || load_download_tasks(&db_path))
        .await
        .map_err(|error| format!("读取任务记录失败：{error}"))?
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
        "--console-log-level=notice".to_string(),
        "--download-result=hide".to_string(),
    ];

    if let Some(file_name) = request
        .file_name
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        args.push(format!("--out={file_name}"));
    }
    args.push(url.clone());

    let file_name = request
        .file_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    insert_download_task(
        &state.db_path,
        &id,
        &url,
        download_dir,
        file_name.as_deref(),
    )?;

    let mut child = Command::new("aria2c")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            let message = if error.kind() == std::io::ErrorKind::NotFound {
                "未找到 aria2c。请先安装 aria2（macOS 可执行：brew install aria2）。".to_string()
            } else {
                format!("启动 aria2c 失败：{error}")
            };
            let payload = DownloadEventPayload {
                id: id.clone(),
                status: DownloadStatus::Failed,
                progress: 0.0,
                speed: None,
                eta: None,
                message: Some(message.clone()),
            started_at: None,
            finished_at: None,
            local_path: None,
            };
            persist_download_event(&state.db_path, &payload);
            message
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
        started_at: None,
        finished_at: None,
        local_path: None,
        },
    );

    let task_state = state.inner().clone();
    let task_app = app.clone();
    let task_id = id.clone();
    let fallback_dir = download_dir.to_string();
    let fallback_name = file_name.clone();

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
                    started_at: None,
                    finished_at: None,
                    // 兜底：若用户显式指定过 file_name，用 `{dir}/{name}` 合成绝对路径。
                    // 与 persist_download_event 的 `COALESCE(local_path, ?)` 配合：
                    // stdout 的 NOTICE 行已经写入的路径优先，本兜底只在之前 NULL 时生效。
                    local_path: fallback_name
                        .as_deref()
                        .map(|name| join_path(&fallback_dir, name)),
                },
                Ok(exit_status) => DownloadEventPayload {
                    id: task_id.clone(),
                    status: DownloadStatus::Failed,
                    progress: 0.0,
                    speed: None,
                    eta: None,
                    message: Some(format!("aria2c 退出码：{exit_status}")),
                started_at: None,
                finished_at: None,
                local_path: None,
                },
                Err(error) => DownloadEventPayload {
                    id: task_id.clone(),
                    status: DownloadStatus::Failed,
                    progress: 0.0,
                    speed: None,
                    eta: None,
                    message: Some(format!("等待 aria2c 结束失败：{error}")),
                started_at: None,
                finished_at: None,
                local_path: None,
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
                started_at: None,
                finished_at: None,
                local_path: None,
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

#[tauri::command]
async fn open_download_folder(id: String, state: State<'_, DownloadTasks>) -> Result<(), String> {
    let db_path = state.db_path.clone();
    let folder = tauri::async_runtime::spawn_blocking(move || find_download_dir(&db_path, &id))
        .await
        .map_err(|error| format!("读取保存目录失败：{error}"))??;

    open_folder(&folder)
}

#[tauri::command]
async fn batch_add_logo(request: BatchLogoRequest) -> Result<BatchLogoResult, String> {
    tauri::async_runtime::spawn_blocking(move || process_batch_add_logo(request))
        .await
        .map_err(|error| format!("处理图片失败：{error}"))?
}

#[tauri::command]
async fn list_logo_presets(
    state: State<'_, DownloadTasks>,
) -> Result<Vec<LogoPresetRecord>, String> {
    let db_path = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || load_logo_presets(&db_path))
        .await
        .map_err(|error| format!("读取方案失败：{error}"))?
}

#[tauri::command]
async fn save_logo_preset(
    request: SaveLogoPresetRequest,
    state: State<'_, DownloadTasks>,
) -> Result<LogoPresetRecord, String> {
    let db_path = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || save_logo_preset_record(&db_path, request))
        .await
        .map_err(|error| format!("保存方案失败：{error}"))?
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
                } else if let Some(path) = parse_complete_line(&line) {
                    emit_download_event(
                        &app,
                        DownloadEventPayload {
                            id: id.clone(),
                            status: DownloadStatus::Running,
                            progress: 0.0,
                            speed: None,
                            eta: None,
                            message: None,
                            started_at: None,
                            finished_at: None,
                            local_path: Some(path),
                        },
                    );
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
                    started_at: None,
                    finished_at: None,
                    local_path: None,
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
                started_at: None,
                finished_at: None,
                local_path: None,
                },
            );
        }
        line.clear();
    }
}

fn join_path(dir: &str, name: &str) -> String {
    let mut trimmed = dir.trim_end_matches(['/', '\\']).to_string();
    let sep = if trimmed.contains('\\') && !trimmed.contains('/') { '\\' } else { '/' };
    trimmed.push(sep);
    trimmed.push_str(name);
    trimmed
}

fn complete_line_re() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\[NOTICE\][^\n\r]*Download complete:\s*(.+?)\s*$").expect("complete line regex")
    })
}

fn parse_complete_line(line: &str) -> Option<String> {
    // aria2 stdout 分片是按 '\r' 切的，尾部可能带 '\r' / '\n'
    let trimmed = line.trim_end_matches(['\r', '\n']);
    complete_line_re()
        .captures(trimmed)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
}

#[cfg(test)]
mod parse_complete_line_tests {
    use super::parse_complete_line;

    #[test]
    fn extracts_absolute_path_from_standard_notice() {
        let line = "12/25 15:23:11 [NOTICE] Download complete: /tmp/foo.mp4";
        assert_eq!(parse_complete_line(line), Some("/tmp/foo.mp4".into()));
    }

    #[test]
    fn handles_path_with_spaces() {
        let line = "[NOTICE] Download complete: /Users/attson/my folder/video 1.mp4\r";
        assert_eq!(
            parse_complete_line(line),
            Some("/Users/attson/my folder/video 1.mp4".into())
        );
    }

    #[test]
    fn returns_none_for_progress_line() {
        let line = "[#abc 12MiB/50MiB(24%) CN:16 DL:5.2MiB ETA:1m30s]";
        assert!(parse_complete_line(line).is_none());
    }

    #[test]
    fn returns_none_when_notice_is_not_download_complete() {
        let line = "12/25 15:23:11 [NOTICE] IPv4 dht: listening on UDP port 6881";
        assert!(parse_complete_line(line).is_none());
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
    started_at: None,
    finished_at: None,
    local_path: None,
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

fn create_download_state(app: &AppHandle) -> Result<DownloadTasks, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法获取应用数据目录：{error}"))?;
    fs::create_dir_all(&app_data_dir).map_err(|error| format!("创建应用数据目录失败：{error}"))?;

    let db_path = app_data_dir.join("aria2-tasks.sqlite3");
    init_database(&db_path)?;
    mark_interrupted_tasks(&db_path)?;
    let next_id = next_download_id(&db_path)?;

    Ok(Arc::new(DownloadState::new(db_path, next_id)))
}

fn init_database(db_path: &Path) -> Result<(), String> {
    let connection =
        Connection::open(db_path).map_err(|error| format!("打开任务数据库失败：{error}"))?;
    connection
        .execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS download_tasks (
                id TEXT PRIMARY KEY,
                url TEXT NOT NULL,
                download_dir TEXT NOT NULL,
                file_name TEXT,
                status TEXT NOT NULL,
                progress REAL NOT NULL DEFAULT 0,
                speed TEXT,
                eta TEXT,
                message TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
            );

            CREATE INDEX IF NOT EXISTS idx_download_tasks_created_at
                ON download_tasks(created_at DESC);
            "#,
        )
        .map_err(|error| format!("初始化任务数据库失败：{error}"))?;

    // 加 3 列（sqlite 无 IF NOT EXISTS，忽略"duplicate column"错误）
    for stmt in [
        "ALTER TABLE download_tasks ADD COLUMN started_at TEXT",
        "ALTER TABLE download_tasks ADD COLUMN finished_at TEXT",
        "ALTER TABLE download_tasks ADD COLUMN local_path TEXT",
    ] {
        if let Err(error) = connection.execute(stmt, []) {
            let msg = error.to_string();
            if !msg.contains("duplicate column name") {
                return Err(format!("为下载任务表加列失败：{msg}"));
            }
        }
    }

    connection
        .execute_batch(
            r#"

            CREATE TABLE IF NOT EXISTS logo_presets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                logo_path TEXT NOT NULL,
                output_dir TEXT NOT NULL,
                logo_x_percent REAL NOT NULL,
                logo_y_percent REAL NOT NULL,
                logo_width_percent REAL NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
            );
            "#,
        )
        .map_err(|error| format!("初始化任务数据库失败：{error}"))?;
    Ok(())
}

fn mark_interrupted_tasks(db_path: &Path) -> Result<(), String> {
    let connection =
        Connection::open(db_path).map_err(|error| format!("打开任务数据库失败：{error}"))?;
    connection
        .execute(
            r#"
            UPDATE download_tasks
            SET status = 'failed',
                speed = NULL,
                eta = NULL,
                message = '应用已退出，任务未继续运行',
                finished_at = COALESCE(finished_at, datetime('now', 'localtime')),
                updated_at = datetime('now', 'localtime')
            WHERE status IN ('queued', 'running')
            "#,
            [],
        )
        .map_err(|error| format!("更新中断任务失败：{error}"))?;
    Ok(())
}

fn next_download_id(db_path: &Path) -> Result<u64, String> {
    let connection =
        Connection::open(db_path).map_err(|error| format!("打开任务数据库失败：{error}"))?;
    let mut statement = connection
        .prepare("SELECT id FROM download_tasks")
        .map_err(|error| format!("读取任务编号失败：{error}"))?;
    let ids = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| format!("读取任务编号失败：{error}"))?;

    let mut max_id = 0;
    for id in ids {
        let id = id.map_err(|error| format!("读取任务编号失败：{error}"))?;
        if let Some(number) = id
            .strip_prefix("dl-")
            .and_then(|value| value.parse::<u64>().ok())
        {
            max_id = max_id.max(number);
        }
    }

    Ok(max_id + 1)
}

fn insert_download_task(
    db_path: &Path,
    id: &str,
    url: &str,
    download_dir: &str,
    file_name: Option<&str>,
) -> Result<(), String> {
    let connection =
        Connection::open(db_path).map_err(|error| format!("打开任务数据库失败：{error}"))?;
    connection
        .execute(
            r#"
            INSERT INTO download_tasks (
                id, url, download_dir, file_name, status, progress, message
            )
            VALUES (?1, ?2, ?3, ?4, 'queued', 0, 'aria2 任务已创建')
            "#,
            params![id, url, download_dir, file_name],
        )
        .map_err(|error| format!("保存任务记录失败：{error}"))?;
    Ok(())
}

fn load_download_tasks(db_path: &Path) -> Result<Vec<DownloadTaskRecord>, String> {
    let connection =
        Connection::open(db_path).map_err(|error| format!("打开任务数据库失败：{error}"))?;
    let mut statement = connection
        .prepare(
            r#"
            SELECT id, url, download_dir, file_name, status, progress, speed, eta, message,
                   created_at, started_at, finished_at, local_path
            FROM download_tasks
            ORDER BY datetime(created_at) DESC, id DESC
            LIMIT 300
            "#,
        )
        .map_err(|error| format!("读取任务记录失败：{error}"))?;

    let rows = statement
        .query_map([], |row| {
            let status: String = row.get(4)?;
            Ok(DownloadTaskRecord {
                id: row.get(0)?,
                url: row.get(1)?,
                download_dir: row.get(2)?,
                file_name: row.get(3)?,
                status: DownloadStatus::from_str(&status),
                progress: row.get::<_, f32>(5)?,
                speed: row.get(6)?,
                eta: row.get(7)?,
                message: row.get(8)?,
                created_at: row.get(9)?,
                started_at: row.get(10)?,
                finished_at: row.get(11)?,
                local_path: row.get(12)?,
            })
        })
        .map_err(|error| format!("读取任务记录失败：{error}"))?;

    let mut tasks = Vec::new();
    for row in rows {
        tasks.push(row.map_err(|error| format!("读取任务记录失败：{error}"))?);
    }
    Ok(tasks)
}

fn find_download_dir(db_path: &Path, id: &str) -> Result<String, String> {
    let connection =
        Connection::open(db_path).map_err(|error| format!("打开任务数据库失败：{error}"))?;
    connection
        .query_row(
            "SELECT download_dir FROM download_tasks WHERE id = ?1",
            params![id],
            |row| row.get::<_, String>(0),
        )
        .map_err(|error| format!("未找到任务保存目录：{error}"))
}

const DOUYIN_MOBILE_UA: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) \
    AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Mobile/15E148 Safari/604.1";

#[tauri::command]
async fn extract_douyin_video(url: String) -> Result<douyin::DouyinVideoInfo, String> {
    let video_id = douyin::extract_video_id(&url)
        .ok_or_else(|| "URL 中未找到 video id".to_string())?;

    let share_url = format!("https://www.iesdouyin.com/share/video/{video_id}/");
    let client = reqwest::Client::builder()
        .user_agent(DOUYIN_MOBILE_UA)
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|error| format!("构造 HTTP 客户端失败：{error}"))?;

    let response = client
        .get(&share_url)
        .header("Accept-Language", "zh-CN,zh;q=0.9")
        .send()
        .await
        .map_err(|error| format!("网络错误：{error}"))?;

    if !response.status().is_success() {
        return Err(format!("网络错误：HTTP {}", response.status()));
    }

    let body = response
        .text()
        .await
        .map_err(|error| format!("读取响应失败：{error}"))?;

    let raw_json = douyin::extract_payload_json(&body)?;
    let root = douyin::parse_render_data(&raw_json)?;
    let raw_mp4 =
        douyin::find_mp4_url(&root).ok_or_else(|| "未在页面数据中找到视频 URL".to_string())?;
    let title = douyin::find_title(&root, &video_id);
    let (mp4_url, resolved_no_wm) = douyin::derive_watermark_removed_url(&raw_mp4);
    Ok(douyin::DouyinVideoInfo {
        mp4_url,
        title,
        has_watermark: !resolved_no_wm,
    })
}

#[tauri::command]
async fn resolve_douyin_url(url: String) -> Result<String, String> {
    if !(url.starts_with("https://v.douyin.com/") || url.starts_with("http://v.douyin.com/")) {
        return Err("仅支持 v.douyin.com 短链".to_string());
    }

    let client = reqwest::Client::builder()
        .user_agent(DOUYIN_MOBILE_UA)
        .connect_timeout(std::time::Duration::from_secs(8))
        .timeout(std::time::Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|error| format!("构造 HTTP 客户端失败：{error}"))?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|error| format!("请求失败：{error}"))?;

    let final_url = response.url().to_string();
    Ok(canonicalize_douyin_url(&final_url))
}

fn canonicalize_douyin_url(url: &str) -> String {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"/video/(\d+)").unwrap());
    if let Some(cap) = re.captures(url) {
        return format!("https://www.douyin.com/video/{}", &cap[1]);
    }
    url.to_string()
}

#[cfg(test)]
mod douyin_tests {
    use super::canonicalize_douyin_url;

    #[test]
    fn extracts_id_from_iesdouyin_share_url() {
        assert_eq!(
            canonicalize_douyin_url(
                "https://www.iesdouyin.com/share/video/7650383172730932515/?region=CN&mid=xxx"
            ),
            "https://www.douyin.com/video/7650383172730932515"
        );
    }

    #[test]
    fn extracts_id_from_douyin_video_url() {
        assert_eq!(
            canonicalize_douyin_url("https://www.douyin.com/video/7650383172730932515"),
            "https://www.douyin.com/video/7650383172730932515"
        );
    }

    #[test]
    fn returns_original_when_no_video_id_found() {
        assert_eq!(
            canonicalize_douyin_url("https://www.douyin.com/note/xxx"),
            "https://www.douyin.com/note/xxx"
        );
    }
}

#[tauri::command]
async fn extract_xhs_note(url: String) -> Result<xhs::XhsNoteInfo, String> {
    xhs::extract_note(&url).await
}

#[tauri::command]
async fn extract_bilibili_video(url: String) -> Result<bilibili::BiliVideoInfo, String> {
    bilibili::extract_video(&url).await
}

#[tauri::command]
async fn extract_youtube_video(
    url: String,
    proxy: Option<String>,
) -> Result<youtube::YoutubeVideoInfo, String> {
    youtube::extract_video(&url, proxy.as_deref()).await
}

#[tauri::command]
async fn open_external_url(url: String) -> Result<(), String> {
    // 只允许 http/https，避免任意命令注入（例如 file://、传参、shell 元字符）
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("仅支持 http/https 协议".to_string());
    }

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut c = std::process::Command::new("open");
        c.arg(&url);
        c
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut c = std::process::Command::new("cmd");
        c.args(["/c", "start", "", &url]);
        c
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(&url);
        c
    };

    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("打开链接失败：{error}"))
}

fn open_folder(folder: &str) -> Result<(), String> {
    let path = Path::new(folder);
    if !path.exists() {
        return Err(format!("保存目录不存在：{folder}"));
    }

    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(target_os = "macos")]
    command.arg(path);

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = std::process::Command::new("explorer");
        command.arg(path);
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = std::process::Command::new("xdg-open");
        command.arg(path);
        command
    };

    command
        .spawn()
        .map_err(|error| format!("打开保存目录失败：{error}"))?;
    Ok(())
}

fn load_logo_presets(db_path: &Path) -> Result<Vec<LogoPresetRecord>, String> {
    let connection =
        Connection::open(db_path).map_err(|error| format!("打开任务数据库失败：{error}"))?;
    let mut statement = connection
        .prepare(
            r#"
            SELECT id, name, logo_path, output_dir, logo_x_percent, logo_y_percent, logo_width_percent, updated_at
            FROM logo_presets
            ORDER BY datetime(updated_at) DESC, id DESC
            "#,
        )
        .map_err(|error| format!("读取方案失败：{error}"))?;

    let rows = statement
        .query_map([], |row| {
            let width: f32 = row.get(6)?;
            Ok(LogoPresetRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                logo_path: row.get(2)?,
                output_dir: row.get(3)?,
                logo_x_percent: row.get(4)?,
                logo_y_percent: row.get(5)?,
                logo_width_percent: width.clamp(1.0, 100.0),
                updated_at: row.get(7)?,
            })
        })
        .map_err(|error| format!("读取方案失败：{error}"))?;

    let mut presets = Vec::new();
    for row in rows {
        presets.push(row.map_err(|error| format!("读取方案失败：{error}"))?);
    }
    Ok(presets)
}

fn save_logo_preset_record(
    db_path: &Path,
    request: SaveLogoPresetRequest,
) -> Result<LogoPresetRecord, String> {
    let name = request.name.trim();
    if name.is_empty() {
        return Err("请输入方案名称".to_string());
    }
    if request.logo_path.trim().is_empty() || request.output_dir.trim().is_empty() {
        return Err("请选择 Logo 和输出目录".to_string());
    }

    let connection =
        Connection::open(db_path).map_err(|error| format!("打开任务数据库失败：{error}"))?;
    connection
        .execute(
            r#"
            INSERT INTO logo_presets (
                name, logo_path, output_dir, logo_x_percent, logo_y_percent, logo_width_percent
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(name) DO UPDATE SET
                logo_path = excluded.logo_path,
                output_dir = excluded.output_dir,
                logo_x_percent = excluded.logo_x_percent,
                logo_y_percent = excluded.logo_y_percent,
                logo_width_percent = excluded.logo_width_percent,
                updated_at = datetime('now', 'localtime')
            "#,
            params![
                name,
                request.logo_path.trim(),
                request.output_dir.trim(),
                request.logo_x_percent.clamp(0.0, 100.0),
                request.logo_y_percent.clamp(0.0, 100.0),
                request.logo_width_percent.clamp(1.0, 100.0)
            ],
        )
        .map_err(|error| format!("保存方案失败：{error}"))?;

    connection
        .query_row(
            r#"
            SELECT id, name, logo_path, output_dir, logo_x_percent, logo_y_percent, logo_width_percent, updated_at
            FROM logo_presets
            WHERE name = ?1
            "#,
            params![name],
            |row| {
                let width: f32 = row.get(6)?;
                Ok(LogoPresetRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    logo_path: row.get(2)?,
                    output_dir: row.get(3)?,
                    logo_x_percent: row.get(4)?,
                    logo_y_percent: row.get(5)?,
                    logo_width_percent: width.clamp(1.0, 100.0),
                    updated_at: row.get(7)?,
                })
            },
        )
        .map_err(|error| format!("读取方案失败：{error}"))
}

fn process_batch_add_logo(request: BatchLogoRequest) -> Result<BatchLogoResult, String> {
    if request.image_paths.is_empty() {
        return Err("请选择要处理的图片".to_string());
    }

    let logo_path = PathBuf::from(request.logo_path.trim());
    if !logo_path.is_file() {
        return Err("请选择有效的 Logo 图片".to_string());
    }

    let output_dir = PathBuf::from(request.output_dir.trim());
    if output_dir.as_os_str().is_empty() {
        return Err("请选择输出目录".to_string());
    }
    fs::create_dir_all(&output_dir).map_err(|error| format!("创建输出目录失败：{error}"))?;

    let logo = image::open(&logo_path).map_err(|error| format!("读取 Logo 失败：{error}"))?;
    let margin = request.margin.unwrap_or(24);
    let logo_width_percent = request.logo_width_percent.unwrap_or(18.0).clamp(1.0, 100.0);
    let custom_position = match (request.logo_x_percent, request.logo_y_percent) {
        (Some(x), Some(y)) => Some((x.clamp(0.0, 100.0), y.clamp(0.0, 100.0))),
        _ => None,
    };

    let mut outputs = Vec::new();
    let mut failed = Vec::new();

    for image_path in &request.image_paths {
        match add_logo_to_image(
            Path::new(image_path),
            &logo,
            &output_dir,
            request.position,
            margin,
            logo_width_percent,
            custom_position,
        ) {
            Ok(output) => outputs.push(path_to_string(output)),
            Err(error) => failed.push(ImageProcessFailure {
                path: image_path.clone(),
                message: error,
            }),
        }
    }

    Ok(BatchLogoResult {
        total: request.image_paths.len(),
        succeeded: outputs.len(),
        outputs,
        failed,
    })
}

fn add_logo_to_image(
    image_path: &Path,
    logo: &image::DynamicImage,
    output_dir: &Path,
    position: LogoPosition,
    margin: u32,
    logo_width_percent: f32,
    custom_position: Option<(f32, f32)>,
) -> Result<PathBuf, String> {
    if !image_path.is_file() {
        return Err("图片文件不存在".to_string());
    }

    let base_image = image::open(image_path).map_err(|error| format!("读取图片失败：{error}"))?;
    let mut base = base_image.to_rgba8();
    let base_width = base.width();
    let base_height = base.height();
    if base_width == 0 || base_height == 0 {
        return Err("图片尺寸无效".to_string());
    }

    let target_logo_width =
        ((base_width as f32 * logo_width_percent / 100.0).round() as u32).max(1);
    let logo_ratio = logo.height() as f32 / logo.width().max(1) as f32;
    let target_logo_height = ((target_logo_width as f32 * logo_ratio).round() as u32).max(1);
    let logo = logo.resize(
        target_logo_width,
        target_logo_height,
        image::imageops::FilterType::Lanczos3,
    );
    let logo = logo.to_rgba8();

    let (x, y) = if let Some((x_percent, y_percent)) = custom_position {
        custom_logo_coordinates(
            base_width,
            base_height,
            logo.width(),
            logo.height(),
            x_percent,
            y_percent,
        )
    } else {
        logo_coordinates(
            position,
            base_width,
            base_height,
            logo.width(),
            logo.height(),
            margin,
        )
    };
    image::imageops::overlay(&mut base, &logo, x.into(), y.into());

    let output_path = unique_logo_output_path(output_dir, image_path)?;
    save_processed_image(base, &output_path)?;
    Ok(output_path)
}

fn save_processed_image(image: image::RgbaImage, output_path: &Path) -> Result<(), String> {
    let dynamic = image::DynamicImage::ImageRgba8(image);
    let extension = output_path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();

    if matches!(extension.as_str(), "jpg" | "jpeg") {
        dynamic
            .to_rgb8()
            .save(output_path)
            .map_err(|error| format!("保存图片失败：{error}"))
    } else {
        dynamic
            .save(output_path)
            .map_err(|error| format!("保存图片失败：{error}"))
    }
}

fn logo_coordinates(
    position: LogoPosition,
    base_width: u32,
    base_height: u32,
    logo_width: u32,
    logo_height: u32,
    margin: u32,
) -> (u32, u32) {
    let max_x = base_width.saturating_sub(logo_width);
    let max_y = base_height.saturating_sub(logo_height);
    let margin_x = margin.min(max_x);
    let margin_y = margin.min(max_y);

    match position {
        LogoPosition::TopLeft => (margin_x, margin_y),
        LogoPosition::TopRight => (max_x.saturating_sub(margin_x), margin_y),
        LogoPosition::BottomLeft => (margin_x, max_y.saturating_sub(margin_y)),
        LogoPosition::BottomRight => (
            max_x.saturating_sub(margin_x),
            max_y.saturating_sub(margin_y),
        ),
        LogoPosition::Center => (max_x / 2, max_y / 2),
    }
}

fn custom_logo_coordinates(
    base_width: u32,
    base_height: u32,
    logo_width: u32,
    logo_height: u32,
    x_percent: f32,
    y_percent: f32,
) -> (u32, u32) {
    let max_x = base_width.saturating_sub(logo_width);
    let max_y = base_height.saturating_sub(logo_height);
    let x = ((base_width as f32 * x_percent / 100.0).round() as u32).min(max_x);
    let y = ((base_height as f32 * y_percent / 100.0).round() as u32).min(max_y);
    (x, y)
}

fn unique_logo_output_path(output_dir: &Path, image_path: &Path) -> Result<PathBuf, String> {
    let stem = image_path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "无法读取图片文件名".to_string())?;
    let extension = image_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("png");

    let mut index = 0;
    loop {
        let filename = if index == 0 {
            format!("{stem}_logo.{extension}")
        } else {
            format!("{stem}_logo_{index}.{extension}")
        };
        let candidate = output_dir.join(filename);
        if !candidate.exists() {
            return Ok(candidate);
        }
        index += 1;
    }
}

fn persist_download_event(db_path: &Path, payload: &DownloadEventPayload) {
    let Ok(connection) = Connection::open(db_path) else {
        return;
    };

    let _ = connection.execute(
        r#"
        UPDATE download_tasks
        SET status = ?2,
            progress = CASE
                WHEN ?3 > 0 OR ?2 IN ('queued', 'completed') THEN ?3
                ELSE progress
            END,
            speed = ?4,
            eta = ?5,
            message = CASE
                WHEN ?6 IS NOT NULL THEN ?6
                ELSE message
            END,
            started_at = CASE
                WHEN started_at IS NULL AND ?2 = 'running' THEN datetime('now', 'localtime')
                ELSE started_at
            END,
            finished_at = CASE
                WHEN finished_at IS NULL AND ?2 IN ('completed', 'failed', 'cancelled') THEN datetime('now', 'localtime')
                ELSE finished_at
            END,
            local_path = COALESCE(local_path, ?7),
            updated_at = datetime('now', 'localtime')
        WHERE id = ?1
        "#,
        params![
            payload.id,
            payload.status.as_str(),
            payload.progress,
            payload.speed,
            payload.eta,
            payload.message,
            payload.local_path
        ],
    );
}

fn emit_download_event(app: &AppHandle, payload: DownloadEventPayload) {
    let state = app.state::<DownloadTasks>();
    persist_download_event(&state.db_path, &payload);
    let _ = app.emit(DOWNLOAD_EVENT, payload);
}

fn path_to_string(path: PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        #[cfg(target_os = "macos")]
        {
            let _ = app.set_activation_policy(ActivationPolicy::Regular);
        }
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                    #[cfg(target_os = "macos")]
                    {
                        let _ = window
                            .app_handle()
                            .set_activation_policy(ActivationPolicy::Accessory);
                    }
                }
            }
        })
        .setup(|app| {
            let state = create_download_state(app.handle())
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
            app.manage(state);
            let ecommerce_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?
                .join("ecommerce");
            let ecommerce_store = EcommerceStore::new(ecommerce_dir)
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
            app.manage(ecommerce_store);
            let clipboard_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?
                .join("clipboard");
            let clipboard_store = clipboard::storage::ClipboardStore::new(clipboard_dir)
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
            let clipboard_watcher_state = clipboard::watcher::ClipboardWatcherState::new();
            let clipboard_store_for_watcher = clipboard_store.clone();
            app.manage(clipboard_watcher_state.clone());
            clipboard::watcher::start_clipboard_watcher(
                app.handle().clone(),
                clipboard_store_for_watcher,
                clipboard_watcher_state,
            );
            if let Err(error) = clipboard::watcher::register_clipboard_shortcut(app.handle()) {
                eprintln!("{error}");
            }
            app.manage(clipboard_store);

            let show_item =
                MenuItem::with_id(app, "show-main", "显示主窗口", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&show_item, &quit_item])?;
            TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&tray_menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show-main" => show_main_window(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_default_download_dir,
            list_download_tasks,
            start_download,
            cancel_download,
            open_download_folder,
            open_external_url,
            resolve_douyin_url,
            extract_douyin_video,
            extract_xhs_note,
            extract_bilibili_video,
            extract_youtube_video,
            batch_add_logo,
            list_logo_presets,
            save_logo_preset,
            ecommerce::commands::import_psd_template,
            ecommerce::commands::list_ecommerce_templates,
            ecommerce::commands::load_ecommerce_template,
            ecommerce::commands::save_ecommerce_template,
            ecommerce::commands::delete_ecommerce_template,
            ecommerce::commands::rename_ecommerce_template,
            ecommerce::commands::save_template_asset,
            ecommerce::commands::list_template_assets,
            ecommerce::commands::delete_template_asset,
            ecommerce::commands::import_template_asset_from_path,
            ecommerce::commands::run_batch_replace_tasks,
            ecommerce::commands::save_batch_replace_outputs,
            clipboard::commands::list_clipboard_items,
            clipboard::commands::delete_clipboard_item,
            clipboard::commands::set_clipboard_item_pinned,
            clipboard::commands::clear_clipboard_history,
            clipboard::commands::get_clipboard_settings,
            clipboard::commands::save_clipboard_settings,
            clipboard::commands::restore_clipboard_item,
            imaging::commands::compress_images,
            imaging::commands::convert_images,
            imaging::commands::read_image_exif,
            imaging::commands::strip_image_exif,
            imaging::commands::write_binary_file,
            imaging::commands::ocr_image
        ])
        .run(tauri::generate_context!())
        .expect("error while running AT Tool");
}
