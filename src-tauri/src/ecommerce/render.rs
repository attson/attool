use std::path::{Path, PathBuf};

use ab_glyph::{FontArc, PxScale};
use image::{imageops, DynamicImage, Rgba, RgbaImage};
use imageproc::drawing::{draw_filled_ellipse_mut, draw_filled_rect_mut, draw_hollow_ellipse_mut, draw_hollow_rect_mut, draw_line_segment_mut, draw_text_mut};
use imageproc::rect::Rect;

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
        match render_row(store, &template, row, &output_dir) {
            Ok(path) => outputs.push(path.to_string_lossy().into_owned()),
            Err(message) => failed.push(ExportFailure { row_index: row.index, field: None, message }),
        }
    }

    Ok(ExportResult { total: request.rows.len(), succeeded: outputs.len(), outputs, failed })
}

fn render_row(store: &EcommerceStore, template: &TemplateProject, row: &BatchRow, output_dir: &Path) -> Result<PathBuf, String> {
    let mut canvas = RgbaImage::from_pixel(template.canvas_width, template.canvas_height, Rgba([255, 255, 255, 255]));
    for layer in flatten_layers(&template.layers) {
        if !layer.visible || matches!(&layer.r#type, TemplateLayerType::Group) {
            continue;
        }
        match &layer.r#type {
            TemplateLayerType::Image => draw_image_layer(store, &mut canvas, layer, row)?,
            TemplateLayerType::Shape => draw_shape_layer(&mut canvas, layer),
            TemplateLayerType::Text => draw_text_layer(&mut canvas, layer, row),
            TemplateLayerType::Group => {}
        }
    }
    let output_path = output_dir.join(make_export_file_name(&row.values, row.index));
    DynamicImage::ImageRgba8(canvas).save(&output_path).map_err(|error| format!("保存 PNG 失败：{error}"))?;
    Ok(output_path)
}

fn draw_image_layer(store: &EcommerceStore, canvas: &mut RgbaImage, layer: &TemplateLayer, row: &BatchRow) -> Result<(), String> {
    let image_data = layer.image.as_ref().ok_or_else(|| format!("图片图层缺少素材：{}", layer.name))?;
    let bound_path = layer
        .binding_key
        .as_ref()
        .and_then(|key| row.values.get(key))
        .filter(|path| !path.trim().is_empty())
        .cloned();
    let image = if let Some(path) = bound_path {
        image::open(&path).map_err(|error| format!("读取图片失败 {path}：{error}"))?
    } else {
        let bytes = store
            .load_asset_bytes(&image_data.asset_id)
            .map_err(|error| format!("读取素材失败 {}: {error}", layer.name))?;
        image::load_from_memory(&bytes).map_err(|error| format!("解码素材失败 {}: {error}", layer.name))?
    };
    let target_w = layer.width.max(1.0) as u32;
    let target_h = layer.height.max(1.0) as u32;
    let (mut resized, offset_x, offset_y) = match image_data.fit {
        ImageFit::Stretch => (image.resize_exact(target_w, target_h, imageops::FilterType::Lanczos3).to_rgba8(), 0_i64, 0_i64),
        ImageFit::Contain => {
            let scale = (target_w as f32 / image.width() as f32).min(target_h as f32 / image.height() as f32);
            let w = (image.width() as f32 * scale).round().max(1.0) as u32;
            let h = (image.height() as f32 * scale).round().max(1.0) as u32;
            let offset_x = ((target_w - w) / 2) as i64;
            let offset_y = ((target_h - h) / 2) as i64;
            (image.resize_exact(w, h, imageops::FilterType::Lanczos3).to_rgba8(), offset_x, offset_y)
        }
        ImageFit::Cover => {
            let scale = (target_w as f32 / image.width() as f32).max(target_h as f32 / image.height() as f32);
            let w = (image.width() as f32 * scale).round().max(1.0) as u32;
            let h = (image.height() as f32 * scale).round().max(1.0) as u32;
            let resized = image.resize_exact(w, h, imageops::FilterType::Lanczos3).to_rgba8();
            let crop_x = ((w - target_w) / 2).min(w.saturating_sub(1));
            let crop_y = ((h - target_h) / 2).min(h.saturating_sub(1));
            (imageops::crop_imm(&resized, crop_x, crop_y, target_w.min(w), target_h.min(h)).to_image(), 0_i64, 0_i64)
        }
    };
    if layer.opacity < 1.0 {
        for pixel in resized.pixels_mut() {
            pixel.0[3] = ((pixel.0[3] as f32) * layer.opacity.clamp(0.0, 1.0)).round() as u8;
        }
    }
    imageops::overlay(canvas, &resized, layer.x.round() as i64 + offset_x, layer.y.round() as i64 + offset_y);
    Ok(())
}

fn draw_shape_layer(canvas: &mut RgbaImage, layer: &TemplateLayer) {
    let Some(shape) = &layer.shape else { return; };
    let fill = shape.fill.as_deref().map(|value| with_layer_alpha(parse_hex(value), layer.opacity));
    let stroke = shape.stroke.as_deref().map(|value| with_layer_alpha(parse_hex(value), layer.opacity));
    let stroke_width = shape.stroke_width.unwrap_or(0.0).max(0.0) as u32;
    let rect = layer_rect(layer);

    match shape.shape {
        ShapeKind::Rect | ShapeKind::RoundRect => {
            if let Some(fill) = fill {
                draw_filled_rect_mut(canvas, rect, fill);
            }
            if let Some(stroke) = stroke {
                for inset in 0..stroke_width {
                    let x = rect.left() + inset as i32;
                    let y = rect.top() + inset as i32;
                    let w = rect.width().saturating_sub(inset * 2);
                    let h = rect.height().saturating_sub(inset * 2);
                    if w > 0 && h > 0 {
                        draw_hollow_rect_mut(canvas, Rect::at(x, y).of_size(w, h), stroke);
                    }
                }
            }
        }
        ShapeKind::Line => {
            if let Some(color) = stroke.or(fill) {
                draw_line_segment_mut(canvas, (layer.x, layer.y), (layer.x + layer.width, layer.y + layer.height), color);
            }
        }
        ShapeKind::Ellipse => {
            let center = (layer.x + layer.width / 2.0, layer.y + layer.height / 2.0);
            let radius_x = (layer.width / 2.0).max(1.0) as i32;
            let radius_y = (layer.height / 2.0).max(1.0) as i32;
            if let Some(fill) = fill {
                draw_filled_ellipse_mut(canvas, (center.0.round() as i32, center.1.round() as i32), radius_x, radius_y, fill);
            }
            if let Some(stroke) = stroke {
                for inset in 0..stroke_width {
                    draw_hollow_ellipse_mut(canvas, (center.0.round() as i32, center.1.round() as i32), radius_x - inset as i32, radius_y - inset as i32, stroke);
                }
            }
        }
    }
}

fn layer_rect(layer: &TemplateLayer) -> Rect {
    Rect::at(layer.x.round() as i32, layer.y.round() as i32).of_size(layer.width.max(1.0) as u32, layer.height.max(1.0) as u32)
}

fn with_layer_alpha(mut color: Rgba<u8>, opacity: f32) -> Rgba<u8> {
    color.0[3] = ((color.0[3] as f32) * opacity.clamp(0.0, 1.0)).round() as u8;
    color
}

fn draw_text_background(canvas: &mut RgbaImage, layer: &TemplateLayer, text_data: &TextLayerData) {
    if let Some(background) = &text_data.background_color {
        draw_filled_rect_mut(canvas, layer_rect(layer), with_layer_alpha(parse_hex(background), layer.opacity));
    }
}

fn draw_text_decoration(canvas: &mut RgbaImage, layer: &TemplateLayer, text_data: &TextLayerData, color: Rgba<u8>) {
    let Some(decoration) = &text_data.text_decoration else { return; };
    if matches!(decoration, TextDecoration::None) { return; }
    let y = match decoration {
        TextDecoration::Underline => layer.y + text_data.font_size + 6.0,
        TextDecoration::LineThrough => layer.y + text_data.font_size * 0.55,
        TextDecoration::None => return,
    };
    draw_line_segment_mut(canvas, (layer.x, y), (layer.x + layer.width, y), color);
}

fn draw_text_layer(canvas: &mut RgbaImage, layer: &TemplateLayer, row: &BatchRow) {
    let Some(text_data) = &layer.text else {
        return;
    };
    let Some(font) = load_font(&text_data.font_family).or_else(|| load_font("PingFang SC")).or_else(default_font) else {
        return;
    };
    let text = layer
        .binding_key
        .as_ref()
        .and_then(|key| row.values.get(key))
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| text_data.text.clone());
    draw_text_background(canvas, layer, text_data);
    let fill = with_layer_alpha(parse_hex(&text_data.color), layer.opacity);
    let scale = PxScale::from(text_data.font_size.max(1.0));

    if let Some(shadow_color) = &text_data.shadow_color {
        let shadow = with_layer_alpha(parse_hex(shadow_color), layer.opacity);
        let mut shadow_layer = RgbaImage::from_pixel(canvas.width(), canvas.height(), Rgba([0, 0, 0, 0]));
        draw_text_mut(
            &mut shadow_layer,
            shadow,
            (layer.x + text_data.shadow_offset_x.unwrap_or(0.0)).round() as i32,
            (layer.y + text_data.shadow_offset_y.unwrap_or(0.0)).round() as i32,
            scale,
            &font,
            &text,
        );
        let blur = text_data.shadow_blur.unwrap_or(0.0).max(0.0);
        let shadow_layer = if blur > 0.0 { imageops::blur(&shadow_layer, blur) } else { shadow_layer };
        imageops::overlay(canvas, &shadow_layer, 0, 0);
    }

    if let (Some(stroke_color), Some(stroke_width)) = (&text_data.stroke_color, text_data.stroke_width) {
        let stroke = with_layer_alpha(parse_hex(stroke_color), layer.opacity);
        let width = stroke_width.max(0.0).round() as i32;
        for dy in -width..=width {
            for dx in -width..=width {
                if dx == 0 && dy == 0 { continue; }
                draw_text_mut(canvas, stroke, layer.x.round() as i32 + dx, layer.y.round() as i32 + dy, scale, &font, &text);
            }
        }
    }

    draw_text_mut(canvas, fill, layer.x.round() as i32, layer.y.round() as i32, scale, &font, &text);
    draw_text_decoration(canvas, layer, text_data, fill);
}

fn load_font(preferred_family: &str) -> Option<FontArc> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let query = fontdb::Query {
        families: &[fontdb::Family::Name(preferred_family)],
        ..fontdb::Query::default()
    };
    let id = db.query(&query)?;
    let face = db.face(id)?;
    let bytes = match &face.source {
        fontdb::Source::File(path) => std::fs::read(path).ok()?,
        fontdb::Source::Binary(data) => data.as_ref().as_ref().to_vec(),
        fontdb::Source::SharedFile(path, _) => std::fs::read(path).ok()?,
    };
    FontArc::try_from_vec(bytes).ok()
}

fn default_font() -> Option<FontArc> {
    for family in ["PingFang SC", "Heiti SC", "Arial Unicode MS", "Noto Sans CJK SC", "Microsoft YaHei"] {
        if let Some(font) = load_font(family) {
            return Some(font);
        }
    }
    None
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
