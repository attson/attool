use std::path::Path;
use std::process::Command;

use tauri::AppHandle;

const UPDATE_BAT: &str = include_str!("../scripts/update-windows.bat");

pub fn apply_windows(app: &AppHandle, staged: &Path) -> Result<(), String> {
    let current_exe = std::env::current_exe()
        .map_err(|error| format!("获取当前 exe 路径失败：{error}"))?;
    let tmp = std::env::temp_dir();
    let script_path = tmp.join(format!("attool-update-{}.bat", std::process::id()));
    std::fs::write(&script_path, UPDATE_BAT)
        .map_err(|error| format!("写入更新脚本失败：{error}"))?;

    // 需要先把 .zip 里的 exe 释放出来 —— 直接调用 PowerShell Expand-Archive
    let extracted_dir = tmp.join(format!("attool-update-{}", std::process::id()));
    if extracted_dir.exists() {
        std::fs::remove_dir_all(&extracted_dir).ok();
    }
    std::fs::create_dir_all(&extracted_dir)
        .map_err(|error| format!("创建 extract 目录失败：{error}"))?;
    let status = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force",
                staged.display(),
                extracted_dir.display()
            ),
        ])
        .status()
        .map_err(|error| format!("Expand-Archive 失败：{error}"))?;
    if !status.success() {
        return Err("Expand-Archive exit != 0".to_string());
    }

    // 找出解压出来的 exe（应该只有一个）
    let new_exe = std::fs::read_dir(&extracted_dir)
        .map_err(|error| format!("读取 extract 目录失败：{error}"))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|p| p.extension().and_then(|s| s.to_str()) == Some("exe"))
        .ok_or("解压后未找到 .exe")?;

    Command::new("cmd")
        .args([
            "/C",
            "start",
            "",
            script_path.to_str().ok_or("script 路径非 UTF-8")?,
            new_exe.to_str().ok_or("new_exe 路径非 UTF-8")?,
            current_exe.to_str().ok_or("current_exe 路径非 UTF-8")?,
        ])
        .spawn()
        .map_err(|error| format!("拉起更新脚本失败：{error}"))?;

    // 注意：不能 remove staged / extracted_dir，脚本还要读
    let _ = staged;
    app.exit(0);
    Ok(())
}
