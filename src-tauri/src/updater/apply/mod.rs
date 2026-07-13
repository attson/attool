use std::path::Path;

use tauri::AppHandle;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

pub fn apply(app: &AppHandle, staged: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        macos::apply_darwin(app, staged)
    }
    #[cfg(target_os = "windows")]
    {
        windows::apply_windows(app, staged)
    }
    #[cfg(target_os = "linux")]
    {
        linux::apply_linux(app, staged)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = (app, staged);
        Err("当前平台不支持自动更新".to_string())
    }
}
