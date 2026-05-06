use std::path::PathBuf;

use tauri::{AppHandle, Manager, State};

use super::{
    batch::{batch_from_folder, read_batch_table},
    models::{BatchDataPreview, TemplateProject, TemplateSummary},
    psd_bridge::import_psd_with_bridge,
    storage::EcommerceStore,
};

#[tauri::command]
pub async fn import_psd_template(
    psd_path: String,
    app: AppHandle,
    store: State<'_, EcommerceStore>,
) -> Result<TemplateProject, String> {
    let bridge = app
        .path()
        .resource_dir()
        .ok()
        .map(|dir| dir.join("python/psd_template_bridge.py"))
        .filter(|path| path.exists())
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("python/psd_template_bridge.py"));
    let project = import_psd_with_bridge(&bridge, PathBuf::from(psd_path).as_path(), store.root_dir())?;
    store.save_template(project)
}

#[tauri::command]
pub async fn list_ecommerce_templates(
    store: State<'_, EcommerceStore>,
) -> Result<Vec<TemplateSummary>, String> {
    store.list_templates()
}

#[tauri::command]
pub async fn load_ecommerce_template(
    id: String,
    store: State<'_, EcommerceStore>,
) -> Result<TemplateProject, String> {
    store.load_template(&id)
}

#[tauri::command]
pub async fn save_ecommerce_template(
    project: TemplateProject,
    store: State<'_, EcommerceStore>,
) -> Result<TemplateProject, String> {
    store.save_template(project)
}


#[tauri::command]
pub async fn import_batch_table(
    path: String,
    required_fields: Vec<String>,
) -> Result<BatchDataPreview, String> {
    read_batch_table(PathBuf::from(path).as_path(), &required_fields)
}

#[tauri::command]
pub async fn create_batch_from_folder(
    folder_path: String,
    image_binding_key: String,
) -> Result<BatchDataPreview, String> {
    batch_from_folder(PathBuf::from(folder_path).as_path(), &image_binding_key)
}
