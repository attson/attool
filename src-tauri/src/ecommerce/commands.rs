use std::path::PathBuf;

use tauri::{AppHandle, Manager, State};

use super::{
    batch::{batch_from_folder, read_batch_table},
    render::{batch_replace_layer_image, export_images},
    models::{BatchDataPreview, ExportRequest, ExportResult, TemplateAsset, TemplateProject, TemplateSummary},
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
pub async fn delete_ecommerce_template(
    id: String,
    store: State<'_, EcommerceStore>,
) -> Result<(), String> {
    store.delete_template(&id)
}

#[tauri::command]
pub async fn rename_ecommerce_template(
    id: String,
    name: String,
    store: State<'_, EcommerceStore>,
) -> Result<TemplateProject, String> {
    store.rename_template(&id, &name)
}

#[tauri::command]
pub async fn save_template_asset(
    name: String,
    mime_type: String,
    bytes: Vec<u8>,
    store: State<'_, EcommerceStore>,
) -> Result<TemplateAsset, String> {
    store.save_asset(name, mime_type, bytes)
}

#[tauri::command]
pub async fn import_template_asset_from_path(
    source_path: String,
    store: State<'_, EcommerceStore>,
) -> Result<TemplateAsset, String> {
    let source = PathBuf::from(&source_path);
    let bytes = std::fs::read(&source)
        .map_err(|error| format!("读取图片失败：{error}"))?;
    let name = source
        .file_name()
        .map(|os| os.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image".to_string());
    store.save_asset(name, String::new(), bytes)
}

#[tauri::command]
pub async fn list_template_assets(
    store: State<'_, EcommerceStore>,
) -> Result<Vec<TemplateAsset>, String> {
    store.list_assets()
}

#[tauri::command]
pub async fn delete_template_asset(
    asset_id: String,
    store: State<'_, EcommerceStore>,
) -> Result<(), String> {
    store.delete_asset(&asset_id)
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


#[tauri::command]
pub async fn export_ecommerce_images(
    request: ExportRequest,
    store: State<'_, EcommerceStore>,
) -> Result<ExportResult, String> {
    export_images(&store, request)
}

#[tauri::command]
pub async fn batch_replace_image_layer(
    template_id: String,
    layer_id: String,
    source_paths: Vec<String>,
    output_dir: String,
    store: State<'_, EcommerceStore>,
) -> Result<ExportResult, String> {
    batch_replace_layer_image(&store, &template_id, &layer_id, &source_paths, &output_dir)
}
