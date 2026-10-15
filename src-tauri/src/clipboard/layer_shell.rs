use std::{
    env,
    ffi::c_int,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        OnceLock,
    },
};

use gtk::{glib::object::ObjectType, prelude::WidgetExt};
use libloading::Library;
use tauri::{AppHandle, Manager};

const LIBRARY_NAME: &str = "libgtk-layer-shell.so.0";
const EDGE_LEFT: c_int = 0;
const EDGE_RIGHT: c_int = 1;
const EDGE_TOP: c_int = 2;
const EDGE_BOTTOM: c_int = 3;
const LAYER_OVERLAY: c_int = 3;
const KEYBOARD_MODE_NONE: c_int = 0;
const KEYBOARD_MODE_EXCLUSIVE: c_int = 1;
const EXCLUSIVE_ZONE_NONE: c_int = 0;
const EXCLUSIVE_ZONE_IGNORE: c_int = -1;

type GtkWindowPtr = *mut gtk::ffi::GtkWindow;
type GdkMonitorPtr = *mut gtk::gdk::ffi::GdkMonitor;
type IsSupportedFn = unsafe extern "C" fn() -> c_int;
type InitForWindowFn = unsafe extern "C" fn(GtkWindowPtr);
type IsLayerWindowFn = unsafe extern "C" fn(GtkWindowPtr) -> c_int;
type SetAnchorFn = unsafe extern "C" fn(GtkWindowPtr, c_int, c_int);
type SetExclusiveZoneFn = unsafe extern "C" fn(GtkWindowPtr, c_int);
type SetKeyboardModeFn = unsafe extern "C" fn(GtkWindowPtr, c_int);
type SetLayerFn = unsafe extern "C" fn(GtkWindowPtr, c_int);
type SetMonitorFn = unsafe extern "C" fn(GtkWindowPtr, GdkMonitorPtr);

struct LayerShellApi {
    _library: Library,
    is_supported: IsSupportedFn,
    init_for_window: InitForWindowFn,
    is_layer_window: IsLayerWindowFn,
    set_anchor: SetAnchorFn,
    set_exclusive_zone: SetExclusiveZoneFn,
    set_keyboard_mode: SetKeyboardModeFn,
    set_layer: SetLayerFn,
    set_monitor: SetMonitorFn,
}

static LAYER_SHELL_API: OnceLock<Result<LayerShellApi, String>> = OnceLock::new();
static HISTORY_LAYER_SHELL_ACTIVE: AtomicBool = AtomicBool::new(false);
static CAPTURE_LAYER_SHELL_ACTIVE: AtomicBool = AtomicBool::new(false);

fn is_wayland_backend(
    gdk_backend: Option<&str>,
    session_type: Option<&str>,
    wayland_display: Option<&str>,
) -> bool {
    if let Some(backend) = gdk_backend.filter(|value| !value.trim().is_empty()) {
        return backend
            .split(',')
            .any(|value| value.trim().eq_ignore_ascii_case("wayland"));
    }

    session_type.is_some_and(|value| value.eq_ignore_ascii_case("wayland"))
        || wayland_display.is_some_and(|value| !value.trim().is_empty())
}

fn current_backend_is_wayland() -> bool {
    let gdk_backend = env::var("GDK_BACKEND").ok();
    let session_type = env::var("XDG_SESSION_TYPE").ok();
    let wayland_display = env::var("WAYLAND_DISPLAY").ok();
    is_wayland_backend(
        gdk_backend.as_deref(),
        session_type.as_deref(),
        wayland_display.as_deref(),
    )
}

fn library_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os("ATTOOL_GTK_LAYER_SHELL_LIB").filter(|value| !value.is_empty())
    {
        candidates.push(PathBuf::from(path));
    }
    if let Ok(executable) = env::current_exe() {
        if let Some(parent) = executable.parent() {
            candidates.push(parent.join(LIBRARY_NAME));
        }
    }
    if let Some(data_dir) = dirs::data_local_dir() {
        candidates.push(data_dir.join("attool/lib").join(LIBRARY_NAME));
    }
    candidates.push(PathBuf::from(LIBRARY_NAME));
    candidates.dedup();
    candidates
}

impl LayerShellApi {
    fn load() -> Result<Self, String> {
        let mut failures = Vec::new();
        for candidate in library_candidates() {
            let library = match unsafe { Library::new(&candidate) } {
                Ok(library) => library,
                Err(error) => {
                    failures.push(format!("{}: {error}", candidate.display()));
                    continue;
                }
            };
            match unsafe { Self::from_library(library) } {
                Ok(api) => return Ok(api),
                Err(error) => failures.push(format!("{}: {error}", candidate.display())),
            }
        }
        Err(format!(
            "无法加载 {LIBRARY_NAME}，剪贴板面板将回退为普通窗口：{}",
            failures.join("; ")
        ))
    }

    unsafe fn from_library(library: Library) -> Result<Self, String> {
        macro_rules! symbol {
            ($name:literal, $ty:ty) => {{
                *unsafe { library.get::<$ty>(concat!($name, "\0").as_bytes()) }
                    .map_err(|error| format!("缺少 {}：{error}", $name))?
            }};
        }

        let is_supported = symbol!("gtk_layer_is_supported", IsSupportedFn);
        let init_for_window = symbol!("gtk_layer_init_for_window", InitForWindowFn);
        let is_layer_window = symbol!("gtk_layer_is_layer_window", IsLayerWindowFn);
        let set_anchor = symbol!("gtk_layer_set_anchor", SetAnchorFn);
        let set_exclusive_zone = symbol!("gtk_layer_set_exclusive_zone", SetExclusiveZoneFn);
        let set_keyboard_mode = symbol!("gtk_layer_set_keyboard_mode", SetKeyboardModeFn);
        let set_layer = symbol!("gtk_layer_set_layer", SetLayerFn);
        let set_monitor = symbol!("gtk_layer_set_monitor", SetMonitorFn);

        Ok(Self {
            _library: library,
            is_supported,
            init_for_window,
            is_layer_window,
            set_anchor,
            set_exclusive_zone,
            set_keyboard_mode,
            set_layer,
            set_monitor,
        })
    }
}

fn layer_shell_api() -> Result<&'static LayerShellApi, String> {
    match LAYER_SHELL_API.get_or_init(LayerShellApi::load) {
        Ok(api) => Ok(api),
        Err(error) => Err(error.clone()),
    }
}

fn configure_window(
    app: &AppHandle,
    label: &str,
    description: &str,
    top: bool,
    bottom: bool,
    exclusive_zone: c_int,
    active: &'static AtomicBool,
) -> Result<bool, String> {
    if !current_backend_is_wayland() {
        return Ok(false);
    }

    let api = layer_shell_api()?;
    if unsafe { (api.is_supported)() } == 0 {
        return Ok(false);
    }

    let window = app
        .get_webview_window(label)
        .ok_or_else(|| format!("找不到{description}"))?;
    let gtk_window = window
        .gtk_window()
        .map_err(|error| format!("无法访问{description} GTK 窗口：{error}"))?;
    if gtk_window.is_realized() {
        return Err(format!(
            "{description} GTK 窗口已完成初始化，无法切换为 layer-shell"
        ));
    }

    let window_ptr = gtk_window.as_ptr() as GtkWindowPtr;
    unsafe {
        (api.init_for_window)(window_ptr);
        if (api.is_layer_window)(window_ptr) == 0 {
            return Err("layer-shell 初始化未生效".to_string());
        }
        (api.set_layer)(window_ptr, LAYER_OVERLAY);
        (api.set_anchor)(window_ptr, EDGE_LEFT, 1);
        (api.set_anchor)(window_ptr, EDGE_RIGHT, 1);
        (api.set_anchor)(window_ptr, EDGE_TOP, top as c_int);
        (api.set_anchor)(window_ptr, EDGE_BOTTOM, bottom as c_int);
        (api.set_exclusive_zone)(window_ptr, exclusive_zone);
        (api.set_keyboard_mode)(window_ptr, KEYBOARD_MODE_EXCLUSIVE);
    }

    if let Some(display) = gtk::gdk::Display::default() {
        if let Some(monitor) = display.primary_monitor().or_else(|| display.monitor(0)) {
            unsafe { (api.set_monitor)(window_ptr, monitor.as_ptr()) };
        }
    }

    active.store(true, Ordering::Release);
    Ok(true)
}

pub(crate) fn configure_history_window(app: &AppHandle) -> Result<bool, String> {
    configure_window(
        app,
        "clipboard-history",
        "剪贴板快捷面板",
        false,
        true,
        EXCLUSIVE_ZONE_NONE,
        &HISTORY_LAYER_SHELL_ACTIVE,
    )
}

pub(crate) fn configure_capture_overlay(app: &AppHandle) -> Result<bool, String> {
    configure_window(
        app,
        "capture-overlay",
        "截图录屏浮层",
        true,
        true,
        EXCLUSIVE_ZONE_IGNORE,
        &CAPTURE_LAYER_SHELL_ACTIVE,
    )
}

pub(crate) fn history_window_is_layer_shell() -> bool {
    HISTORY_LAYER_SHELL_ACTIVE.load(Ordering::Acquire)
}

pub(crate) fn capture_overlay_is_layer_shell() -> bool {
    CAPTURE_LAYER_SHELL_ACTIVE.load(Ordering::Acquire)
}

pub(crate) fn set_capture_overlay_input_region(
    app: &AppHandle,
    region: Option<(i32, i32, i32, i32)>,
) -> Result<(), String> {
    let window = app
        .get_webview_window("capture-overlay")
        .ok_or_else(|| "找不到截图录屏浮层".to_string())?;
    let gtk_window = window
        .gtk_window()
        .map_err(|error| format!("无法访问截图录屏浮层 GTK 窗口：{error}"))?;

    if capture_overlay_is_layer_shell() {
        let api = layer_shell_api()?;
        let keyboard_mode = if region.is_some() {
            KEYBOARD_MODE_NONE
        } else {
            KEYBOARD_MODE_EXCLUSIVE
        };
        unsafe { (api.set_keyboard_mode)(gtk_window.as_ptr() as GtkWindowPtr, keyboard_mode) };
    }

    if let Some((x, y, width, height)) = region {
        let rectangle = gtk::cairo::RectangleInt::new(x, y, width.max(1), height.max(1));
        let input_region = gtk::cairo::Region::create_rectangle(&rectangle);
        gtk_window.input_shape_combine_region(Some(&input_region));
    } else {
        gtk_window.input_shape_combine_region(None);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_wayland_backend;

    #[test]
    fn explicit_x11_backend_overrides_wayland_session() {
        assert!(!is_wayland_backend(
            Some("x11"),
            Some("wayland"),
            Some("wayland-1")
        ));
    }

    #[test]
    fn accepts_wayland_in_backend_fallback_list() {
        assert!(is_wayland_backend(Some("wayland,x11"), Some("x11"), None));
    }

    #[test]
    fn detects_wayland_from_session_environment() {
        assert!(is_wayland_backend(None, Some("WAYLAND"), None));
        assert!(is_wayland_backend(None, None, Some("wayland-0")));
        assert!(!is_wayland_backend(None, Some("x11"), None));
    }
}
