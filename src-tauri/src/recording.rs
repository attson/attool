use base64::Engine;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use uuid::Uuid;

const MAX_CHUNK_BYTES: usize = 1024 * 1024;
pub const DEFAULT_RECORDING_SHORTCUT: &str = "CommandOrControl+Shift+R";

static REGISTERED_SHORTCUT: OnceLock<Mutex<String>> = OnceLock::new();

fn registered_shortcut_slot() -> &'static Mutex<String> {
    REGISTERED_SHORTCUT.get_or_init(|| Mutex::new(String::new()))
}

fn shortcut_config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法获取应用数据目录：{error}"))?;
    fs::create_dir_all(&directory).map_err(|error| format!("创建应用数据目录失败：{error}"))?;
    Ok(directory.join("recording-config.json"))
}

pub fn configured_recording_shortcut(app: &AppHandle) -> String {
    let Ok(path) = shortcut_config_path(app) else {
        return DEFAULT_RECORDING_SHORTCUT.to_string();
    };
    fs::read_to_string(path)
        .ok()
        .and_then(|body| serde_json::from_str::<serde_json::Value>(&body).ok())
        .and_then(|value| value.get("shortcut")?.as_str().map(str::to_string))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_RECORDING_SHORTCUT.to_string())
}

fn current_recording_shortcut() -> String {
    registered_shortcut_slot()
        .lock()
        .map(|value| value.clone())
        .unwrap_or_default()
}

fn write_recording_shortcut(app: &AppHandle, shortcut: &str) -> Result<(), String> {
    let body = serde_json::json!({ "shortcut": shortcut }).to_string();
    fs::write(shortcut_config_path(app)?, body)
        .map_err(|error| format!("写入录屏快捷键配置失败：{error}"))
}

fn install_recording_shortcut(app: &AppHandle, shortcut: Shortcut) -> Result<(), String> {
    let handle = app.clone();
    app.global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            crate::show_main_window(&handle);
            let _ = handle.emit("recording://toggle", ());
        })
        .map_err(|error| format!("注册录屏快捷键失败：{error}"))
}

pub fn register_recording_shortcut(app: &AppHandle) -> Result<(), String> {
    if let Some(reason) = crate::imaging::capture::global_shortcut_unavailable_reason() {
        return Err(reason.to_string());
    }
    let shortcut = configured_recording_shortcut(app);
    let parsed = shortcut
        .parse::<Shortcut>()
        .map_err(|error| format!("解析录屏快捷键失败：{error}"))?;
    install_recording_shortcut(app, parsed)?;
    if let Ok(mut current) = registered_shortcut_slot().lock() {
        *current = shortcut;
    }
    Ok(())
}

fn reregister_recording_shortcut(app: &AppHandle, next: &str) -> Result<(), String> {
    if let Some(reason) = crate::imaging::capture::global_shortcut_unavailable_reason() {
        return Err(reason.to_string());
    }
    let next = next.trim();
    if next.is_empty() {
        return Err("请填入有效的快捷键".to_string());
    }
    let parsed = next
        .parse::<Shortcut>()
        .map_err(|error| format!("快捷键格式非法：{error}"))?;
    let previous = current_recording_shortcut();
    if let Ok(previous_parsed) = previous.parse::<Shortcut>() {
        let _ = app.global_shortcut().unregister(previous_parsed);
    }
    if let Err(error) = install_recording_shortcut(app, parsed) {
        if let Ok(previous_parsed) = previous.parse::<Shortcut>() {
            let _ = install_recording_shortcut(app, previous_parsed);
        }
        return Err(error);
    }
    if let Err(error) = write_recording_shortcut(app, next) {
        let _ = app.global_shortcut().unregister(parsed);
        if let Ok(previous_parsed) = previous.parse::<Shortcut>() {
            let _ = install_recording_shortcut(app, previous_parsed);
        }
        return Err(error);
    }
    if let Ok(mut current) = registered_shortcut_slot().lock() {
        *current = next.to_string();
    }
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingShortcutInfo {
    shortcut: String,
    default_shortcut: String,
    unavailable_reason: Option<String>,
    system_shortcut_command: Option<String>,
}

#[tauri::command]
pub fn get_recording_shortcut(app: AppHandle) -> RecordingShortcutInfo {
    let current = current_recording_shortcut();
    RecordingShortcutInfo {
        shortcut: if current.is_empty() {
            configured_recording_shortcut(&app)
        } else {
            current
        },
        default_shortcut: DEFAULT_RECORDING_SHORTCUT.to_string(),
        unavailable_reason: crate::imaging::capture::global_shortcut_unavailable_reason()
            .map(str::to_string),
        system_shortcut_command: {
            #[cfg(target_os = "linux")]
            {
                crate::capture_command::system_recording_command()
            }
            #[cfg(not(target_os = "linux"))]
            {
                None
            }
        },
    }
}

#[tauri::command]
pub fn set_recording_shortcut(
    app: AppHandle,
    shortcut: String,
) -> Result<RecordingShortcutInfo, String> {
    reregister_recording_shortcut(&app, &shortcut)?;
    Ok(RecordingShortcutInfo {
        shortcut,
        default_shortcut: DEFAULT_RECORDING_SHORTCUT.to_string(),
        unavailable_reason: None,
        system_shortcut_command: None,
    })
}

struct PendingRecording {
    temporary_path: PathBuf,
    writer: BufWriter<File>,
    bytes_written: u64,
}

#[derive(Clone, Default)]
pub struct RecordingStore {
    sessions: Arc<Mutex<HashMap<String, PendingRecording>>>,
    completed: Arc<Mutex<HashSet<PathBuf>>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingSession {
    pub session_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingSaved {
    pub output_path: String,
    pub size: u64,
}

impl RecordingStore {
    fn begin(&self, cache_dir: &Path) -> Result<RecordingSession, String> {
        fs::create_dir_all(cache_dir).map_err(|error| format!("创建录制缓存失败：{error}"))?;
        let session_id = Uuid::new_v4().to_string();
        let temporary_path = cache_dir.join(format!(".attool-recording-{session_id}.part"));
        let file =
            File::create(&temporary_path).map_err(|error| format!("创建录制文件失败：{error}"))?;
        let pending = PendingRecording {
            temporary_path,
            writer: BufWriter::new(file),
            bytes_written: 0,
        };
        self.sessions
            .lock()
            .map_err(|_| "录制会话状态不可用".to_string())?
            .insert(session_id.clone(), pending);
        Ok(RecordingSession { session_id })
    }

    fn append(&self, session_id: &str, base64: &str) -> Result<u64, String> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(base64)
            .map_err(|error| format!("录制数据解码失败：{error}"))?;
        if bytes.len() > MAX_CHUNK_BYTES {
            return Err("录制数据块超过 1 MB 限制".to_string());
        }
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| "录制会话状态不可用".to_string())?;
        let pending = sessions
            .get_mut(session_id)
            .ok_or_else(|| "录制会话不存在或已结束".to_string())?;
        pending
            .writer
            .write_all(&bytes)
            .map_err(|error| format!("写入录制文件失败：{error}"))?;
        pending.bytes_written += bytes.len() as u64;
        Ok(pending.bytes_written)
    }

    fn finish(&self, session_id: &str, extension: &str) -> Result<RecordingSaved, String> {
        if !matches!(extension, "webm" | "mp4") {
            return Err("不支持的录屏文件格式".to_string());
        }
        let mut pending = self.take_session(session_id)?;
        if let Err(error) = pending.writer.flush() {
            drop(pending.writer);
            let _ = fs::remove_file(&pending.temporary_path);
            return Err(format!("刷新录制文件失败：{error}"));
        }
        if let Err(error) = pending.writer.get_ref().sync_all() {
            drop(pending.writer);
            let _ = fs::remove_file(&pending.temporary_path);
            return Err(format!("同步录制文件失败：{error}"));
        }
        drop(pending.writer);

        let output_path = pending
            .temporary_path
            .parent()
            .ok_or_else(|| "录制缓存目录无效".to_string())?
            .join(format!("attool-recording-{session_id}.{extension}"));
        if let Err(error) = fs::rename(&pending.temporary_path, &output_path) {
            let _ = fs::remove_file(&pending.temporary_path);
            return Err(format!("完成录制缓存失败：{error}"));
        }
        self.completed
            .lock()
            .map_err(|_| "录制缓存状态不可用".to_string())?
            .insert(output_path.clone());
        Ok(RecordingSaved {
            output_path: output_path.to_string_lossy().into_owned(),
            size: pending.bytes_written,
        })
    }

    fn save(&self, source: &str, target: &str) -> Result<RecordingSaved, String> {
        let source_path = PathBuf::from(source);
        if !self
            .completed
            .lock()
            .map_err(|_| "录制缓存状态不可用".to_string())?
            .contains(&source_path)
        {
            return Err("录制缓存不存在或已清理".to_string());
        }

        let target_path = PathBuf::from(target.trim());
        let file_name = target_path
            .file_name()
            .filter(|name| !name.is_empty())
            .ok_or_else(|| "请选择有效的保存文件".to_string())?;
        let parent = target_path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).map_err(|error| format!("创建保存目录失败：{error}"))?;

        let temporary_path = parent.join(format!(
            ".{}.attool-{}.part",
            file_name.to_string_lossy(),
            Uuid::new_v4()
        ));
        if let Err(error) = fs::copy(&source_path, &temporary_path) {
            let _ = fs::remove_file(&temporary_path);
            return Err(format!("复制录制文件失败：{error}"));
        }
        if let Err(error) = replace_file(&temporary_path, &target_path) {
            let _ = fs::remove_file(&temporary_path);
            return Err(error);
        }
        let size = fs::metadata(&target_path)
            .map_err(|error| format!("读取录制文件失败：{error}"))?
            .len();
        Ok(RecordingSaved {
            output_path: target_path.to_string_lossy().into_owned(),
            size,
        })
    }

    fn discard(&self, path: &str) -> Result<(), String> {
        let path = PathBuf::from(path);
        let mut completed = self
            .completed
            .lock()
            .map_err(|_| "录制缓存状态不可用".to_string())?;
        if !completed.contains(&path) {
            return Ok(());
        }
        match fs::remove_file(&path) {
            Ok(()) => {
                completed.remove(&path);
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                completed.remove(&path);
                Ok(())
            }
            Err(error) => Err(format!("清理录制缓存失败：{error}")),
        }
    }

    fn cancel(&self, session_id: &str) -> Result<(), String> {
        let pending = self.take_session(session_id)?;
        drop(pending.writer);
        match fs::remove_file(pending.temporary_path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("清理临时录制文件失败：{error}")),
        }
    }

    fn take_session(&self, session_id: &str) -> Result<PendingRecording, String> {
        self.sessions
            .lock()
            .map_err(|_| "录制会话状态不可用".to_string())?
            .remove(session_id)
            .ok_or_else(|| "录制会话不存在或已结束".to_string())
    }
}

#[cfg(not(target_os = "windows"))]
fn replace_file(source: &Path, target: &Path) -> Result<(), String> {
    fs::rename(source, target).map_err(|error| format!("完成录制文件失败：{error}"))
}

#[cfg(target_os = "windows")]
fn replace_file(source: &Path, target: &Path) -> Result<(), String> {
    if !target.exists() {
        return fs::rename(source, target).map_err(|error| format!("完成录制文件失败：{error}"));
    }

    let file_name = target
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_default();
    let backup = target.with_file_name(format!(".{file_name}.attool-backup-{}", Uuid::new_v4()));
    fs::rename(target, &backup).map_err(|error| format!("准备替换已有文件失败：{error}"))?;
    if let Err(error) = fs::rename(source, target) {
        let _ = fs::rename(&backup, target);
        return Err(format!("完成录制文件失败：{error}"));
    }
    let _ = fs::remove_file(backup);
    Ok(())
}

#[tauri::command]
pub async fn begin_recording_file(
    app: tauri::AppHandle,
    store: tauri::State<'_, RecordingStore>,
) -> Result<RecordingSession, String> {
    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("获取缓存目录失败：{error}"))?
        .join("recordings");
    store.begin(&cache_dir)
}

#[tauri::command]
pub async fn append_recording_chunk(
    session_id: String,
    base64: String,
    store: tauri::State<'_, RecordingStore>,
) -> Result<u64, String> {
    store.append(&session_id, &base64)
}

#[tauri::command]
pub async fn finish_recording_file(
    session_id: String,
    extension: String,
    store: tauri::State<'_, RecordingStore>,
) -> Result<RecordingSaved, String> {
    store.finish(&session_id, &extension)
}

#[tauri::command]
pub async fn cancel_recording_file(
    session_id: String,
    store: tauri::State<'_, RecordingStore>,
) -> Result<(), String> {
    store.cancel(&session_id)
}

#[tauri::command]
pub async fn save_recording_file(
    source: String,
    target: String,
    store: tauri::State<'_, RecordingStore>,
) -> Result<RecordingSaved, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || store.save(&source, &target))
        .await
        .map_err(|error| format!("保存录屏任务异常：{error}"))?
}

#[tauri::command]
pub async fn discard_recording_file(
    path: String,
    store: tauri::State<'_, RecordingStore>,
) -> Result<(), String> {
    store.discard(&path)
}

#[tauri::command]
pub fn default_recording_path(file_name: String) -> Result<String, String> {
    let name = Path::new(&file_name)
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| "录屏文件名无效".to_string())?;
    if name != Path::new(&file_name).as_os_str() {
        return Err("录屏文件名不能包含目录".to_string());
    }
    let directory = dirs::video_dir()
        .or_else(dirs::download_dir)
        .or_else(dirs::home_dir)
        .ok_or_else(|| "无法确定默认保存目录".to_string())?;
    Ok(directory.join(name).to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn open_recording_folder(path: String) -> Result<(), String> {
    let parent = Path::new(&path)
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| "录制文件没有有效的保存目录".to_string())?;
    crate::open_folder(&parent.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_chunks_and_moves_completed_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = RecordingStore::default();
        let session = store.begin(dir.path()).unwrap();

        assert_eq!(store.append(&session.session_id, "YWJj").unwrap(), 3);
        assert_eq!(store.append(&session.session_id, "ZGVm").unwrap(), 6);
        let saved = store.finish(&session.session_id, "webm").unwrap();

        assert_eq!(saved.size, 6);
        assert_eq!(fs::read(saved.output_path).unwrap(), b"abcdef");
    }

    #[test]
    fn cancel_removes_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = RecordingStore::default();
        let session = store.begin(dir.path()).unwrap();

        store.cancel(&session.session_id).unwrap();

        assert!(fs::read_dir(dir.path()).unwrap().next().is_none());
    }

    #[test]
    fn save_replaces_an_existing_file() {
        let cache = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        let target = output.path().join("capture.webm");
        fs::write(&target, b"old").unwrap();
        let store = RecordingStore::default();
        let session = store.begin(cache.path()).unwrap();

        store.append(&session.session_id, "bmV3").unwrap();
        let completed = store.finish(&session.session_id, "webm").unwrap();
        store
            .save(&completed.output_path, target.to_str().unwrap())
            .unwrap();

        assert_eq!(fs::read(target).unwrap(), b"new");
    }

    #[test]
    fn discard_only_removes_tracked_cache_files() {
        let cache = tempfile::tempdir().unwrap();
        let untracked = cache.path().join("untracked.webm");
        fs::write(&untracked, b"keep").unwrap();
        let store = RecordingStore::default();
        let session = store.begin(cache.path()).unwrap();
        store.append(&session.session_id, "bmV3").unwrap();
        let completed = store.finish(&session.session_id, "webm").unwrap();

        store.discard(untracked.to_str().unwrap()).unwrap();
        assert!(untracked.exists());
        store.discard(&completed.output_path).unwrap();
        assert!(!Path::new(&completed.output_path).exists());
    }
}
