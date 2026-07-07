use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub const CAPTURE_SHORTCUT: &str = if cfg!(target_os = "macos") {
    "cmd+shift+a"
} else {
    "ctrl+shift+a"
};
pub const CAPTURE_EVENT: &str = "capture-completed";

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CaptureEventPayload {
    output_path: String,
}

pub fn register_capture_shortcut(app: &AppHandle) -> Result<(), String> {
    let shortcut: Shortcut = CAPTURE_SHORTCUT
        .parse()
        .map_err(|error| format!("解析截图快捷键失败：{error}"))?;
    let handle = app.clone();
    app.global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            let inner = handle.clone();
            // screencapture -i blocks until the user finishes selecting; run off-thread.
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
                    // 取消/超时/未找到都 emit 一个错误事件，前端决定要不要 toast
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
        // screencapture flags:
        //   -i interactive region select (default) — user can hit Space to switch to window mode
        //   -iW same but starts in window select mode
        //   -x silent (no shutter sound)
        //   -T <seconds> delay before capture
        //   (no flag = full desktop, non-interactive)
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
            "full" => {} // no flag = capture everything
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
