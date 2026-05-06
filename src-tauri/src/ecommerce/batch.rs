use std::{collections::HashMap, path::Path};

use calamine::{open_workbook_auto, Reader};
use uuid::Uuid;

use super::models::{BatchDataPreview, BatchRow};

pub fn validate_batch_fields(required: &[String], incoming: &[String]) -> BatchDataPreview {
    BatchDataPreview {
        fields: incoming.to_vec(),
        rows: Vec::new(),
        missing_fields: required.iter().filter(|field| !incoming.contains(field)).cloned().collect(),
        unused_fields: incoming.iter().filter(|field| !required.contains(field)).cloned().collect(),
    }
}

pub fn read_batch_table(path: &Path, required_fields: &[String]) -> Result<BatchDataPreview, String> {
    match path.extension().and_then(|value| value.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("csv") => read_csv(path, required_fields),
        Some("xlsx") | Some("xls") => read_excel(path, required_fields),
        _ => Err("请选择 CSV、XLS 或 XLSX 表格".to_string()),
    }
}

fn read_csv(path: &Path, required_fields: &[String]) -> Result<BatchDataPreview, String> {
    let mut reader = csv::Reader::from_path(path).map_err(|error| format!("读取 CSV 失败：{error}"))?;
    let fields: Vec<String> = reader
        .headers()
        .map_err(|error| format!("读取 CSV 表头失败：{error}"))?
        .iter()
        .map(str::to_string)
        .collect();
    let mut rows = Vec::new();
    for (index, record) in reader.records().enumerate() {
        let record = record.map_err(|error| format!("读取 CSV 第 {} 行失败：{error}", index + 2))?;
        let mut values = HashMap::new();
        for (field, value) in fields.iter().zip(record.iter()) {
            values.insert(field.clone(), value.to_string());
        }
        rows.push(BatchRow { id: Uuid::new_v4().to_string(), index, values });
    }
    Ok(with_field_validation(fields, rows, required_fields))
}

fn read_excel(path: &Path, required_fields: &[String]) -> Result<BatchDataPreview, String> {
    let mut workbook = open_workbook_auto(path).map_err(|error| format!("读取 Excel 失败：{error}"))?;
    let sheet_name = workbook.sheet_names().first().cloned().ok_or_else(|| "Excel 没有工作表".to_string())?;
    let range = workbook.worksheet_range(&sheet_name).map_err(|error| format!("读取 Excel 工作表失败：{error}"))?;
    let mut rows_iter = range.rows();
    let header = rows_iter.next().ok_or_else(|| "Excel 表格为空".to_string())?;
    let fields: Vec<String> = header.iter().map(|cell| cell.to_string()).collect();
    let mut rows = Vec::new();
    for (index, row) in rows_iter.enumerate() {
        let mut values = HashMap::new();
        for (field, cell) in fields.iter().zip(row.iter()) {
            values.insert(field.clone(), cell.to_string());
        }
        rows.push(BatchRow { id: Uuid::new_v4().to_string(), index, values });
    }
    Ok(with_field_validation(fields, rows, required_fields))
}

pub fn batch_from_folder(folder: &Path, image_binding_key: &str) -> Result<BatchDataPreview, String> {
    if !folder.is_dir() {
        return Err("请选择图片文件夹".to_string());
    }
    let mut paths: Vec<_> = std::fs::read_dir(folder)
        .map_err(|error| format!("读取图片文件夹失败：{error}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| is_supported_image(path))
        .collect();
    paths.sort();

    let mut rows = Vec::new();
    for (index, path) in paths.into_iter().enumerate() {
        let mut values = HashMap::new();
        values.insert(image_binding_key.to_string(), path.to_string_lossy().into_owned());
        values.insert(
            "name".to_string(),
            path.file_stem().and_then(|value| value.to_str()).unwrap_or("image").to_string(),
        );
        rows.push(BatchRow { id: Uuid::new_v4().to_string(), index, values });
    }

    Ok(BatchDataPreview {
        fields: vec![image_binding_key.to_string(), "name".to_string()],
        rows,
        missing_fields: Vec::new(),
        unused_fields: Vec::new(),
    })
}

fn with_field_validation(fields: Vec<String>, rows: Vec<BatchRow>, required_fields: &[String]) -> BatchDataPreview {
    let validation = validate_batch_fields(required_fields, &fields);
    BatchDataPreview { fields, rows, missing_fields: validation.missing_fields, unused_fields: validation.unused_fields }
}

fn is_supported_image(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()).map(str::to_ascii_lowercase).as_deref(),
        Some("png" | "jpg" | "jpeg" | "webp")
    )
}
