//! Linux GPU 渲染兜底。
//!
//! WebKitGTK 在 DMABUF/GBM 渲染路径上遇到 GPU 环境异常时会在 C 层 SIGABRT,
//! 整个进程崩溃且无法被 Rust 捕获。因此只能在启动前探测已知的崩溃信号,事前
//! 设置 `WEBKIT_DISABLE_DMABUF_RENDERER=1` 把 WebKit 引到软件合成路径避让,
//! 而非事后接住。探测只读文件、不触碰 EGL/GBM,因此探测本身绝不会崩。
//!
//! GPU 环境正常时不做任何改动,保留硬件加速。用户已显式设置该变量时一律尊重。
//!
//! 逻辑移植自 atstarter 的 gpu_linux.go。

use std::fs;
use std::path::Path;

const DMABUF_KEY: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";

/// 在 Linux 上启动 Tauri/WebKitGTK 前调用:探测到已知会导致 abort 的 GPU 环境
/// 信号时,兜底禁用 WebKit DMABUF 渲染器。
pub fn maybe_disable_dmabuf() {
    if std::env::var_os(DMABUF_KEY).is_some() {
        return; // 用户已表态,不干预
    }
    if let Some(reason) = should_disable_dmabuf(Path::new("/")) {
        // SAFETY: 在 run() 最开始、任何线程/WebKit 初始化之前调用,此时进程为单线程。
        unsafe { std::env::set_var(DMABUF_KEY, "1") };
        println!("[gpu] 检测到 GPU 渲染异常,已禁用 WebKit DMABUF 渲染器:{reason}");
    }
}

/// 在以 `root` 为根的文件树上判断是否应禁用 DMABUF 渲染(root 通常为 "/",
/// 测试时传临时目录)。返回 `Some(reason)` 表示应禁用,`None` 表示保持默认。
fn should_disable_dmabuf(root: &Path) -> Option<String> {
    // 信号一:NVIDIA 驱动 Driver/library version mismatch。
    // 内核模块版本与用户态库版本对不上时 EGL 初始化会失败(升级驱动后未重启的典型症状)。
    if let Some(kernel_ver) = read_nvrm_version(root) {
        if let Some(lib_ver) = read_nvidia_lib_version(root) {
            if lib_ver != kernel_ver {
                return Some(format!(
                    "NVIDIA 驱动版本不一致(内核模块 {kernel_ver} ≠ 用户态库 {lib_ver}),多为升级后未重启"
                ));
            }
        }
    }

    // 信号二:没有任何可用的 DRI render 节点,说明没有可走的 GPU 渲染路径。
    if !has_render_node(root) {
        return Some("未发现可用的 /dev/dri render 设备,无 GPU 渲染路径".to_string());
    }

    None
}

/// 读取 `/proc/driver/nvidia/version` 中 NVRM 行的内核模块版本号。
fn read_nvrm_version(root: &Path) -> Option<String> {
    let data = fs::read_to_string(root.join("proc/driver/nvidia/version")).ok()?;
    // 形如 "NVRM version: NVIDIA UNIX x86_64 Kernel Module  550.120  ..."
    let idx = data.find("NVRM version:")?;
    extract_version(&data[idx..])
}

/// 在常见库目录下查找 `libGLX_nvidia.so.<ver>` 并抽出版本号。
fn read_nvidia_lib_version(root: &Path) -> Option<String> {
    const LIB_DIRS: [&str; 3] = ["usr/lib/x86_64-linux-gnu", "usr/lib64", "usr/lib"];
    for d in LIB_DIRS {
        let Ok(entries) = fs::read_dir(root.join(d)) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(rest) = name.strip_prefix("libGLX_nvidia.so.") {
                if is_version(rest) {
                    return Some(rest.to_string());
                }
            }
        }
    }
    None
}

/// 判断 `/dev/dri` 下是否存在 `renderD*` 节点。
fn has_render_node(root: &Path) -> bool {
    let Ok(entries) = fs::read_dir(root.join("dev/dri")) else {
        return false;
    };
    entries.flatten().any(|e| {
        e.file_name()
            .to_string_lossy()
            .starts_with("renderD")
    })
}

/// 从一段文本里抽出第一个形如 `<num>.<num>[.<num>]` 的版本号。
fn extract_version(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            let mut dots = 0;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                if bytes[i] == b'.' {
                    // 只在下一位是数字时才把点纳入版本,避免吞掉句尾的点。
                    if i + 1 >= bytes.len() || !bytes[i + 1].is_ascii_digit() {
                        break;
                    }
                    dots += 1;
                }
                i += 1;
            }
            let candidate = &text[start..i];
            if dots >= 1 {
                return Some(candidate.to_string());
            }
        } else {
            i += 1;
        }
    }
    None
}

/// 判断整段字符串是否恰为 `<num>.<num>[.<num>...]` 形式的版本号。
fn is_version(s: &str) -> bool {
    !s.is_empty()
        && s.contains('.')
        && s.bytes().all(|b| b.is_ascii_digit() || b == b'.')
        && !s.starts_with('.')
        && !s.ends_with('.')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(root: &Path, rel: &str, content: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, content).unwrap();
    }

    #[test]
    fn extract_version_basic() {
        assert_eq!(extract_version("NVRM version: ... 550.120  Sun").as_deref(), Some("550.120"));
        assert_eq!(extract_version("Kernel Module  535.183.01  Fri").as_deref(), Some("535.183.01"));
        assert_eq!(extract_version("no digits here").as_deref(), None);
    }

    #[test]
    fn is_version_rules() {
        assert!(is_version("550.120"));
        assert!(is_version("535.183.01"));
        assert!(!is_version("550"));
        assert!(!is_version(""));
        assert!(!is_version(".5"));
        assert!(!is_version("5."));
    }

    #[test]
    fn mismatch_triggers_disable() {
        let dir = tempdir();
        let root = dir.path();
        write(root, "proc/driver/nvidia/version", "NVRM version: NVIDIA UNIX x86_64 Kernel Module  550.120  Sun");
        write(root, "usr/lib/x86_64-linux-gnu/libGLX_nvidia.so.535.183.01", "");
        write(root, "dev/dri/renderD128", "");
        let reason = should_disable_dmabuf(root);
        assert!(reason.is_some());
        assert!(reason.unwrap().contains("版本不一致"));
    }

    #[test]
    fn matching_versions_keep_hardware() {
        let dir = tempdir();
        let root = dir.path();
        write(root, "proc/driver/nvidia/version", "NVRM version: ... 550.120  Sun");
        write(root, "usr/lib/x86_64-linux-gnu/libGLX_nvidia.so.550.120", "");
        write(root, "dev/dri/renderD128", "");
        assert!(should_disable_dmabuf(root).is_none());
    }

    #[test]
    fn no_render_node_triggers_disable() {
        let dir = tempdir();
        let root = dir.path();
        // 无 nvidia、无 /dev/dri
        let reason = should_disable_dmabuf(root);
        assert!(reason.is_some());
        assert!(reason.unwrap().contains("render 设备"));
    }

    // 手动跑:`cargo test --lib gpu_linux::tests::probe_real_root -- --ignored --nocapture`
    // 用真实 "/" 探测本机,打印实际决策,用于人工确认兜底在当前环境是否正确触发。
    #[test]
    #[ignore]
    fn probe_real_root() {
        match should_disable_dmabuf(Path::new("/")) {
            Some(reason) => println!("[probe] 本机会禁用 DMABUF,原因:{reason}"),
            None => println!("[probe] 本机保留硬件加速(不禁用)"),
        }
        println!("[probe] NVRM 内核版本 = {:?}", read_nvrm_version(Path::new("/")));
        println!("[probe] NVIDIA 库版本 = {:?}", read_nvidia_lib_version(Path::new("/")));
        println!("[probe] 有 render 节点 = {}", has_render_node(Path::new("/")));
    }

    #[test]
    fn healthy_non_nvidia_keeps_hardware() {
        let dir = tempdir();
        let root = dir.path();
        write(root, "dev/dri/renderD128", ""); // 有渲染节点、无 nvidia
        assert!(should_disable_dmabuf(root).is_none());
    }

    // 极简 tempdir,避免引入 dev-dependency。
    fn tempdir() -> TempDir {
        let mut base = std::env::temp_dir();
        let uniq = format!(
            "attool-gpu-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        base.push(uniq);
        fs::create_dir_all(&base).unwrap();
        TempDir { path: base }
    }

    struct TempDir {
        path: std::path::PathBuf,
    }
    impl TempDir {
        fn path(&self) -> &Path {
            &self.path
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
