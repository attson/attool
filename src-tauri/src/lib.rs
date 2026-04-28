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
use tauri::{AppHandle, Emitter, Manager, State};
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
        "--console-log-level=warn".to_string(),
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

#[tauri::command]
async fn open_download_folder(id: String, state: State<'_, DownloadTasks>) -> Result<(), String> {
    let db_path = state.db_path.clone();
    let folder = tauri::async_runtime::spawn_blocking(move || find_download_dir(&db_path, &id))
        .await
        .map_err(|error| format!("读取保存目录失败：{error}"))??;

    open_folder(&folder)
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
            SELECT id, url, download_dir, file_name, status, progress, speed, eta, message, created_at
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
            updated_at = datetime('now', 'localtime')
        WHERE id = ?1
        "#,
        params![
            payload.id,
            payload.status.as_str(),
            payload.progress,
            payload.speed,
            payload.eta,
            payload.message
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

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state = create_download_state(app.handle())
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_default_download_dir,
            list_download_tasks,
            start_download,
            cancel_download,
            open_download_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running AT Tool");
}
