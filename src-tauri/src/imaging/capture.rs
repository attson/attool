use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub const DEFAULT_CAPTURE_SHORTCUT: &str = "CommandOrControl+Shift+A";
pub const CAPTURE_EVENT: &str = "capture-completed";

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CaptureEventPayload {
    output_path: String,
}

/// Currently-registered shortcut string (Tauri format like "CommandOrControl+Shift+A").
static REGISTERED: OnceLock<Mutex<String>> = OnceLock::new();

fn registered_slot() -> &'static Mutex<String> {
    REGISTERED.get_or_init(|| Mutex::new(String::new()))
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法获取应用数据目录：{error}"))?;
    fs::create_dir_all(&dir).map_err(|error| format!("创建应用数据目录失败：{error}"))?;
    Ok(dir.join("capture-config.json"))
}

fn load_stored_shortcut(app: &AppHandle) -> String {
    let Ok(path) = config_path(app) else {
        return DEFAULT_CAPTURE_SHORTCUT.to_string();
    };
    match fs::read_to_string(&path) {
        Ok(body) => match serde_json::from_str::<serde_json::Value>(&body) {
            Ok(v) => v
                .get("shortcut")
                .and_then(|s| s.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(DEFAULT_CAPTURE_SHORTCUT)
                .to_string(),
            Err(_) => DEFAULT_CAPTURE_SHORTCUT.to_string(),
        },
        Err(_) => DEFAULT_CAPTURE_SHORTCUT.to_string(),
    }
}

fn write_stored_shortcut(app: &AppHandle, shortcut: &str) -> Result<(), String> {
    let path = config_path(app)?;
    let body = serde_json::json!({ "shortcut": shortcut }).to_string();
    fs::write(&path, body).map_err(|error| format!("写入截图快捷键配置失败：{error}"))
}

pub fn current_shortcut() -> String {
    registered_slot().lock().map(|g| g.clone()).unwrap_or_default()
}

pub fn register_capture_shortcut(app: &AppHandle) -> Result<(), String> {
    let stored = load_stored_shortcut(app);
    register_shortcut_str(app, &stored)
}

pub fn reregister_capture_shortcut(app: &AppHandle, next: &str) -> Result<(), String> {
    let next = next.trim();
    if next.is_empty() {
        return Err("请填入有效的快捷键".to_string());
    }
    // Validate parses before touching the current registration
    let parsed: Shortcut = next
        .parse()
        .map_err(|error| format!("快捷键格式非法：{error}"))?;

    // Unregister the previously-registered one (if any)
    let previous = current_shortcut();
    if !previous.is_empty() {
        if let Ok(prev_parsed) = previous.parse::<Shortcut>() {
            let _ = app.global_shortcut().unregister(prev_parsed);
        }
    }

    if let Err(error) = install_handler(app, parsed) {
        // Best-effort: try to restore the previous binding so the user is not stuck without one
        if !previous.is_empty() {
            if let Ok(prev_parsed) = previous.parse::<Shortcut>() {
                let _ = install_handler(app, prev_parsed);
            }
        }
        return Err(error);
    }

    write_stored_shortcut(app, next)?;
    if let Ok(mut g) = registered_slot().lock() {
        *g = next.to_string();
    }
    Ok(())
}

fn register_shortcut_str(app: &AppHandle, shortcut_str: &str) -> Result<(), String> {
    let parsed: Shortcut = shortcut_str
        .parse()
        .map_err(|error| format!("解析截图快捷键失败：{error}"))?;
    install_handler(app, parsed)?;
    if let Ok(mut g) = registered_slot().lock() {
        *g = shortcut_str.to_string();
    }
    Ok(())
}

fn install_handler(app: &AppHandle, shortcut: Shortcut) -> Result<(), String> {
    let handle = app.clone();
    app.global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            let inner = handle.clone();
            std::thread::spawn(move || match capture_screen("region", 0) {
                Ok(path) => {
                    if let Some(window) = inner.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                    }
                    let _ = inner.emit(
                        CAPTURE_EVENT,
                        CaptureEventPayload {
                            output_path: path.to_string_lossy().into_owned(),
                        },
                    );
                }
                Err(err) => {
                    let _ = inner.emit("capture-failed", err);
                }
            });
        })
        .map_err(|error| format!("注册截图快捷键失败：{error}"))
}

/// Capture the screen to a PNG file and return its absolute path.
///
/// - `mode`: "region" (interactive rectangle select) | "window" (interactive window select) | "full" (whole desktop)
/// - `delay_seconds`: 0-10 seconds before the capture fires (macOS only supports this natively)
pub fn capture_screen(mode: &str, delay_seconds: u32) -> Result<PathBuf, String> {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let filename = format!("attool-capture-{ts}.png");
    // Save inside $HOME/.cache/... so Tauri's asset:// protocol (scoped to $HOME/**) can read the preview.
    let cache_root = dirs::cache_dir()
        .or_else(dirs::home_dir)
        .ok_or_else(|| "无法确定缓存目录".to_string())?;
    let dir = cache_root.join("attool").join("captures");
    fs::create_dir_all(&dir).map_err(|error| format!("创建截图缓存目录失败：{error}"))?;
    let output_path = dir.join(&filename);

    #[cfg(target_os = "macos")]
    {
        let mut cmd = Command::new("screencapture");
        cmd.arg("-x"); // silent
        if delay_seconds > 0 {
            cmd.arg("-T").arg(delay_seconds.to_string());
        }
        match mode {
            "region" => {
                cmd.arg("-i");
            }
            "window" => {
                cmd.arg("-iW");
            }
            "full" => {}
            other => return Err(format!("不支持的截图模式：{other}")),
        }
        cmd.arg(&output_path);

        let status = cmd
            .status()
            .map_err(|error| format!("启动 screencapture 失败：{error}"))?;
        if !status.success() {
            return Err("截图未完成（可能被取消）".to_string());
        }
        if !output_path.is_file() {
            return Err("截图未生成文件（可能被取消）".to_string());
        }
        return Ok(output_path);
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (mode, delay_seconds, output_path);
        Err("目前仅 macOS 支持系统截图（Windows / Linux 待接入）".to_string())
    }
}
