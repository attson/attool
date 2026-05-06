use std::fs;

use attool_lib::ecommerce::batch::{batch_from_folder, read_batch_table, validate_batch_fields};

#[test]
fn reads_csv_batch_table() {
    let dir = tempfile::tempdir().unwrap();
    let csv_path = dir.path().join("batch.csv");
    fs::write(&csv_path, "title,product_image\n双人椅,/tmp/a.png\n").unwrap();

    let preview = read_batch_table(&csv_path, &["title".to_string(), "product_image".to_string()]).unwrap();
    assert_eq!(preview.fields, vec!["title", "product_image"]);
    assert_eq!(preview.rows[0].values.get("title").unwrap(), "双人椅");
    assert!(preview.missing_fields.is_empty());
}

#[test]
fn reports_missing_and_unused_fields() {
    let result = validate_batch_fields(&["title".to_string(), "product_image".to_string()], &["title".to_string(), "price".to_string()]);
    assert_eq!(result.missing_fields, vec!["product_image"]);
    assert_eq!(result.unused_fields, vec!["price"]);
}

#[test]
fn creates_batch_from_image_folder() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.png"), b"not-real-image-but-valid-path").unwrap();
    fs::write(dir.path().join("notes.txt"), b"skip").unwrap();

    let preview = batch_from_folder(dir.path(), "product_image").unwrap();
    assert_eq!(preview.fields, vec!["product_image", "name"]);
    assert_eq!(preview.rows.len(), 1);
    assert_eq!(preview.rows[0].values.get("name").unwrap(), "a");
}
