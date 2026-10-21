use std::{
    path::{Path, PathBuf},
    process::Command,
};

#[cfg(debug_assertions)]
use std::{io::Write, process::Stdio};

use super::models::TemplateProject;

#[cfg(debug_assertions)]
const EMBEDDED_BRIDGE: &[u8] = include_bytes!("../../python/psd_template_bridge.py");

#[cfg(debug_assertions)]
fn embedded_bridge_source() -> &'static [u8] {
    EMBEDDED_BRIDGE
}

pub fn import_psd(psd_path: &Path, output_dir: &Path) -> Result<TemplateProject, String> {
    validate_psd_path(psd_path)?;
    std::fs::create_dir_all(output_dir)
        .map_err(|error| format!("创建模板输出目录失败：{error}"))?;

    let executable =
        std::env::current_exe().map_err(|error| format!("无法定位 AT Tool 可执行文件：{error}"))?;
    let sidecar = sidecar_path_for_executable(&executable)?;
    if sidecar.is_file() {
        let output = Command::new(&sidecar)
            .arg("--psd")
            .arg(psd_path)
            .arg("--output-dir")
            .arg(output_dir)
            .output()
            .map_err(|error| format!("启动 PSD 解析器失败：{error}"))?;
        return parse_bridge_output(output);
    }

    #[cfg(debug_assertions)]
    return import_psd_with_development_python(psd_path, output_dir);

    #[cfg(not(debug_assertions))]
    Err(format!(
        "PSD 解析器缺失，请重新安装 AT Tool：{}",
        sidecar.display()
    ))
}

fn sidecar_path_for_executable(executable: &Path) -> Result<PathBuf, String> {
    let directory = executable.parent().ok_or("无法解析 AT Tool 安装目录")?;
    Ok(directory.join(format!("psd-bridge{}", std::env::consts::EXE_SUFFIX)))
}

#[cfg(debug_assertions)]
fn import_psd_with_development_python(
    psd_path: &Path,
    output_dir: &Path,
) -> Result<TemplateProject, String> {
    let mut child = Command::new("python3")
        .arg("-")
        .arg("--psd")
        .arg(psd_path)
        .arg("--output-dir")
        .arg(output_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("启动 PSD 解析器失败：{error}"))?;

    let write_result = child
        .stdin
        .take()
        .ok_or_else(|| "无法写入 PSD 解析器".to_string())?
        .write_all(embedded_bridge_source());
    if let Err(error) = write_result {
        child.kill().ok();
        child.wait().ok();
        return Err(format!("写入 PSD 解析器失败：{error}"));
    }

    let output = child
        .wait_with_output()
        .map_err(|error| format!("等待 PSD 解析器失败：{error}"))?;
    parse_bridge_output(output)
}

fn validate_psd_path(psd_path: &Path) -> Result<(), String> {
    if !psd_path.is_file() {
        return Err("PSD 文件不存在".to_string());
    }
    if psd_path.extension().and_then(|value| value.to_str()) != Some("psd") {
        return Err("请选择 PSD 文件".to_string());
    }
    Ok(())
}

fn parse_bridge_output(output: std::process::Output) -> Result<TemplateProject, String> {
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("PSD 解析失败：{}", stderr.trim()));
    }

    serde_json::from_slice::<TemplateProject>(&output.stdout)
        .map_err(|error| format!("读取 PSD 解析结果失败：{error}"))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[cfg(debug_assertions)]
    use super::embedded_bridge_source;
    use super::sidecar_path_for_executable;

    #[cfg(debug_assertions)]
    #[test]
    fn embeds_the_psd_bridge_in_the_application_binary() {
        let source = std::str::from_utf8(embedded_bridge_source()).unwrap();

        assert!(source.starts_with("#!/usr/bin/env python3"));
        assert!(source.contains("from psd_tools import PSDImage"));
        assert!(source.contains("def main():"));
    }

    #[test]
    fn resolves_the_sidecar_next_to_the_application_executable() {
        let executable = Path::new("/opt/attool/attool");
        let expected =
            Path::new("/opt/attool").join(format!("psd-bridge{}", std::env::consts::EXE_SUFFIX));

        assert_eq!(sidecar_path_for_executable(executable).unwrap(), expected);
    }
}
