use std::path::PathBuf;

use attool_lib::ecommerce::psd_bridge::import_psd_with_bridge;

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
}
