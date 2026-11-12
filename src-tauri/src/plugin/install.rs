use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

use chrono::Utc;
use ed25519_dalek::VerifyingKey;
use rusqlite::{params, Connection, OptionalExtension, Row};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{
    package::{extract_package, HostEnvironment, NativeService, PackageError},
    remote_install::{OperationSnapshot, OperationStage},
};

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
    pub native_service: Option<NativeService>,
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

#[derive(Debug)]
pub struct PreparedPlugin {
    pub package: super::package::InspectedPackage,
    staging_root: PathBuf,
    payload_root: PathBuf,
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
    pub operation_id: Option<String>,
    pub correlation_id: Option<String>,
    pub steps: Vec<String>,
    pub final_available_version: Option<String>,
}

struct StoredOperation {
    action: String,
    operation_id: String,
    correlation_id: String,
    app_id: String,
    name: String,
    version: String,
    publisher_id: String,
    publisher_name: String,
    publisher_key_id: String,
    entry: Option<String>,
    permissions_json: String,
    stage: String,
    steps_json: String,
    downloaded_bytes: i64,
    total_bytes: i64,
    error_code: Option<String>,
    message: Option<String>,
    final_available_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginVersionState {
    pub current_version: String,
    pub previous_version: Option<String>,
    pub pending_version: Option<String>,
    pub awaiting_launch: bool,
}

#[derive(Debug)]
pub struct RollbackResult {
    pub installed: InstalledPlugin,
    pub operation_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RollbackMarker {
    app_id: String,
    failed_version: String,
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
        store.recover_interrupted_rollbacks()?;
        store.recover_pending_upgrades()?;
        Ok(store)
    }

    pub fn install(
        &self,
        package_path: &Path,
        environment: &HostEnvironment,
        trusted_keys: &HashMap<String, VerifyingKey>,
    ) -> Result<InstalledPlugin, InstallError> {
        let prepared = self.prepare_install(
            package_path,
            environment,
            trusted_keys,
            &Uuid::new_v4().to_string(),
        )?;
        self.activate_prepared(prepared)
    }

    pub fn catalog_download_path(&self, operation_id: &str, file_name: &str) -> PathBuf {
        self.root
            .join("downloads")
            .join(operation_id)
            .join(file_name)
    }

    pub fn remove_catalog_download(&self, operation_id: &str) {
        let _ = fs::remove_dir_all(self.root.join("downloads").join(operation_id));
    }

    pub fn prepare_install(
        &self,
        package_path: &Path,
        environment: &HostEnvironment,
        trusted_keys: &HashMap<String, VerifyingKey>,
        operation_id: &str,
    ) -> Result<PreparedPlugin, InstallError> {
        let preview = super::package::inspect_package(package_path, environment, trusted_keys)?;
        if self.find(&preview.app_id)?.is_some() {
            return Err(InstallError::new(
                "PLUGIN_APP_ID_CONFLICT",
                "An application with this appId is already installed",
            ));
        }

        self.extract_prepared(package_path, environment, trusted_keys, operation_id)
    }

    pub fn prepare_upgrade(
        &self,
        package_path: &Path,
        environment: &HostEnvironment,
        trusted_keys: &HashMap<String, VerifyingKey>,
        operation_id: &str,
    ) -> Result<PreparedPlugin, InstallError> {
        let preview = super::package::inspect_package(package_path, environment, trusted_keys)?;
        self.validate_upgrade_target(&preview.app_id, &preview.version)?;
        self.extract_prepared(package_path, environment, trusted_keys, operation_id)
    }

    pub fn validate_upgrade_target(
        &self,
        app_id: &str,
        target_version: &str,
    ) -> Result<InstalledPlugin, InstallError> {
        let current = self.find(app_id)?.ok_or_else(|| {
            InstallError::new(
                "PLUGIN_NOT_FOUND",
                "Installed plugin was not found for upgrade",
            )
        })?;
        let current_version = Version::parse(&current.version).map_err(|_| {
            InstallError::new(
                "PLUGIN_REGISTRY_READ_FAILED",
                "Installed plugin version is invalid",
            )
        })?;
        let target_version = Version::parse(target_version).map_err(|_| {
            InstallError::new("PLUGIN_VERSION_INVALID", "Upgrade version is invalid")
        })?;
        if target_version <= current_version {
            return Err(InstallError::new(
                "PLUGIN_UPDATE_NOT_NEWER",
                "Catalog upgrade version must be strictly newer than current",
            ));
        }
        let state = self.version_state(app_id)?;
        if state.pending_version.is_some() || state.awaiting_launch {
            return Err(InstallError::new(
                "PLUGIN_UPDATE_STATE_CONFLICT",
                "Plugin already has an unfinished version transition",
            ));
        }
        Ok(current)
    }

    fn extract_prepared(
        &self,
        package_path: &Path,
        environment: &HostEnvironment,
        trusted_keys: &HashMap<String, VerifyingKey>,
        operation_id: &str,
    ) -> Result<PreparedPlugin, InstallError> {
        let staging_root = self.root.join("staging").join(operation_id);
        if staging_root.exists() {
            return Err(InstallError::new(
                "PLUGIN_STAGING_CONFLICT",
                "Plugin staging operation already exists",
            ));
        }
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

        Ok(PreparedPlugin {
            package,
            staging_root,
            payload_root: staged_payload,
        })
    }

    pub fn activate_prepared(
        &self,
        prepared: PreparedPlugin,
    ) -> Result<InstalledPlugin, InstallError> {
        self.activate_prepared_with_context(prepared, None, None)
    }

    pub fn activate_prepared_with_context(
        &self,
        prepared: PreparedPlugin,
        operation_id: Option<&str>,
        correlation_id: Option<&str>,
    ) -> Result<InstalledPlugin, InstallError> {
        let PreparedPlugin {
            package,
            staging_root,
            payload_root,
        } = prepared;

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
        fs::rename(&payload_root, &final_root).map_err(|error| {
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
        let native_service_json = serialize_native_service(&package.native_service)?;
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
                        entry, permissions_json, files_json, native_service_json, enabled, installed_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11)",
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
                        native_service_json,
                        installed_at,
                    ],
                )
                .map_err(|error| {
                    InstallError::new(
                        "PLUGIN_REGISTRY_WRITE_FAILED",
                        format!("Cannot update plugin registry: {error}"),
                    )
                })?;
            insert_plugin_version(&transaction, &package, &installed_at)?;
            let history_steps = if operation_id.is_some() {
                &[
                    "download",
                    "verify",
                    "staging",
                    "health-check",
                    "activate",
                    "complete",
                ][..]
            } else {
                &[
                    "signature",
                    "hashes",
                    "target",
                    "hostApi",
                    "staging",
                    "health",
                    "activation",
                ][..]
            };
            insert_history_with_context(
                &transaction,
                &package.app_id,
                "install",
                &package.version,
                &installed_at,
                history_steps,
                operation_id,
                correlation_id,
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

        self.find(&package.app_id)?.ok_or_else(|| {
            InstallError::new(
                "PLUGIN_REGISTRY_WRITE_FAILED",
                "Installed plugin is missing from registry",
            )
        })
    }

    pub fn mark_pending_upgrade(
        &self,
        prepared: &PreparedPlugin,
        operation_id: &str,
        correlation_id: &str,
    ) -> Result<(), InstallError> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(registry_write_error)?;
        let installed_at = Utc::now().to_rfc3339();
        insert_plugin_version(&transaction, &prepared.package, &installed_at)?;
        let changed = transaction
            .execute(
                "UPDATE installed_plugins
                 SET pending_version = ?1, pending_operation_id = ?2,
                     pending_correlation_id = ?3
                 WHERE app_id = ?4 AND pending_version IS NULL AND awaiting_launch = 0",
                params![
                    prepared.package.version,
                    operation_id,
                    correlation_id,
                    prepared.package.app_id,
                ],
            )
            .map_err(registry_write_error)?;
        if changed != 1 {
            return Err(InstallError::new(
                "PLUGIN_UPDATE_STATE_CONFLICT",
                "Plugin version state changed before pending upgrade was recorded",
            ));
        }
        transaction.commit().map_err(registry_write_error)
    }

    pub fn clear_pending_upgrade(
        &self,
        app_id: &str,
        operation_id: &str,
    ) -> Result<(), InstallError> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(registry_write_error)?;
        let pending_version = transaction
            .query_row(
                "SELECT pending_version FROM installed_plugins
                 WHERE app_id = ?1 AND pending_operation_id = ?2",
                params![app_id, operation_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .map_err(registry_read_error)?;
        if let Some(version) = pending_version {
            transaction
                .execute(
                    "DELETE FROM plugin_versions WHERE app_id = ?1 AND version = ?2",
                    params![app_id, version],
                )
                .map_err(registry_write_error)?;
        }
        transaction
            .execute(
                "UPDATE installed_plugins
                 SET pending_version = NULL, pending_operation_id = NULL,
                     pending_correlation_id = NULL
                 WHERE app_id = ?1 AND pending_operation_id = ?2",
                params![app_id, operation_id],
            )
            .map_err(registry_write_error)?;
        transaction.commit().map_err(registry_write_error)
    }

    pub fn activate_prepared_upgrade(
        &self,
        prepared: PreparedPlugin,
        operation_id: &str,
        correlation_id: &str,
    ) -> Result<InstalledPlugin, InstallError> {
        let PreparedPlugin {
            package,
            staging_root,
            payload_root,
        } = prepared;
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
        fs::create_dir_all(final_root.parent().expect("version directory has a parent")).map_err(
            |error| {
                let _ = fs::remove_dir_all(&staging_root);
                InstallError::new(
                    "PLUGIN_ACTIVATION_FAILED",
                    format!("Cannot create application directory: {error}"),
                )
            },
        )?;
        fs::rename(&payload_root, &final_root).map_err(|error| {
            let _ = fs::remove_dir_all(&staging_root);
            InstallError::new(
                "PLUGIN_ACTIVATION_FAILED",
                format!("Cannot activate plugin upgrade: {error}"),
            )
        })?;

        let result = self.activate_upgrade_registry(&package, operation_id, correlation_id);
        let previous_to_remove = match result {
            Ok(previous) => previous,
            Err(error) => {
                if let Err(cleanup_error) = remove_directory_if_exists(
                    &final_root,
                    "PLUGIN_ACTIVATION_CLEANUP_FAILED",
                    "Cannot remove failed plugin activation",
                )
                .and_then(|_| {
                    remove_directory_if_exists(
                        &staging_root,
                        "PLUGIN_ACTIVATION_CLEANUP_FAILED",
                        "Cannot clean failed plugin staging",
                    )
                }) {
                    return Err(InstallError::new(
                        cleanup_error.code,
                        format!(
                            "Plugin activation failed ({error}); cleanup will retry after restart: {}",
                            cleanup_error.message
                        ),
                    ));
                }
                return Err(error);
            }
        };
        let _ = fs::remove_dir_all(&staging_root);
        if let Some(version) = previous_to_remove {
            let _ = fs::remove_dir_all(self.root.join("apps").join(&package.app_id).join(version));
        }
        self.find(&package.app_id)?.ok_or_else(|| {
            InstallError::new(
                "PLUGIN_REGISTRY_WRITE_FAILED",
                "Upgraded plugin is missing from registry",
            )
        })
    }

    fn activate_upgrade_registry(
        &self,
        package: &super::package::InspectedPackage,
        operation_id: &str,
        correlation_id: &str,
    ) -> Result<Option<String>, InstallError> {
        let permissions_json = serde_json::to_string(&package.permissions).map_err(|error| {
            InstallError::new("PLUGIN_REGISTRY_WRITE_FAILED", error.to_string())
        })?;
        let files_json = serde_json::to_string(&package.files).map_err(|error| {
            InstallError::new("PLUGIN_REGISTRY_WRITE_FAILED", error.to_string())
        })?;
        let native_service_json = serialize_native_service(&package.native_service)?;
        let installed_at = Utc::now().to_rfc3339();
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(registry_write_error)?;
        let (current_version, previous_version): (String, Option<String>) = transaction
            .query_row(
                "SELECT version, previous_version FROM installed_plugins
                 WHERE app_id = ?1 AND pending_version = ?2 AND pending_operation_id = ?3",
                params![package.app_id, package.version, operation_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|error| {
                InstallError::new(
                    "PLUGIN_UPDATE_STATE_CONFLICT",
                    format!("Pending upgrade state is unavailable: {error}"),
                )
            })?;
        let changed = transaction
            .execute(
                "UPDATE installed_plugins SET
                    name = ?1, version = ?2, publisher_id = ?3, publisher_name = ?4,
                    publisher_key_id = ?5, entry = ?6, permissions_json = ?7,
                    files_json = ?8, native_service_json = ?9, installed_at = ?10, previous_version = ?11,
                    pending_version = NULL, pending_operation_id = NULL,
                    pending_correlation_id = NULL, awaiting_launch = 1,
                    launch_operation_id = ?12, launch_correlation_id = ?13
                 WHERE app_id = ?14 AND version = ?11 AND pending_version = ?2
                       AND pending_operation_id = ?12",
                params![
                    package.name,
                    package.version,
                    package.publisher_id,
                    package.publisher_name,
                    package.publisher_key_id,
                    package.entry,
                    permissions_json,
                    files_json,
                    native_service_json,
                    installed_at,
                    current_version,
                    operation_id,
                    correlation_id,
                    package.app_id,
                ],
            )
            .map_err(registry_write_error)?;
        if changed != 1 {
            return Err(InstallError::new(
                "PLUGIN_UPDATE_STATE_CONFLICT",
                "Pending upgrade state changed before activation",
            ));
        }
        if let Some(version) = &previous_version {
            transaction
                .execute(
                    "DELETE FROM plugin_versions WHERE app_id = ?1 AND version = ?2",
                    params![package.app_id, version],
                )
                .map_err(registry_write_error)?;
        }
        insert_history_with_context(
            &transaction,
            &package.app_id,
            "update",
            &package.version,
            &installed_at,
            &[
                "download",
                "verify",
                "staging",
                "health-check",
                "activate",
                "complete",
            ],
            Some(operation_id),
            Some(correlation_id),
        )?;
        transaction.commit().map_err(registry_write_error)?;
        Ok(previous_version)
    }

    pub fn version_state(&self, app_id: &str) -> Result<PluginVersionState, InstallError> {
        self.connection()?
            .query_row(
                "SELECT version, previous_version, pending_version, awaiting_launch
                 FROM installed_plugins WHERE app_id = ?1",
                [app_id],
                |row| {
                    Ok(PluginVersionState {
                        current_version: row.get(0)?,
                        previous_version: row.get(1)?,
                        pending_version: row.get(2)?,
                        awaiting_launch: row.get(3)?,
                    })
                },
            )
            .map_err(registry_read_error)
    }

    pub fn launch_operation_id(
        &self,
        app_id: &str,
        version: &str,
    ) -> Result<Option<String>, InstallError> {
        self.connection()?
            .query_row(
                "SELECT launch_operation_id FROM installed_plugins
                 WHERE app_id = ?1 AND version = ?2 AND awaiting_launch = 1",
                params![app_id, version],
                |row| row.get(0),
            )
            .optional()
            .map_err(registry_read_error)
            .map(|value| value.flatten())
    }

    pub fn confirm_current_launch(
        &self,
        app_id: &str,
        version: &str,
    ) -> Result<bool, InstallError> {
        let changed = self
            .connection()?
            .execute(
                "UPDATE installed_plugins
                 SET awaiting_launch = 0, launch_operation_id = NULL,
                     launch_correlation_id = NULL
                 WHERE app_id = ?1 AND version = ?2 AND awaiting_launch = 1",
                params![app_id, version],
            )
            .map_err(registry_write_error)?;
        Ok(changed == 1)
    }

    pub fn rollback_current_launch(
        &self,
        app_id: &str,
        failed_version: &str,
    ) -> Result<Option<RollbackResult>, InstallError> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(registry_write_error)?;
        let state = transaction.query_row(
            "SELECT previous_version, enabled, launch_operation_id, launch_correlation_id
             FROM installed_plugins
             WHERE app_id = ?1 AND version = ?2 AND awaiting_launch = 1",
            params![app_id, failed_version],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, bool>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        );
        let (previous_version, enabled, operation_id, correlation_id) = match state {
            Ok(state) => state,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(error) => return Err(registry_read_error(error)),
        };
        let previous_version = previous_version.ok_or_else(|| {
            InstallError::new(
                "PLUGIN_ROLLBACK_FAILED",
                "No previous plugin version is available",
            )
        })?;
        let operation_id = operation_id.ok_or_else(|| {
            InstallError::new(
                "PLUGIN_ROLLBACK_FAILED",
                "Rollback operation identity is missing",
            )
        })?;
        let correlation_id = correlation_id.ok_or_else(|| {
            InstallError::new(
                "PLUGIN_ROLLBACK_FAILED",
                "Rollback correlation identity is missing",
            )
        })?;
        let previous =
            plugin_version(&transaction, &self.root, app_id, &previous_version, enabled)?;
        verify_rollback_target(&previous)?;

        let permissions_json = serde_json::to_string(&previous.permissions).map_err(|error| {
            InstallError::new(
                "PLUGIN_ROLLBACK_FAILED",
                format!("Cannot restore previous plugin permissions: {error}"),
            )
        })?;
        let files_json = serde_json::to_string(&previous.files).map_err(|error| {
            InstallError::new(
                "PLUGIN_ROLLBACK_FAILED",
                format!("Cannot restore previous plugin files: {error}"),
            )
        })?;
        let native_service_json = serialize_native_service(&previous.native_service)?;

        let rollback_root = self
            .root
            .join("staging")
            .join(format!("rollback-{}", Uuid::new_v4()));
        let staged_failed_root = rollback_root.join("failed-current");
        let failed_root = self.root.join("apps").join(app_id).join(failed_version);
        let marker = serde_json::to_vec(&RollbackMarker {
            app_id: app_id.to_string(),
            failed_version: failed_version.to_string(),
        })
        .map_err(|error| {
            InstallError::new(
                "PLUGIN_ROLLBACK_FAILED",
                format!("Cannot serialize plugin rollback state: {error}"),
            )
        })?;
        fs::create_dir_all(&rollback_root).map_err(|error| {
            InstallError::new(
                "PLUGIN_ROLLBACK_FAILED",
                format!("Cannot prepare plugin rollback: {error}"),
            )
        })?;
        if let Err(error) = fs::write(rollback_root.join("rollback.json"), marker) {
            let _ = fs::remove_dir_all(&rollback_root);
            return Err(InstallError::new(
                "PLUGIN_ROLLBACK_FAILED",
                format!("Cannot persist plugin rollback state: {error}"),
            ));
        }
        if let Err(error) = fs::rename(&failed_root, &staged_failed_root) {
            let _ = fs::remove_dir_all(&rollback_root);
            return Err(InstallError::new(
                "PLUGIN_ROLLBACK_FAILED",
                format!("Cannot stage failed plugin version: {error}"),
            ));
        }

        let write_result = (|| -> Result<(), InstallError> {
            let changed = transaction
                .execute(
                    "UPDATE installed_plugins SET
                    name = ?1, version = ?2, publisher_id = ?3, publisher_name = ?4,
                    publisher_key_id = ?5, entry = ?6, permissions_json = ?7,
                    files_json = ?8, native_service_json = ?9, installed_at = ?10, previous_version = NULL,
                    awaiting_launch = 0, launch_operation_id = NULL,
                    launch_correlation_id = NULL
                 WHERE app_id = ?11 AND version = ?12 AND awaiting_launch = 1",
                    params![
                        previous.name,
                        previous.version,
                        previous.publisher_id,
                        previous.publisher_name,
                        previous.publisher_key_id,
                        previous.entry,
                        permissions_json,
                        files_json,
                        native_service_json,
                        previous.installed_at,
                        app_id,
                        failed_version,
                    ],
                )
                .map_err(registry_write_error)?;
            if changed != 1 {
                return Err(InstallError::new(
                    "PLUGIN_ROLLBACK_FAILED",
                    "Plugin version state changed before rollback",
                ));
            }
            transaction
                .execute(
                    "DELETE FROM plugin_versions WHERE app_id = ?1 AND version = ?2",
                    params![app_id, failed_version],
                )
                .map_err(registry_write_error)?;
            insert_history_result(
                &transaction,
                app_id,
                "rollback",
                &previous.version,
                "rolled-back",
                None,
                Some(&operation_id),
                Some(&correlation_id),
                &[
                    "download",
                    "verify",
                    "staging",
                    "health-check",
                    "activate",
                    "complete",
                    "rolled-back",
                ],
            )?;
            transaction.commit().map_err(registry_write_error)
        })();
        if let Err(error) = write_result {
            if let Err(restore_error) = fs::rename(&staged_failed_root, &failed_root) {
                return Err(InstallError::new(
                    "PLUGIN_ROLLBACK_COMPENSATION_FAILED",
                    format!(
                        "Plugin rollback failed ({error}); failed version could not be restored: {restore_error}"
                    ),
                ));
            }
            let _ = fs::remove_dir_all(&rollback_root);
            return Err(error);
        }
        let _ = fs::remove_dir_all(&rollback_root);
        Ok(Some(RollbackResult {
            installed: previous,
            operation_id,
        }))
    }

    pub fn discard_prepared(&self, prepared: PreparedPlugin) {
        let _ = fs::remove_dir_all(prepared.staging_root);
    }

    pub fn read_prepared_asset(
        &self,
        prepared: &PreparedPlugin,
        relative: &str,
    ) -> Result<PluginAsset, InstallError> {
        if !is_safe_relative_path(relative) {
            return Err(InstallError::new(
                "PLUGIN_ASSET_PATH_INVALID",
                "Plugin asset path is invalid",
            ));
        }
        let expected_hash = prepared.package.files.get(relative).ok_or_else(|| {
            InstallError::new(
                "PLUGIN_ASSET_NOT_DECLARED",
                "Plugin asset is not declared in the signed manifest",
            )
        })?;
        let bytes = fs::read(prepared.payload_root.join(relative)).map_err(|_| {
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

    pub fn list(&self) -> Result<Vec<InstalledPlugin>, InstallError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT app_id, name, version, publisher_id, publisher_name, publisher_key_id,
                        entry, permissions_json, files_json, native_service_json, enabled, installed_at
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
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, bool>(10)?,
                    row.get::<_, String>(11)?,
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
                native_service_json,
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
            let native_service = deserialize_native_service(native_service_json.as_deref())?;
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
                native_service,
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
        if self.version_state(app_id)?.pending_version.is_some() {
            return Err(InstallError::new(
                "PLUGIN_UPDATE_STATE_CONFLICT",
                "Plugin cannot be uninstalled while an upgrade is pending",
            ));
        }
        let removal_root = self
            .root
            .join("staging")
            .join(format!("uninstall-{}", Uuid::new_v4()));
        let removed_package = removal_root.join("package");
        let removed_data = removal_root.join("data");
        let app_root = self.root.join("apps").join(app_id);
        fs::create_dir_all(&removal_root).map_err(|error| {
            InstallError::new(
                "PLUGIN_UNINSTALL_FAILED",
                format!("Cannot prepare uninstall staging: {error}"),
            )
        })?;
        fs::rename(&app_root, &removed_package).map_err(|error| {
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
                let _ = fs::rename(&removed_package, &app_root);
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
                    "DELETE FROM plugin_versions WHERE app_id = ?1",
                    params![app_id],
                )
                .map_err(registry_write_error)?;
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
            let _ = fs::rename(&removed_package, &app_root);
            let _ = fs::remove_dir_all(&removal_root);
            return Err(error);
        }

        let _ = fs::remove_dir_all(&removal_root);
        Ok(())
    }

    pub fn history(&self) -> Result<Vec<PluginHistoryRecord>, InstallError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT id, app_id, action, version, occurred_at, result, error_code,
                        operation_id, correlation_id, steps_json, final_available_version
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
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, Option<String>>(10)?,
                ))
            })
            .map_err(registry_read_error)?;
        let mut history = Vec::new();
        for row in rows {
            let (
                id,
                app_id,
                action,
                version,
                occurred_at,
                result,
                error_code,
                operation_id,
                correlation_id,
                steps_json,
                final_available_version,
            ) = row.map_err(registry_read_error)?;
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
                operation_id,
                correlation_id,
                steps,
                final_available_version,
            });
        }
        Ok(history)
    }

    pub fn record_catalog_failure(
        &self,
        app_id: &str,
        version: &str,
        action: &str,
        operation_id: &str,
        correlation_id: &str,
        error_code: &str,
        steps: &[String],
        final_available_version: Option<&str>,
    ) -> Result<(), InstallError> {
        let steps_json = serde_json::to_string(steps).map_err(|error| {
            InstallError::new(
                "PLUGIN_REGISTRY_WRITE_FAILED",
                format!("Cannot serialize plugin history: {error}"),
            )
        })?;
        self.connection()?
            .execute(
                "INSERT INTO plugin_history (
                    app_id, action, version, occurred_at, result, error_code,
                    operation_id, correlation_id, steps_json, final_available_version
                 ) VALUES (?1, ?2, ?3, ?4, 'failed', ?5, ?6, ?7, ?8, ?9)",
                params![
                    app_id,
                    action,
                    version,
                    Utc::now().to_rfc3339(),
                    error_code,
                    operation_id,
                    correlation_id,
                    steps_json,
                    final_available_version,
                ],
            )
            .map_err(|error| {
                InstallError::new(
                    "PLUGIN_REGISTRY_WRITE_FAILED",
                    format!("Cannot write plugin failure history: {error}"),
                )
            })?;
        Ok(())
    }

    pub fn save_operation(&self, operation: &OperationSnapshot) -> Result<(), InstallError> {
        let permissions_json = serde_json::to_string(&operation.permissions).map_err(|error| {
            InstallError::new(
                "PLUGIN_REGISTRY_WRITE_FAILED",
                format!("Cannot serialize plugin operation permissions: {error}"),
            )
        })?;
        let steps_json = serde_json::to_string(&operation.steps).map_err(|error| {
            InstallError::new(
                "PLUGIN_REGISTRY_WRITE_FAILED",
                format!("Cannot serialize plugin operation steps: {error}"),
            )
        })?;
        let downloaded_bytes = i64::try_from(operation.downloaded_bytes).map_err(|_| {
            InstallError::new(
                "PLUGIN_REGISTRY_WRITE_FAILED",
                "Plugin downloaded byte count is too large",
            )
        })?;
        let total_bytes = i64::try_from(operation.total_bytes).map_err(|_| {
            InstallError::new(
                "PLUGIN_REGISTRY_WRITE_FAILED",
                "Plugin total byte count is too large",
            )
        })?;
        let now = Utc::now().to_rfc3339();
        self.connection()?
            .execute(
                "INSERT INTO plugin_operations (
                    operation_id, correlation_id, action, app_id, name, version,
                    publisher_id, publisher_name, publisher_key_id, entry,
                    permissions_json, stage, steps_json, downloaded_bytes, total_bytes,
                    error_code, message, final_available_version, created_at, updated_at
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                    ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?19
                 ) ON CONFLICT(operation_id) DO UPDATE SET
                    entry = excluded.entry,
                    stage = excluded.stage,
                    steps_json = excluded.steps_json,
                    downloaded_bytes = excluded.downloaded_bytes,
                    total_bytes = excluded.total_bytes,
                    error_code = excluded.error_code,
                    message = excluded.message,
                    final_available_version = excluded.final_available_version,
                    updated_at = excluded.updated_at",
                params![
                    operation.operation_id,
                    operation.correlation_id,
                    operation.action,
                    operation.app_id,
                    operation.name,
                    operation.version,
                    operation.publisher_id,
                    operation.publisher_name,
                    operation.publisher_key_id,
                    operation.entry,
                    permissions_json,
                    operation.stage.as_str(),
                    steps_json,
                    downloaded_bytes,
                    total_bytes,
                    operation.error_code,
                    operation.message,
                    operation.final_available_version,
                    now,
                ],
            )
            .map_err(registry_write_error)?;
        Ok(())
    }

    pub fn operation(&self, operation_id: &str) -> Result<Option<OperationSnapshot>, InstallError> {
        let connection = self.connection()?;
        let stored = connection
            .query_row(
                &format!("{} WHERE operation_id = ?1", operation_select_sql()),
                [operation_id],
                stored_operation_from_row,
            )
            .optional()
            .map_err(registry_read_error)?;
        stored.map(decode_operation).transpose()
    }

    pub fn latest_operation(&self) -> Result<Option<OperationSnapshot>, InstallError> {
        let connection = self.connection()?;
        let stored = connection
            .query_row(
                &format!("{} ORDER BY id DESC LIMIT 1", operation_select_sql()),
                [],
                stored_operation_from_row,
            )
            .optional()
            .map_err(registry_read_error)?;
        stored.map(decode_operation).transpose()
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
                    native_service_json TEXT,
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
                    operation_id TEXT,
                    correlation_id TEXT,
                    steps_json TEXT NOT NULL,
                    final_available_version TEXT
                );
                CREATE TABLE IF NOT EXISTS plugin_versions (
                    app_id TEXT NOT NULL,
                    version TEXT NOT NULL,
                    name TEXT NOT NULL,
                    publisher_id TEXT NOT NULL,
                    publisher_name TEXT NOT NULL,
                    publisher_key_id TEXT NOT NULL,
                    entry TEXT NOT NULL,
                    permissions_json TEXT NOT NULL,
                    files_json TEXT NOT NULL,
                    native_service_json TEXT,
                    installed_at TEXT NOT NULL,
                    PRIMARY KEY (app_id, version)
                );
                CREATE TABLE IF NOT EXISTS plugin_operations (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    operation_id TEXT NOT NULL UNIQUE,
                    correlation_id TEXT NOT NULL,
                    action TEXT NOT NULL,
                    app_id TEXT NOT NULL,
                    name TEXT NOT NULL,
                    version TEXT NOT NULL,
                    publisher_id TEXT NOT NULL,
                    publisher_name TEXT NOT NULL,
                    publisher_key_id TEXT NOT NULL,
                    entry TEXT,
                    permissions_json TEXT NOT NULL,
                    stage TEXT NOT NULL,
                    steps_json TEXT NOT NULL,
                    downloaded_bytes INTEGER NOT NULL,
                    total_bytes INTEGER NOT NULL,
                    error_code TEXT,
                    message TEXT,
                    final_available_version TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
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
        for (column, definition) in [
            ("operation_id", "TEXT"),
            ("correlation_id", "TEXT"),
            ("final_available_version", "TEXT"),
        ] {
            let exists: bool = connection
                .query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM pragma_table_info('plugin_history') WHERE name = ?1
                    )",
                    [column],
                    |row| row.get(0),
                )
                .map_err(registry_read_error)?;
            if !exists {
                connection
                    .execute(
                        &format!("ALTER TABLE plugin_history ADD COLUMN {column} {definition}"),
                        [],
                    )
                    .map_err(|error| {
                        InstallError::new(
                            "PLUGIN_STORE_OPEN_FAILED",
                            format!("Cannot migrate plugin history: {error}"),
                        )
                    })?;
            }
        }
        for (column, definition) in [
            ("previous_version", "TEXT"),
            ("pending_version", "TEXT"),
            ("pending_operation_id", "TEXT"),
            ("pending_correlation_id", "TEXT"),
            ("launch_operation_id", "TEXT"),
            ("launch_correlation_id", "TEXT"),
            ("awaiting_launch", "INTEGER NOT NULL DEFAULT 0"),
            ("native_service_json", "TEXT"),
        ] {
            let exists: bool = connection
                .query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM pragma_table_info('installed_plugins') WHERE name = ?1
                    )",
                    [column],
                    |row| row.get(0),
                )
                .map_err(registry_read_error)?;
            if !exists {
                connection
                    .execute(
                        &format!("ALTER TABLE installed_plugins ADD COLUMN {column} {definition}"),
                        [],
                    )
                    .map_err(|error| {
                        InstallError::new(
                            "PLUGIN_STORE_OPEN_FAILED",
                            format!("Cannot migrate plugin version state: {error}"),
                        )
                    })?;
            }
        }
        let has_version_native_service: bool = connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM pragma_table_info('plugin_versions')
                    WHERE name = 'native_service_json'
                )",
                [],
                |row| row.get(0),
            )
            .map_err(registry_read_error)?;
        if !has_version_native_service {
            connection
                .execute(
                    "ALTER TABLE plugin_versions ADD COLUMN native_service_json TEXT",
                    [],
                )
                .map_err(|error| {
                    InstallError::new(
                        "PLUGIN_STORE_OPEN_FAILED",
                        format!("Cannot migrate plugin native service metadata: {error}"),
                    )
                })?;
        }
        connection
            .execute(
                "INSERT OR IGNORE INTO plugin_versions (
                    app_id, version, name, publisher_id, publisher_name, publisher_key_id,
                    entry, permissions_json, files_json, native_service_json, installed_at
                 )
                 SELECT app_id, version, name, publisher_id, publisher_name, publisher_key_id,
                        entry, permissions_json, files_json, native_service_json, installed_at
                 FROM installed_plugins",
                [],
            )
            .map_err(|error| {
                InstallError::new(
                    "PLUGIN_STORE_OPEN_FAILED",
                    format!("Cannot migrate installed plugin versions: {error}"),
                )
            })?;
        Ok(())
    }

    fn recover_pending_upgrades(&self) -> Result<(), InstallError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT app_id, pending_version, pending_operation_id
                 FROM installed_plugins WHERE pending_operation_id IS NOT NULL",
            )
            .map_err(registry_read_error)?;
        let pending = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(registry_read_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(registry_read_error)?;
        drop(statement);
        for (app_id, pending_version, operation_id) in pending {
            remove_directory_if_exists(
                &self.root.join("staging").join(&operation_id),
                "PLUGIN_STORE_OPEN_FAILED",
                "Cannot clean pending plugin staging",
            )?;
            remove_directory_if_exists(
                &self.root.join("downloads").join(&operation_id),
                "PLUGIN_STORE_OPEN_FAILED",
                "Cannot clean pending plugin download",
            )?;
            if let Some(version) = &pending_version {
                remove_directory_if_exists(
                    &self.root.join("apps").join(&app_id).join(version),
                    "PLUGIN_STORE_OPEN_FAILED",
                    "Cannot clean pending plugin version",
                )?;
            }
            self.clear_pending_upgrade(&app_id, &operation_id)?;
        }
        Ok(())
    }

    fn recover_interrupted_rollbacks(&self) -> Result<(), InstallError> {
        let staging_root = self.root.join("staging");
        if !staging_root.exists() {
            return Ok(());
        }
        let entries = fs::read_dir(&staging_root).map_err(|error| {
            InstallError::new(
                "PLUGIN_STORE_OPEN_FAILED",
                format!("Cannot inspect plugin rollback staging: {error}"),
            )
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                InstallError::new(
                    "PLUGIN_STORE_OPEN_FAILED",
                    format!("Cannot inspect plugin rollback entry: {error}"),
                )
            })?;
            if !entry.file_name().to_string_lossy().starts_with("rollback-") {
                continue;
            }
            let rollback_root = entry.path();
            let marker_path = rollback_root.join("rollback.json");
            if !marker_path.is_file() {
                continue;
            }
            let marker: RollbackMarker =
                serde_json::from_slice(&fs::read(&marker_path).map_err(|error| {
                    InstallError::new(
                        "PLUGIN_STORE_OPEN_FAILED",
                        format!("Cannot read plugin rollback state: {error}"),
                    )
                })?)
                .map_err(|error| {
                    InstallError::new(
                        "PLUGIN_STORE_OPEN_FAILED",
                        format!("Plugin rollback state is invalid: {error}"),
                    )
                })?;
            if !is_safe_store_segment(&marker.app_id)
                || Version::parse(&marker.failed_version).is_err()
            {
                return Err(InstallError::new(
                    "PLUGIN_STORE_OPEN_FAILED",
                    "Plugin rollback state contains an invalid application or version",
                ));
            }
            let current_version = match self.connection()?.query_row(
                "SELECT version FROM installed_plugins WHERE app_id = ?1",
                [&marker.app_id],
                |row| row.get::<_, String>(0),
            ) {
                Ok(version) => Some(version),
                Err(rusqlite::Error::QueryReturnedNoRows) => None,
                Err(error) => return Err(registry_read_error(error)),
            };
            let failed_root = self
                .root
                .join("apps")
                .join(&marker.app_id)
                .join(&marker.failed_version);
            let staged_failed_root = rollback_root.join("failed-current");
            if current_version.as_deref() == Some(marker.failed_version.as_str()) {
                if failed_root.exists() && staged_failed_root.exists() {
                    return Err(InstallError::new(
                        "PLUGIN_STORE_OPEN_FAILED",
                        "Plugin rollback has conflicting current and staged versions",
                    ));
                }
                if !failed_root.exists() {
                    fs::rename(&staged_failed_root, &failed_root).map_err(|error| {
                        InstallError::new(
                            "PLUGIN_STORE_OPEN_FAILED",
                            format!("Cannot restore interrupted plugin rollback: {error}"),
                        )
                    })?;
                }
            }
            fs::remove_dir_all(&rollback_root).map_err(|error| {
                InstallError::new(
                    "PLUGIN_STORE_OPEN_FAILED",
                    format!("Cannot clean plugin rollback staging: {error}"),
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

fn operation_select_sql() -> &'static str {
    "SELECT action, operation_id, correlation_id, app_id, name, version,
            publisher_id, publisher_name, publisher_key_id, entry,
            permissions_json, stage, steps_json, downloaded_bytes, total_bytes,
            error_code, message, final_available_version
     FROM plugin_operations"
}

fn stored_operation_from_row(row: &Row<'_>) -> rusqlite::Result<StoredOperation> {
    Ok(StoredOperation {
        action: row.get(0)?,
        operation_id: row.get(1)?,
        correlation_id: row.get(2)?,
        app_id: row.get(3)?,
        name: row.get(4)?,
        version: row.get(5)?,
        publisher_id: row.get(6)?,
        publisher_name: row.get(7)?,
        publisher_key_id: row.get(8)?,
        entry: row.get(9)?,
        permissions_json: row.get(10)?,
        stage: row.get(11)?,
        steps_json: row.get(12)?,
        downloaded_bytes: row.get(13)?,
        total_bytes: row.get(14)?,
        error_code: row.get(15)?,
        message: row.get(16)?,
        final_available_version: row.get(17)?,
    })
}

fn decode_operation(stored: StoredOperation) -> Result<OperationSnapshot, InstallError> {
    let stage = OperationStage::parse(&stored.stage).ok_or_else(|| {
        InstallError::new(
            "PLUGIN_REGISTRY_READ_FAILED",
            "Plugin operation stage is invalid",
        )
    })?;
    let permissions = serde_json::from_str(&stored.permissions_json).map_err(|error| {
        InstallError::new(
            "PLUGIN_REGISTRY_READ_FAILED",
            format!("Plugin operation permissions are corrupted: {error}"),
        )
    })?;
    let steps = serde_json::from_str(&stored.steps_json).map_err(|error| {
        InstallError::new(
            "PLUGIN_REGISTRY_READ_FAILED",
            format!("Plugin operation steps are corrupted: {error}"),
        )
    })?;
    let downloaded_bytes = u64::try_from(stored.downloaded_bytes).map_err(|_| {
        InstallError::new(
            "PLUGIN_REGISTRY_READ_FAILED",
            "Plugin downloaded byte count is invalid",
        )
    })?;
    let total_bytes = u64::try_from(stored.total_bytes).map_err(|_| {
        InstallError::new(
            "PLUGIN_REGISTRY_READ_FAILED",
            "Plugin total byte count is invalid",
        )
    })?;
    Ok(OperationSnapshot {
        action: stored.action,
        operation_id: stored.operation_id,
        correlation_id: stored.correlation_id,
        app_id: stored.app_id,
        name: stored.name,
        version: stored.version,
        publisher_id: stored.publisher_id,
        publisher_name: stored.publisher_name,
        publisher_key_id: stored.publisher_key_id,
        entry: stored.entry,
        permissions,
        stage,
        steps,
        downloaded_bytes,
        total_bytes,
        error_code: stored.error_code,
        message: stored.message,
        final_available_version: stored.final_available_version,
    })
}

fn is_safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

fn is_safe_store_segment(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && !value.contains('/')
        && !value.contains('\\')
}

fn remove_directory_if_exists(path: &Path, code: &str, context: &str) -> Result<(), InstallError> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(InstallError::new(code, format!("{context}: {error}"))),
    }
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
    insert_history_with_context(
        transaction,
        app_id,
        action,
        version,
        occurred_at,
        steps,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn insert_history_with_context(
    transaction: &rusqlite::Transaction<'_>,
    app_id: &str,
    action: &str,
    version: &str,
    occurred_at: &str,
    steps: &[&str],
    operation_id: Option<&str>,
    correlation_id: Option<&str>,
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
                app_id, action, version, occurred_at, result, error_code,
                operation_id, correlation_id, steps_json, final_available_version
             ) VALUES (?1, ?2, ?3, ?4, 'success', NULL, ?5, ?6, ?7, ?3)",
            params![
                app_id,
                action,
                version,
                occurred_at,
                operation_id,
                correlation_id,
                steps_json,
            ],
        )
        .map_err(|error| {
            InstallError::new(
                "PLUGIN_REGISTRY_WRITE_FAILED",
                format!("Cannot write plugin history: {error}"),
            )
        })?;
    Ok(())
}

fn insert_history_result(
    transaction: &rusqlite::Transaction<'_>,
    app_id: &str,
    action: &str,
    version: &str,
    result: &str,
    error_code: Option<&str>,
    operation_id: Option<&str>,
    correlation_id: Option<&str>,
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
                app_id, action, version, occurred_at, result, error_code,
                operation_id, correlation_id, steps_json, final_available_version
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?3)",
            params![
                app_id,
                action,
                version,
                Utc::now().to_rfc3339(),
                result,
                error_code,
                operation_id,
                correlation_id,
                steps_json,
            ],
        )
        .map_err(registry_write_error)?;
    Ok(())
}

fn insert_plugin_version(
    transaction: &rusqlite::Transaction<'_>,
    package: &super::package::InspectedPackage,
    installed_at: &str,
) -> Result<(), InstallError> {
    let permissions_json = serde_json::to_string(&package.permissions)
        .map_err(|error| InstallError::new("PLUGIN_REGISTRY_WRITE_FAILED", error.to_string()))?;
    let files_json = serde_json::to_string(&package.files)
        .map_err(|error| InstallError::new("PLUGIN_REGISTRY_WRITE_FAILED", error.to_string()))?;
    let native_service_json = serialize_native_service(&package.native_service)?;
    transaction
        .execute(
            "INSERT INTO plugin_versions (
                app_id, version, name, publisher_id, publisher_name, publisher_key_id,
                entry, permissions_json, files_json, native_service_json, installed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                package.app_id,
                package.version,
                package.name,
                package.publisher_id,
                package.publisher_name,
                package.publisher_key_id,
                package.entry,
                permissions_json,
                files_json,
                native_service_json,
                installed_at,
            ],
        )
        .map_err(registry_write_error)?;
    Ok(())
}

fn serialize_native_service(
    service: &Option<NativeService>,
) -> Result<Option<String>, InstallError> {
    service
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| {
            InstallError::new(
                "PLUGIN_REGISTRY_WRITE_FAILED",
                format!("Cannot serialize native service metadata: {error}"),
            )
        })
}

fn deserialize_native_service(value: Option<&str>) -> Result<Option<NativeService>, InstallError> {
    value
        .map(serde_json::from_str)
        .transpose()
        .map_err(|error| {
            InstallError::new(
                "PLUGIN_REGISTRY_READ_FAILED",
                format!("Plugin native service metadata is corrupted: {error}"),
            )
        })
}

fn plugin_version(
    transaction: &rusqlite::Transaction<'_>,
    root: &Path,
    app_id: &str,
    version: &str,
    enabled: bool,
) -> Result<InstalledPlugin, InstallError> {
    let row = transaction
        .query_row(
            "SELECT name, publisher_id, publisher_name, publisher_key_id, entry,
                    permissions_json, files_json, native_service_json, installed_at
             FROM plugin_versions WHERE app_id = ?1 AND version = ?2",
            params![app_id, version],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, String>(8)?,
                ))
            },
        )
        .map_err(registry_read_error)?;
    let permissions = serde_json::from_str(&row.5).map_err(|error| {
        InstallError::new(
            "PLUGIN_REGISTRY_READ_FAILED",
            format!("Plugin permissions are corrupted: {error}"),
        )
    })?;
    let files = serde_json::from_str(&row.6).map_err(|error| {
        InstallError::new(
            "PLUGIN_REGISTRY_READ_FAILED",
            format!("Plugin files are corrupted: {error}"),
        )
    })?;
    let native_service = deserialize_native_service(row.7.as_deref())?;
    Ok(InstalledPlugin {
        app_id: app_id.to_string(),
        name: row.0,
        version: version.to_string(),
        publisher_id: row.1,
        publisher_name: row.2,
        publisher_key_id: row.3,
        entry: row.4,
        permissions,
        native_service,
        enabled,
        installed_at: row.8,
        files,
        root: root.join("apps").join(app_id).join(version),
    })
}

fn verify_rollback_target(plugin: &InstalledPlugin) -> Result<(), InstallError> {
    let expected_hash = plugin.files.get(&plugin.entry).ok_or_else(|| {
        InstallError::new(
            "PLUGIN_ROLLBACK_FAILED",
            "Previous plugin entry is not declared",
        )
    })?;
    let entry = fs::read(plugin.root.join(&plugin.entry)).map_err(|error| {
        InstallError::new(
            "PLUGIN_ROLLBACK_FAILED",
            format!("Previous plugin entry is not readable: {error}"),
        )
    })?;
    if hex::encode(Sha256::digest(entry)) != *expected_hash {
        return Err(InstallError::new(
            "PLUGIN_ROLLBACK_FAILED",
            "Previous plugin entry failed integrity verification",
        ));
    }
    Ok(())
}

fn registry_write_error(error: rusqlite::Error) -> InstallError {
    InstallError::new(
        "PLUGIN_REGISTRY_WRITE_FAILED",
        format!("Cannot update plugin registry: {error}"),
    )
}

fn registry_read_error(error: rusqlite::Error) -> InstallError {
    InstallError::new(
        "PLUGIN_REGISTRY_READ_FAILED",
        format!("Cannot read plugin registry: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        fs::{self, File},
        io::Write,
        sync::{Arc, Barrier, Mutex},
        thread,
    };

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

    fn valid_package_version_with_permissions(
        directory: &std::path::Path,
        version: &str,
        permissions: &str,
    ) -> (std::path::PathBuf, SigningKey) {
        let path = directory.join(format!("codec-{version}.atpkg"));
        let signing_key = SigningKey::from_bytes(&TEST_KEY_SEED);
        let license = b"P0 test license";
        let index = b"<!doctype html><title>Codec</title>";
        let manifest = format!(
            concat!(
                "{{\"appId\":\"com.attson.codec\",",
                "\"files\":{{\"LICENSE\":\"{}\",\"ui/index.html\":\"{}\"}},",
                "\"hostApi\":{{\"max\":\"1.x\",\"min\":\"1.0.0\"}},",
                "\"name\":\"编码转换\",",
                "\"permissions\":{},",
                "\"publisher\":{{\"id\":\"com.attson\",\"keyId\":\"attool-p0-test\",\"name\":\"Attson\"}},",
                "\"schemaVersion\":1,",
                "\"target\":{{\"arch\":\"{}\",\"os\":\"{}\"}},",
                "\"ui\":{{\"entry\":\"ui/index.html\"}},",
                "\"version\":\"{}\"}}"
            ),
            sha256(license),
            sha256(index),
            permissions,
            std::env::consts::ARCH,
            std::env::consts::OS,
            version
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

    fn valid_package_version(
        directory: &std::path::Path,
        version: &str,
    ) -> (std::path::PathBuf, SigningKey) {
        valid_package_version_with_permissions(directory, version, "[\"host.theme.read\"]")
    }

    fn valid_package(directory: &std::path::Path) -> (std::path::PathBuf, SigningKey) {
        valid_package_version(directory, "0.1.0-alpha.0")
    }

    fn native_service_package(directory: &std::path::Path) -> (std::path::PathBuf, SigningKey) {
        let path = directory.join("http.atpkg");
        let signing_key = SigningKey::from_bytes(&TEST_KEY_SEED);
        let index = b"<!doctype html><title>HTTP</title>";
        let service = b"native-service";
        let service_hash = sha256(service);
        let manifest = serde_json::json!({
            "appId": "com.attson.http",
            "files": {
                "native/athttp-service": service_hash,
                "ui/index.html": sha256(index)
            },
            "hostApi": { "max": "2.x", "min": "2.0.0" },
            "name": "HTTP request",
            "nativeService": {
                "entry": "native/athttp-service",
                "protocolVersion": 1,
                "sha256": service_hash,
                "version": "0.1.0-alpha.0"
            },
            "permissions": ["host.http.request", "host.http.stream", "host.http.data"],
            "publisher": { "id": "com.attson", "keyId": "attool-p0-test", "name": "Attson" },
            "schemaVersion": 1,
            "target": { "arch": std::env::consts::ARCH, "os": std::env::consts::OS },
            "ui": { "entry": "ui/index.html" },
            "version": "0.1.0-alpha.0"
        });
        let manifest = serde_json::to_string(&manifest).unwrap();
        let signature = base64::engine::general_purpose::STANDARD
            .encode(signing_key.sign(manifest.as_bytes()).to_bytes());
        let mut archive = ZipWriter::new(File::create(&path).unwrap());
        for (name, bytes) in [
            ("manifest.json", manifest.as_bytes()),
            ("manifest.sig", signature.as_bytes()),
            ("ui/index.html", index.as_slice()),
            ("native/athttp-service", service.as_slice()),
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
    fn installs_and_persists_native_service_metadata() {
        let directory = tempfile::tempdir().unwrap();
        let (package_path, signing_key) = native_service_package(directory.path());
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);
        let store_root = directory.path().join("plugins");
        let store = PluginStore::open(store_root.clone()).unwrap();

        let installed = store
            .install(&package_path, &HostEnvironment::current("2.0.0"), &keys)
            .unwrap();
        assert_eq!(
            installed.native_service.as_ref().unwrap().entry,
            "native/athttp-service"
        );
        drop(store);

        let reopened = PluginStore::open(store_root).unwrap();
        let installed = reopened.list().unwrap().pop().unwrap();
        let service = installed.native_service.unwrap();
        assert_eq!(service.version, installed.version);
        assert_eq!(service.sha256, installed.files[&service.entry]);
    }

    #[test]
    fn keeps_a_prepared_plugin_out_of_the_registry_until_activation() {
        let directory = tempfile::tempdir().unwrap();
        let (package_path, signing_key) = valid_package(directory.path());
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);
        let store_root = directory.path().join("plugins");
        let store = PluginStore::open(store_root.clone()).unwrap();

        let prepared = store
            .prepare_install(
                &package_path,
                &HostEnvironment::current("1.0.0"),
                &keys,
                "operation-1",
            )
            .unwrap();

        assert!(store.list().unwrap().is_empty());
        assert_eq!(
            store
                .read_prepared_asset(&prepared, "ui/index.html")
                .unwrap()
                .bytes,
            b"<!doctype html><title>Codec</title>"
        );

        let installed = store.activate_prepared(prepared).unwrap();
        assert_eq!(installed.app_id, "com.attson.codec");
        assert_eq!(store.list().unwrap().len(), 1);
        assert!(!store_root.join("staging/operation-1").exists());
    }

    #[test]
    fn discards_a_failed_health_probe_without_installing_the_plugin() {
        let directory = tempfile::tempdir().unwrap();
        let (package_path, signing_key) = valid_package(directory.path());
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);
        let store_root = directory.path().join("plugins");
        let store = PluginStore::open(store_root.clone()).unwrap();
        let prepared = store
            .prepare_install(
                &package_path,
                &HostEnvironment::current("1.0.0"),
                &keys,
                "operation-2",
            )
            .unwrap();

        store.discard_prepared(prepared);

        assert!(store.list().unwrap().is_empty());
        assert!(!store_root.join("staging/operation-2").exists());
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

    #[test]
    fn persists_remote_install_failure_identity() {
        let directory = tempfile::tempdir().unwrap();
        let store = PluginStore::open(directory.path().join("plugins")).unwrap();

        store
            .record_catalog_failure(
                "com.attson.codec",
                "1.0.0",
                "update",
                "operation-1",
                "correlation-1",
                "PLUGIN_DOWNLOAD_HASH_MISMATCH",
                &["download".to_string(), "error".to_string()],
                Some("0.9.0"),
            )
            .unwrap();

        let history = store.history().unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].action, "update");
        assert_eq!(history[0].result, "failed");
        assert_eq!(
            history[0].error_code.as_deref(),
            Some("PLUGIN_DOWNLOAD_HASH_MISMATCH")
        );
        assert_eq!(history[0].operation_id.as_deref(), Some("operation-1"));
        assert_eq!(history[0].correlation_id.as_deref(), Some("correlation-1"));
        assert_eq!(history[0].steps, vec!["download", "error"]);
        assert_eq!(history[0].final_available_version.as_deref(), Some("0.9.0"));
    }

    #[test]
    fn migrates_legacy_history_for_remote_operation_identity() {
        let directory = tempfile::tempdir().unwrap();
        let store_root = directory.path().join("plugins");
        std::fs::create_dir_all(&store_root).unwrap();
        let database = store_root.join("registry.sqlite3");
        let connection = rusqlite::Connection::open(database).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE plugin_history (
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
            .unwrap();
        drop(connection);

        let store = PluginStore::open(store_root).unwrap();
        store
            .record_catalog_failure(
                "com.attson.codec",
                "1.0.0",
                "install",
                "operation-legacy",
                "correlation-legacy",
                "PLUGIN_DOWNLOAD_INTERRUPTED",
                &["download".to_string(), "error".to_string()],
                None,
            )
            .unwrap();

        let history = store.history().unwrap();
        assert_eq!(history[0].operation_id.as_deref(), Some("operation-legacy"));
        assert_eq!(
            history[0].correlation_id.as_deref(),
            Some("correlation-legacy")
        );
    }

    #[test]
    fn persists_and_recovers_the_latest_operation_snapshot() {
        let directory = tempfile::tempdir().unwrap();
        let store_root = directory.path().join("plugins");
        let store = PluginStore::open(store_root.clone()).unwrap();
        let mut operation = crate::plugin::remote_install::OperationSnapshot {
            action: "update".to_string(),
            operation_id: "operation-refresh".to_string(),
            correlation_id: "correlation-refresh".to_string(),
            app_id: "com.attson.codec".to_string(),
            name: "编码转换".to_string(),
            version: "1.1.0".to_string(),
            publisher_id: "com.attson".to_string(),
            publisher_name: "Attson".to_string(),
            publisher_key_id: "codec-key".to_string(),
            entry: Some("ui/index.html".to_string()),
            permissions: vec!["host.theme.read".to_string()],
            stage: crate::plugin::remote_install::OperationStage::HealthCheck,
            steps: vec![
                "download".to_string(),
                "verify".to_string(),
                "staging".to_string(),
                "health-check".to_string(),
            ],
            downloaded_bytes: 4096,
            total_bytes: 4096,
            error_code: None,
            message: None,
            final_available_version: Some("1.0.0".to_string()),
        };

        store.save_operation(&operation).unwrap();
        operation
            .advance_to(crate::plugin::remote_install::OperationStage::Error)
            .unwrap();
        operation.error_code = Some("PLUGIN_HEALTH_CHECK_FAILED".to_string());
        operation.message = Some("Application health check failed".to_string());
        store.save_operation(&operation).unwrap();
        drop(store);

        let reopened = PluginStore::open(store_root).unwrap();
        assert_eq!(reopened.latest_operation().unwrap(), Some(operation));
    }

    #[test]
    fn retry_history_keeps_the_original_failed_operation() {
        let directory = tempfile::tempdir().unwrap();
        let store = PluginStore::open(directory.path().join("plugins")).unwrap();
        for operation_id in ["operation-original", "operation-retry"] {
            store
                .record_catalog_failure(
                    "com.attson.codec",
                    "1.1.0",
                    "update",
                    operation_id,
                    &format!("correlation-{operation_id}"),
                    "PLUGIN_DOWNLOAD_INTERRUPTED",
                    &["download".to_string(), "error".to_string()],
                    Some("1.0.0"),
                )
                .unwrap();
        }

        let operation_ids = store
            .history()
            .unwrap()
            .into_iter()
            .map(|record| record.operation_id.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(operation_ids, vec!["operation-retry", "operation-original"]);
    }

    #[test]
    fn atomically_upgrades_current_and_preserves_disabled_state() {
        let directory = tempfile::tempdir().unwrap();
        let (version_one, signing_key) = valid_package_version(directory.path(), "1.0.0");
        let (version_two, _) = valid_package_version(directory.path(), "1.1.0");
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);
        let environment = HostEnvironment::current("1.0.0");
        let store = PluginStore::open(directory.path().join("plugins")).unwrap();
        store.install(&version_one, &environment, &keys).unwrap();
        store.set_enabled("com.attson.codec", false).unwrap();

        let prepared = store
            .prepare_upgrade(&version_two, &environment, &keys, "operation-upgrade")
            .unwrap();
        store
            .mark_pending_upgrade(&prepared, "operation-upgrade", "correlation-upgrade")
            .unwrap();
        assert_eq!(store.list().unwrap()[0].version, "1.0.0");

        let upgraded = store
            .activate_prepared_upgrade(prepared, "operation-upgrade", "correlation-upgrade")
            .unwrap();
        let state = store.version_state("com.attson.codec").unwrap();
        assert_eq!(upgraded.version, "1.1.0");
        assert!(!upgraded.enabled);
        assert_eq!(state.current_version, "1.1.0");
        assert_eq!(state.previous_version.as_deref(), Some("1.0.0"));
        assert_eq!(state.pending_version, None);
        assert!(state.awaiting_launch);

        assert!(store
            .confirm_current_launch("com.attson.codec", "1.1.0")
            .unwrap());
        assert!(
            !store
                .version_state("com.attson.codec")
                .unwrap()
                .awaiting_launch
        );
    }

    #[test]
    fn permission_reduction_preserves_disabled_state_and_user_data() {
        let directory = tempfile::tempdir().unwrap();
        let (version_one, signing_key) = valid_package_version_with_permissions(
            directory.path(),
            "1.0.0",
            "[\"host.theme.read\",\"host.notification.show\"]",
        );
        let (version_two, _) = valid_package_version(directory.path(), "1.1.0");
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);
        let environment = HostEnvironment::current("1.0.0");
        let store_root = directory.path().join("plugins");
        let store = PluginStore::open(store_root.clone()).unwrap();
        store.install(&version_one, &environment, &keys).unwrap();
        store.set_enabled("com.attson.codec", false).unwrap();
        let data_file = store_root.join("data/com.attson.codec/settings.json");
        fs::create_dir_all(data_file.parent().unwrap()).unwrap();
        fs::write(&data_file, b"user settings").unwrap();

        let prepared = store
            .prepare_upgrade(&version_two, &environment, &keys, "operation-reduced")
            .unwrap();
        store
            .mark_pending_upgrade(&prepared, "operation-reduced", "correlation-reduced")
            .unwrap();
        let upgraded = store
            .activate_prepared_upgrade(prepared, "operation-reduced", "correlation-reduced")
            .unwrap();

        assert!(!upgraded.enabled);
        assert_eq!(upgraded.permissions, ["host.theme.read"]);
        assert_eq!(fs::read(data_file).unwrap(), b"user settings");
    }

    #[test]
    fn rolls_back_the_first_failed_launch_to_previous() {
        let directory = tempfile::tempdir().unwrap();
        let (version_one, signing_key) = valid_package_version(directory.path(), "1.0.0");
        let (version_two, _) = valid_package_version(directory.path(), "1.1.0");
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);
        let environment = HostEnvironment::current("1.0.0");
        let store_root = directory.path().join("plugins");
        let store = PluginStore::open(store_root.clone()).unwrap();
        store.install(&version_one, &environment, &keys).unwrap();
        let prepared = store
            .prepare_upgrade(&version_two, &environment, &keys, "operation-rollback")
            .unwrap();
        store
            .mark_pending_upgrade(&prepared, "operation-rollback", "correlation-rollback")
            .unwrap();
        store
            .activate_prepared_upgrade(prepared, "operation-rollback", "correlation-rollback")
            .unwrap();

        let rollback = store
            .rollback_current_launch("com.attson.codec", "1.1.0")
            .unwrap()
            .expect("the first failed launch must roll back");

        assert_eq!(rollback.installed.version, "1.0.0");
        assert_eq!(rollback.operation_id, "operation-rollback");
        assert!(!store_root.join("apps/com.attson.codec/1.1.0").exists());
        assert!(store_root.join("apps/com.attson.codec/1.0.0").exists());
        assert_eq!(store.history().unwrap()[0].result, "rolled-back");
    }

    #[test]
    fn restart_discards_pending_upgrade_and_keeps_current() {
        let directory = tempfile::tempdir().unwrap();
        let (version_one, signing_key) = valid_package_version(directory.path(), "1.0.0");
        let (version_two, _) = valid_package_version(directory.path(), "1.1.0");
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);
        let environment = HostEnvironment::current("1.0.0");
        let store_root = directory.path().join("plugins");
        let store = PluginStore::open(store_root.clone()).unwrap();
        store.install(&version_one, &environment, &keys).unwrap();
        let prepared = store
            .prepare_upgrade(&version_two, &environment, &keys, "operation-restart")
            .unwrap();
        store
            .mark_pending_upgrade(&prepared, "operation-restart", "correlation-restart")
            .unwrap();
        drop(prepared);
        drop(store);

        let reopened = PluginStore::open(store_root.clone()).unwrap();
        let state = reopened.version_state("com.attson.codec").unwrap();
        assert_eq!(state.current_version, "1.0.0");
        assert_eq!(state.pending_version, None);
        assert!(!store_root.join("staging/operation-restart").exists());
    }

    #[test]
    fn migrates_a_legacy_install_into_the_version_registry_without_losing_state() {
        let directory = tempfile::tempdir().unwrap();
        let store_root = directory.path().join("plugins");
        let app_root = store_root.join("apps/com.attson.codec/1.0.0");
        let data_root = store_root.join("data/com.attson.codec");
        fs::create_dir_all(app_root.join("ui")).unwrap();
        fs::create_dir_all(&data_root).unwrap();
        let entry = b"<!doctype html><title>Legacy Codec</title>";
        fs::write(app_root.join("ui/index.html"), entry).unwrap();
        fs::write(data_root.join("settings.json"), b"{\"mode\":\"legacy\"}").unwrap();
        let database = store_root.join("registry.sqlite3");
        let connection = rusqlite::Connection::open(&database).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE installed_plugins (
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
                );",
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO installed_plugins (
                    app_id, name, version, publisher_id, publisher_name, publisher_key_id,
                    entry, permissions_json, files_json, enabled, installed_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, ?10)",
                rusqlite::params![
                    "com.attson.codec",
                    "编码转换",
                    "1.0.0",
                    "com.attson",
                    "Attson",
                    "attool-p0-test",
                    "ui/index.html",
                    "[\"host.theme.read\"]",
                    format!("{{\"ui/index.html\":\"{}\"}}", sha256(entry)),
                    "2026-11-01T00:00:00Z",
                ],
            )
            .unwrap();
        drop(connection);

        let store = PluginStore::open(store_root.clone()).unwrap();
        let installed = store.list().unwrap();
        let state = store.version_state("com.attson.codec").unwrap();
        let version_count: i64 = store
            .connection()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM plugin_versions WHERE app_id = ?1 AND version = ?2",
                rusqlite::params!["com.attson.codec", "1.0.0"],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].version, "1.0.0");
        assert!(!installed[0].enabled);
        assert_eq!(installed[0].permissions, ["host.theme.read"]);
        assert_eq!(state.current_version, "1.0.0");
        assert_eq!(state.previous_version, None);
        assert_eq!(version_count, 1);
        assert_eq!(
            fs::read_to_string(data_root.join("settings.json")).unwrap(),
            "{\"mode\":\"legacy\"}"
        );
    }

    #[test]
    fn a_second_successful_upgrade_keeps_only_one_previous_generation() {
        let directory = tempfile::tempdir().unwrap();
        let (version_one, signing_key) = valid_package_version(directory.path(), "1.0.0");
        let (version_two, _) = valid_package_version(directory.path(), "1.1.0");
        let (version_three, _) = valid_package_version(directory.path(), "1.2.0");
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);
        let environment = HostEnvironment::current("1.0.0");
        let store_root = directory.path().join("plugins");
        let store = PluginStore::open(store_root.clone()).unwrap();
        store.install(&version_one, &environment, &keys).unwrap();

        let prepared = store
            .prepare_upgrade(&version_two, &environment, &keys, "upgrade-one")
            .unwrap();
        store
            .mark_pending_upgrade(&prepared, "upgrade-one", "correlation-one")
            .unwrap();
        store
            .activate_prepared_upgrade(prepared, "upgrade-one", "correlation-one")
            .unwrap();
        store
            .confirm_current_launch("com.attson.codec", "1.1.0")
            .unwrap();

        let prepared = store
            .prepare_upgrade(&version_three, &environment, &keys, "upgrade-two")
            .unwrap();
        store
            .mark_pending_upgrade(&prepared, "upgrade-two", "correlation-two")
            .unwrap();
        store
            .activate_prepared_upgrade(prepared, "upgrade-two", "correlation-two")
            .unwrap();

        let state = store.version_state("com.attson.codec").unwrap();
        let versions: Vec<String> = {
            let connection = store.connection().unwrap();
            let mut statement = connection
                .prepare("SELECT version FROM plugin_versions WHERE app_id = ?1 ORDER BY version")
                .unwrap();
            statement
                .query_map(["com.attson.codec"], |row| row.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        assert_eq!(state.current_version, "1.2.0");
        assert_eq!(state.previous_version.as_deref(), Some("1.1.0"));
        assert_eq!(versions, ["1.1.0", "1.2.0"]);
        assert!(!store_root.join("apps/com.attson.codec/1.0.0").exists());
    }

    #[test]
    fn activation_transaction_failure_keeps_current_and_allows_retry() {
        let directory = tempfile::tempdir().unwrap();
        let (version_one, signing_key) = valid_package_version(directory.path(), "1.0.0");
        let (version_two, _) = valid_package_version(directory.path(), "1.1.0");
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);
        let environment = HostEnvironment::current("1.0.0");
        let store_root = directory.path().join("plugins");
        let store = PluginStore::open(store_root.clone()).unwrap();
        store.install(&version_one, &environment, &keys).unwrap();
        let prepared = store
            .prepare_upgrade(&version_two, &environment, &keys, "failed-activation")
            .unwrap();
        store
            .mark_pending_upgrade(&prepared, "failed-activation", "failed-correlation")
            .unwrap();
        store
            .connection()
            .unwrap()
            .execute_batch(
                "CREATE TRIGGER fail_upgrade_activation
                 BEFORE UPDATE OF version ON installed_plugins
                 WHEN NEW.version = '1.1.0'
                 BEGIN SELECT RAISE(ABORT, 'injected activation failure'); END;",
            )
            .unwrap();

        let error = store
            .activate_prepared_upgrade(prepared, "failed-activation", "failed-correlation")
            .unwrap_err();
        store
            .clear_pending_upgrade("com.attson.codec", "failed-activation")
            .unwrap();
        assert_eq!(error.code, "PLUGIN_REGISTRY_WRITE_FAILED");
        assert_eq!(store.list().unwrap()[0].version, "1.0.0");
        assert!(!store_root.join("apps/com.attson.codec/1.1.0").exists());

        store
            .connection()
            .unwrap()
            .execute_batch("DROP TRIGGER fail_upgrade_activation")
            .unwrap();
        let prepared = store
            .prepare_upgrade(&version_two, &environment, &keys, "retry-activation")
            .unwrap();
        store
            .mark_pending_upgrade(&prepared, "retry-activation", "retry-correlation")
            .unwrap();
        let upgraded = store
            .activate_prepared_upgrade(prepared, "retry-activation", "retry-correlation")
            .unwrap();
        assert_eq!(upgraded.version, "1.1.0");
    }

    #[test]
    fn rollback_failure_keeps_the_failed_current_version_active() {
        let directory = tempfile::tempdir().unwrap();
        let (version_one, signing_key) = valid_package_version(directory.path(), "1.0.0");
        let (version_two, _) = valid_package_version(directory.path(), "1.1.0");
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);
        let environment = HostEnvironment::current("1.0.0");
        let store_root = directory.path().join("plugins");
        let store = PluginStore::open(store_root.clone()).unwrap();
        store.install(&version_one, &environment, &keys).unwrap();
        let prepared = store
            .prepare_upgrade(&version_two, &environment, &keys, "rollback-failure")
            .unwrap();
        store
            .mark_pending_upgrade(&prepared, "rollback-failure", "rollback-correlation")
            .unwrap();
        store
            .activate_prepared_upgrade(prepared, "rollback-failure", "rollback-correlation")
            .unwrap();
        fs::remove_dir_all(store_root.join("apps/com.attson.codec/1.0.0")).unwrap();

        let error = store
            .rollback_current_launch("com.attson.codec", "1.1.0")
            .unwrap_err();
        let state = store.version_state("com.attson.codec").unwrap();

        assert_eq!(error.code, "PLUGIN_ROLLBACK_FAILED");
        assert_eq!(state.current_version, "1.1.0");
        assert_eq!(state.previous_version.as_deref(), Some("1.0.0"));
        assert!(state.awaiting_launch);
        assert!(store_root.join("apps/com.attson.codec/1.1.0").exists());
    }

    #[test]
    fn restart_while_awaiting_launch_preserves_rollback_capability() {
        let directory = tempfile::tempdir().unwrap();
        let (version_one, signing_key) = valid_package_version(directory.path(), "1.0.0");
        let (version_two, _) = valid_package_version(directory.path(), "1.1.0");
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);
        let environment = HostEnvironment::current("1.0.0");
        let store_root = directory.path().join("plugins");
        let store = PluginStore::open(store_root.clone()).unwrap();
        store.install(&version_one, &environment, &keys).unwrap();
        let prepared = store
            .prepare_upgrade(&version_two, &environment, &keys, "restart-launch")
            .unwrap();
        store
            .mark_pending_upgrade(&prepared, "restart-launch", "restart-correlation")
            .unwrap();
        store
            .activate_prepared_upgrade(prepared, "restart-launch", "restart-correlation")
            .unwrap();
        drop(store);

        let reopened = PluginStore::open(store_root).unwrap();
        assert!(
            reopened
                .version_state("com.attson.codec")
                .unwrap()
                .awaiting_launch
        );
        let rollback = reopened
            .rollback_current_launch("com.attson.codec", "1.1.0")
            .unwrap()
            .unwrap();
        assert_eq!(rollback.installed.version, "1.0.0");
    }

    #[test]
    fn restart_restores_a_failed_version_staged_by_an_interrupted_rollback() {
        let directory = tempfile::tempdir().unwrap();
        let (version_one, signing_key) = valid_package_version(directory.path(), "1.0.0");
        let (version_two, _) = valid_package_version(directory.path(), "1.1.0");
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);
        let environment = HostEnvironment::current("1.0.0");
        let store_root = directory.path().join("plugins");
        let store = PluginStore::open(store_root.clone()).unwrap();
        store.install(&version_one, &environment, &keys).unwrap();
        let prepared = store
            .prepare_upgrade(&version_two, &environment, &keys, "interrupted-rollback")
            .unwrap();
        store
            .mark_pending_upgrade(&prepared, "interrupted-rollback", "interrupted-correlation")
            .unwrap();
        store
            .activate_prepared_upgrade(prepared, "interrupted-rollback", "interrupted-correlation")
            .unwrap();

        let rollback_root = store_root.join("staging/rollback-interrupted");
        fs::create_dir_all(&rollback_root).unwrap();
        fs::write(
            rollback_root.join("rollback.json"),
            br#"{"appId":"com.attson.codec","failedVersion":"1.1.0"}"#,
        )
        .unwrap();
        fs::rename(
            store_root.join("apps/com.attson.codec/1.1.0"),
            rollback_root.join("failed-current"),
        )
        .unwrap();
        drop(store);

        let reopened = PluginStore::open(store_root.clone()).unwrap();
        assert!(store_root.join("apps/com.attson.codec/1.1.0").exists());
        assert!(!rollback_root.exists());
        let rollback = reopened
            .rollback_current_launch("com.attson.codec", "1.1.0")
            .unwrap()
            .unwrap();
        assert_eq!(rollback.installed.version, "1.0.0");
    }

    #[test]
    fn concurrent_disable_and_upgrade_preserve_the_disabled_state() {
        let directory = tempfile::tempdir().unwrap();
        let (version_one, signing_key) = valid_package_version(directory.path(), "1.0.0");
        let (version_two, _) = valid_package_version(directory.path(), "1.1.0");
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);
        let environment = HostEnvironment::current("1.0.0");
        let store = PluginStore::open(directory.path().join("plugins")).unwrap();
        store.install(&version_one, &environment, &keys).unwrap();
        let prepared = store
            .prepare_upgrade(&version_two, &environment, &keys, "concurrent-upgrade")
            .unwrap();
        store
            .mark_pending_upgrade(&prepared, "concurrent-upgrade", "concurrent-correlation")
            .unwrap();
        let store = Arc::new(Mutex::new(store));
        let barrier = Arc::new(Barrier::new(3));

        let upgrade_store = Arc::clone(&store);
        let upgrade_barrier = Arc::clone(&barrier);
        let upgrade = thread::spawn(move || {
            upgrade_barrier.wait();
            upgrade_store.lock().unwrap().activate_prepared_upgrade(
                prepared,
                "concurrent-upgrade",
                "concurrent-correlation",
            )
        });
        let disable_store = Arc::clone(&store);
        let disable_barrier = Arc::clone(&barrier);
        let disable = thread::spawn(move || {
            disable_barrier.wait();
            disable_store
                .lock()
                .unwrap()
                .set_enabled("com.attson.codec", false)
        });
        barrier.wait();

        upgrade.join().unwrap().unwrap();
        disable.join().unwrap().unwrap();
        let installed = store.lock().unwrap().list().unwrap().remove(0);
        assert_eq!(installed.version, "1.1.0");
        assert!(!installed.enabled);
    }

    #[test]
    fn uninstall_rejects_a_pending_upgrade_and_succeeds_after_it_is_cleared() {
        let directory = tempfile::tempdir().unwrap();
        let (version_one, signing_key) = valid_package_version(directory.path(), "1.0.0");
        let (version_two, _) = valid_package_version(directory.path(), "1.1.0");
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);
        let environment = HostEnvironment::current("1.0.0");
        let store = PluginStore::open(directory.path().join("plugins")).unwrap();
        store.install(&version_one, &environment, &keys).unwrap();
        let prepared = store
            .prepare_upgrade(&version_two, &environment, &keys, "pending-uninstall")
            .unwrap();
        store
            .mark_pending_upgrade(&prepared, "pending-uninstall", "pending-correlation")
            .unwrap();

        let error = store.uninstall("com.attson.codec", false).unwrap_err();
        assert_eq!(error.code, "PLUGIN_UPDATE_STATE_CONFLICT");
        assert_eq!(store.list().unwrap()[0].version, "1.0.0");

        store
            .clear_pending_upgrade("com.attson.codec", "pending-uninstall")
            .unwrap();
        store.discard_prepared(prepared);
        store.uninstall("com.attson.codec", false).unwrap();
        assert!(store.list().unwrap().is_empty());
    }
}
