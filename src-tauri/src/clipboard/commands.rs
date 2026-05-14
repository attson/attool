use tauri::{AppHandle, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

use super::{
    models::{ClipboardHistoryItem, ClipboardHistorySettings, ClipboardItemKind},
    storage::ClipboardStore,
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
pub async fn get_clipboard_settings() -> Result<ClipboardHistorySettings, String> {
    Ok(ClipboardHistorySettings::default())
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
            return Err("当前版本暂不支持从历史恢复图片到系统剪贴板".to_string());
        }
    }
    .map_err(|error| format!("写入剪贴板失败：{error}"))?;

    store.touch_copied(&id)
}
