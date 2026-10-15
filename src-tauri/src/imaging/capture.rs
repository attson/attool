use std::fs;
use std::path::{Path, PathBuf};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::process::Command;
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Monitor, WebviewUrl,
    WebviewWindowBuilder,
};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub const DEFAULT_CAPTURE_SHORTCUT: &str = "CommandOrControl+Shift+A";
const OVERLAY_DESKTOP_FRAME_EXTENSION: &str = "png";
const OVERLAY_UNMAP_SETTLE_TIME: Duration = Duration::from_millis(120);
const OVERLAY_READY_TIMEOUT: Duration = Duration::from_secs(1);
const WAYLAND_SHORTCUT_UNAVAILABLE: &str = "当前是 Wayland 会话，内置全局快捷键仅支持 X11";

fn select_primary_or_first<T>(primary: Option<T>, available: Vec<T>) -> Option<T> {
    primary.or_else(|| available.into_iter().next())
}

pub(crate) fn primary_or_first_monitor(app: &AppHandle) -> Result<Monitor, String> {
    // Some Linux backends enumerate monitors without designating a primary one.
    let primary = app.primary_monitor().ok().flatten();
    let available = if primary.is_none() {
        app.available_monitors()
            .map_err(|error| format!("枚举显示器失败：{error}"))?
    } else {
        Vec::new()
    };
    select_primary_or_first(primary, available).ok_or_else(|| "未找到可用显示器".to_string())
}

/// Currently-registered shortcut string (Tauri format like "CommandOrControl+Shift+A").
static REGISTERED: OnceLock<Mutex<String>> = OnceLock::new();
static OVERLAY_OPEN: OnceLock<Mutex<()>> = OnceLock::new();
static OVERLAY_HIDDEN_AT: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
static OVERLAY_READY: OnceLock<(Mutex<bool>, Condvar)> = OnceLock::new();

fn registered_slot() -> &'static Mutex<String> {
    REGISTERED.get_or_init(|| Mutex::new(String::new()))
}

fn overlay_open_slot() -> &'static Mutex<()> {
    OVERLAY_OPEN.get_or_init(|| Mutex::new(()))
}

fn overlay_hidden_at_slot() -> &'static Mutex<Option<Instant>> {
    OVERLAY_HIDDEN_AT.get_or_init(|| Mutex::new(None))
}

fn overlay_ready_slot() -> &'static (Mutex<bool>, Condvar) {
    OVERLAY_READY.get_or_init(|| (Mutex::new(false), Condvar::new()))
}

fn capture_mode_needs_desktop_frame(initial_mode: &str) -> bool {
    initial_mode != "recording"
}

fn overlay_settle_delay(last_hidden: Option<Instant>, now: Instant) -> Duration {
    let Some(hidden_at) = last_hidden else {
        return Duration::ZERO;
    };
    OVERLAY_UNMAP_SETTLE_TIME.saturating_sub(now.saturating_duration_since(hidden_at))
}

fn wait_for_overlay_unmap() {
    let last_hidden = overlay_hidden_at_slot().lock().ok().and_then(|slot| *slot);
    let delay = overlay_settle_delay(last_hidden, Instant::now());
    if !delay.is_zero() {
        std::thread::sleep(delay);
    }
}

fn mark_capture_overlay_hidden() {
    if let Ok(mut slot) = overlay_hidden_at_slot().lock() {
        *slot = Some(Instant::now());
    }
}

pub fn mark_capture_overlay_ready() {
    let (ready, signal) = overlay_ready_slot();
    if let Ok(mut ready) = ready.lock() {
        *ready = true;
        signal.notify_all();
    }
}

fn wait_for_capture_overlay_ready() -> Result<(), String> {
    let (ready, signal) = overlay_ready_slot();
    let ready = ready
        .lock()
        .map_err(|_| "截图浮层就绪状态异常".to_string())?;
    if *ready {
        return Ok(());
    }
    let (ready, _) = signal
        .wait_timeout_while(ready, OVERLAY_READY_TIMEOUT, |ready| !*ready)
        .map_err(|_| "截图浮层就绪状态异常".to_string())?;
    if *ready {
        Ok(())
    } else {
        Err("截图浮层初始化超时".to_string())
    }
}

fn capture_overlay_is_ready() -> bool {
    overlay_ready_slot()
        .0
        .lock()
        .map(|ready| *ready)
        .unwrap_or(false)
}

fn parse_cosmic_screenshot_path(stdout: &[u8]) -> Option<PathBuf> {
    String::from_utf8_lossy(stdout)
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(PathBuf::from)
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法获取应用数据目录：{error}"))?;
    fs::create_dir_all(&dir).map_err(|error| format!("创建应用数据目录失败：{error}"))?;
    Ok(dir.join("capture-config.json"))
}

pub fn configured_shortcut(app: &AppHandle) -> String {
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

fn is_wayland_session(session_type: Option<&str>, wayland_display: Option<&str>) -> bool {
    session_type.is_some_and(|value| value.eq_ignore_ascii_case("wayland"))
        || wayland_display.is_some_and(|value| !value.trim().is_empty())
}

pub fn global_shortcut_unavailable_reason() -> Option<&'static str> {
    #[cfg(target_os = "linux")]
    {
        let session_type = std::env::var("XDG_SESSION_TYPE").ok();
        let wayland_display = std::env::var("WAYLAND_DISPLAY").ok();
        if is_wayland_session(session_type.as_deref(), wayland_display.as_deref()) {
            return Some(WAYLAND_SHORTCUT_UNAVAILABLE);
        }
    }
    None
}

fn write_stored_shortcut(app: &AppHandle, shortcut: &str) -> Result<(), String> {
    let path = config_path(app)?;
    let body = serde_json::json!({ "shortcut": shortcut }).to_string();
    fs::write(&path, body).map_err(|error| format!("写入截图快捷键配置失败：{error}"))
}

pub fn current_shortcut() -> String {
    registered_slot()
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default()
}

pub fn register_capture_shortcut(app: &AppHandle) -> Result<(), String> {
    if let Some(reason) = global_shortcut_unavailable_reason() {
        return Err(reason.to_string());
    }
    let stored = configured_shortcut(app);
    register_shortcut_str(app, &stored)
}

pub fn reregister_capture_shortcut(app: &AppHandle, next: &str) -> Result<(), String> {
    if let Some(reason) = global_shortcut_unavailable_reason() {
        return Err(reason.to_string());
    }
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
            trigger_capture_overlay(&handle);
        })
        .map_err(|error| format!("注册截图快捷键失败：{error}"))
}

pub fn trigger_capture_overlay(app: &AppHandle) {
    let inner = app.clone();
    std::thread::spawn(move || {
        if let Err(err) = open_capture_overlay(&inner, "screenshot", None) {
            let _ = inner.emit("capture-failed", err);
        }
    });
}

pub fn trigger_recording_overlay(app: &AppHandle) {
    if let Some(overlay) = app.get_webview_window("capture-overlay") {
        if overlay.is_visible().unwrap_or(false) {
            let _ = overlay.emit("capture-overlay-recording-toggle", ());
            return;
        }
    }
    let inner = app.clone();
    std::thread::spawn(move || {
        if let Err(err) = open_capture_overlay(&inner, "recording", None) {
            let _ = inner.emit("capture-failed", err);
        }
    });
}

// ---------- overlay ----------

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OverlayInitPayload {
    request_id: String,
    image_path: String,
    screen_width: f64,
    screen_height: f64,
    usable_height: f64,
    covers_system_ui: bool,
    scale_factor: f64,
    windows: Vec<super::windows::WindowRect>,
    initial_mode: String,
    recording_options: OverlayRecordingOptions,
}

fn desktop_bottom_panel_inset(desktop: &str, anchor: &str, size: &str) -> f64 {
    if !desktop.to_ascii_lowercase().contains("cosmic") || !anchor.eq_ignore_ascii_case("bottom") {
        return 0.0;
    }
    match size.to_ascii_lowercase().as_str() {
        "s" => 40.0,
        "m" => 52.0,
        "l" => 64.0,
        _ => 52.0,
    }
}

#[derive(Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayRecordingOptions {
    pub resolution: String,
    pub frame_rate: u32,
    pub system_audio: bool,
    pub microphone: bool,
}

impl Default for OverlayRecordingOptions {
    fn default() -> Self {
        Self {
            resolution: "original".to_string(),
            frame_rate: 30,
            system_audio: true,
            microphone: false,
        }
    }
}

/// Take a silent full-desktop screenshot and hand it to the transparent overlay window,
/// which then handles region select + on-canvas annotation.
pub fn open_capture_overlay(
    app: &AppHandle,
    initial_mode: &str,
    recording_options: Option<OverlayRecordingOptions>,
) -> Result<(), String> {
    run_open_overlay(app, initial_mode, recording_options)
}

fn run_open_overlay(
    app: &AppHandle,
    initial_mode: &str,
    recording_options: Option<OverlayRecordingOptions>,
) -> Result<(), String> {
    let Ok(_open_guard) = overlay_open_slot().try_lock() else {
        return Ok(());
    };
    let overlay = app
        .get_webview_window("capture-overlay")
        .ok_or_else(|| "找不到截图覆盖窗".to_string())?;
    if overlay.is_visible().unwrap_or(false) {
        return Ok(());
    }

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let needs_desktop_frame = capture_mode_needs_desktop_frame(initial_mode);
    let image_path = if needs_desktop_frame {
        wait_for_overlay_unmap();
        let cache_root = dirs::cache_dir()
            .or_else(dirs::home_dir)
            .ok_or_else(|| "无法确定缓存目录".to_string())?;
        let dir = cache_root.join("attool").join("captures");
        fs::create_dir_all(&dir).map_err(|error| format!("创建缓存目录失败：{error}"))?;
        let path = dir.join(format!(
            "attool-desktop-{ts}.{OVERLAY_DESKTOP_FRAME_EXTENSION}"
        ));
        capture_fullscreen_silent(&path)?;
        if !path.is_file() {
            return Err("桌面截图失败：未生成截图文件".to_string());
        }
        path
    } else {
        PathBuf::new()
    };

    // Position overlay to cover the selected monitor
    let monitor = primary_or_first_monitor(app)?;
    let scale = monitor.scale_factor();
    let physical_size = monitor.size();
    let physical_pos = monitor.position();
    let logical_width = physical_size.width as f64 / scale;
    let logical_height = physical_size.height as f64 / scale;
    let logical_x = physical_pos.x as f64 / scale;
    let logical_y = physical_pos.y as f64 / scale;
    let work_area = monitor.work_area();
    let reported_work_bottom = ((work_area.position.y as f64 + work_area.size.height as f64
        - physical_pos.y as f64)
        / scale)
        .clamp(0.0, logical_height);
    let panel_inset = desktop_bottom_panel_inset(
        &std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
        &std::env::var("COSMIC_PANEL_ANCHOR").unwrap_or_default(),
        &std::env::var("COSMIC_PANEL_SIZE").unwrap_or_default(),
    );
    #[cfg(target_os = "linux")]
    let layer_shell_active = crate::clipboard::layer_shell::capture_overlay_is_layer_shell();
    #[cfg(not(target_os = "linux"))]
    let layer_shell_active = false;
    let usable_height = if layer_shell_active {
        logical_height
    } else {
        reported_work_bottom.min((logical_height - panel_inset).max(1.0))
    };
    if !layer_shell_active {
        overlay
            .set_position(LogicalPosition::new(logical_x, logical_y))
            .map_err(|error| format!("移动覆盖窗失败：{error}"))?;
    }
    overlay
        .set_size(LogicalSize::new(logical_width, logical_height))
        .map_err(|error| format!("调整覆盖窗尺寸失败：{error}"))?;

    let payload = OverlayInitPayload {
        request_id: ts.to_string(),
        image_path: image_path.to_string_lossy().into_owned(),
        screen_width: logical_width,
        screen_height: logical_height,
        usable_height,
        covers_system_ui: layer_shell_active,
        scale_factor: scale,
        windows: if needs_desktop_frame {
            super::windows::list_visible_windows(scale)
        } else {
            Vec::new()
        },
        initial_mode: if initial_mode == "recording" {
            "recording".to_string()
        } else {
            "screenshot".to_string()
        },
        recording_options: recording_options.unwrap_or_default(),
    };
    let was_ready = capture_overlay_is_ready();
    if !was_ready {
        overlay
            .show()
            .map_err(|error| format!("显示覆盖窗失败：{error}"))?;
        wait_for_capture_overlay_ready()?;
    }
    overlay
        .emit("capture-overlay-init", payload)
        .map_err(|error| format!("初始化覆盖窗失败：{error}"))?;
    if was_ready {
        overlay
            .show()
            .map_err(|error| format!("显示覆盖窗失败：{error}"))?;
    }
    let _ = overlay.set_focus();
    Ok(())
}

/// 把主屏静默截取为 PNG 存到 path。macOS 用系统自带的 screencapture(不含鼠标);
/// 其他平台用 xcap(X11/Windows 可用,Wayland 尽力而为)。
#[cfg(target_os = "macos")]
fn capture_fullscreen_silent(path: &std::path::Path) -> Result<(), String> {
    // 为什么不用 CGDisplayCreateImage:
    // AT Tool 采用 ad-hoc 签名(tauri.conf.json 的 signingIdentity: "-"),
    // 在 macOS Sonoma/Sequoia 上,TCC 对 ad-hoc 签名的 App 会“记不住”
    // 屏幕录制授权——每次重启都当作新进程再弹一次系统权限窗,即便用户
    // 已在“系统设置 → 隐私与安全性 → 屏幕录制”里勾选了 AT Tool。
    // /usr/sbin/screencapture 是 Apple 签名的系统二进制,子进程调用它
    // 可以拿到完整桌面而不会触发 TCC 反复弹窗。
    let status = Command::new("/usr/sbin/screencapture")
        .arg("-x") // 静默,不播放快门音
        .arg("-t")
        .arg("png")
        .arg(path)
        .status()
        .map_err(|error| format!("启动 screencapture 失败：{error}"))?;
    if !status.success() {
        return Err(format!(
            "桌面截图失败:screencapture 退出码 {}",
            status.code().unwrap_or(-1)
        ));
    }
    if !path.is_file() {
        return Err("桌面截图失败:未生成截图文件".to_string());
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn capture_fullscreen_silent(path: &std::path::Path) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    if std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .contains("cosmic")
    {
        match capture_fullscreen_with_cosmic(path) {
            Ok(()) => return Ok(()),
            Err(error) => eprintln!("COSMIC 截图快速路径不可用，回退到 xcap：{error}"),
        }
    }

    use xcap::Monitor;
    let monitors = Monitor::all().map_err(|error| format!("枚举显示器失败：{error}"))?;
    let monitor = monitors
        .iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .or_else(|| monitors.first())
        .ok_or_else(|| "未找到显示器".to_string())?;
    let image = monitor
        .capture_image()
        .map_err(|error| format!("截图失败（Wayland 会话可能受限，建议使用 X11 登录）：{error}"))?;
    // Compatibility fallback for desktops without a native fast path.
    image
        .save(path)
        .map_err(|error| format!("保存截图失败：{error}"))?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn capture_fullscreen_with_cosmic(path: &Path) -> Result<(), String> {
    let output_dir = path
        .parent()
        .ok_or_else(|| "截图缓存目录无效".to_string())?;
    let output = Command::new("cosmic-screenshot")
        .arg("--interactive=false")
        .arg("--modal=false")
        .arg("--notify=false")
        .arg("--save-dir")
        .arg(output_dir)
        .output()
        .map_err(|error| format!("启动 cosmic-screenshot 失败：{error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cosmic-screenshot 退出码 {}：{}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let generated = parse_cosmic_screenshot_path(&output.stdout)
        .ok_or_else(|| "cosmic-screenshot 未返回截图路径".to_string())?;
    if !generated.is_file() {
        return Err(format!(
            "cosmic-screenshot 未生成文件：{}",
            generated.display()
        ));
    }
    if generated != path {
        fs::rename(&generated, path)
            .or_else(|_| {
                fs::copy(&generated, path)?;
                fs::remove_file(&generated)
            })
            .map_err(|error| format!("保存 COSMIC 截图失败：{error}"))?;
    }
    Ok(())
}

pub fn close_capture_overlay(app: &AppHandle) -> Result<(), String> {
    if let Some(overlay) = app.get_webview_window("capture-overlay") {
        overlay
            .hide()
            .map_err(|error| format!("隐藏覆盖窗失败：{error}"))?;
        mark_capture_overlay_hidden();
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
        if overlay.hide().is_ok() {
            mark_capture_overlay_hidden();
        }
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

    let monitor = primary_or_first_monitor(app)?;
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
        if overlay.hide().is_ok() {
            mark_capture_overlay_hidden();
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_wayland_from_session_type() {
        assert!(is_wayland_session(Some("wayland"), None));
        assert!(is_wayland_session(Some("Wayland"), None));
    }

    #[test]
    fn detects_wayland_from_display_socket() {
        assert!(is_wayland_session(None, Some("wayland-1")));
        assert!(!is_wayland_session(None, Some("  ")));
    }

    #[test]
    fn leaves_x11_sessions_supported() {
        assert!(!is_wayland_session(Some("x11"), None));
        assert!(!is_wayland_session(None, None));
    }

    #[test]
    fn overlay_desktop_frame_uses_a_webview_supported_image_format() {
        assert!(matches!(OVERLAY_DESKTOP_FRAME_EXTENSION, "png" | "jpg" | "jpeg" | "webp"),
            "overlay background must use a WebView-supported image format"
        );
        assert_ne!(
            OVERLAY_DESKTOP_FRAME_EXTENSION, "bmp",
            "WKWebView does not reliably render BMP, which leaves the capture overlay blank"
        );
    }

    #[test]
    fn reserves_space_for_a_bottom_cosmic_panel() {
        assert_eq!(desktop_bottom_panel_inset("COSMIC", "Bottom", "L"), 64.0);
        assert_eq!(desktop_bottom_panel_inset("COSMIC", "Top", "L"), 0.0);
        assert_eq!(desktop_bottom_panel_inset("GNOME", "Bottom", "L"), 0.0);
    }

    #[test]
    fn selects_primary_monitor_when_available() {
        assert_eq!(select_primary_or_first(Some(2), vec![1, 3]), Some(2));
    }

    #[test]
    fn falls_back_to_first_monitor_when_primary_is_unavailable() {
        assert_eq!(select_primary_or_first(None, vec![1, 2]), Some(1));
        assert_eq!(select_primary_or_first::<u8>(None, vec![]), None);
    }

    #[test]
    fn recording_overlay_does_not_need_a_desktop_frame() {
        assert!(!capture_mode_needs_desktop_frame("recording"));
        assert!(capture_mode_needs_desktop_frame("screenshot"));
    }

    #[test]
    fn compositor_wait_only_covers_the_unsettled_remainder() {
        let now = std::time::Instant::now();
        assert_eq!(overlay_settle_delay(None, now), std::time::Duration::ZERO);
        assert_eq!(
            overlay_settle_delay(Some(now - std::time::Duration::from_millis(40)), now),
            std::time::Duration::from_millis(80)
        );
        assert_eq!(
            overlay_settle_delay(Some(now - std::time::Duration::from_millis(120)), now),
            std::time::Duration::ZERO
        );
    }

    #[test]
    fn parses_cosmic_screenshot_output_path() {
        assert_eq!(
            parse_cosmic_screenshot_path(b"/tmp/captures/Screenshot_2026-10-15.png\n"),
            Some(PathBuf::from("/tmp/captures/Screenshot_2026-10-15.png"))
        );
        assert_eq!(parse_cosmic_screenshot_path(b"\n"), None);
    }
}
