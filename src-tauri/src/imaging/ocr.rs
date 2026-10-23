use std::path::{Path, PathBuf};
use std::process::Command;

struct TesseractRuntime {
    program: PathBuf,
    tessdata: Option<PathBuf>,
}

fn run_with_runtime(
    input: &Path,
    langs: &str,
    runtime: &TesseractRuntime,
) -> Result<String, String> {
    if !matches!(
        langs,
        "eng+chi_sim" | "eng+chi_tra" | "chi_sim" | "eng" | "eng+jpn"
    ) {
        return Err(format!("不支持的 OCR 语言组合：{langs}"));
    }
    let mut command = Command::new(&runtime.program);
    command.arg(input).arg("-").arg("-l").arg(langs);
    if let Some(tessdata) = &runtime.tessdata {
        command.arg("--tessdata-dir").arg(tessdata);
    }
    let output = command.output().map_err(|error| {
        format!(
            "AT Tool 内置 OCR 运行时启动失败（{}）：{error}",
            runtime.program.display()
        )
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("tesseract 识别失败：{stderr}"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Run Tesseract on the given image and return recognized text.
pub fn run_tesseract(input: &Path, langs: &str) -> Result<String, String> {
    if !input.is_file() {
        return Err(format!("图片文件不存在：{}", input.display()));
    }

    #[cfg(not(debug_assertions))]
    let runtime = {
        let bundled = crate::bundled_tools::tesseract()?;
        TesseractRuntime {
            tessdata: Some(bundled.root.join("tessdata")),
            program: bundled.entrypoint,
        }
    };

    #[cfg(debug_assertions)]
    let runtime = TesseractRuntime {
        program: std::env::var_os("ATTOOL_TESSERACT_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("tesseract")),
        tessdata: std::env::var_os("ATTOOL_TESSDATA_PREFIX").map(PathBuf::from),
    };

    run_with_runtime(input, langs, &runtime)
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::{run_with_runtime, TesseractRuntime};

    #[test]
    fn bundled_tesseract_uses_its_own_language_models() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input.png");
        let tessdata = directory.path().join("tessdata");
        let program = directory.path().join("tesseract");
        std::fs::write(&input, b"image").unwrap();
        std::fs::create_dir_all(&tessdata).unwrap();
        std::fs::write(
            &program,
            format!(
                r#"#!/bin/sh
[ "$1" = "{}" ] || exit 42
[ "$2" = "-" ] || exit 43
[ "$3" = "-l" ] || exit 44
[ "$4" = "eng+chi_sim" ] || exit 45
[ "$5" = "--tessdata-dir" ] || exit 46
[ "$6" = "{}" ] || exit 47
printf '安装即用'
"#,
                input.display(),
                tessdata.display()
            ),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&program).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&program, permissions).unwrap();

        let text = run_with_runtime(
            &input,
            "eng+chi_sim",
            &TesseractRuntime {
                program,
                tessdata: Some(tessdata),
            },
        )
        .unwrap();

        assert_eq!(text, "安装即用");
    }

    #[test]
    fn rejects_language_models_that_are_not_shipped_with_the_app() {
        let runtime = TesseractRuntime {
            program: "tesseract".into(),
            tessdata: None,
        };

        let error =
            run_with_runtime(std::path::Path::new("unused.png"), "eng+kor", &runtime).unwrap_err();

        assert_eq!(error, "不支持的 OCR 语言组合：eng+kor");
    }
}
