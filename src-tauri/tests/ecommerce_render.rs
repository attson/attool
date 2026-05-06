use std::{collections::HashMap, fs};

use attool_lib::ecommerce::{
    models::{BatchRow, ExportRequest, ImageFit, ImageLayerData, TemplateAsset, TemplateLayer, TemplateLayerType, TemplateProject, TextAlign, TextLayerData},
    render::export_images,
    storage::EcommerceStore,
};

fn sample_project(asset_path: String) -> TemplateProject {
    TemplateProject {
        id: "tpl-render".to_string(),
        name: "render".to_string(),
        canvas_width: 120,
        canvas_height: 120,
        layers: vec![TemplateLayer {
            id: "image".to_string(),
            name: "{{product_image}} 商品图".to_string(),
            r#type: TemplateLayerType::Image,
            x: 0.0,
            y: 0.0,
            width: 120.0,
            height: 120.0,
            visible: true,
            opacity: 1.0,
            rotation: 0.0,
            binding_key: Some("product_image".to_string()),
            locked: Some(false),
            children: None,
            text: None,
            image: Some(ImageLayerData { asset_id: "asset".to_string(), fit: ImageFit::Stretch, replaceable: true }),
            shape: None,
        }, TemplateLayer {
            id: "title".to_string(),
            name: "{{title}} 大标题".to_string(),
            r#type: TemplateLayerType::Text,
            x: 8.0,
            y: 48.0,
            width: 110.0,
            height: 40.0,
            visible: true,
            opacity: 1.0,
            rotation: 0.0,
            binding_key: Some("title".to_string()),
            locked: Some(false),
            children: None,
            text: Some(TextLayerData {
                text: "默认标题".to_string(),
                font_family: "PingFang SC".to_string(),
                font_size: 18.0,
                font_weight: serde_json::json!(700),
                color: "#ffffff".to_string(),
                stroke_color: None,
                stroke_width: None,
                letter_spacing: None,
                line_height: None,
                align: Some(TextAlign::Left),
            }),
            image: None,
            shape: None,
        }],
        assets: vec![TemplateAsset { id: "asset".to_string(), name: "asset.png".to_string(), path: asset_path, source_layer_id: None, mime_type: "image/png".to_string(), width: 120, height: 120 }],
        source_psd_path: None,
        preview_path: None,
        created_at: "2026-05-07 00:00:00".to_string(),
        updated_at: "2026-05-07 00:00:00".to_string(),
    }
}

#[test]
fn exports_png_for_batch_row() {
    let dir = tempfile::tempdir().unwrap();
    let input_path = dir.path().join("input.png");
    image::RgbaImage::from_pixel(120, 120, image::Rgba([255, 0, 0, 255])).save(&input_path).unwrap();

    let store = EcommerceStore::new(dir.path().join("store")).unwrap();
    store.save_template(sample_project(input_path.to_string_lossy().into_owned())).unwrap();

    let output_dir = dir.path().join("out");
    let mut values = HashMap::new();
    values.insert("product_image".to_string(), input_path.to_string_lossy().into_owned());
    values.insert("name".to_string(), "red-chair".to_string());
    values.insert("title".to_string(), "批量标题".to_string());
    let request = ExportRequest {
        template_id: "tpl-render".to_string(),
        output_dir: output_dir.to_string_lossy().into_owned(),
        rows: vec![BatchRow { id: "row-1".to_string(), index: 0, values }],
    };

    let result = export_images(&store, request).unwrap();
    assert_eq!(result.succeeded, 1);
    assert!(fs::metadata(output_dir.join("red-chair.png")).unwrap().is_file());
    let exported = image::open(output_dir.join("red-chair.png")).unwrap().to_rgba8();
    let non_red_pixels = exported.pixels().filter(|pixel| pixel.0 != [255, 0, 0, 255]).count();
    assert!(non_red_pixels > 0, "text rendering should change pixels over the red image");
}
