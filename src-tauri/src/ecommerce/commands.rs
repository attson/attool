use std::path::PathBuf;

use tauri::{AppHandle, Manager, State};

use super::{
    models::{TemplateProject, TemplateSummary},
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
