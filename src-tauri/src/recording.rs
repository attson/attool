use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
#[cfg(target_os = "linux")]
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
#[cfg(target_os = "linux")]
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use uuid::Uuid;

use crate::recording_preview::RecordingPreviewServer;

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
            crate::imaging::capture::trigger_recording_overlay(&handle);
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

#[cfg(target_os = "linux")]
enum NativeRecordingControl {
    Start {
        region: NativeCaptureRegion,
        resolution: String,
        frame_rate: u32,
        response: mpsc::SyncSender<Result<NativeRecordingStarted, String>>,
    },
    Pause,
    Resume,
    Stop(mpsc::SyncSender<Result<RecordingSaved, String>>),
    Cancel(mpsc::SyncSender<Result<(), String>>),
}

#[cfg(target_os = "linux")]
struct NativeRecordingController {
    control: mpsc::Sender<NativeRecordingControl>,
}

#[derive(Clone, Default)]
pub struct RecordingStore {
    sessions: Arc<Mutex<HashMap<String, PendingRecording>>>,
    completed: Arc<Mutex<HashSet<PathBuf>>>,
    previews: Arc<RecordingPreviewServer>,
    #[cfg(target_os = "linux")]
    native_sessions: Arc<Mutex<HashMap<String, NativeRecordingController>>>,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayRecordingResult {
    output_path: String,
    size: u64,
    source_label: String,
    elapsed_ms: u64,
    extension: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeRecordingCapabilities {
    supported: bool,
    audio_supported: bool,
    window_supported: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeRecordingStarted {
    session_id: String,
    source_label: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeRecordingPrepared {
    session_id: String,
    source_label: String,
    preview_path: String,
    source_width: u32,
    source_height: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeCaptureRegion {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl RecordingStore {
    fn track_completed(&self, output_path: PathBuf) -> Result<(), String> {
        self.completed
            .lock()
            .map_err(|_| "录制缓存状态不可用".to_string())?
            .insert(output_path);
        Ok(())
    }

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
        self.track_completed(output_path.clone())?;
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
                self.previews.unregister(&path);
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                completed.remove(&path);
                self.previews.unregister(&path);
                Ok(())
            }
            Err(error) => Err(format!("清理录制缓存失败：{error}")),
        }
    }

    fn preview_url(&self, path: &str) -> Result<String, String> {
        let path = PathBuf::from(path);
        if !self
            .completed
            .lock()
            .map_err(|_| "录制缓存状态不可用".to_string())?
            .contains(&path)
        {
            return Err("录制缓存不存在或已清理".to_string());
        }
        self.previews.register(path)
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

fn scaled_dimensions(width: u32, height: u32, resolution: &str) -> (u32, u32) {
    let max_height = match resolution {
        "1080" => 1080,
        "720" => 720,
        _ => return (width, height),
    };
    if height <= max_height {
        return (width, height);
    }
    let scaled_width = ((width as u64 * max_height as u64 / height as u64) as u32) & !1;
    (scaled_width.max(2), max_height)
}

fn normalize_native_region(
    source_width: u32,
    source_height: u32,
    region: NativeCaptureRegion,
) -> Result<NativeCaptureRegion, String> {
    let width = region.width & !1;
    let height = region.height & !1;
    let right = region
        .x
        .checked_add(width)
        .ok_or_else(|| "录制区域横向坐标溢出".to_string())?;
    let bottom = region
        .y
        .checked_add(height)
        .ok_or_else(|| "录制区域纵向坐标溢出".to_string())?;
    if width < 2 || height < 2 {
        return Err("录制区域至少需要 2 × 2 像素".to_string());
    }
    if right > source_width || bottom > source_height {
        return Err("录制区域超出捕获源边界".to_string());
    }
    Ok(NativeCaptureRegion {
        width,
        height,
        ..region
    })
}

fn native_pipeline_description(
    source_width: u32,
    source_height: u32,
    region: NativeCaptureRegion,
    resolution: &str,
) -> Result<String, String> {
    let region = normalize_native_region(source_width, source_height, region)?;
    let right = source_width - region.x - region.width;
    let bottom = source_height - region.y - region.height;
    let (target_width, target_height) = scaled_dimensions(region.width, region.height, resolution);
    let bitrate = if target_height <= 720 {
        4_000_000
    } else {
        8_000_000
    };
    Ok(format!(
        "appsrc name=source ! videoconvert ! videocrop left={} right={right} top={} bottom={bottom} ! videoscale ! video/x-raw,width={target_width},height={target_height},pixel-aspect-ratio=1/1 ! queue ! vp8enc deadline=1 cpu-used=8 threads=4 target-bitrate={bitrate} ! webmmux ! filesink name=sink",
        region.x, region.y
    ))
}

fn native_recording_supported() -> bool {
    cfg!(target_os = "linux")
        && std::env::var("XDG_SESSION_TYPE")
            .map(|value| value.eq_ignore_ascii_case("wayland"))
            .unwrap_or(false)
}

#[tauri::command]
pub fn get_native_recording_capabilities() -> NativeRecordingCapabilities {
    let supported = native_recording_supported();
    NativeRecordingCapabilities {
        supported,
        audio_supported: !supported,
        window_supported: !supported,
    }
}

#[cfg(target_os = "linux")]
fn run_native_recording(
    session_id: String,
    cache_dir: PathBuf,
    app: AppHandle,
    store: RecordingStore,
    controls: mpsc::Receiver<NativeRecordingControl>,
    prepared: mpsc::SyncSender<Result<NativeRecordingPrepared, String>>,
) {
    let result =
        run_native_recording_inner(&session_id, &cache_dir, store.clone(), controls, &prepared);
    if let Ok(mut sessions) = store.native_sessions.lock() {
        sessions.remove(&session_id);
    }
    if let Err(error) = result {
        let _ = prepared.try_send(Err(error.clone()));
        let _ = app.emit(
            "recording://native-error",
            serde_json::json!({ "sessionId": session_id, "message": error }),
        );
    }
}

#[cfg(target_os = "linux")]
fn run_native_recording_inner(
    session_id: &str,
    cache_dir: &Path,
    store: RecordingStore,
    controls: mpsc::Receiver<NativeRecordingControl>,
    prepared: &mpsc::SyncSender<Result<NativeRecordingPrepared, String>>,
) -> Result<(), String> {
    use gst::prelude::*;
    use gstreamer as gst;

    fs::create_dir_all(cache_dir).map_err(|error| format!("创建录制缓存失败：{error}"))?;
    configure_bundled_gstreamer()?;
    gst::init().map_err(|error| format!("初始化视频编码器失败：{error}"))?;

    let monitor = xcap::Monitor::all()
        .map_err(|error| format!("读取显示器失败：{error}"))?
        .into_iter()
        .next()
        .ok_or_else(|| "没有可录制的显示器".to_string())?;
    let (recorder, frames) = monitor
        .video_recorder()
        .map_err(|error| format!("打开系统选屏器失败：{error}"))?;
    recorder
        .start()
        .map_err(|error| format!("启动屏幕采集失败：{error}"))?;
    let first_frame = frames
        .recv_timeout(Duration::from_secs(15))
        .map_err(|_| "选择屏幕后未收到画面，请重新录制".to_string())?;
    let source_width = first_frame.width;
    let source_height = first_frame.height;
    let preview_path = cache_dir.join(format!("attool-recording-source-{session_id}.png"));
    let preview = image::RgbaImage::from_raw(source_width, source_height, first_frame.raw)
        .ok_or_else(|| "录屏首帧像素格式无效".to_string())?;
    preview
        .save(&preview_path)
        .map_err(|error| format!("保存录屏首帧失败：{error}"))?;
    if prepared
        .send(Ok(NativeRecordingPrepared {
            session_id: session_id.to_string(),
            source_label: format!("Wayland 屏幕 · {source_width} × {source_height}"),
            preview_path: preview_path.to_string_lossy().into_owned(),
            source_width,
            source_height,
        }))
        .is_err()
    {
        recorder.stop().ok();
        let _ = fs::remove_file(&preview_path);
        return Err("录屏界面已关闭".to_string());
    }

    let (region, resolution, frame_rate, start_response) = loop {
        while frames.try_recv().is_ok() {}
        match controls.recv_timeout(Duration::from_millis(50)) {
            Ok(NativeRecordingControl::Start {
                region,
                resolution,
                frame_rate,
                response,
            }) => break (region, resolution, frame_rate, response),
            Ok(NativeRecordingControl::Cancel(response)) => {
                recorder.stop().ok();
                let _ = fs::remove_file(&preview_path);
                let _ = response.send(Ok(()));
                return Ok(());
            }
            Ok(NativeRecordingControl::Stop(response)) => {
                recorder.stop().ok();
                let _ = fs::remove_file(&preview_path);
                let _ = response.send(Err("录屏尚未开始".to_string()));
                return Ok(());
            }
            Ok(NativeRecordingControl::Pause | NativeRecordingControl::Resume) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                recorder.stop().ok();
                let _ = fs::remove_file(&preview_path);
                return Ok(());
            }
        }
    };

    let region = match normalize_native_region(source_width, source_height, region) {
        Ok(region) => region,
        Err(error) => {
            let _ = start_response.send(Err(error));
            recorder.stop().ok();
            let _ = fs::remove_file(&preview_path);
            return Ok(());
        }
    };
    let (target_width, target_height) = scaled_dimensions(region.width, region.height, &resolution);
    let temporary_path = cache_dir.join(format!(".attool-recording-{session_id}.webm.part"));
    let output_path = cache_dir.join(format!("attool-recording-{session_id}.webm"));
    let initialized = (|| {
        let pipeline_description =
            native_pipeline_description(source_width, source_height, region, &resolution)?;
        let pipeline = gst::parse::launch(&pipeline_description)
            .map_err(|error| format!("创建视频编码管线失败：{error}"))?
            .downcast::<gst::Pipeline>()
            .map_err(|_| "视频编码管线类型无效".to_string())?;
        let source = pipeline
            .by_name("source")
            .and_then(|element| element.downcast::<gstreamer_app::AppSrc>().ok())
            .ok_or_else(|| "视频编码输入不可用".to_string())?;
        let sink = pipeline
            .by_name("sink")
            .ok_or_else(|| "视频编码输出不可用".to_string())?;
        sink.set_property("location", temporary_path.to_string_lossy().as_ref());
        source.set_caps(Some(
            &gst::Caps::builder("video/x-raw")
                .field("format", "RGBA")
                .field("width", source_width as i32)
                .field("height", source_height as i32)
                .field(
                    "framerate",
                    gst::Fraction::new(frame_rate.clamp(1, 60) as i32, 1),
                )
                .build(),
        ));
        source.set_format(gst::Format::Time);
        source.set_is_live(true);
        source.set_do_timestamp(true);
        source.set_block(true);
        pipeline
            .set_state(gst::State::Playing)
            .map_err(|error| format!("启动视频编码器失败：{error:?}"))?;
        Ok::<_, String>((pipeline, source))
    })();
    let (pipeline, source) = match initialized {
        Ok(initialized) => initialized,
        Err(error) => {
            let _ = start_response.send(Err(error));
            recorder.stop().ok();
            let _ = fs::remove_file(&preview_path);
            let _ = fs::remove_file(&temporary_path);
            return Ok(());
        }
    };
    while frames.try_recv().is_ok() {}
    let _ = fs::remove_file(&preview_path);
    if start_response
        .send(Ok(NativeRecordingStarted {
            session_id: session_id.to_string(),
            source_label: format!("区域录屏 · {target_width} × {target_height}"),
        }))
        .is_err()
    {
        recorder.stop().ok();
        pipeline.set_state(gst::State::Null).ok();
        let _ = fs::remove_file(&temporary_path);
        return Err("录屏界面已关闭".to_string());
    }

    let finish = loop {
        match controls.try_recv() {
            Ok(NativeRecordingControl::Pause) => {
                if let Err(error) = recorder.stop() {
                    break NativeFinish::Error(format!("暂停屏幕采集失败：{error}"));
                }
                while frames.try_recv().is_ok() {}
                let paused_finish = loop {
                    match controls.recv() {
                        Ok(NativeRecordingControl::Resume) => match recorder.start() {
                            Ok(()) => break None,
                            Err(error) => {
                                break Some(NativeFinish::Error(format!(
                                    "继续屏幕采集失败：{error}"
                                )))
                            }
                        },
                        Ok(NativeRecordingControl::Stop(response)) => {
                            break Some(NativeFinish::Save(response));
                        }
                        Ok(NativeRecordingControl::Cancel(response)) => {
                            break Some(NativeFinish::Cancel(Some(response)));
                        }
                        Ok(NativeRecordingControl::Start { response, .. }) => {
                            let _ = response.send(Err("录屏已经开始".to_string()));
                        }
                        Ok(NativeRecordingControl::Pause) => {}
                        Err(_) => break Some(NativeFinish::Cancel(None)),
                    }
                };
                if let Some(finish) = paused_finish {
                    break finish;
                }
            }
            Ok(NativeRecordingControl::Resume) => {}
            Ok(NativeRecordingControl::Stop(response)) => break NativeFinish::Save(response),
            Ok(NativeRecordingControl::Cancel(response)) => {
                break NativeFinish::Cancel(Some(response))
            }
            Ok(NativeRecordingControl::Start { response, .. }) => {
                let _ = response.send(Err("录屏已经开始".to_string()));
            }
            Err(mpsc::TryRecvError::Disconnected) => break NativeFinish::Cancel(None),
            Err(mpsc::TryRecvError::Empty) => {}
        }

        match frames.recv_timeout(Duration::from_millis(50)) {
            Ok(frame) => {
                if let Err(error) = source.push_buffer(gst::Buffer::from_mut_slice(frame.raw)) {
                    break NativeFinish::Error(format!("写入录屏画面失败：{error:?}"));
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break NativeFinish::Error("屏幕采集连接已断开".to_string());
            }
        }
    };

    recorder.stop().ok();
    match finish {
        NativeFinish::Save(response) => {
            let result = (|| {
                source
                    .end_of_stream()
                    .map_err(|error| format!("结束视频编码失败：{error:?}"))?;
                let bus = pipeline
                    .bus()
                    .ok_or_else(|| "视频编码总线不可用".to_string())?;
                let message = bus.timed_pop_filtered(
                    gst::ClockTime::from_seconds(15),
                    &[gst::MessageType::Eos, gst::MessageType::Error],
                );
                if let Some(message) = message {
                    if let gst::MessageView::Error(error) = message.view() {
                        return Err(format!("视频编码失败：{}", error.error()));
                    }
                } else {
                    return Err("等待视频编码结束超时".to_string());
                }
                pipeline.set_state(gst::State::Null).ok();
                fs::rename(&temporary_path, &output_path)
                    .map_err(|error| format!("完成录屏缓存失败：{error}"))?;
                let size = fs::metadata(&output_path)
                    .map_err(|error| format!("读取录屏缓存失败：{error}"))?
                    .len();
                store.track_completed(output_path.clone())?;
                Ok(RecordingSaved {
                    output_path: output_path.to_string_lossy().into_owned(),
                    size,
                })
            })();
            if result.is_err() {
                pipeline.set_state(gst::State::Null).ok();
                let _ = fs::remove_file(&temporary_path);
            }
            let _ = response.send(result);
        }
        NativeFinish::Cancel(response) => {
            pipeline.set_state(gst::State::Null).ok();
            let result = match fs::remove_file(&temporary_path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(format!("清理临时录屏失败：{error}")),
            };
            if let Some(response) = response {
                let _ = response.send(result);
            }
        }
        NativeFinish::Error(error) => {
            pipeline.set_state(gst::State::Null).ok();
            let _ = fs::remove_file(&temporary_path);
            return Err(error);
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
#[cfg(debug_assertions)]
fn configure_bundled_gstreamer() -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "linux")]
#[cfg(not(debug_assertions))]
fn configure_bundled_gstreamer() -> Result<(), String> {
    let runtime = crate::bundled_tools::gstreamer()
        .map_err(|error| format!("AT Tool 内置 GStreamer 运行时损坏：{error}"))?;
    let plugins = runtime.root.join("plugins");
    if !plugins.is_dir() {
        return Err(
            "AT Tool 内置 GStreamer 运行时损坏：缺少插件目录，请重新安装或更新 AT Tool".to_string(),
        );
    }
    let registry = runtime.root.join("registry.bin");
    std::env::set_var("GST_PLUGIN_PATH_1_0", &plugins);
    std::env::set_var("GST_PLUGIN_SYSTEM_PATH_1_0", "");
    std::env::set_var("GST_REGISTRY_1_0", registry);
    Ok(())
}

#[cfg(target_os = "linux")]
enum NativeFinish {
    Save(mpsc::SyncSender<Result<RecordingSaved, String>>),
    Cancel(Option<mpsc::SyncSender<Result<(), String>>>),
    Error(String),
}

#[cfg(target_os = "linux")]
fn native_control_sender(
    store: &RecordingStore,
    session_id: &str,
) -> Result<mpsc::Sender<NativeRecordingControl>, String> {
    store
        .native_sessions
        .lock()
        .map_err(|_| "原生录屏会话状态不可用".to_string())?
        .get(session_id)
        .map(|session| session.control.clone())
        .ok_or_else(|| "原生录屏会话不存在或已结束".to_string())
}

#[cfg(target_os = "linux")]
#[tauri::command]
pub async fn prepare_native_recording(
    app: AppHandle,
    store: tauri::State<'_, RecordingStore>,
) -> Result<NativeRecordingPrepared, String> {
    if !native_recording_supported() {
        return Err("当前会话不需要原生 Wayland 录屏".to_string());
    }
    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("获取缓存目录失败：{error}"))?
        .join("recordings");
    let session_id = Uuid::new_v4().to_string();
    let (control_tx, control_rx) = mpsc::channel();
    let (prepared_tx, prepared_rx) = mpsc::sync_channel(1);
    let worker_session_id = session_id.clone();
    let worker_store = store.inner().clone();
    let worker_app = app.clone();
    std::thread::spawn(move || {
        run_native_recording(
            worker_session_id,
            cache_dir,
            worker_app,
            worker_store,
            control_rx,
            prepared_tx,
        );
    });
    let prepared = tauri::async_runtime::spawn_blocking(move || {
        prepared_rx
            .recv()
            .map_err(|_| "原生录屏准备任务异常结束".to_string())?
    })
    .await
    .map_err(|error| format!("等待原生录屏准备失败：{error}"))??;
    store
        .native_sessions
        .lock()
        .map_err(|_| "原生录屏会话状态不可用".to_string())?
        .insert(
            session_id,
            NativeRecordingController {
                control: control_tx,
            },
        );
    Ok(prepared)
}

#[cfg(not(target_os = "linux"))]
#[tauri::command]
pub async fn prepare_native_recording(
    _app: AppHandle,
    _store: tauri::State<'_, RecordingStore>,
) -> Result<NativeRecordingPrepared, String> {
    Err("当前平台不支持原生 Wayland 录屏".to_string())
}

#[cfg(target_os = "linux")]
#[tauri::command]
pub async fn start_native_recording(
    session_id: String,
    region: NativeCaptureRegion,
    resolution: String,
    frame_rate: u32,
    store: tauri::State<'_, RecordingStore>,
) -> Result<NativeRecordingStarted, String> {
    let sender = native_control_sender(&store, &session_id)?;
    let (response_tx, response_rx) = mpsc::sync_channel(1);
    sender
        .send(NativeRecordingControl::Start {
            region,
            resolution,
            frame_rate,
            response: response_tx,
        })
        .map_err(|_| "原生录屏会话已结束".to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        response_rx
            .recv()
            .map_err(|_| "原生录屏启动任务异常退出".to_string())?
    })
    .await
    .map_err(|error| format!("等待原生录屏启动失败：{error}"))?
}

#[cfg(not(target_os = "linux"))]
#[tauri::command]
pub async fn start_native_recording(
    _session_id: String,
    _region: NativeCaptureRegion,
    _resolution: String,
    _frame_rate: u32,
    _store: tauri::State<'_, RecordingStore>,
) -> Result<NativeRecordingStarted, String> {
    Err("当前平台不支持原生 Wayland 录屏".to_string())
}

#[cfg(target_os = "linux")]
#[tauri::command]
pub async fn pause_native_recording(
    session_id: String,
    store: tauri::State<'_, RecordingStore>,
) -> Result<(), String> {
    native_control_sender(&store, &session_id)?
        .send(NativeRecordingControl::Pause)
        .map_err(|_| "原生录屏会话已结束".to_string())
}

#[cfg(not(target_os = "linux"))]
#[tauri::command]
pub async fn pause_native_recording(
    _session_id: String,
    _store: tauri::State<'_, RecordingStore>,
) -> Result<(), String> {
    Err("当前平台不支持原生 Wayland 录屏".to_string())
}

#[cfg(target_os = "linux")]
#[tauri::command]
pub async fn resume_native_recording(
    session_id: String,
    store: tauri::State<'_, RecordingStore>,
) -> Result<(), String> {
    native_control_sender(&store, &session_id)?
        .send(NativeRecordingControl::Resume)
        .map_err(|_| "原生录屏会话已结束".to_string())
}

#[cfg(not(target_os = "linux"))]
#[tauri::command]
pub async fn resume_native_recording(
    _session_id: String,
    _store: tauri::State<'_, RecordingStore>,
) -> Result<(), String> {
    Err("当前平台不支持原生 Wayland 录屏".to_string())
}

#[cfg(target_os = "linux")]
#[tauri::command]
pub async fn stop_native_recording(
    session_id: String,
    store: tauri::State<'_, RecordingStore>,
) -> Result<RecordingSaved, String> {
    let controller = store
        .native_sessions
        .lock()
        .map_err(|_| "原生录屏会话状态不可用".to_string())?
        .remove(&session_id)
        .ok_or_else(|| "原生录屏会话不存在或已结束".to_string())?;
    let (response_tx, response_rx) = mpsc::sync_channel(1);
    controller
        .control
        .send(NativeRecordingControl::Stop(response_tx))
        .map_err(|_| "原生录屏会话已结束".to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        response_rx
            .recv()
            .map_err(|_| "原生录屏结束任务异常退出".to_string())?
    })
    .await
    .map_err(|error| format!("等待原生录屏结束失败：{error}"))?
}

#[cfg(not(target_os = "linux"))]
#[tauri::command]
pub async fn stop_native_recording(
    _session_id: String,
    _store: tauri::State<'_, RecordingStore>,
) -> Result<RecordingSaved, String> {
    Err("当前平台不支持原生 Wayland 录屏".to_string())
}

#[cfg(target_os = "linux")]
#[tauri::command]
pub async fn cancel_native_recording(
    session_id: String,
    store: tauri::State<'_, RecordingStore>,
) -> Result<(), String> {
    let controller = store
        .native_sessions
        .lock()
        .map_err(|_| "原生录屏会话状态不可用".to_string())?
        .remove(&session_id)
        .ok_or_else(|| "原生录屏会话不存在或已结束".to_string())?;
    let (response_tx, response_rx) = mpsc::sync_channel(1);
    controller
        .control
        .send(NativeRecordingControl::Cancel(response_tx))
        .map_err(|_| "原生录屏会话已结束".to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        response_rx
            .recv()
            .map_err(|_| "原生录屏清理任务异常退出".to_string())?
    })
    .await
    .map_err(|error| format!("等待原生录屏清理失败：{error}"))?
}

#[cfg(not(target_os = "linux"))]
#[tauri::command]
pub async fn cancel_native_recording(
    _session_id: String,
    _store: tauri::State<'_, RecordingStore>,
) -> Result<(), String> {
    Err("当前平台不支持原生 Wayland 录屏".to_string())
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
pub fn recording_preview_url(
    path: String,
    store: tauri::State<'_, RecordingStore>,
) -> Result<String, String> {
    store.preview_url(&path)
}

#[tauri::command]
pub fn present_recording_result(
    app: AppHandle,
    result: OverlayRecordingResult,
) -> Result<(), String> {
    if !matches!(result.extension.as_str(), "webm" | "mp4") {
        return Err("不支持的录屏文件格式".to_string());
    }
    crate::show_main_window(&app);
    app.emit("recording://completed", result)
        .map_err(|error| format!("发送录屏完成事件失败：{error}"))?;
    crate::imaging::capture::close_capture_overlay(&app)
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
    fn scales_recording_dimensions_without_upscaling() {
        assert_eq!(scaled_dimensions(3840, 2160, "1080"), (1920, 1080));
        assert_eq!(scaled_dimensions(2560, 1440, "720"), (1280, 720));
        assert_eq!(scaled_dimensions(1920, 1080, "original"), (1920, 1080));
        assert_eq!(scaled_dimensions(1280, 720, "1080"), (1280, 720));
    }

    #[test]
    fn normalizes_native_crop_inside_source_bounds() {
        assert_eq!(
            normalize_native_region(
                1920,
                1080,
                NativeCaptureRegion {
                    x: 101,
                    y: 55,
                    width: 801,
                    height: 603,
                }
            )
            .unwrap(),
            NativeCaptureRegion {
                x: 101,
                y: 55,
                width: 800,
                height: 602,
            }
        );
        assert!(normalize_native_region(
            1920,
            1080,
            NativeCaptureRegion {
                x: 1800,
                y: 0,
                width: 200,
                height: 200,
            }
        )
        .is_err());
    }

    #[test]
    fn native_pipeline_crops_before_scaling() {
        let pipeline = native_pipeline_description(
            1920,
            1080,
            NativeCaptureRegion {
                x: 100,
                y: 50,
                width: 1280,
                height: 720,
            },
            "720",
        )
        .unwrap();

        assert!(pipeline.contains("videocrop left=100 right=540 top=50 bottom=310"));
        assert!(pipeline.find("videocrop").unwrap() < pipeline.find("videoscale").unwrap());
        assert!(pipeline.contains("video/x-raw,width=1280,height=720"));
    }

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

    #[test]
    fn preview_only_serves_tracked_recordings() {
        let cache = tempfile::tempdir().unwrap();
        let untracked = cache.path().join("untracked.webm");
        fs::write(&untracked, b"private").unwrap();
        let store = RecordingStore::default();

        assert!(store.preview_url(untracked.to_str().unwrap()).is_err());

        let session = store.begin(cache.path()).unwrap();
        store.append(&session.session_id, "dmlkZW8=").unwrap();
        let completed = store.finish(&session.session_id, "webm").unwrap();
        let url = store.preview_url(&completed.output_path).unwrap();

        assert!(url.starts_with("http://127.0.0.1:"));
    }
}
