use std::path::Path;
use std::process::Command;

/// Run tesseract on the given image and return recognized text.
/// Requires `tesseract` binary and language packs installed on the host.
/// Default language: eng + chi_sim (Simplified Chinese + English).
pub fn run_tesseract(input: &Path, langs: &str) -> Result<String, String> {
    if !input.is_file() {
        return Err(format!("图片文件不存在：{}", input.display()));
    }
    let output = Command::new("tesseract")
        .arg(input)
        .arg("-") // stdout
        .arg("-l")
        .arg(langs)
        .output()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                "未找到 tesseract。请先安装：macOS `brew install tesseract tesseract-lang` / Ubuntu `apt install tesseract-ocr tesseract-ocr-chi-sim` / Windows 装 UB-Mannheim tesseract。".to_string()
            } else {
                format!("启动 tesseract 失败：{error}")
            }
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("tesseract 识别失败：{stderr}"));
    }
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    Ok(text)
}
