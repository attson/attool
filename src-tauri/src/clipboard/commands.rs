use tauri::State;

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
