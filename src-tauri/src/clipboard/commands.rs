use tauri::{image::Image, AppHandle, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

use super::{
    models::{ClipboardHistoryItem, ClipboardHistorySettings, ClipboardItemKind},
    storage::ClipboardStore,
    watcher::ClipboardWatcherState,
};

#[tauri::command]
pub async fn list_clipboard_items(
    kind: Option<ClipboardItemKind>,
    query: Option<String>,
    store: State<'_, ClipboardStore>,
) -> Result<Vec<ClipboardHistoryItem>, String> {
    store.list_items(kind, query.as_deref())
}

#[tauri::command]
pub async fn delete_clipboard_item(
    id: String,
    store: State<'_, ClipboardStore>,
) -> Result<(), String> {
    store.delete_item(&id)
}

#[tauri::command]
pub async fn set_clipboard_item_pinned(
    id: String,
    is_pinned: bool,
    store: State<'_, ClipboardStore>,
) -> Result<(), String> {
    store.set_pinned(&id, is_pinned)
}

#[tauri::command]
pub async fn clear_clipboard_history(store: State<'_, ClipboardStore>) -> Result<(), String> {
    store.clear_unpinned()
}

#[tauri::command]
pub async fn get_clipboard_settings(
    store: State<'_, ClipboardStore>,
) -> Result<ClipboardHistorySettings, String> {
    store.load_settings()
}

#[tauri::command]
pub async fn save_clipboard_settings(
    app: AppHandle,
    settings: ClipboardHistorySettings,
    watcher: State<'_, ClipboardWatcherState>,
    store: State<'_, ClipboardStore>,
) -> Result<ClipboardHistorySettings, String> {
    // 若快捷键有变化,先尝试重新注册(失败则整体报错,不落库,让前端提示用户换一个)
    let previous = store.load_settings().map(|s| s.shortcut).unwrap_or_default();
    if settings.shortcut.trim() != previous.trim() {
        super::watcher::reregister_clipboard_shortcut(&app, &settings.shortcut)?;
    }
    store.save_settings(&settings)?;
    watcher.set_enabled(settings.capture_enabled);
    Ok(settings)
}


#[tauri::command]
pub async fn restore_clipboard_item(
    id: String,
    app: AppHandle,
    store: State<'_, ClipboardStore>,
) -> Result<(), String> {
    let item = store
        .list_items(None, None)?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| "剪贴板历史不存在".to_string())?;

    match item.kind {
        ClipboardItemKind::Text => app.clipboard().write_text(item.content_text.clone()),
        ClipboardItemKind::Files => app.clipboard().write_text(item.file_paths.join("\n")),
        ClipboardItemKind::Image => {
            let path = item.asset_path.ok_or_else(|| "剪贴板图片资源不存在".to_string())?;
            let decoded = image::open(path).map_err(|error| format!("读取剪贴板图片失败：{error}"))?.to_rgba8();
            let (width, height) = decoded.dimensions();
            let image = Image::new_owned(decoded.into_raw(), width, height);
            app.clipboard().write_image(&image)
        }
    }
    .map_err(|error| format!("写入剪贴板失败：{error}"))?;

    store.touch_copied(&id)
}
