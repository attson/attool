use super::compress::compress_image as run_compress;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
