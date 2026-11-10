use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

use chrono::Utc;
use ed25519_dalek::VerifyingKey;
use rusqlite::{params, Connection};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::package::{extract_package, HostEnvironment, PackageError};

#[derive(Debug, Clone)]
pub struct PluginStore {
    root: PathBuf,
    database: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPlugin {
    pub app_id: String,
    pub name: String,
    pub version: String,
    pub publisher_id: String,
    pub publisher_name: String,
    pub publisher_key_id: String,
    pub entry: String,
    pub permissions: Vec<String>,
    pub enabled: bool,
    pub installed_at: String,
    #[serde(skip_serializing)]
    pub files: BTreeMap<String, String>,
    #[serde(skip_serializing)]
    pub root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct PluginAsset {
    pub bytes: Vec<u8>,
    pub content_type: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginHistoryRecord {
    pub id: i64,
    pub app_id: String,
    pub action: String,
    pub version: String,
    pub occurred_at: String,
    pub result: String,
    pub error_code: Option<String>,
    pub steps: Vec<String>,
}

#[derive(Debug)]
pub struct InstallError {
    pub code: String,
    pub message: String,
}

impl InstallError {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl From<PackageError> for InstallError {
    fn from(error: PackageError) -> Self {
        Self::new(error.code, error.message)
    }
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for InstallError {}

impl PluginStore {
    pub fn open(root: PathBuf) -> Result<Self, InstallError> {
        fs::create_dir_all(&root).map_err(|error| {
            InstallError::new(
                "PLUGIN_STORE_OPEN_FAILED",
                format!("Cannot create plugin store: {error}"),
            )
        })?;
        let store = Self {
            database: root.join("registry.sqlite3"),
            root,
        };
        store.initialize()?;
        Ok(store)
    }

    pub fn install(
        &self,
        package_path: &Path,
        environment: &HostEnvironment,
        trusted_keys: &HashMap<String, VerifyingKey>,
    ) -> Result<InstalledPlugin, InstallError> {
        let preview = super::package::inspect_package(package_path, environment, trusted_keys)?;
        if self.find(&preview.app_id)?.is_some() {
            return Err(InstallError::new(
                "PLUGIN_APP_ID_CONFLICT",
                "An application with this appId is already installed",
            ));
        }

        let staging_root = self.root.join("staging").join(Uuid::new_v4().to_string());
        let staged_payload = staging_root.join("payload");
        let package =
            match extract_package(package_path, &staged_payload, environment, trusted_keys) {
                Ok(package) => package,
                Err(error) => {
                    let _ = fs::remove_dir_all(&staging_root);
                    return Err(error.into());
                }
            };
        let staged_entry = staged_payload.join(&package.entry);
        if !staged_entry.is_file() {
            let _ = fs::remove_dir_all(&staging_root);
            return Err(InstallError::new(
                "PLUGIN_HEALTH_CHECK_FAILED",
                "Plugin UI entry is not a readable file",
            ));
        }

        let final_root = self
            .root
            .join("apps")
            .join(&package.app_id)
            .join(&package.version);
        if final_root.exists() {
            let _ = fs::remove_dir_all(&staging_root);
            return Err(InstallError::new(
                "PLUGIN_VERSION_CONFLICT",
                "This plugin version already exists",
            ));
        }
        let final_parent = final_root.parent().expect("version directory has a parent");
        fs::create_dir_all(final_parent).map_err(|error| {
            let _ = fs::remove_dir_all(&staging_root);
            InstallError::new(
                "PLUGIN_ACTIVATION_FAILED",
                format!("Cannot create application directory: {error}"),
            )
        })?;
        fs::rename(&staged_payload, &final_root).map_err(|error| {
            let _ = fs::remove_dir_all(&staging_root);
            InstallError::new(
                "PLUGIN_ACTIVATION_FAILED",
                format!("Cannot activate plugin: {error}"),
            )
        })?;

        let installed_at = Utc::now().to_rfc3339();
        let permissions_json = serde_json::to_string(&package.permissions).map_err(|error| {
            InstallError::new(
                "PLUGIN_REGISTRY_WRITE_FAILED",
                format!("Cannot serialize plugin permissions: {error}"),
            )
        })?;
        let files_json = serde_json::to_string(&package.files).map_err(|error| {
            InstallError::new(
                "PLUGIN_REGISTRY_WRITE_FAILED",
                format!("Cannot serialize plugin files: {error}"),
            )
        })?;
        let write_result = self.connection().and_then(|mut connection| {
            let transaction = connection.transaction().map_err(|error| {
                InstallError::new(
                    "PLUGIN_REGISTRY_WRITE_FAILED",
                    format!("Cannot start plugin registry transaction: {error}"),
                )
            })?;
            transaction
                .execute(
                    "INSERT INTO installed_plugins (
                        app_id, name, version, publisher_id, publisher_name, publisher_key_id,
                        entry, permissions_json, files_json, enabled, installed_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, ?10)",
                    params![
                        package.app_id,
                        package.name,
                        package.version,
                        package.publisher_id,
                        package.publisher_name,
                        package.publisher_key_id,
                        package.entry,
                        permissions_json,
                        files_json,
                        installed_at,
                    ],
                )
                .map_err(|error| {
                    InstallError::new(
                        "PLUGIN_REGISTRY_WRITE_FAILED",
                        format!("Cannot update plugin registry: {error}"),
                    )
                })?;
            insert_history(
                &transaction,
                &package.app_id,
                "install",
                &package.version,
                &installed_at,
                &[
                    "signature",
                    "hashes",
                    "target",
                    "hostApi",
                    "staging",
                    "health",
                    "activation",
                ],
            )?;
            transaction.commit().map_err(|error| {
                InstallError::new(
                    "PLUGIN_REGISTRY_WRITE_FAILED",
                    format!("Cannot commit plugin registry: {error}"),
                )
            })?;
            Ok(())
        });
        if let Err(error) = write_result {
            let _ = fs::remove_dir_all(&final_root);
            let _ = fs::remove_dir_all(&staging_root);
            return Err(error);
        }
        let _ = fs::remove_dir_all(&staging_root);

        self.find(&preview.app_id)?.ok_or_else(|| {
            InstallError::new(
                "PLUGIN_REGISTRY_WRITE_FAILED",
                "Installed plugin is missing from registry",
            )
        })
    }

    pub fn list(&self) -> Result<Vec<InstalledPlugin>, InstallError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT app_id, name, version, publisher_id, publisher_name, publisher_key_id,
                        entry, permissions_json, files_json, enabled, installed_at
                 FROM installed_plugins ORDER BY name, app_id",
            )
            .map_err(registry_read_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, bool>(9)?,
                    row.get::<_, String>(10)?,
                ))
            })
            .map_err(registry_read_error)?;
        let mut plugins = Vec::new();
        for row in rows {
            let (
                app_id,
                name,
                version,
                publisher_id,
                publisher_name,
                publisher_key_id,
                entry,
                permissions_json,
                files_json,
                enabled,
                installed_at,
            ) = row.map_err(registry_read_error)?;
            let permissions = serde_json::from_str(&permissions_json).map_err(|error| {
                InstallError::new(
                    "PLUGIN_REGISTRY_READ_FAILED",
                    format!("Plugin permissions are corrupted: {error}"),
                )
            })?;
            let files = serde_json::from_str(&files_json).map_err(|error| {
                InstallError::new(
                    "PLUGIN_REGISTRY_READ_FAILED",
                    format!("Plugin file declarations are corrupted: {error}"),
                )
            })?;
            let plugin_root = self.root.join("apps").join(&app_id).join(&version);
            plugins.push(InstalledPlugin {
                app_id,
                name,
                version,
                publisher_id,
                publisher_name,
                publisher_key_id,
                entry,
                permissions,
                enabled,
                installed_at,
                files,
                root: plugin_root,
            });
        }
        Ok(plugins)
    }

    pub fn list_enabled(&self) -> Result<Vec<InstalledPlugin>, InstallError> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|plugin| plugin.enabled)
            .collect())
    }

    pub fn read_asset(&self, app_id: &str, relative: &str) -> Result<PluginAsset, InstallError> {
        if !is_safe_relative_path(relative) {
            return Err(InstallError::new(
                "PLUGIN_ASSET_PATH_INVALID",
                "Plugin asset path is invalid",
            ));
        }
        let plugin = self.find(app_id)?.ok_or_else(|| {
            InstallError::new("PLUGIN_NOT_FOUND", "Installed plugin was not found")
        })?;
        if !plugin.enabled {
            return Err(InstallError::new(
                "PLUGIN_DISABLED",
                "Plugin is not enabled",
            ));
        }
        let expected_hash = plugin.files.get(relative).ok_or_else(|| {
            InstallError::new(
                "PLUGIN_ASSET_NOT_DECLARED",
                "Plugin asset is not declared in the signed manifest",
            )
        })?;
        let path = plugin.root.join(relative);
        let bytes = fs::read(path).map_err(|_| {
            InstallError::new("PLUGIN_ASSET_NOT_FOUND", "Plugin asset was not found")
        })?;
        if hex::encode(Sha256::digest(&bytes)) != *expected_hash {
            return Err(InstallError::new(
                "PLUGIN_ASSET_HASH_MISMATCH",
                "Plugin asset hash does not match the signed manifest",
            ));
        }
        Ok(PluginAsset {
            bytes,
            content_type: content_type(relative),
        })
    }

    pub fn set_enabled(&self, app_id: &str, enabled: bool) -> Result<(), InstallError> {
        let plugin = self.find(app_id)?.ok_or_else(|| {
            InstallError::new("PLUGIN_NOT_FOUND", "Installed plugin was not found")
        })?;
        if enabled {
            let expected_hash = plugin.files.get(&plugin.entry).ok_or_else(|| {
                InstallError::new(
                    "PLUGIN_HEALTH_CHECK_FAILED",
                    "Plugin UI entry is not declared",
                )
            })?;
            let entry = fs::read(plugin.root.join(&plugin.entry)).map_err(|_| {
                InstallError::new(
                    "PLUGIN_HEALTH_CHECK_FAILED",
                    "Plugin UI entry is not a readable file",
                )
            })?;
            if hex::encode(Sha256::digest(&entry)) != *expected_hash {
                return Err(InstallError::new(
                    "PLUGIN_HEALTH_CHECK_FAILED",
                    "Plugin UI entry failed integrity verification",
                ));
            }
        }
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(|error| {
            InstallError::new(
                "PLUGIN_REGISTRY_WRITE_FAILED",
                format!("Cannot start plugin state transaction: {error}"),
            )
        })?;
        transaction
            .execute(
                "UPDATE installed_plugins SET enabled = ?1 WHERE app_id = ?2",
                params![enabled, app_id],
            )
            .map_err(|error| {
                InstallError::new(
                    "PLUGIN_REGISTRY_WRITE_FAILED",
                    format!("Cannot update plugin state: {error}"),
                )
            })?;
        let action = if enabled { "enable" } else { "disable" };
        insert_history(
            &transaction,
            app_id,
            action,
            &plugin.version,
            &Utc::now().to_rfc3339(),
            if enabled {
                &["health", "registry"]
            } else {
                &["registry"]
            },
        )?;
        transaction.commit().map_err(|error| {
            InstallError::new(
                "PLUGIN_REGISTRY_WRITE_FAILED",
                format!("Cannot commit plugin state: {error}"),
            )
        })?;
        Ok(())
    }

    pub fn uninstall(&self, app_id: &str, delete_data: bool) -> Result<(), InstallError> {
        let plugin = self.find(app_id)?.ok_or_else(|| {
            InstallError::new("PLUGIN_NOT_FOUND", "Installed plugin was not found")
        })?;
        let removal_root = self
            .root
            .join("staging")
            .join(format!("uninstall-{}", Uuid::new_v4()));
        let removed_package = removal_root.join("package");
        let removed_data = removal_root.join("data");
        fs::create_dir_all(&removal_root).map_err(|error| {
            InstallError::new(
                "PLUGIN_UNINSTALL_FAILED",
                format!("Cannot prepare uninstall staging: {error}"),
            )
        })?;
        fs::rename(&plugin.root, &removed_package).map_err(|error| {
            let _ = fs::remove_dir_all(&removal_root);
            InstallError::new(
                "PLUGIN_UNINSTALL_FAILED",
                format!("Cannot stage plugin removal: {error}"),
            )
        })?;

        let data_root = self.data_root(app_id);
        let data_was_staged = delete_data && data_root.exists();
        if data_was_staged {
            if let Err(error) = fs::rename(&data_root, &removed_data) {
                let _ = fs::rename(&removed_package, &plugin.root);
                let _ = fs::remove_dir_all(&removal_root);
                return Err(InstallError::new(
                    "PLUGIN_DATA_DELETE_FAILED",
                    format!("Cannot stage plugin data removal: {error}"),
                ));
            }
        }

        let delete_result = self.connection().and_then(|mut connection| {
            let transaction = connection.transaction().map_err(|error| {
                InstallError::new(
                    "PLUGIN_REGISTRY_WRITE_FAILED",
                    format!("Cannot start plugin uninstall transaction: {error}"),
                )
            })?;
            transaction
                .execute(
                    "DELETE FROM installed_plugins WHERE app_id = ?1",
                    params![app_id],
                )
                .map_err(|error| {
                    InstallError::new(
                        "PLUGIN_REGISTRY_WRITE_FAILED",
                        format!("Cannot remove plugin from registry: {error}"),
                    )
                })?;
            insert_history(
                &transaction,
                app_id,
                "uninstall",
                &plugin.version,
                &Utc::now().to_rfc3339(),
                if delete_data {
                    &["package", "registry", "data"]
                } else {
                    &["package", "registry"]
                },
            )?;
            transaction.commit().map_err(|error| {
                InstallError::new(
                    "PLUGIN_REGISTRY_WRITE_FAILED",
                    format!("Cannot commit plugin uninstall: {error}"),
                )
            })?;
            Ok(())
        });
        if let Err(error) = delete_result {
            if data_was_staged {
                let _ = fs::rename(&removed_data, &data_root);
            }
            let _ = fs::rename(&removed_package, &plugin.root);
            let _ = fs::remove_dir_all(&removal_root);
            return Err(error);
        }

        let _ = fs::remove_dir_all(&removal_root);
        if let Some(app_root) = plugin.root.parent() {
            let _ = fs::remove_dir(app_root);
        }
        Ok(())
    }

    pub fn history(&self) -> Result<Vec<PluginHistoryRecord>, InstallError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT id, app_id, action, version, occurred_at, result, error_code, steps_json
                 FROM plugin_history ORDER BY id DESC",
            )
            .map_err(registry_read_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, String>(7)?,
                ))
            })
            .map_err(registry_read_error)?;
        let mut history = Vec::new();
        for row in rows {
            let (id, app_id, action, version, occurred_at, result, error_code, steps_json) =
                row.map_err(registry_read_error)?;
            let steps = serde_json::from_str(&steps_json).map_err(|error| {
                InstallError::new(
                    "PLUGIN_REGISTRY_READ_FAILED",
                    format!("Plugin history steps are corrupted: {error}"),
                )
            })?;
            history.push(PluginHistoryRecord {
                id,
                app_id,
                action,
                version,
                occurred_at,
                result,
                error_code,
                steps,
            });
        }
        Ok(history)
    }

    fn find(&self, app_id: &str) -> Result<Option<InstalledPlugin>, InstallError> {
        Ok(self
            .list()?
            .into_iter()
            .find(|plugin| plugin.app_id == app_id))
    }

    fn data_root(&self, app_id: &str) -> PathBuf {
        self.root.join("data").join(app_id)
    }

    fn initialize(&self) -> Result<(), InstallError> {
        let connection = self.connection()?;
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS installed_plugins (
                    app_id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    version TEXT NOT NULL,
                    publisher_id TEXT NOT NULL,
                    publisher_name TEXT NOT NULL,
                    publisher_key_id TEXT NOT NULL,
                    entry TEXT NOT NULL,
                    permissions_json TEXT NOT NULL,
                    files_json TEXT NOT NULL DEFAULT '{}',
                    enabled INTEGER NOT NULL,
                    installed_at TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS plugin_history (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    app_id TEXT NOT NULL,
                    action TEXT NOT NULL,
                    version TEXT NOT NULL,
                    occurred_at TEXT NOT NULL,
                    result TEXT NOT NULL,
                    error_code TEXT,
                    steps_json TEXT NOT NULL
                );",
            )
            .map_err(|error| {
                InstallError::new(
                    "PLUGIN_STORE_OPEN_FAILED",
                    format!("Cannot initialize plugin registry: {error}"),
                )
            })?;
        let has_files_column: bool = connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM pragma_table_info('installed_plugins') WHERE name = 'files_json'
                )",
                [],
                |row| row.get(0),
            )
            .map_err(registry_read_error)?;
        if !has_files_column {
            connection
                .execute(
                    "ALTER TABLE installed_plugins
                     ADD COLUMN files_json TEXT NOT NULL DEFAULT '{}'",
                    [],
                )
                .map_err(|error| {
                    InstallError::new(
                        "PLUGIN_STORE_OPEN_FAILED",
                        format!("Cannot migrate plugin registry: {error}"),
                    )
                })?;
        }
        Ok(())
    }

    fn connection(&self) -> Result<Connection, InstallError> {
        Connection::open(&self.database).map_err(|error| {
            InstallError::new(
                "PLUGIN_STORE_OPEN_FAILED",
                format!("Cannot open plugin registry: {error}"),
            )
        })
    }
}

fn is_safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

fn content_type(path: &str) -> &'static str {
    match Path::new(path).extension().and_then(|value| value.to_str()) {
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") | Some("map") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("wasm") => "application/wasm",
        Some("txt") | Some("md") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn insert_history(
    transaction: &rusqlite::Transaction<'_>,
    app_id: &str,
    action: &str,
    version: &str,
    occurred_at: &str,
    steps: &[&str],
) -> Result<(), InstallError> {
    let steps_json = serde_json::to_string(steps).map_err(|error| {
        InstallError::new(
            "PLUGIN_REGISTRY_WRITE_FAILED",
            format!("Cannot serialize plugin history: {error}"),
        )
    })?;
    transaction
        .execute(
            "INSERT INTO plugin_history (
                app_id, action, version, occurred_at, result, error_code, steps_json
             ) VALUES (?1, ?2, ?3, ?4, 'success', NULL, ?5)",
            params![app_id, action, version, occurred_at, steps_json],
        )
        .map_err(|error| {
            InstallError::new(
                "PLUGIN_REGISTRY_WRITE_FAILED",
                format!("Cannot write plugin history: {error}"),
            )
        })?;
    Ok(())
}

fn registry_read_error(error: rusqlite::Error) -> InstallError {
    InstallError::new(
        "PLUGIN_REGISTRY_READ_FAILED",
        format!("Cannot read plugin registry: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, fs::File, io::Write};

    use base64::Engine;
    use ed25519_dalek::{Signer, SigningKey};
    use sha2::{Digest, Sha256};
    use zip::{write::SimpleFileOptions, ZipWriter};

    use super::PluginStore;
    use crate::plugin::package::HostEnvironment;

    const TEST_KEY_SEED: [u8; 32] = [
        0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c,
        0xc4, 0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae,
        0x7f, 0x60,
    ];

    fn sha256(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    fn valid_package(directory: &std::path::Path) -> (std::path::PathBuf, SigningKey) {
        let path = directory.join("codec.atpkg");
        let signing_key = SigningKey::from_bytes(&TEST_KEY_SEED);
        let license = b"P0 test license";
        let index = b"<!doctype html><title>Codec</title>";
        let manifest = format!(
            concat!(
                "{{\"appId\":\"com.attson.codec\",",
                "\"files\":{{\"LICENSE\":\"{}\",\"ui/index.html\":\"{}\"}},",
                "\"hostApi\":{{\"max\":\"1.x\",\"min\":\"1.0.0\"}},",
                "\"name\":\"编码转换\",",
                "\"permissions\":[\"host.theme.read\"],",
                "\"publisher\":{{\"id\":\"com.attson\",\"keyId\":\"attool-p0-test\",\"name\":\"Attson\"}},",
                "\"schemaVersion\":1,",
                "\"target\":{{\"arch\":\"{}\",\"os\":\"{}\"}},",
                "\"ui\":{{\"entry\":\"ui/index.html\"}},",
                "\"version\":\"0.1.0-alpha.0\"}}"
            ),
            sha256(license),
            sha256(index),
            std::env::consts::ARCH,
            std::env::consts::OS
        );
        let signature = base64::engine::general_purpose::STANDARD
            .encode(signing_key.sign(manifest.as_bytes()).to_bytes());
        let mut archive = ZipWriter::new(File::create(&path).unwrap());
        for (name, bytes) in [
            ("manifest.json", manifest.as_bytes()),
            ("manifest.sig", signature.as_bytes()),
            ("LICENSE", license.as_slice()),
            ("ui/index.html", index.as_slice()),
        ] {
            archive
                .start_file(name, SimpleFileOptions::default())
                .unwrap();
            archive.write_all(bytes).unwrap();
        }
        archive.finish().unwrap();
        (path, signing_key)
    }

    #[test]
    fn installs_and_persists_a_verified_plugin() {
        let directory = tempfile::tempdir().unwrap();
        let (package_path, signing_key) = valid_package(directory.path());
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);
        let environment = HostEnvironment::current("1.0.0");
        let store = PluginStore::open(directory.path().join("plugins")).unwrap();

        let installed = store.install(&package_path, &environment, &keys).unwrap();

        assert_eq!(installed.app_id, "com.attson.codec");
        assert!(installed.enabled);
        assert_eq!(
            std::fs::read_to_string(installed.root.join("ui/index.html")).unwrap(),
            "<!doctype html><title>Codec</title>"
        );
        drop(store);

        let reopened = PluginStore::open(directory.path().join("plugins")).unwrap();
        let apps = reopened.list().unwrap();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].app_id, "com.attson.codec");
        assert!(apps[0].enabled);
    }

    #[test]
    fn disables_and_reenables_an_installed_plugin() {
        let directory = tempfile::tempdir().unwrap();
        let (package_path, signing_key) = valid_package(directory.path());
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);
        let store = PluginStore::open(directory.path().join("plugins")).unwrap();
        store
            .install(&package_path, &HostEnvironment::current("1.0.0"), &keys)
            .unwrap();

        store.set_enabled("com.attson.codec", false).unwrap();
        assert!(!store.list().unwrap()[0].enabled);
        assert!(store.list_enabled().unwrap().is_empty());

        store.set_enabled("com.attson.codec", true).unwrap();
        assert!(store.list().unwrap()[0].enabled);
        assert_eq!(store.list_enabled().unwrap().len(), 1);
    }

    #[test]
    fn serves_only_enabled_declared_assets_with_matching_hashes() {
        let directory = tempfile::tempdir().unwrap();
        let (package_path, signing_key) = valid_package(directory.path());
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);
        let store = PluginStore::open(directory.path().join("plugins")).unwrap();
        let installed = store
            .install(&package_path, &HostEnvironment::current("1.0.0"), &keys)
            .unwrap();

        let asset = store
            .read_asset("com.attson.codec", "ui/index.html")
            .unwrap();
        assert_eq!(asset.bytes, b"<!doctype html><title>Codec</title>");
        assert_eq!(asset.content_type, "text/html; charset=utf-8");

        let undeclared = store
            .read_asset("com.attson.codec", "ui/other.js")
            .expect_err("undeclared asset must be rejected");
        assert_eq!(undeclared.code, "PLUGIN_ASSET_NOT_DECLARED");

        store.set_enabled("com.attson.codec", false).unwrap();
        let disabled = store
            .read_asset("com.attson.codec", "ui/index.html")
            .expect_err("disabled plugin must not be served");
        assert_eq!(disabled.code, "PLUGIN_DISABLED");

        store.set_enabled("com.attson.codec", true).unwrap();
        std::fs::write(installed.root.join("ui/index.html"), "modified").unwrap();
        let modified = store
            .read_asset("com.attson.codec", "ui/index.html")
            .expect_err("modified asset must not be served");
        assert_eq!(modified.code, "PLUGIN_ASSET_HASH_MISMATCH");
    }

    #[test]
    fn keeps_a_plugin_disabled_when_its_entry_hash_is_invalid() {
        let directory = tempfile::tempdir().unwrap();
        let (package_path, signing_key) = valid_package(directory.path());
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);
        let store = PluginStore::open(directory.path().join("plugins")).unwrap();
        let installed = store
            .install(&package_path, &HostEnvironment::current("1.0.0"), &keys)
            .unwrap();
        store.set_enabled("com.attson.codec", false).unwrap();
        std::fs::write(installed.root.join("ui/index.html"), "modified").unwrap();

        let error = store
            .set_enabled("com.attson.codec", true)
            .expect_err("a damaged plugin must not be reenabled");

        assert_eq!(error.code, "PLUGIN_HEALTH_CHECK_FAILED");
        assert!(!store.list().unwrap()[0].enabled);
    }

    #[test]
    fn uninstall_preserves_data_unless_deletion_is_explicit() {
        let directory = tempfile::tempdir().unwrap();
        let (package_path, signing_key) = valid_package(directory.path());
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);
        let store = PluginStore::open(directory.path().join("plugins")).unwrap();
        let environment = HostEnvironment::current("1.0.0");
        let installed = store.install(&package_path, &environment, &keys).unwrap();
        let data_root = store.data_root("com.attson.codec");
        std::fs::create_dir_all(&data_root).unwrap();
        std::fs::write(data_root.join("saved.txt"), "keep me").unwrap();

        store.uninstall("com.attson.codec", false).unwrap();

        assert!(store.list().unwrap().is_empty());
        assert!(!installed.root.exists());
        assert_eq!(
            std::fs::read_to_string(data_root.join("saved.txt")).unwrap(),
            "keep me"
        );

        store.install(&package_path, &environment, &keys).unwrap();
        store.uninstall("com.attson.codec", true).unwrap();
        assert!(!data_root.exists());
    }

    #[test]
    fn rolls_back_activation_when_registry_write_fails() {
        let directory = tempfile::tempdir().unwrap();
        let (package_path, signing_key) = valid_package(directory.path());
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);
        let store_root = directory.path().join("plugins");
        let store = PluginStore::open(store_root.clone()).unwrap();
        store
            .connection()
            .unwrap()
            .execute_batch(
                "CREATE TRIGGER reject_plugin_insert
                 BEFORE INSERT ON installed_plugins
                 BEGIN SELECT RAISE(ABORT, 'forced failure'); END;",
            )
            .unwrap();

        let error = store
            .install(&package_path, &HostEnvironment::current("1.0.0"), &keys)
            .expect_err("registry write must fail");

        assert_eq!(error.code, "PLUGIN_REGISTRY_WRITE_FAILED");
        assert!(store.list().unwrap().is_empty());
        assert!(!store_root
            .join("apps/com.attson.codec/0.1.0-alpha.0")
            .exists());
        let staging_is_empty = std::fs::read_dir(store_root.join("staging"))
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(true);
        assert!(staging_is_empty);
    }

    #[test]
    fn persists_plugin_management_history() {
        let directory = tempfile::tempdir().unwrap();
        let (package_path, signing_key) = valid_package(directory.path());
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);
        let store_root = directory.path().join("plugins");
        let store = PluginStore::open(store_root.clone()).unwrap();
        let environment = HostEnvironment::current("1.0.0");

        store.install(&package_path, &environment, &keys).unwrap();
        store.set_enabled("com.attson.codec", false).unwrap();
        store.set_enabled("com.attson.codec", true).unwrap();
        store.uninstall("com.attson.codec", false).unwrap();
        drop(store);

        let reopened = PluginStore::open(store_root).unwrap();
        let actions = reopened
            .history()
            .unwrap()
            .into_iter()
            .map(|record| record.action)
            .collect::<Vec<_>>();
        assert_eq!(actions, vec!["uninstall", "enable", "disable", "install"]);
    }
}
