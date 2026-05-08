use std::path::PathBuf;

use tauri::{AppHandle, Manager, State};

use super::{
    batch::{batch_from_folder, read_batch_table},
    render::export_images,
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
pub async fn save_pasted_template_asset(
    project_id: String,
    name: String,
    mime_type: String,
    bytes: Vec<u8>,
    store: State<'_, EcommerceStore>,
) -> Result<TemplateAsset, String> {
    if bytes.is_empty() {
        return Err("剪贴板图片为空".to_string());
    }
    persist_template_asset(&store, &project_id, name, mime_type, bytes, "剪贴板图片")
}

#[tauri::command]
pub async fn import_template_asset_from_path(
    project_id: String,
    source_path: String,
    store: State<'_, EcommerceStore>,
) -> Result<TemplateAsset, String> {
    let source = PathBuf::from(&source_path);
    let bytes = std::fs::read(&source)
        .map_err(|error| format!("读取图片失败：{error}"))?;
    if bytes.is_empty() {
        return Err("图片为空".to_string());
    }
    let name = source
        .file_name()
        .map(|os| os.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image".to_string());
    persist_template_asset(&store, &project_id, name, String::new(), bytes, "图片")
}

fn persist_template_asset(
    store: &EcommerceStore,
    project_id: &str,
    name: String,
    mime_type: String,
    bytes: Vec<u8>,
    label: &str,
) -> Result<TemplateAsset, String> {
    let image = image::load_from_memory(&bytes)
        .map_err(|error| format!("{label}格式不支持：{error}"))?;
    let asset_id = format!("asset-{}", uuid::Uuid::new_v4().simple());
    let extension = image_extension(&name, &mime_type);
    let asset_dir = store.template_dir(project_id).join("assets");
    std::fs::create_dir_all(&asset_dir)
        .map_err(|error| format!("创建素材目录失败：{error}"))?;
    let path = asset_dir.join(format!("{asset_id}.{extension}"));
    std::fs::write(&path, &bytes)
        .map_err(|error| format!("保存{label}失败：{error}"))?;

    Ok(TemplateAsset {
        id: asset_id,
        name,
        path: path.to_string_lossy().into_owned(),
        source_layer_id: None,
        mime_type: if mime_type.trim().is_empty() {
            format!("image/{extension}")
        } else {
            mime_type
        },
        width: image.width(),
        height: image.height(),
    })
}

fn image_extension(name: &str, mime_type: &str) -> &'static str {
    match mime_type {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ if name.to_ascii_lowercase().ends_with(".jpg") || name.to_ascii_lowercase().ends_with(".jpeg") => "jpg",
        _ if name.to_ascii_lowercase().ends_with(".webp") => "webp",
        _ if name.to_ascii_lowercase().ends_with(".gif") => "gif",
        _ => "png",
    }
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
