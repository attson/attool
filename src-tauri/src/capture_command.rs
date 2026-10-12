use crate::clipboard::watcher::show_clipboard_history;
use crate::imaging::capture::trigger_capture_overlay;
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Manager};

const CAPTURE_ARG: &str = "--capture";
const CLIPBOARD_HISTORY_ARG: &str = "--clipboard-history";
const SOCKET_FILE: &str = "attool-capture.sock";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CommandRequest {
    Capture,
    ClipboardHistory,
}

impl CommandRequest {
    fn message(self) -> &'static [u8] {
        match self {
            Self::Capture => b"capture\n",
            Self::ClipboardHistory => b"clipboard-history\n",
        }
    }
}

struct ExternalCommandSocket(PathBuf);

impl Drop for ExternalCommandSocket {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn requested_command_from_args(args: &[OsString]) -> Option<CommandRequest> {
    if args.iter().any(|arg| arg == CAPTURE_ARG) {
        Some(CommandRequest::Capture)
    } else if args.iter().any(|arg| arg == CLIPBOARD_HISTORY_ARG) {
        Some(CommandRequest::ClipboardHistory)
    } else {
        None
    }
}

pub(crate) fn requested_command() -> Option<CommandRequest> {
    requested_command_from_args(&std::env::args_os().collect::<Vec<_>>())
}

fn socket_path() -> Result<PathBuf, String> {
    if let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(runtime_dir).join(SOCKET_FILE));
    }
    let cache_dir = dirs::cache_dir().ok_or_else(|| "无法确定快捷键命令通信目录".to_string())?;
    Ok(cache_dir.join("attool").join(SOCKET_FILE))
}

fn send_command(path: &Path, request: CommandRequest) -> Result<(), String> {
    let mut stream = UnixStream::connect(path).map_err(|error| error.to_string())?;
    stream
        .write_all(request.message())
        .map_err(|error| error.to_string())
}

pub(crate) fn forward_if_running(request: CommandRequest) -> bool {
    socket_path()
        .and_then(|path| send_command(&path, request))
        .is_ok()
}

fn parse_command_message(payload: &str) -> Option<CommandRequest> {
    match payload.trim() {
        "capture" => Some(CommandRequest::Capture),
        "clipboard-history" => Some(CommandRequest::ClipboardHistory),
        _ => None,
    }
}

fn trigger_command(app: &AppHandle, request: CommandRequest) -> Result<(), String> {
    match request {
        CommandRequest::Capture => {
            trigger_capture_overlay(app);
            Ok(())
        }
        CommandRequest::ClipboardHistory => show_clipboard_history(app),
    }
}

pub fn start_listener(app: &AppHandle) -> Result<(), String> {
    let path = socket_path()?;
    if UnixStream::connect(&path).is_ok() {
        return Ok(());
    }
    if path.exists() {
        fs::remove_file(&path).map_err(|error| format!("清理旧快捷键命令 Socket 失败：{error}"))?;
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("创建快捷键命令通信目录失败：{error}"))?;
    }

    let listener =
        UnixListener::bind(&path).map_err(|error| format!("启动快捷键命令监听失败：{error}"))?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("设置快捷键命令 Socket 权限失败：{error}"))?;
    app.manage(ExternalCommandSocket(path));

    let handle = app.clone();
    std::thread::spawn(move || {
        for incoming in listener.incoming() {
            let Ok(stream) = incoming else {
                continue;
            };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
            let mut payload = String::new();
            if stream.take(64).read_to_string(&mut payload).is_ok() {
                if let Some(request) = parse_command_message(&payload) {
                    if let Err(error) = trigger_command(&handle, request) {
                        eprintln!("处理外部快捷键命令失败：{error}");
                    }
                }
            }
        }
    });
    Ok(())
}

pub(crate) fn schedule_initial_command(app: &AppHandle, request: CommandRequest) {
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(700));
        if let Err(error) = trigger_command(&handle, request) {
            eprintln!("处理启动快捷键命令失败：{error}");
        }
    });
}

fn shell_quote(value: &str) -> String {
    if !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '_' | '-' | '.' | ':'))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn system_command(argument: &str) -> Option<String> {
    let executable = std::env::current_exe().ok()?;
    Some(format!(
        "{} {argument}",
        shell_quote(&executable.to_string_lossy()),
    ))
}

pub fn system_capture_command() -> Option<String> {
    system_command(CAPTURE_ARG)
}

pub fn system_clipboard_history_command() -> Option<String> {
    system_command(CLIPBOARD_HISTORY_ARG)
}

fn shortcut_settings_candidates(desktop: &str) -> Vec<(&'static str, &'static [&'static str])> {
    let desktop = desktop.to_ascii_lowercase();
    if desktop.contains("cosmic") {
        vec![("cosmic-settings", &["keyboard"])]
    } else if desktop.contains("gnome") {
        vec![("gnome-control-center", &["keyboard"])]
    } else if desktop.contains("kde") || desktop.contains("plasma") {
        vec![
            ("systemsettings6", &["kcm_keys"]),
            ("systemsettings5", &["kcm_keys"]),
        ]
    } else {
        vec![
            ("cosmic-settings", &["keyboard"]),
            ("gnome-control-center", &["keyboard"]),
            ("systemsettings6", &["kcm_keys"]),
        ]
    }
}

pub fn open_shortcut_settings() -> Result<(), String> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let mut last_error = None;
    for (program, args) in shortcut_settings_candidates(&desktop) {
        match std::process::Command::new(program).args(args).spawn() {
            Ok(_) => return Ok(()),
            Err(error) => last_error = Some(error),
        }
    }
    Err(format!(
        "无法打开系统键盘设置：{}",
        last_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| "未找到设置程序".to_string())
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_external_command_arguments() {
        assert_eq!(
            requested_command_from_args(&[OsString::from("attool"), OsString::from("--capture")]),
            Some(CommandRequest::Capture)
        );
        assert_eq!(
            requested_command_from_args(&[
                OsString::from("attool"),
                OsString::from("--clipboard-history")
            ]),
            Some(CommandRequest::ClipboardHistory)
        );
        assert_eq!(
            requested_command_from_args(&[OsString::from("attool")]),
            None
        );
    }

    #[test]
    fn parses_external_command_messages() {
        assert_eq!(
            parse_command_message("capture\n"),
            Some(CommandRequest::Capture)
        );
        assert_eq!(
            parse_command_message("clipboard-history\n"),
            Some(CommandRequest::ClipboardHistory)
        );
        assert_eq!(parse_command_message("unknown\n"), None);
    }

    #[test]
    fn quotes_executable_paths_for_shell_commands() {
        assert_eq!(shell_quote("/usr/bin/attool"), "/usr/bin/attool");
        assert_eq!(
            shell_quote("/home/user/AT Tool/attool"),
            "'/home/user/AT Tool/attool'"
        );
        assert_eq!(shell_quote("a'b"), "'a'\\''b'");
    }

    #[test]
    fn selects_current_desktop_settings() {
        assert_eq!(
            shortcut_settings_candidates("COSMIC")[0],
            ("cosmic-settings", &["keyboard"][..])
        );
        assert_eq!(
            shortcut_settings_candidates("KDE")[0],
            ("systemsettings6", &["kcm_keys"][..])
        );
    }
}
