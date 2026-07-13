fn main() {
    let pk = std::env::var("ATTOOL_UPDATE_VERIFY_PUBLIC_KEY").unwrap_or_default();
    println!("cargo:rustc-env=ATTOOL_UPDATE_VERIFY_PUBLIC_KEY={pk}");
    println!("cargo:rerun-if-env-changed=ATTOOL_UPDATE_VERIFY_PUBLIC_KEY");
    tauri_build::build()
}
