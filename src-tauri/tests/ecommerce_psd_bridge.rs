use std::path::PathBuf;

use attool_lib::ecommerce::{models::TemplateLayer, psd_bridge::import_psd_with_bridge};

fn flatten_layers(layers: &[TemplateLayer]) -> Vec<&TemplateLayer> {
    let mut flattened = Vec::new();
    for layer in layers {
        flattened.push(layer);
        if let Some(children) = &layer.children {
            flattened.extend(flatten_layers(children));
        }
    }
    flattened
}

#[test]
fn imports_user_psd_when_available() {
    let psd_path = PathBuf::from("/Users/attson/Documents/店铺/双人椅/双人主图2+活动 - 4链接 拷贝 2.psd");
    if !psd_path.exists() {
        eprintln!("skipping user PSD smoke test because file is not present");
        return;
    }

    let bridge = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("python/psd_template_bridge.py");
    let output = tempfile::tempdir().unwrap();
    let project = import_psd_with_bridge(&bridge, &psd_path, output.path()).unwrap();

    assert_eq!(project.canvas_width, 1000);
    assert_eq!(project.canvas_height, 1000);
    assert!(project.layers.len() >= 10);
    assert!(project.assets.iter().any(|asset| asset.name.contains("LOGO")));

    let flat_layers = flatten_layers(&project.layers);
    let title = flat_layers.iter().find(|layer| layer.name == "大标题").and_then(|layer| layer.text.as_ref()).unwrap();
    assert!((title.letter_spacing.unwrap() - -2.428737).abs() < 0.001);

    let style_title = flat_layers.iter().find(|layer| layer.name == "双人黑色 北欧风").and_then(|layer| layer.text.as_ref()).unwrap();
    assert_eq!(style_title.line_height, Some(114.42438));
    assert_eq!(style_title.stroke_color.as_deref(), Some("#000000"));
    assert_eq!(style_title.stroke_width, Some(1.0));
}
