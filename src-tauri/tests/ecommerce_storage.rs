use std::fs;

use attool_lib::ecommerce::{models::*, storage::EcommerceStore};

fn sample_project() -> TemplateProject {
    TemplateProject {
        id: "tpl-test".to_string(),
        name: "双人椅主图".to_string(),
        canvas_width: 1000,
        canvas_height: 1000,
        layers: vec![TemplateLayer {
            id: "layer-title".to_string(),
            name: "{{title}} 大标题".to_string(),
            r#type: TemplateLayerType::Text,
            x: 10.0,
            y: 900.0,
            width: 980.0,
            height: 80.0,
            visible: true,
            opacity: 1.0,
            rotation: 0.0,
            binding_key: Some("title".to_string()),
            locked: Some(false),
            children: None,
            text: Some(TextLayerData {
                text: "便携小沙发".to_string(),
                font_family: "STHupo".to_string(),
                font_size: 48.0,
                font_weight: serde_json::json!(700),
                color: "#ffffff".to_string(),
                stroke_color: None,
                stroke_width: None,
                letter_spacing: None,
                line_height: None,
                align: Some(TextAlign::Center),
            }),
            image: None,
            shape: None,
        }],
        assets: vec![],
        source_psd_path: Some("/tmp/source.psd".to_string()),
        preview_path: None,
        created_at: "2026-05-07 00:00:00".to_string(),
        updated_at: "2026-05-07 00:00:00".to_string(),
    }
}

#[test]
fn saves_lists_and_loads_templates() {
    let root = std::env::temp_dir().join(format!("attool-store-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let store = EcommerceStore::new(root.clone()).unwrap();

    let saved = store.save_template(sample_project()).unwrap();
    assert_eq!(saved.name, "双人椅主图");

    let summaries = store.list_templates().unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, "tpl-test");

    let loaded = store.load_template("tpl-test").unwrap();
    assert_eq!(loaded.layers[0].binding_key.as_deref(), Some("title"));

    fs::remove_dir_all(root).unwrap();
}
