use std::{path::Path, process::Command};

use super::models::TemplateProject;

pub fn import_psd_with_bridge(
    bridge_script: &Path,
    psd_path: &Path,
    output_dir: &Path,
) -> Result<TemplateProject, String> {
    if !psd_path.is_file() {
        return Err("PSD 文件不存在".to_string());
    }
    if psd_path.extension().and_then(|value| value.to_str()) != Some("psd") {
        return Err("请选择 PSD 文件".to_string());
    }

    std::fs::create_dir_all(output_dir).map_err(|error| format!("创建模板输出目录失败：{error}"))?;
    let output = Command::new("python3")
        .arg(bridge_script)
        .arg("--psd")
        .arg(psd_path)
        .arg("--output-dir")
        .arg(output_dir)
        .output()
        .map_err(|error| format!("启动 PSD 解析器失败：{error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("PSD 解析失败：{}", stderr.trim()));
    }

    serde_json::from_slice::<TemplateProject>(&output.stdout)
        .map_err(|error| format!("读取 PSD 解析结果失败：{error}"))
}
