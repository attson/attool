use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    // Cargo:rustc-env is single-line; write PEM to $OUT_DIR/verify_public_key.pem
    // and include_str!() it from the main crate. This preserves the multi-line
    // PEM as-is (empty file → updater disabled).
    let pk = std::env::var("ATTOOL_UPDATE_VERIFY_PUBLIC_KEY").unwrap_or_default();
    let out =
        PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("verify_public_key.pem");
    fs::write(&out, pk.as_bytes()).expect("write verify_public_key.pem");
    println!("cargo:rerun-if-env-changed=ATTOOL_UPDATE_VERIFY_PUBLIC_KEY");

    let target = std::env::var("TARGET").expect("TARGET");
    let extension = if target.contains("windows") {
        ".exe"
    } else {
        ""
    };
    let bridge = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
        .join("binaries")
        .join(format!("psd-bridge-{target}{extension}"));
    println!("cargo:rerun-if-changed={}", bridge.display());
    println!(
        "cargo:rustc-env=ATTOOL_PSD_BRIDGE_PATH={}",
        bridge.display()
    );

    let runtime_root =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join("runtime")
            .join(&target);
    println!("cargo:rerun-if-changed={}", runtime_root.display());
    generate_bundled_tools(&runtime_root);

    tauri_build::build()
}

fn generate_bundled_tools(runtime_root: &Path) {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let destination = out_dir.join("bundled_tools.rs");
    let release = std::env::var("PROFILE").as_deref() == Ok("release");
    if release && !runtime_root.is_dir() {
        panic!(
            "bundled runtime is missing for {}; run pnpm build:runtime",
            runtime_root.display()
        );
    }

    let mut generated = String::new();
    let mut runtimes = vec![
        ("TESSERACT", "tesseract", executable_path("tesseract")),
        ("ARIA2", "aria2", executable_path("aria2c")),
        ("QUICKJS", "quickjs", executable_path("qjs")),
        ("NOTICES", "notices", "THIRD_PARTY_NOTICES.md".to_string()),
    ];
    if std::env::var("TARGET")
        .map(|target| target.contains("linux"))
        .unwrap_or(false)
    {
        runtimes.push(("GSTREAMER", "gstreamer", "plugins".to_string()));
    }
    for (constant, directory, entrypoint) in runtimes {
        let root = runtime_root.join(directory);
        let version =
            fs::read_to_string(root.join("VERSION")).unwrap_or_else(|_| "development".to_string());
        let version = version.trim();
        let mut files = Vec::new();
        collect_files(&root, &root, &mut files);
        files.retain(|(relative, _)| relative != "VERSION");
        files.sort_by(|left, right| left.0.cmp(&right.0));
        if release && (files.is_empty() || !root.join(&entrypoint).exists()) {
            panic!(
                "bundled {directory} runtime is incomplete in {}",
                root.display()
            );
        }
        generated.push_str(&format!(
            "pub static {constant}_FILES: &[EmbeddedFile] = &[\n"
        ));
        for (relative, path) in files {
            let executable = relative.starts_with("bin/");
            generated.push_str(&format!(
                "    EmbeddedFile {{ relative_path: {relative:?}, bytes: include_bytes!({path:?}), executable: {executable} }},\n",
                path = path.to_string_lossy()
            ));
        }
        generated.push_str("];\n");
        generated.push_str(&format!(
            "pub static {constant}: EmbeddedRuntime = EmbeddedRuntime {{ name: {directory:?}, version: {version:?}, entrypoint: {entrypoint:?}, files: {constant}_FILES }};\n"
        ));
    }
    fs::write(destination, generated).expect("write bundled_tools.rs");
}

fn executable_path(name: &str) -> String {
    if std::env::var("TARGET")
        .map(|target| target.contains("windows"))
        .unwrap_or(false)
    {
        format!("bin/{name}.exe")
    } else {
        format!("bin/{name}")
    }
}

fn collect_files(root: &Path, directory: &Path, files: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, files);
        } else if path.is_file() {
            let relative = path
                .strip_prefix(root)
                .expect("runtime path is under root")
                .to_string_lossy()
                .replace('\\', "/");
            files.push((relative, path));
        }
    }
}
