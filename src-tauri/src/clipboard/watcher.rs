use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use super::storage::ClipboardStore;

const CLIPBOARD_EVENT: &str = "clipboard-history-updated";
const DEFAULT_SHORTCUT: &str = "CommandOrControl+Shift+V";

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

pub fn start_clipboard_watcher(app: AppHandle, store: ClipboardStore, state: ClipboardWatcherState) {
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
                            let _ = app.emit(CLIPBOARD_EVENT, ());
                            let limit = store
                                .load_settings()
                                .map(|settings| settings.retention_limit)
                                .unwrap_or(500);
                            let _ = store.enforce_retention(limit);
                        }
                    }
                }
            }
            thread::sleep(Duration::from_millis(800));
        }
    });
}

pub fn register_clipboard_shortcut(app: &AppHandle) -> Result<(), String> {
    let shortcut: Shortcut = DEFAULT_SHORTCUT
        .parse()
        .map_err(|error| format!("解析剪贴板快捷键失败：{error}"))?;
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
