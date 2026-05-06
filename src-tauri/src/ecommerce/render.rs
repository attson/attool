use std::path::{Path, PathBuf};

use image::{imageops, DynamicImage, Rgba, RgbaImage};

use super::{models::*, storage::EcommerceStore};

pub fn export_images(store: &EcommerceStore, request: ExportRequest) -> Result<ExportResult, String> {
    let template = store.load_template(&request.template_id)?;
    let output_dir = PathBuf::from(request.output_dir.trim());
    if output_dir.as_os_str().is_empty() {
        return Err("请选择输出目录".to_string());
    }
    std::fs::create_dir_all(&output_dir).map_err(|error| format!("创建输出目录失败：{error}"))?;

    let mut outputs = Vec::new();
    let mut failed = Vec::new();
    for row in &request.rows {
        match render_row(&template, row, &output_dir) {
            Ok(path) => outputs.push(path.to_string_lossy().into_owned()),
            Err(message) => failed.push(ExportFailure { row_index: row.index, field: None, message }),
        }
    }

    Ok(ExportResult { total: request.rows.len(), succeeded: outputs.len(), outputs, failed })
}

fn render_row(template: &TemplateProject, row: &BatchRow, output_dir: &Path) -> Result<PathBuf, String> {
    let mut canvas = RgbaImage::from_pixel(template.canvas_width, template.canvas_height, Rgba([255, 255, 255, 255]));
    for layer in flatten_layers(&template.layers) {
        if !layer.visible || matches!(&layer.r#type, TemplateLayerType::Group) {
            continue;
        }
        match &layer.r#type {
            TemplateLayerType::Image => draw_image_layer(&mut canvas, template, layer, row)?,
            TemplateLayerType::Shape => draw_shape_layer(&mut canvas, layer),
            TemplateLayerType::Text => draw_text_layer_noop(&mut canvas, layer),
            TemplateLayerType::Group => {}
        }
    }
    let output_path = output_dir.join(make_export_file_name(&row.values, row.index));
    DynamicImage::ImageRgba8(canvas).save(&output_path).map_err(|error| format!("保存 PNG 失败：{error}"))?;
    Ok(output_path)
}

fn draw_image_layer(canvas: &mut RgbaImage, template: &TemplateProject, layer: &TemplateLayer, row: &BatchRow) -> Result<(), String> {
    let image_data = layer.image.as_ref().ok_or_else(|| format!("图片图层缺少素材：{}", layer.name))?;
    let source_path = layer
        .binding_key
        .as_ref()
        .and_then(|key| row.values.get(key))
        .filter(|path| !path.trim().is_empty())
        .cloned()
        .or_else(|| template.assets.iter().find(|asset| asset.id == image_data.asset_id).map(|asset| asset.path.clone()))
        .ok_or_else(|| format!("图片图层没有可用路径：{}", layer.name))?;
    let image = image::open(&source_path).map_err(|error| format!("读取图片失败 {source_path}：{error}"))?;
    let resized = image.resize_exact(layer.width.max(1.0) as u32, layer.height.max(1.0) as u32, imageops::FilterType::Lanczos3).to_rgba8();
    imageops::overlay(canvas, &resized, layer.x.round() as i64, layer.y.round() as i64);
    Ok(())
}

fn draw_shape_layer(canvas: &mut RgbaImage, layer: &TemplateLayer) {
    let color = parse_hex(layer.shape.as_ref().and_then(|shape| shape.fill.as_deref()).unwrap_or("#000000"));
    let min_x = layer.x.max(0.0) as u32;
    let min_y = layer.y.max(0.0) as u32;
    let max_x = (layer.x + layer.width).max(0.0) as u32;
    let max_y = (layer.y + layer.height).max(0.0) as u32;
    for y in min_y..max_y.min(canvas.height()) {
        for x in min_x..max_x.min(canvas.width()) {
            canvas.put_pixel(x, y, color);
        }
    }
}

fn draw_text_layer_noop(_canvas: &mut RgbaImage, _layer: &TemplateLayer) {
    // The next task replaces this no-op with system font rendering.
}

fn flatten_layers(layers: &[TemplateLayer]) -> Vec<&TemplateLayer> {
    let mut result = Vec::new();
    for layer in layers {
        result.push(layer);
        if let Some(children) = &layer.children {
            result.extend(flatten_layers(children));
        }
    }
    result
}

fn parse_hex(value: &str) -> Rgba<u8> {
    let hex = value.trim_start_matches('#');
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
        return Rgba([r, g, b, 255]);
    }
    Rgba([0, 0, 0, 255])
}

fn make_export_file_name(values: &std::collections::HashMap<String, String>, row_index: usize) -> String {
    let raw = values.get("name").or_else(|| values.get("title")).cloned().unwrap_or_else(|| format!("{:03}", row_index + 1));
    let safe: String = raw
        .chars()
        .map(|ch| if matches!(ch, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || ch.is_whitespace() { '_' } else { ch })
        .collect();
    format!("{}.png", if safe.trim().is_empty() { format!("{:03}", row_index + 1) } else { safe })
}
