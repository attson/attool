use super::capture::capture_screen as run_capture;
use super::compress::compress_image as run_compress;
use super::convert::convert_image as run_convert;
use super::exif::{read_exif as run_read_exif, strip_exif as run_strip_exif};
use super::ocr::run_tesseract;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

// ---- compress ----

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressRequest {
    pub input_paths: Vec<String>,
    pub output_dir: String,
    pub quality: Option<u8>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressItemResult {
    pub input_path: String,
    pub output_path: String,
    pub original_size: u64,
    pub compressed_size: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressFailure {
    pub input_path: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressResponse {
    pub succeeded: Vec<CompressItemResult>,
    pub failed: Vec<CompressFailure>,
}

#[tauri::command]
pub async fn compress_images(request: CompressRequest) -> Result<CompressResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let output_dir = PathBuf::from(request.output_dir.trim());
        if output_dir.as_os_str().is_empty() {
            return Err("请选择输出目录".to_string());
        }
        if request.input_paths.is_empty() {
            return Err("请选择要压缩的图片".to_string());
        }
        let quality = request.quality.unwrap_or(80).clamp(1, 100);

        let mut succeeded = Vec::new();
        let mut failed = Vec::new();

        for path_str in &request.input_paths {
            let input = PathBuf::from(path_str);
            match run_compress(&input, &output_dir, quality) {
                Ok(res) => succeeded.push(CompressItemResult {
                    input_path: path_str.clone(),
                    output_path: res.output_path.to_string_lossy().into_owned(),
                    original_size: res.original_size,
                    compressed_size: res.compressed_size,
                }),
                Err(err) => failed.push(CompressFailure {
                    input_path: path_str.clone(),
                    message: err,
                }),
            }
        }

        Ok(CompressResponse { succeeded, failed })
    })
    .await
    .map_err(|error| format!("压缩任务异常：{error}"))?
}

// ---- convert ----

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertRequest {
    pub input_paths: Vec<String>,
    pub output_dir: String,
    pub target_format: String,
    pub quality: Option<u8>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertItemResult {
    pub input_path: String,
    pub output_path: String,
    pub original_size: u64,
    pub converted_size: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertResponse {
    pub succeeded: Vec<ConvertItemResult>,
    pub failed: Vec<CompressFailure>,
}

#[tauri::command]
pub async fn convert_images(request: ConvertRequest) -> Result<ConvertResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let output_dir = PathBuf::from(request.output_dir.trim());
        if output_dir.as_os_str().is_empty() {
            return Err("请选择输出目录".to_string());
        }
        if request.input_paths.is_empty() {
            return Err("请选择要转换的图片".to_string());
        }
        let quality = request.quality.unwrap_or(90).clamp(1, 100);

        let mut succeeded = Vec::new();
        let mut failed = Vec::new();
        for path_str in &request.input_paths {
            let input = PathBuf::from(path_str);
            match run_convert(&input, &output_dir, &request.target_format, quality) {
                Ok(res) => succeeded.push(ConvertItemResult {
                    input_path: path_str.clone(),
                    output_path: res.output_path.to_string_lossy().into_owned(),
                    original_size: res.original_size,
                    converted_size: res.converted_size,
                }),
                Err(err) => failed.push(CompressFailure {
                    input_path: path_str.clone(),
                    message: err,
                }),
            }
        }
        Ok(ConvertResponse { succeeded, failed })
    })
    .await
    .map_err(|error| format!("转换任务异常：{error}"))?
}

// ---- exif ----

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExifField {
    pub tag: String,
    pub value: String,
}

#[tauri::command]
pub async fn read_image_exif(path: String) -> Result<Vec<ExifField>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let fields = run_read_exif(&PathBuf::from(&path))?;
        Ok(fields
            .into_iter()
            .map(|f| ExifField {
                tag: f.tag,
                value: f.value,
            })
            .collect())
    })
    .await
    .map_err(|error| format!("读取 EXIF 异常：{error}"))?
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StripExifRequest {
    pub input_path: String,
    pub output_dir: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StripExifResponse {
    pub output_path: String,
}

#[tauri::command]
pub async fn strip_image_exif(request: StripExifRequest) -> Result<StripExifResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let output = run_strip_exif(
            &PathBuf::from(&request.input_path),
            &PathBuf::from(&request.output_dir),
        )?;
        Ok(StripExifResponse {
            output_path: output.to_string_lossy().into_owned(),
        })
    })
    .await
    .map_err(|error| format!("清 EXIF 异常：{error}"))?
}

// ---- ocr ----

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrRequest {
    pub input_path: String,
    pub langs: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrResponse {
    pub text: String,
}

#[tauri::command]
pub async fn ocr_image(request: OcrRequest) -> Result<OcrResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let langs = request
            .langs
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("eng+chi_sim");
        let text = run_tesseract(&PathBuf::from(&request.input_path), langs)?;
        Ok(OcrResponse { text })
    })
    .await
    .map_err(|error| format!("OCR 异常：{error}"))?
}

// ---- capture ----

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureRequest {
    pub mode: String, // "region" | "window" | "full"
    pub delay_seconds: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureResponse {
    pub output_path: String,
}

#[tauri::command]
pub async fn capture_screen(request: CaptureRequest) -> Result<CaptureResponse, String> {
    // NB: don't offload to spawn_blocking — screencapture -i needs the front-most GUI focus
    // and the user's input; running from an unblocking context is fine.
    let path = run_capture(&request.mode, request.delay_seconds.unwrap_or(0).min(10))?;
    Ok(CaptureResponse {
        output_path: path.to_string_lossy().into_owned(),
    })
}

// ---- write binary file (for canvas export) ----

#[tauri::command]
pub async fn write_binary_file(path: String, base64: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&base64)
            .map_err(|error| format!("Base64 解码失败：{error}"))?;
        if let Some(parent) = std::path::Path::new(&path).parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|error| format!("创建目录失败：{error}"))?;
            }
        }
        fs::write(&path, bytes).map_err(|error| format!("写入文件失败：{error}"))
    })
    .await
    .map_err(|error| format!("写入任务异常：{error}"))?
}
