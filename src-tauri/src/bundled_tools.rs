#![cfg_attr(debug_assertions, allow(dead_code))]

use crate::embedded_runtime::{materialize, EmbeddedFile, EmbeddedRuntime, MaterializedRuntime};

include!(concat!(env!("OUT_DIR"), "/bundled_tools.rs"));

fn cache_root() -> std::path::PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("attool")
        .join("runtime")
}

pub fn tesseract() -> Result<MaterializedRuntime, String> {
    materialize(&cache_root(), &TESSERACT)
}

pub fn aria2() -> Result<MaterializedRuntime, String> {
    materialize(&cache_root(), &ARIA2)
}

pub fn quickjs() -> Result<MaterializedRuntime, String> {
    materialize(&cache_root(), &QUICKJS)
}

pub fn notices() -> Result<MaterializedRuntime, String> {
    materialize(&cache_root(), &NOTICES)
}

#[cfg(target_os = "linux")]
pub fn gstreamer() -> Result<MaterializedRuntime, String> {
    materialize(&cache_root(), &GSTREAMER)
}
