use tauri::State;

use super::cancel::HttpCancelState;
use super::models::{
    HttpCollectionFolderRow, HttpCollectionRequestRow, HttpCollectionRow, HttpEnvRow,
    HttpEnvVarRow, HttpHistoryRow, HttpRequestSpec, HttpResponseInfo, HttpTabRow,
};
use super::send;
use super::storage::HttpStore;

#[tauri::command]
pub async fn send_http(
    request: HttpRequestSpec,
    cancel_token_id: Option<String>,
    cancel_state: State<'_, HttpCancelState>,
) -> Result<HttpResponseInfo, String> {
    send::send(request, cancel_token_id, Some(cancel_state.inner())).await
}

#[tauri::command]
pub fn cancel_http(cancel_token_id: String, cancel_state: State<'_, HttpCancelState>) -> bool {
    cancel_state.cancel(&cancel_token_id)
}

// ---- tabs ----

#[tauri::command]
pub fn list_http_tabs(store: State<'_, HttpStore>) -> Result<Vec<HttpTabRow>, String> {
    store.list_tabs()
}

#[tauri::command]
pub fn upsert_http_tab(row: HttpTabRow, store: State<'_, HttpStore>) -> Result<(), String> {
    store.upsert_tab(row)
}

#[tauri::command]
pub fn delete_http_tab(id: String, store: State<'_, HttpStore>) -> Result<(), String> {
    store.delete_tab(&id)
}

#[tauri::command]
pub fn set_active_http_tab(id: String, store: State<'_, HttpStore>) -> Result<(), String> {
    store.set_active_tab(&id)
}

// ---- history ----

#[tauri::command]
pub fn list_http_history(
    limit: Option<u32>,
    store: State<'_, HttpStore>,
) -> Result<Vec<HttpHistoryRow>, String> {
    store.list_history(limit.unwrap_or(500))
}

#[tauri::command]
pub fn insert_http_history(row: HttpHistoryRow, store: State<'_, HttpStore>) -> Result<(), String> {
    store.insert_history(row)
}

#[tauri::command]
pub fn delete_http_history(id: String, store: State<'_, HttpStore>) -> Result<(), String> {
    store.delete_history(&id)
}

#[tauri::command]
pub fn clear_http_history(store: State<'_, HttpStore>) -> Result<(), String> {
    store.clear_history()
}

// ---- envs ----

#[tauri::command]
pub fn list_http_envs(store: State<'_, HttpStore>) -> Result<Vec<HttpEnvRow>, String> {
    store.list_envs()
}

#[tauri::command]
pub fn upsert_http_env(row: HttpEnvRow, store: State<'_, HttpStore>) -> Result<(), String> {
    store.upsert_env(row)
}

#[tauri::command]
pub fn delete_http_env(id: String, store: State<'_, HttpStore>) -> Result<(), String> {
    store.delete_env(&id)
}

#[tauri::command]
pub fn set_active_http_env(id: String, store: State<'_, HttpStore>) -> Result<(), String> {
    store.set_active_env(&id)
}

// ---- env vars ----

#[tauri::command]
pub fn list_http_env_vars(
    env_id: String,
    store: State<'_, HttpStore>,
) -> Result<Vec<HttpEnvVarRow>, String> {
    store.list_env_vars(&env_id)
}

#[tauri::command]
pub fn upsert_http_env_var(
    row: HttpEnvVarRow,
    store: State<'_, HttpStore>,
) -> Result<(), String> {
    store.upsert_env_var(row)
}

#[tauri::command]
pub fn delete_http_env_var(id: String, store: State<'_, HttpStore>) -> Result<(), String> {
    store.delete_env_var(&id)
}

// ---- collections ----

#[tauri::command]
pub fn list_http_collections(store: State<'_, HttpStore>) -> Result<Vec<HttpCollectionRow>, String> {
    store.list_collections()
}

#[tauri::command]
pub fn list_http_collection_folders(
    store: State<'_, HttpStore>,
) -> Result<Vec<HttpCollectionFolderRow>, String> {
    store.list_collection_folders()
}

#[tauri::command]
pub fn list_http_collection_requests(
    store: State<'_, HttpStore>,
) -> Result<Vec<HttpCollectionRequestRow>, String> {
    store.list_collection_requests()
}

#[tauri::command]
pub fn upsert_http_collection(
    row: HttpCollectionRow,
    store: State<'_, HttpStore>,
) -> Result<(), String> {
    store.upsert_collection(row)
}

#[tauri::command]
pub fn upsert_http_collection_folder(
    row: HttpCollectionFolderRow,
    store: State<'_, HttpStore>,
) -> Result<(), String> {
    store.upsert_collection_folder(row)
}

#[tauri::command]
pub fn upsert_http_collection_request(
    row: HttpCollectionRequestRow,
    store: State<'_, HttpStore>,
) -> Result<(), String> {
    store.upsert_collection_request(row)
}

#[tauri::command]
pub fn delete_http_collection(id: String, store: State<'_, HttpStore>) -> Result<(), String> {
    store.delete_collection(&id)
}

#[tauri::command]
pub fn delete_http_collection_request(
    id: String,
    store: State<'_, HttpStore>,
) -> Result<(), String> {
    store.delete_collection_request(&id)
}
