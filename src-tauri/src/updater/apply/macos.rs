use std::path::{Path, PathBuf};
use std::process::Command;

use tauri::AppHandle;

pub fn apply_darwin(app: &AppHandle, staged: &Path) -> Result<(), String> {
    let bundle_path = current_app_bundle_path()?;
    let staging = std::env::temp_dir().join(format!("attool-update-{}", std::process::id()));
    if staging.exists() {
        std::fs::remove_dir_all(&staging).ok();
    }
    std::fs::create_dir_all(&staging)
        .map_err(|error| format!("创建 staging 目录失败：{error}"))?;

    let status = Command::new("tar")
        .args([
            "-xzf",
            staged.to_str().ok_or("staged 路径非 UTF-8")?,
            "-C",
            staging.to_str().ok_or("staging 路径非 UTF-8")?,
        ])
        .status()
        .map_err(|error| format!("tar 解压失败：{error}"))?;
    if !status.success() {
        return Err("tar 解压 exit != 0".to_string());
    }

    let bundle_basename = bundle_path
        .file_name()
        .ok_or("无法解析 bundle basename")?
        .to_owned();
    let new_bundle = staging.join(&bundle_basename);
    if !new_bundle.exists() {
        return Err(format!(
            "archive 中缺少 {}",
            bundle_basename.to_string_lossy()
        ));
    }

    let trash = bundle_path.with_extension("app.old");
    if trash.exists() {
        std::fs::remove_dir_all(&trash).ok();
    }
    std::fs::rename(&bundle_path, &trash)
        .map_err(|error| format!("移动旧 .app 失败：{error}"))?;
    if let Err(error) = std::fs::rename(&new_bundle, &bundle_path) {
        std::fs::rename(&trash, &bundle_path).ok();
        return Err(format!("替换 .app 失败：{error}"));
    }
    std::fs::remove_dir_all(&trash).ok();
    std::fs::remove_dir_all(&staging).ok();
    std::fs::remove_file(staged).ok();

    Command::new("open")
        .arg(&bundle_path)
        .spawn()
        .map_err(|error| format!("重新启动 .app 失败：{error}"))?;
    app.exit(0);
    Ok(())
}

fn current_app_bundle_path() -> Result<PathBuf, String> {
    let exe = std::env::current_exe()
        .map_err(|error| format!("获取当前可执行路径失败：{error}"))?;
    // exe: /path/to/AT Tool.app/Contents/MacOS/attool
    let mut cursor = exe.as_path();
    while let Some(parent) = cursor.parent() {
        if parent.extension().and_then(|s| s.to_str()) == Some("app") {
            return Ok(parent.to_path_buf());
        }
        cursor = parent;
    }
    Err("未在当前 exe 路径中找到 .app bundle".to_string())
}
