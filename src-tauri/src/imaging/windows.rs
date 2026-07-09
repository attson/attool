use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowRect {
    pub owner: String,
    pub title: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub layer: i32,
}

#[cfg(target_os = "macos")]
pub fn list_visible_windows(_scale: f64) -> Vec<WindowRect> {
    use core_foundation::array::{CFArray, CFArrayRef};
    use core_foundation::base::{CFType, CFTypeRef, TCFType};
    use core_foundation::dictionary::CFDictionaryRef;
    use core_foundation::number::{CFNumber, CFNumberRef};
    use core_foundation::string::{CFString, CFStringRef};
    use core_graphics::window::{
        kCGNullWindowID, kCGWindowListExcludeDesktopElements, kCGWindowListOptionOnScreenOnly,
        CGWindowListCopyWindowInfo,
    };
    use std::ffi::c_void;

    // Look up a key in a CFDictionary, returning the raw CFTypeRef (or null).
    unsafe fn dict_get(dict: CFDictionaryRef, key: &CFString) -> CFTypeRef {
        let mut value: *const c_void = std::ptr::null();
        let found = core_foundation::dictionary::CFDictionaryGetValueIfPresent(
            dict,
            key.as_concrete_TypeRef() as *const c_void,
            &mut value,
        );
        if found == 0 {
            std::ptr::null()
        } else {
            value as CFTypeRef
        }
    }

    unsafe fn read_string(dict: CFDictionaryRef, key: &CFString) -> Option<String> {
        let v = dict_get(dict, key);
        if v.is_null() {
            return None;
        }
        let s = CFString::wrap_under_get_rule(v as CFStringRef);
        Some(s.to_string())
    }

    unsafe fn read_i32(dict: CFDictionaryRef, key: &CFString) -> Option<i32> {
        let v = dict_get(dict, key);
        if v.is_null() {
            return None;
        }
        let n = CFNumber::wrap_under_get_rule(v as CFNumberRef);
        n.to_i32()
    }

    unsafe fn read_f64(dict: CFDictionaryRef, key: &CFString) -> Option<f64> {
        let v = dict_get(dict, key);
        if v.is_null() {
            return None;
        }
        let n = CFNumber::wrap_under_get_rule(v as CFNumberRef);
        n.to_f64()
    }

    unsafe {
        let opts = kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements;
        let raw = CGWindowListCopyWindowInfo(opts, kCGNullWindowID);
        if raw.is_null() {
            return Vec::new();
        }
        let arr: CFArray<CFType> = CFArray::wrap_under_create_rule(raw as CFArrayRef);

        let key_bounds = CFString::from_static_string("kCGWindowBounds");
        let key_owner = CFString::from_static_string("kCGWindowOwnerName");
        let key_name = CFString::from_static_string("kCGWindowName");
        let key_layer = CFString::from_static_string("kCGWindowLayer");
        let key_alpha = CFString::from_static_string("kCGWindowAlpha");
        let key_x = CFString::from_static_string("X");
        let key_y = CFString::from_static_string("Y");
        let key_w = CFString::from_static_string("Width");
        let key_h = CFString::from_static_string("Height");

        let mut out: Vec<WindowRect> = Vec::new();

        for i in 0..arr.len() {
            let item = match arr.get(i as isize) {
                Some(v) => v,
                None => continue,
            };
            let dict_ref = item.as_CFTypeRef() as CFDictionaryRef;

            let layer = read_i32(dict_ref, &key_layer).unwrap_or(0);
            if layer != 0 {
                continue;
            }
            let alpha = read_f64(dict_ref, &key_alpha).unwrap_or(1.0);
            if alpha < 0.05 {
                continue;
            }

            let bounds_ref = dict_get(dict_ref, &key_bounds);
            if bounds_ref.is_null() {
                continue;
            }
            let bounds_dict_ref = bounds_ref as CFDictionaryRef;

            let fx = read_f64(bounds_dict_ref, &key_x).unwrap_or(0.0);
            let fy = read_f64(bounds_dict_ref, &key_y).unwrap_or(0.0);
            let fw = read_f64(bounds_dict_ref, &key_w).unwrap_or(0.0);
            let fh = read_f64(bounds_dict_ref, &key_h).unwrap_or(0.0);
            if fw < 40.0 || fh < 40.0 {
                continue;
            }

            let owner = read_string(dict_ref, &key_owner).unwrap_or_default();
            let title = read_string(dict_ref, &key_name).unwrap_or_default();

            out.push(WindowRect {
                owner,
                title,
                x: fx,
                y: fy,
                w: fw,
                h: fh,
                layer,
            });
        }
        out
    }
}

#[cfg(not(target_os = "macos"))]
pub fn list_visible_windows(_scale: f64) -> Vec<WindowRect> {
    use xcap::Window;
    // Window::all() 在 Wayland 下可能失败 —— 降级为空列表，不破坏区域选择
    let Ok(windows) = Window::all() else {
        return Vec::new();
    };
    windows
        .into_iter()
        .filter(|w| !w.is_minimized().unwrap_or(true))
        .filter_map(|w| {
            let x = w.x().ok()? as f64;
            let y = w.y().ok()? as f64;
            let width = w.width().ok()? as f64;
            let height = w.height().ok()? as f64;
            // 与 macOS 实现一致：过滤掉过小窗口
            if width < 40.0 || height < 40.0 {
                return None;
            }
            Some(WindowRect {
                owner: w.app_name().unwrap_or_default(),
                title: w.title().unwrap_or_default(),
                x,
                y,
                w: width,
                h: height,
                layer: 0,
            })
        })
        .collect()
}
