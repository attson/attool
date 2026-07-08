use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub const DEFAULT_CAPTURE_SHORTCUT: &str = "CommandOrControl+Shift+A";

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
            std::thread::spawn(move || {
                if let Err(err) = open_capture_overlay(&inner) {
                    let _ = inner.emit("capture-failed", err);
                }
            });
        })
        .map_err(|error| format!("注册截图快捷键失败：{error}"))
}

// ---------- overlay ----------

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OverlayInitPayload {
    image_path: String,
    screen_width: f64,
    screen_height: f64,
    scale_factor: f64,
    windows: Vec<super::windows::WindowRect>,
}

/// Take a silent full-desktop screenshot and hand it to the transparent overlay window,
/// which then handles region select + on-canvas annotation.
pub fn open_capture_overlay(app: &AppHandle) -> Result<(), String> {
    let overlay = app
        .get_webview_window("capture-overlay")
        .ok_or_else(|| "找不到截图覆盖窗".to_string())?;

    // Hide overlay if it's somehow visible, so it isn't captured. Also give the desktop a moment to repaint.
    let _ = overlay.hide();
    std::thread::sleep(std::time::Duration::from_millis(120));

    // Snap the whole desktop silently, no cursor
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let cache_root = dirs::cache_dir()
        .or_else(dirs::home_dir)
        .ok_or_else(|| "无法确定缓存目录".to_string())?;
    let dir = cache_root.join("attool").join("captures");
    fs::create_dir_all(&dir).map_err(|error| format!("创建缓存目录失败：{error}"))?;
    let image_path = dir.join(format!("attool-desktop-{ts}.png"));

    #[cfg(target_os = "macos")]
    {
        let status = Command::new("screencapture")
            .arg("-x") // silent
            .arg("-C") // no cursor
            .arg("-t")
            .arg("png")
            .arg(&image_path)
            .status()
            .map_err(|error| format!("启动 screencapture 失败：{error}"))?;
        if !status.success() || !image_path.is_file() {
            return Err("桌面截图失败".to_string());
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        return Err("目前仅 macOS 支持浮层截图（Windows / Linux 待接入）".to_string());
    }

    // Position overlay to cover primary monitor
    let monitor = app
        .primary_monitor()
        .map_err(|error| format!("获取主显示器失败：{error}"))?
        .ok_or_else(|| "未找到主显示器".to_string())?;
    let scale = monitor.scale_factor();
    let physical_size = monitor.size();
    let physical_pos = monitor.position();
    let logical_width = physical_size.width as f64 / scale;
    let logical_height = physical_size.height as f64 / scale;
    let logical_x = physical_pos.x as f64 / scale;
    let logical_y = physical_pos.y as f64 / scale;

    overlay
        .set_position(LogicalPosition::new(logical_x, logical_y))
        .map_err(|error| format!("移动覆盖窗失败：{error}"))?;
    overlay
        .set_size(LogicalSize::new(logical_width, logical_height))
        .map_err(|error| format!("调整覆盖窗尺寸失败：{error}"))?;

    let payload = OverlayInitPayload {
        image_path: image_path.to_string_lossy().into_owned(),
        screen_width: logical_width,
        screen_height: logical_height,
        scale_factor: scale,
        windows: super::windows::list_visible_windows(scale),
    };
    let _ = overlay.emit("capture-overlay-init", payload.clone());

    overlay
        .show()
        .map_err(|error| format!("显示覆盖窗失败：{error}"))?;
    let _ = overlay.set_focus();
    // Re-emit shortly after show so the webview has time to attach the listener on first launch
    let overlay_clone = overlay.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(180));
        let _ = overlay_clone.emit("capture-overlay-init", payload);
    });
    Ok(())
}

pub fn close_capture_overlay(app: &AppHandle) -> Result<(), String> {
    if let Some(overlay) = app.get_webview_window("capture-overlay") {
        overlay
            .hide()
            .map_err(|error| format!("隐藏覆盖窗失败：{error}"))?;
    }
    Ok(())
}

/// Persist the composited (screenshot + annotations) PNG to the captures dir AND write it to
/// the system clipboard as an image. Returns the saved file path.
///
/// Doing the clipboard write server-side avoids the flaky JS `writeImage(Uint8Array)` path in
/// tauri-plugin-clipboard-manager v2 (Image.fromBytes → RGBA on some macOS builds silently no-ops).
pub fn commit_capture_overlay(app: &AppHandle, png_base64: &str) -> Result<PathBuf, String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(png_base64.trim())
        .map_err(|error| format!("Base64 解码失败：{error}"))?;

    let cache_root = dirs::cache_dir()
        .or_else(dirs::home_dir)
        .ok_or_else(|| "无法确定缓存目录".to_string())?;
    let dir = cache_root.join("attool").join("captures");
    fs::create_dir_all(&dir).map_err(|error| format!("创建缓存目录失败：{error}"))?;
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let path = dir.join(format!("attool-capture-{ts}.png"));
    fs::write(&path, &bytes).map_err(|error| format!("写文件失败：{error}"))?;

    // Decode PNG → RGBA and write as image to the clipboard.
    let dyn_img = image::load_from_memory(&bytes).map_err(|error| format!("PNG 解码失败：{error}"))?;
    let rgba = dyn_img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let raw = rgba.into_raw();
    let image = tauri::image::Image::new_owned(raw, w, h);
    if let Err(error) = app.clipboard().write_image(&image) {
        // Non-fatal: file was saved, just log
        eprintln!("[capture] clipboard write_image failed: {error}");
    }

    if let Some(overlay) = app.get_webview_window("capture-overlay") {
        let _ = overlay.hide();
    }

    Ok(path)
}

/// Create a floating always-on-top window that shows the composited PNG as a "pin".
/// Multiple pins can exist simultaneously; each gets a unique label based on ts.
pub fn pin_capture_overlay(app: &AppHandle, png_base64: &str) -> Result<PathBuf, String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(png_base64.trim())
        .map_err(|error| format!("Base64 解码失败：{error}"))?;

    // Save file for the pin window to load
    let cache_root = dirs::cache_dir()
        .or_else(dirs::home_dir)
        .ok_or_else(|| "无法确定缓存目录".to_string())?;
    let dir = cache_root.join("attool").join("pins");
    fs::create_dir_all(&dir).map_err(|error| format!("创建 pin 缓存目录失败：{error}"))?;
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let path = dir.join(format!("attool-pin-{ts}.png"));
    fs::write(&path, &bytes).map_err(|error| format!("写 pin 文件失败：{error}"))?;

    // Read PNG dimensions to size the window (assume PNG bytes contain valid dimensions in bytes 16..24)
    let dyn_img = image::load_from_memory(&bytes).map_err(|error| format!("PNG 解码失败：{error}"))?;
    let img_w = dyn_img.width() as f64;
    let img_h = dyn_img.height() as f64;

    let monitor = app
        .primary_monitor()
        .map_err(|error| format!("获取主显示器失败：{error}"))?
        .ok_or_else(|| "未找到主显示器".to_string())?;
    let scale = monitor.scale_factor();
    let logical_w = img_w / scale;
    let logical_h = img_h / scale;

    // Cap the initial size to ~80% of screen so huge captures still fit
    let screen_size = monitor.size();
    let max_w = (screen_size.width as f64 * 0.8) / scale;
    let max_h = (screen_size.height as f64 * 0.8) / scale;
    let fit_scale = (max_w / logical_w).min(max_h / logical_h).min(1.0);
    let win_w = logical_w * fit_scale;
    let win_h = logical_h * fit_scale;

    // Center on primary monitor
    let screen_pos = monitor.position();
    let cx = screen_pos.x as f64 / scale + (screen_size.width as f64 / scale - win_w) / 2.0;
    let cy = screen_pos.y as f64 / scale + (screen_size.height as f64 / scale - win_h) / 2.0;

    // URL query so App.vue routes to the pin component and reads the image path
    let encoded_path = percent_encoding::utf8_percent_encode(
        &path.to_string_lossy(),
        percent_encoding::NON_ALPHANUMERIC,
    )
    .to_string();
    let url = format!("index.html?pin={encoded_path}");

    let label = format!("capture-pin-{ts}");
    let win = WebviewWindowBuilder::new(app, &label, WebviewUrl::App(url.into()))
        .title("AT Tool Pin")
        .inner_size(win_w, win_h)
        .position(cx, cy)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .resizable(true)
        .shadow(true)
        .skip_taskbar(true)
        .visible(false)
        .build()
        .map_err(|error| format!("创建 pin 窗失败：{error}"))?;
    let _ = win.show();
    let _ = win.set_focus();

    if let Some(overlay) = app.get_webview_window("capture-overlay") {
        let _ = overlay.hide();
    }
    Ok(path)
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
