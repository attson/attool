use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateProject {
    pub id: String,
    pub name: String,
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub layers: Vec<TemplateLayer>,
    pub assets: Vec<TemplateAsset>,
    pub source_psd_path: Option<String>,
    pub preview_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateLayer {
    pub id: String,
    pub name: String,
    pub r#type: TemplateLayerType,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub visible: bool,
    pub opacity: f32,
    pub rotation: f32,
    pub binding_key: Option<String>,
    pub locked: Option<bool>,
    pub children: Option<Vec<TemplateLayer>>,
    pub text: Option<TextLayerData>,
    pub image: Option<ImageLayerData>,
    pub shape: Option<ShapeLayerData>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TemplateLayerType {
    Text,
    Image,
    Shape,
    Group,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextLayerData {
    pub text: String,
    pub font_family: String,
    pub font_size: f32,
    pub font_weight: serde_json::Value,
    pub color: String,
    pub stroke_color: Option<String>,
    pub stroke_width: Option<f32>,
    pub letter_spacing: Option<f32>,
    pub line_height: Option<f32>,
    pub align: Option<TextAlign>,
    pub font_style: Option<TextFontStyle>,
    pub text_decoration: Option<TextDecoration>,
    pub background_color: Option<String>,
    pub background_radius: Option<f32>,
    pub shadow_color: Option<String>,
    pub shadow_blur: Option<f32>,
    pub shadow_offset_x: Option<f32>,
    pub shadow_offset_y: Option<f32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TextFontStyle {
    Normal,
    Italic,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TextDecoration {
    None,
    Underline,
    LineThrough,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageLayerData {
    pub asset_id: String,
    pub fit: ImageFit,
    pub replaceable: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ImageFit {
    Cover,
    Contain,
    Stretch,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShapeLayerData {
    pub shape: ShapeKind,
    pub fill: Option<String>,
    pub stroke: Option<String>,
    pub stroke_width: Option<f32>,
    pub radius: Option<f32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ShapeKind {
    Rect,
    RoundRect,
    Ellipse,
    Line,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateAsset {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub data_url: String,
    pub source_layer_id: Option<String>,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateSummary {
    pub id: String,
    pub name: String,
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub preview_path: Option<String>,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchRow {
    pub id: String,
    pub index: usize,
    pub values: std::collections::HashMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchDataPreview {
    pub fields: Vec<String>,
    pub rows: Vec<BatchRow>,
    pub unused_fields: Vec<String>,
    pub missing_fields: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub template_id: String,
    pub output_dir: String,
    pub rows: Vec<BatchRow>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportFailure {
    pub row_index: usize,
    pub field: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub total: usize,
    pub succeeded: usize,
    pub outputs: Vec<String>,
    pub failed: Vec<ExportFailure>,
}
