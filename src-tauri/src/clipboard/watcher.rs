use std::{
    io::Cursor,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    thread,
    time::Duration,
};

use image::{ImageBuffer, ImageFormat, Rgba};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use super::{models::DEFAULT_CLIPBOARD_SHORTCUT, storage::ClipboardStore};

const CLIPBOARD_EVENT: &str = "clipboard-history-updated";

/// 当前已注册的剪贴板快捷键字符串(Tauri 格式)。用于修改时先注销旧的。
static REGISTERED: OnceLock<Mutex<String>> = OnceLock::new();

fn registered_slot() -> &'static Mutex<String> {
    REGISTERED.get_or_init(|| Mutex::new(String::new()))
}

fn current_clipboard_shortcut() -> String {
    registered_slot().lock().map(|g| g.clone()).unwrap_or_default()
}

#[derive(Clone, Debug)]
pub struct ClipboardWatcherState {
    capture_enabled: Arc<AtomicBool>,
}

impl ClipboardWatcherState {
    pub fn new() -> Self {
        Self {
            capture_enabled: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.capture_enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.capture_enabled.store(enabled, Ordering::Relaxed);
    }
}

pub fn start_clipboard_watcher(
    app: AppHandle,
    store: ClipboardStore,
    state: ClipboardWatcherState,
) {
    thread::spawn(move || {
        let mut last_text = String::new();
        loop {
            if state.is_enabled() {
                if let Ok(text) = app.clipboard().read_text() {
                    if text != last_text {
                        last_text = text.clone();
                        let result = if looks_like_file_paths(&text) {
                            store.insert_files(parse_file_paths(&text))
                        } else {
                            store.insert_text(&text)
                        };
                        if matches!(result, Ok(Some(_))) {
                            emit_update_and_enforce_retention(&app, &store);
                        }
                    }
                }

                if let Ok(image) = app.clipboard().read_image() {
                    if let Ok(bytes) = encode_png(image.rgba(), image.width(), image.height()) {
                        if matches!(
                            store.insert_image_bytes(
                                "image/png",
                                &bytes,
                                image.width(),
                                image.height()
                            ),
                            Ok(Some(_))
                        ) {
                            emit_update_and_enforce_retention(&app, &store);
                        }
                    }
                }
            }
            thread::sleep(Duration::from_millis(800));
        }
    });
}

/// 按给定快捷键字符串注册剪贴板面板快捷键。空串回退到默认值。
pub fn register_clipboard_shortcut(app: &AppHandle, shortcut_str: &str) -> Result<(), String> {
    let shortcut_str = if shortcut_str.trim().is_empty() {
        DEFAULT_CLIPBOARD_SHORTCUT
    } else {
        shortcut_str.trim()
    };
    let shortcut: Shortcut = shortcut_str
        .parse()
        .map_err(|error| format!("解析剪贴板快捷键失败：{error}"))?;
    install_clipboard_handler(app, shortcut)?;
    if let Ok(mut g) = registered_slot().lock() {
        *g = shortcut_str.to_string();
    }
    Ok(())
}

/// 把剪贴板快捷键改为 next:先注销旧的,注册新的,失败则回滚到旧的。
pub fn reregister_clipboard_shortcut(app: &AppHandle, next: &str) -> Result<(), String> {
    let next = next.trim();
    if next.is_empty() {
        return Err("请填入有效的快捷键".to_string());
    }
    let parsed: Shortcut = next
        .parse()
        .map_err(|error| format!("快捷键格式非法：{error}"))?;

    let previous = current_clipboard_shortcut();
    if !previous.is_empty() {
        if let Ok(prev_parsed) = previous.parse::<Shortcut>() {
            let _ = app.global_shortcut().unregister(prev_parsed);
        }
    }

    if let Err(error) = install_clipboard_handler(app, parsed) {
        // 回滚:尽力恢复旧绑定,避免用户失去可用快捷键
        if !previous.is_empty() {
            if let Ok(prev_parsed) = previous.parse::<Shortcut>() {
                let _ = install_clipboard_handler(app, prev_parsed);
            }
        }
        return Err(error);
    }

    if let Ok(mut g) = registered_slot().lock() {
        *g = next.to_string();
    }
    Ok(())
}

fn install_clipboard_handler(app: &AppHandle, shortcut: Shortcut) -> Result<(), String> {
    let handle = app.clone();
    app.global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                if let Some(window) = handle.get_webview_window("clipboard-history") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = window.center();
                    let _ = handle.emit("clipboard-history-opened", ());
                }
            }
        })
        .map_err(|error| format!("注册剪贴板快捷键失败：{error}"))
}

fn emit_update_and_enforce_retention(app: &AppHandle, store: &ClipboardStore) {
    let _ = app.emit(CLIPBOARD_EVENT, ());
    let limit = store
        .load_settings()
        .map(|settings| settings.retention_limit)
        .unwrap_or(500);
    let _ = store.enforce_retention(limit);
}

fn encode_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    let image = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, rgba.to_vec())
        .ok_or_else(|| "剪贴板图片数据无效".to_string())?;
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut bytes, ImageFormat::Png)
        .map_err(|error| format!("编码剪贴板图片失败：{error}"))?;
    Ok(bytes.into_inner())
}

fn looks_like_file_paths(value: &str) -> bool {
    let paths = parse_file_paths(value);
    !paths.is_empty() && paths.iter().all(|path| Path::new(path).is_absolute())
}

fn parse_file_paths(value: &str) -> Vec<String> {
    value
        .lines()
        .map(|line| line.trim().trim_start_matches("file://").to_string())
        .filter(|line| !line.is_empty())
        .collect()
}
