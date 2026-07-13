use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use tauri::AppHandle;

const INSTALL_SCRIPT: &str = include_str!("../scripts/install-linux.sh");

pub fn apply_linux(app: &AppHandle, staged: &Path) -> Result<(), String> {
    let current_exe = std::env::current_exe()
        .map_err(|error| format!("获取当前 exe 路径失败：{error}"))?;
    let script_path =
        std::env::temp_dir().join(format!("attool-update-{}.sh", std::process::id()));
    std::fs::write(&script_path, INSTALL_SCRIPT)
        .map_err(|error| format!("写入安装脚本失败：{error}"))?;
    let mut perms = std::fs::metadata(&script_path)
        .map_err(|error| format!("读取脚本权限失败：{error}"))?
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script_path, perms)
        .map_err(|error| format!("设置脚本可执行失败：{error}"))?;

    let pid = std::process::id().to_string();
    Command::new(&script_path)
        .args([
            &pid,
            staged.to_str().ok_or("staged 路径非 UTF-8")?,
            current_exe.to_str().ok_or("current_exe 路径非 UTF-8")?,
        ])
        .spawn()
        .map_err(|error| format!("拉起安装脚本失败：{error}"))?;
    app.exit(0);
    Ok(())
}
