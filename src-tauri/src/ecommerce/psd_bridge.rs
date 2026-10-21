use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
};

#[cfg(debug_assertions)]
use std::process::Stdio;

use super::models::TemplateProject;

#[cfg(debug_assertions)]
const EMBEDDED_BRIDGE_SOURCE: &[u8] = include_bytes!("../../python/psd_template_bridge.py");

#[cfg(not(debug_assertions))]
const EMBEDDED_BRIDGE_BINARY: &[u8] = include_bytes!(env!("ATTOOL_PSD_BRIDGE_PATH"));

#[cfg(debug_assertions)]
fn embedded_bridge_source() -> &'static [u8] {
    EMBEDDED_BRIDGE_SOURCE
}

pub fn import_psd(psd_path: &Path, output_dir: &Path) -> Result<TemplateProject, String> {
    validate_psd_path(psd_path)?;
    std::fs::create_dir_all(output_dir)
        .map_err(|error| format!("创建模板输出目录失败：{error}"))?;

    #[cfg(not(debug_assertions))]
    let sidecar = materialize_embedded_bridge(
        &dirs::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("attool"),
        env!("CARGO_PKG_VERSION"),
        EMBEDDED_BRIDGE_BINARY,
    )?;

    #[cfg(debug_assertions)]
    let sidecar = {
        let executable = std::env::current_exe()
            .map_err(|error| format!("无法定位 AT Tool 可执行文件：{error}"))?;
        sidecar_path_for_executable(&executable)?
    };
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

#[cfg(debug_assertions)]
fn sidecar_path_for_executable(executable: &Path) -> Result<PathBuf, String> {
    let directory = executable.parent().ok_or("无法解析 AT Tool 安装目录")?;
    Ok(directory.join(format!("psd-bridge{}", std::env::consts::EXE_SUFFIX)))
}

fn materialize_embedded_bridge(
    cache_root: &Path,
    version: &str,
    bridge: &[u8],
) -> Result<PathBuf, String> {
    let directory = cache_root.join("psd-bridge").join(version);
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("创建 PSD 解析器目录失败：{error}"))?;

    let destination = directory.join(format!("psd-bridge{}", std::env::consts::EXE_SUFFIX));
    if file_matches(&destination, bridge) {
        set_executable(&destination)?;
        return Ok(destination);
    }

    let mut temporary = tempfile::NamedTempFile::new_in(&directory)
        .map_err(|error| format!("创建 PSD 解析器临时文件失败：{error}"))?;
    temporary
        .write_all(bridge)
        .map_err(|error| format!("写入 PSD 解析器失败：{error}"))?;

    set_executable(temporary.path())?;

    if let Err(error) = temporary.persist(&destination) {
        if file_matches(&destination, bridge) {
            return Ok(destination);
        } else {
            return Err(format!("安装 PSD 解析器失败：{}", error.error));
        }
    }

    Ok(destination)
}

fn file_matches(path: &Path, expected: &[u8]) -> bool {
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    if file
        .metadata()
        .map(|metadata| metadata.len() != expected.len() as u64)
        .unwrap_or(true)
    {
        return false;
    }

    let mut offset = 0;
    let mut buffer = [0_u8; 64 * 1024];
    while offset < expected.len() {
        let Ok(read) = file.read(&mut buffer) else {
            return false;
        };
        if read == 0 || buffer[..read] != expected[offset..offset + read] {
            return false;
        }
        offset += read;
    }
    true
}

fn set_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path)
            .map_err(|error| format!("读取 PSD 解析器权限失败：{error}"))?
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions)
            .map_err(|error| format!("设置 PSD 解析器权限失败：{error}"))?;
    }
    Ok(())
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
    #[cfg(debug_assertions)]
    use std::path::Path;

    use super::materialize_embedded_bridge;
    #[cfg(debug_assertions)]
    use super::{embedded_bridge_source, sidecar_path_for_executable};

    #[cfg(debug_assertions)]
    #[test]
    fn embeds_the_psd_bridge_in_the_application_binary() {
        let source = std::str::from_utf8(embedded_bridge_source()).unwrap();

        assert!(source.starts_with("#!/usr/bin/env python3"));
        assert!(source.contains("from psd_tools import PSDImage"));
        assert!(source.contains("def main():"));
    }

    #[cfg(debug_assertions)]
    #[test]
    fn resolves_the_sidecar_next_to_the_application_executable() {
        let executable = Path::new("/opt/attool/attool");
        let expected =
            Path::new("/opt/attool").join(format!("psd-bridge{}", std::env::consts::EXE_SUFFIX));

        assert_eq!(sidecar_path_for_executable(executable).unwrap(), expected);
    }

    #[test]
    fn materializes_the_embedded_bridge_in_a_versioned_cache_directory() {
        let cache = tempfile::tempdir().unwrap();

        let path = materialize_embedded_bridge(cache.path(), "0.8.45", b"embedded bridge").unwrap();

        assert_eq!(
            path,
            cache
                .path()
                .join("psd-bridge")
                .join("0.8.45")
                .join(format!("psd-bridge{}", std::env::consts::EXE_SUFFIX))
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"embedded bridge");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o755
            );
        }
    }

    #[test]
    fn replaces_a_corrupt_cached_bridge_with_the_embedded_binary() {
        let cache = tempfile::tempdir().unwrap();
        let path = materialize_embedded_bridge(cache.path(), "0.8.45", b"wrong contents!").unwrap();

        materialize_embedded_bridge(cache.path(), "0.8.45", b"embedded bridge").unwrap();

        assert_eq!(std::fs::read(path).unwrap(), b"embedded bridge");
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn embedded_release_bridge_is_executable() {
        let cache = tempfile::tempdir().unwrap();
        let path = materialize_embedded_bridge(
            cache.path(),
            env!("CARGO_PKG_VERSION"),
            super::EMBEDDED_BRIDGE_BINARY,
        )
        .unwrap();

        let output = std::process::Command::new(path)
            .arg("--help")
            .output()
            .unwrap();

        assert!(output.status.success());
    }
}
