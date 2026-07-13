use std::fs;
use std::path::PathBuf;

fn main() {
    // Cargo:rustc-env is single-line; write PEM to $OUT_DIR/verify_public_key.pem
    // and include_str!() it from the main crate. This preserves the multi-line
    // PEM as-is (empty file → updater disabled).
    let pk = std::env::var("ATTOOL_UPDATE_VERIFY_PUBLIC_KEY").unwrap_or_default();
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"))
        .join("verify_public_key.pem");
    fs::write(&out, pk.as_bytes()).expect("write verify_public_key.pem");
    println!("cargo:rerun-if-env-changed=ATTOOL_UPDATE_VERIFY_PUBLIC_KEY");
    tauri_build::build()
}
