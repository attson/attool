use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub struct EmbeddedFile {
    pub relative_path: &'static str,
    pub bytes: &'static [u8],
    pub executable: bool,
}

pub struct EmbeddedRuntime {
    pub name: &'static str,
    pub version: &'static str,
    pub entrypoint: &'static str,
    pub files: &'static [EmbeddedFile],
}

#[cfg_attr(debug_assertions, allow(dead_code))]
pub struct MaterializedRuntime {
    pub root: PathBuf,
    pub entrypoint: PathBuf,
}

pub fn materialize(
    cache_root: &Path,
    runtime: &EmbeddedRuntime,
) -> Result<MaterializedRuntime, String> {
    let root = cache_root.join(runtime.name).join(runtime.version);
    for file in runtime.files {
        let destination = root.join(file.relative_path);
        if file_matches(&destination, file.bytes) {
            set_executable(&destination, file.executable)?;
            continue;
        }
        let parent = destination
            .parent()
            .ok_or_else(|| format!("运行时路径无效：{}", destination.display()))?;
        std::fs::create_dir_all(parent).map_err(|error| format!("创建运行时目录失败：{error}"))?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)
            .map_err(|error| format!("创建运行时临时文件失败：{error}"))?;
        temporary
            .write_all(file.bytes)
            .map_err(|error| format!("写入运行时文件失败：{error}"))?;
        set_executable(temporary.path(), file.executable)?;
        if let Err(error) = temporary.persist(&destination) {
            if !file_matches(&destination, file.bytes) {
                return Err(format!(
                    "安装运行时文件失败（{}）：{}",
                    destination.display(),
                    error.error
                ));
            }
        }
    }

    let entrypoint = root.join(runtime.entrypoint);
    if !entrypoint.exists() {
        return Err(format!(
            "AT Tool 内置运行时损坏（{}）：缺少 {}",
            runtime.name, runtime.entrypoint
        ));
    }
    cleanup_old_versions(&root);
    Ok(MaterializedRuntime { root, entrypoint })
}

fn cleanup_old_versions(current: &Path) {
    let Some(parent) = current.parent() else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path != current && path.is_dir() {
            std::fs::remove_dir_all(path).ok();
        }
    }
}

fn file_matches(path: &Path, expected: &[u8]) -> bool {
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    if file
        .metadata()
        .map(|metadata| metadata.len() != expected.len() as u64)
        .unwrap_or(true)
    {
        return false;
    }
    let mut actual = Vec::with_capacity(expected.len());
    file.read_to_end(&mut actual).is_ok() && actual == expected
}

fn set_executable(path: &Path, executable: bool) -> Result<(), String> {
    #[cfg(unix)]
    if executable {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path)
            .map_err(|error| format!("读取运行时文件权限失败：{error}"))?
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions)
            .map_err(|error| format!("设置运行时文件权限失败：{error}"))?;
    }
    #[cfg(not(unix))]
    let _ = executable;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{materialize, EmbeddedFile, EmbeddedRuntime};

    static FILES: &[EmbeddedFile] = &[
        EmbeddedFile {
            relative_path: "bin/tool",
            bytes: b"runtime executable",
            executable: true,
        },
        EmbeddedFile {
            relative_path: "share/model.data",
            bytes: b"runtime model",
            executable: false,
        },
    ];

    static RUNTIME: EmbeddedRuntime = EmbeddedRuntime {
        name: "sample",
        version: "1.2.3",
        entrypoint: "bin/tool",
        files: FILES,
    };

    #[test]
    fn repairs_a_corrupt_runtime_cache_before_returning_the_entrypoint() {
        let cache = tempfile::tempdir().unwrap();
        let corrupt = cache.path().join("sample/1.2.3/bin/tool");
        let old = cache.path().join("sample/1.1.0/bin/tool");
        std::fs::create_dir_all(corrupt.parent().unwrap()).unwrap();
        std::fs::create_dir_all(old.parent().unwrap()).unwrap();
        std::fs::write(&corrupt, b"corrupt").unwrap();
        std::fs::write(&old, b"old").unwrap();

        let materialized = materialize(cache.path(), &RUNTIME).unwrap();

        assert_eq!(materialized.root, cache.path().join("sample/1.2.3"));
        assert_eq!(materialized.entrypoint, corrupt);
        assert_eq!(
            std::fs::read(&materialized.entrypoint).unwrap(),
            b"runtime executable"
        );
        assert_eq!(
            std::fs::read(materialized.root.join("share/model.data")).unwrap(),
            b"runtime model"
        );
        assert!(!old.exists());
    }
}
