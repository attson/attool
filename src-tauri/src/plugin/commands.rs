use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use ed25519_dalek::VerifyingKey;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use super::{
    catalog::{CatalogError, CatalogInstallPlan, CatalogRuntime, CatalogSnapshot},
    install::{
        InstallError, InstalledPlugin, PluginAsset, PluginHistoryRecord, PluginStore,
        PreparedPlugin,
    },
    package::{inspect_package, HostEnvironment, InspectedPackage, PackageError},
    remote_install::{
        download_artifact, validate_accepted_permissions, validate_package_against_catalog,
        OperationSnapshot, OperationStage, RemoteInstallError,
    },
};

const P0_TEST_PUBLIC_KEY: [u8; 32] = [
    0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
    0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
];

#[derive(Clone)]
pub struct PluginRuntime {
    store: Arc<Mutex<PluginStore>>,
    environment: HostEnvironment,
    trusted_keys: Arc<HashMap<String, VerifyingKey>>,
    client: reqwest::Client,
    operations: Arc<Mutex<HashMap<String, OperationSnapshot>>>,
    pending: Arc<Mutex<HashMap<String, PendingCatalogInstall>>>,
}

struct PendingCatalogInstall {
    prepared: PreparedPlugin,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogInstallResult {
    pub operation: OperationSnapshot,
    pub installed: InstalledPlugin,
}

impl PluginRuntime {
    pub fn p0(store: PluginStore) -> Result<Self, String> {
        let key = VerifyingKey::from_bytes(&P0_TEST_PUBLIC_KEY)
            .map_err(|error| format!("invalid P0 plugin public key: {error}"))?;
        Ok(Self {
            store: Arc::new(Mutex::new(store)),
            environment: HostEnvironment::current("1.0.0"),
            trusted_keys: Arc::new(HashMap::from([("attool-p0-test".to_string(), key)])),
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(60))
                .redirect(reqwest::redirect::Policy::limited(3))
                .build()
                .map_err(|error| format!("cannot create plugin download client: {error}"))?,
            operations: Arc::new(Mutex::new(HashMap::new())),
            pending: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    fn inspect(&self, path: PathBuf) -> Result<InspectedPackage, PluginCommandError> {
        inspect_package(&path, &self.environment, &self.trusted_keys).map_err(Into::into)
    }

    fn install(&self, path: PathBuf) -> Result<InstalledPlugin, PluginCommandError> {
        self.store
            .lock()
            .map_err(|_| {
                PluginCommandError::new("PLUGIN_STORE_LOCK_FAILED", "Plugin store lock failed")
            })?
            .install(&path, &self.environment, &self.trusted_keys)
            .map_err(Into::into)
    }

    fn list(&self) -> Result<Vec<InstalledPlugin>, PluginCommandError> {
        self.store
            .lock()
            .map_err(|_| {
                PluginCommandError::new("PLUGIN_STORE_LOCK_FAILED", "Plugin store lock failed")
            })?
            .list()
            .map_err(Into::into)
    }

    fn history(&self) -> Result<Vec<PluginHistoryRecord>, PluginCommandError> {
        self.store
            .lock()
            .map_err(|_| {
                PluginCommandError::new("PLUGIN_STORE_LOCK_FAILED", "Plugin store lock failed")
            })?
            .history()
            .map_err(Into::into)
    }

    fn set_enabled(&self, app_id: String, enabled: bool) -> Result<(), PluginCommandError> {
        self.store
            .lock()
            .map_err(|_| {
                PluginCommandError::new("PLUGIN_STORE_LOCK_FAILED", "Plugin store lock failed")
            })?
            .set_enabled(&app_id, enabled)
            .map_err(Into::into)
    }

    fn uninstall(&self, app_id: String, delete_data: bool) -> Result<(), PluginCommandError> {
        self.store
            .lock()
            .map_err(|_| {
                PluginCommandError::new("PLUGIN_STORE_LOCK_FAILED", "Plugin store lock failed")
            })?
            .uninstall(&app_id, delete_data)
            .map_err(Into::into)
    }

    pub(super) fn read_asset(
        &self,
        app_id: &str,
        relative: &str,
    ) -> Result<PluginAsset, PluginCommandError> {
        self.store
            .lock()
            .map_err(|_| {
                PluginCommandError::new("PLUGIN_STORE_LOCK_FAILED", "Plugin store lock failed")
            })?
            .read_asset(app_id, relative)
            .map_err(Into::into)
    }

    pub(super) fn read_probe_asset(
        &self,
        operation_id: &str,
        relative: &str,
    ) -> Result<PluginAsset, PluginCommandError> {
        let pending = self.pending.lock().map_err(|_| {
            PluginCommandError::new(
                "PLUGIN_OPERATION_LOCK_FAILED",
                "Plugin operation lock failed",
            )
        })?;
        let operation = pending.get(operation_id).ok_or_else(|| {
            PluginCommandError::new(
                "PLUGIN_OPERATION_NOT_FOUND",
                "Plugin operation was not found",
            )
        })?;
        self.store
            .lock()
            .map_err(|_| {
                PluginCommandError::new("PLUGIN_STORE_LOCK_FAILED", "Plugin store lock failed")
            })?
            .read_prepared_asset(&operation.prepared, relative)
            .map_err(Into::into)
    }

    async fn start_catalog_install(
        &self,
        plan: CatalogInstallPlan,
        accepted_permissions: Vec<String>,
        app: &AppHandle,
    ) -> Result<OperationSnapshot, PluginCommandError> {
        validate_accepted_permissions(&accepted_permissions, &plan.permissions)?;
        let operation_id = Uuid::new_v4().to_string();
        let correlation_id = Uuid::new_v4().to_string();
        let snapshot = OperationSnapshot::new(operation_id.clone(), correlation_id.clone(), &plan);
        self.reserve_operation(snapshot.clone())?;
        let _ = app.emit("plugin://operation-state", snapshot);

        let download_path = self
            .store
            .lock()
            .map_err(|_| {
                PluginCommandError::with_correlation(
                    "PLUGIN_STORE_LOCK_FAILED",
                    "Plugin store lock failed",
                    &correlation_id,
                )
            })?
            .catalog_download_path(&operation_id, &plan.artifact.file_name);
        let runtime = self.clone();
        let event_app = app.clone();
        let progress_operation_id = operation_id.clone();
        if let Err(error) = download_artifact(
            &self.client,
            &plan.artifact,
            &download_path,
            move |downloaded| {
                runtime.update_operation(&event_app, &progress_operation_id, |state| {
                    state.downloaded_bytes = downloaded;
                });
            },
        )
        .await
        {
            self.remove_download(&operation_id);
            return Err(self.fail_operation(app, &operation_id, error.code, error.message));
        }

        self.update_operation(app, &operation_id, |state| {
            state.stage = OperationStage::Verify;
        });
        let prepare_runtime = self.clone();
        let prepare_plan = plan.clone();
        let prepare_operation_id = operation_id.clone();
        let prepared = match tauri::async_runtime::spawn_blocking(move || {
            prepare_runtime.prepare_catalog_package(
                &download_path,
                &prepare_operation_id,
                &prepare_plan,
            )
        })
        .await
        {
            Ok(prepared) => prepared,
            Err(error) => {
                self.remove_download(&operation_id);
                return Err(self.fail_operation(
                    app,
                    &operation_id,
                    "PLUGIN_WORKER_FAILED",
                    format!("Plugin verification worker failed: {error}"),
                ));
            }
        };
        self.remove_download(&operation_id);
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                return Err(self.fail_operation(app, &operation_id, &error.code, error.message));
            }
        };

        self.update_operation(app, &operation_id, |state| {
            state.stage = OperationStage::Staging;
            state.entry = Some(prepared.package.entry.clone());
        });
        match self.pending.lock() {
            Ok(mut pending) => {
                pending.insert(operation_id.clone(), PendingCatalogInstall { prepared });
            }
            Err(_) => {
                if let Ok(store) = self.store.lock() {
                    store.discard_prepared(prepared);
                }
                return Err(self.fail_operation(
                    app,
                    &operation_id,
                    "PLUGIN_OPERATION_LOCK_FAILED",
                    "Plugin operation lock failed",
                ));
            }
        }
        self.update_operation(app, &operation_id, |state| {
            state.stage = OperationStage::HealthCheck;
        });
        self.operation(&operation_id)
    }

    fn prepare_catalog_package(
        &self,
        package_path: &std::path::Path,
        operation_id: &str,
        plan: &CatalogInstallPlan,
    ) -> Result<PreparedPlugin, PluginCommandError> {
        let keys = HashMap::from([(plan.publisher_key_id.clone(), plan.publisher_key)]);
        let inspected = inspect_package(package_path, &self.environment, &keys)?;
        validate_package_against_catalog(&inspected, plan)?;
        self.store
            .lock()
            .map_err(|_| {
                PluginCommandError::new("PLUGIN_STORE_LOCK_FAILED", "Plugin store lock failed")
            })?
            .prepare_install(package_path, &self.environment, &keys, operation_id)
            .map_err(Into::into)
    }

    fn complete_catalog_install(
        &self,
        operation_id: &str,
        app: &AppHandle,
    ) -> Result<CatalogInstallResult, PluginCommandError> {
        let correlation_id = self.operation(operation_id)?.correlation_id;
        let mut pending_operations = self.pending.lock().map_err(|_| {
            PluginCommandError::with_correlation(
                "PLUGIN_OPERATION_LOCK_FAILED",
                "Plugin operation lock failed",
                &correlation_id,
            )
        })?;
        if !pending_operations.contains_key(operation_id) {
            return Err(PluginCommandError::with_correlation(
                "PLUGIN_OPERATION_NOT_PENDING",
                "Plugin operation is not waiting for health check",
                &correlation_id,
            ));
        }
        let store = self.store.lock().map_err(|_| {
            PluginCommandError::with_correlation(
                "PLUGIN_STORE_LOCK_FAILED",
                "Plugin store lock failed",
                &correlation_id,
            )
        })?;
        let pending = pending_operations
            .remove(operation_id)
            .expect("pending operation was checked while locked");
        drop(pending_operations);
        self.update_operation(app, operation_id, |state| {
            state.stage = OperationStage::Activate;
        });
        let activation = store.activate_prepared_with_context(
            pending.prepared,
            Some(operation_id),
            Some(&correlation_id),
        );
        drop(store);
        let installed = match activation {
            Ok(installed) => installed,
            Err(error) => {
                return Err(self.fail_operation(app, operation_id, &error.code, error.message));
            }
        };
        self.update_operation(app, operation_id, |state| {
            state.stage = OperationStage::Complete;
        });
        Ok(CatalogInstallResult {
            operation: self.operation(operation_id)?,
            installed,
        })
    }

    fn fail_health_check(
        &self,
        operation_id: &str,
        app: &AppHandle,
    ) -> Result<OperationSnapshot, PluginCommandError> {
        let correlation_id = self.operation(operation_id)?.correlation_id;
        let mut pending_operations = self.pending.lock().map_err(|_| {
            PluginCommandError::with_correlation(
                "PLUGIN_OPERATION_LOCK_FAILED",
                "Plugin operation lock failed",
                &correlation_id,
            )
        })?;
        if !pending_operations.contains_key(operation_id) {
            return Err(PluginCommandError::with_correlation(
                "PLUGIN_OPERATION_NOT_PENDING",
                "Plugin operation is not waiting for health check",
                &correlation_id,
            ));
        }
        let store = self.store.lock().map_err(|_| {
            PluginCommandError::with_correlation(
                "PLUGIN_STORE_LOCK_FAILED",
                "Plugin store lock failed",
                &correlation_id,
            )
        })?;
        let pending = pending_operations
            .remove(operation_id)
            .expect("pending operation was checked while locked");
        drop(pending_operations);
        store.discard_prepared(pending.prepared);
        drop(store);
        self.fail_operation(
            app,
            operation_id,
            "PLUGIN_HEALTH_CHECK_FAILED",
            "Plugin did not complete the isolated Host API handshake",
        );
        self.operation(operation_id)
    }

    fn operation(&self, operation_id: &str) -> Result<OperationSnapshot, PluginCommandError> {
        self.operations
            .lock()
            .map_err(|_| {
                PluginCommandError::new(
                    "PLUGIN_OPERATION_LOCK_FAILED",
                    "Plugin operation lock failed",
                )
            })?
            .get(operation_id)
            .cloned()
            .ok_or_else(|| {
                PluginCommandError::new(
                    "PLUGIN_OPERATION_NOT_FOUND",
                    "Plugin operation was not found",
                )
            })
    }

    fn reserve_operation(&self, snapshot: OperationSnapshot) -> Result<(), PluginCommandError> {
        let mut operations = self.operations.lock().map_err(|_| {
            PluginCommandError::with_correlation(
                "PLUGIN_OPERATION_LOCK_FAILED",
                "Plugin operation lock failed",
                &snapshot.correlation_id,
            )
        })?;
        if operations.values().any(|operation| {
            operation.app_id == snapshot.app_id
                && !matches!(
                    operation.stage,
                    OperationStage::Complete | OperationStage::Error
                )
        }) {
            return Err(PluginCommandError::with_correlation(
                "PLUGIN_INSTALL_IN_PROGRESS",
                "Another install operation is already active for this application",
                &snapshot.correlation_id,
            ));
        }
        operations.insert(snapshot.operation_id.clone(), snapshot);
        Ok(())
    }

    fn update_operation(
        &self,
        app: &AppHandle,
        operation_id: &str,
        update: impl FnOnce(&mut OperationSnapshot),
    ) {
        let snapshot = self.operations.lock().ok().and_then(|mut operations| {
            let state = operations.get_mut(operation_id)?;
            update(state);
            Some(state.clone())
        });
        if let Some(snapshot) = snapshot {
            let _ = app.emit("plugin://operation-state", snapshot);
        }
    }

    fn fail_operation(
        &self,
        app: &AppHandle,
        operation_id: &str,
        code: &str,
        message: impl Into<String>,
    ) -> PluginCommandError {
        let message = message.into();
        let previous = self.operation(operation_id).ok();
        self.update_operation(app, operation_id, |state| {
            state.stage = OperationStage::Error;
            state.error_code = Some(code.to_string());
            state.message = Some(message.clone());
        });
        if let Some(previous) = previous.filter(|state| state.stage != OperationStage::Error) {
            if let Ok(store) = self.store.lock() {
                if let Err(error) = store.record_catalog_failure(
                    &previous.app_id,
                    &previous.version,
                    &previous.operation_id,
                    &previous.correlation_id,
                    code,
                    previous.stage.as_str(),
                ) {
                    eprintln!("cannot record plugin operation failure: {error}");
                }
            }
        }
        let correlation_id = self
            .operation(operation_id)
            .map(|operation| operation.correlation_id)
            .unwrap_or_default();
        PluginCommandError::with_correlation(code, message, &correlation_id)
    }

    fn remove_download(&self, operation_id: &str) {
        if let Ok(store) = self.store.lock() {
            store.remove_catalog_download(operation_id);
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCommandError {
    pub code: String,
    pub message: String,
    pub correlation_id: String,
}

impl PluginCommandError {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            correlation_id: Uuid::new_v4().to_string(),
        }
    }

    fn with_correlation(
        code: impl Into<String>,
        message: impl Into<String>,
        correlation_id: &str,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            correlation_id: correlation_id.to_string(),
        }
    }
}

impl From<CatalogError> for PluginCommandError {
    fn from(error: CatalogError) -> Self {
        Self::new(error.code, error.message)
    }
}

impl From<PackageError> for PluginCommandError {
    fn from(error: PackageError) -> Self {
        Self::new(error.code, error.message)
    }
}

impl From<InstallError> for PluginCommandError {
    fn from(error: InstallError) -> Self {
        Self::new(error.code, error.message)
    }
}

impl From<RemoteInstallError> for PluginCommandError {
    fn from(error: RemoteInstallError) -> Self {
        Self::new(error.code, error.message)
    }
}

fn join_error(error: tauri::Error) -> PluginCommandError {
    PluginCommandError::new(
        "PLUGIN_WORKER_FAILED",
        format!("Plugin worker failed: {error}"),
    )
}

#[tauri::command]
pub async fn plugin_inspect_package(
    path: String,
    runtime: State<'_, PluginRuntime>,
) -> Result<InspectedPackage, PluginCommandError> {
    let runtime = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || runtime.inspect(PathBuf::from(path)))
        .await
        .map_err(join_error)?
}

#[tauri::command]
pub async fn plugin_install_package(
    path: String,
    runtime: State<'_, PluginRuntime>,
) -> Result<InstalledPlugin, PluginCommandError> {
    let runtime = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || runtime.install(PathBuf::from(path)))
        .await
        .map_err(join_error)?
}

#[tauri::command]
pub async fn plugin_list_installed(
    runtime: State<'_, PluginRuntime>,
) -> Result<Vec<InstalledPlugin>, PluginCommandError> {
    let runtime = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || runtime.list())
        .await
        .map_err(join_error)?
}

#[tauri::command]
pub async fn plugin_list_history(
    runtime: State<'_, PluginRuntime>,
) -> Result<Vec<PluginHistoryRecord>, PluginCommandError> {
    let runtime = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || runtime.history())
        .await
        .map_err(join_error)?
}

#[tauri::command]
pub async fn plugin_set_enabled(
    app_id: String,
    enabled: bool,
    runtime: State<'_, PluginRuntime>,
) -> Result<(), PluginCommandError> {
    let runtime = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || runtime.set_enabled(app_id, enabled))
        .await
        .map_err(join_error)?
}

#[tauri::command]
pub async fn plugin_uninstall(
    app_id: String,
    delete_data: bool,
    runtime: State<'_, PluginRuntime>,
) -> Result<(), PluginCommandError> {
    let runtime = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || runtime.uninstall(app_id, delete_data))
        .await
        .map_err(join_error)?
}

#[tauri::command]
pub async fn plugin_catalog_refresh(
    runtime: State<'_, CatalogRuntime>,
) -> Result<CatalogSnapshot, PluginCommandError> {
    runtime.refresh().await.map_err(Into::into)
}

#[tauri::command]
pub async fn plugin_catalog_list(
    runtime: State<'_, CatalogRuntime>,
) -> Result<Option<CatalogSnapshot>, PluginCommandError> {
    runtime.list().await.map_err(Into::into)
}

#[tauri::command]
pub async fn plugin_install_catalog_app(
    app_id: String,
    accepted_permissions: Vec<String>,
    runtime: State<'_, PluginRuntime>,
    catalog: State<'_, CatalogRuntime>,
    app: AppHandle,
) -> Result<OperationSnapshot, PluginCommandError> {
    let plan = catalog.install_plan(&app_id).await?;
    runtime
        .start_catalog_install(plan, accepted_permissions, &app)
        .await
}

#[tauri::command]
pub async fn plugin_complete_catalog_install(
    operation_id: String,
    runtime: State<'_, PluginRuntime>,
    app: AppHandle,
) -> Result<CatalogInstallResult, PluginCommandError> {
    let runtime = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        runtime.complete_catalog_install(&operation_id, &app)
    })
    .await
    .map_err(join_error)?
}

#[tauri::command]
pub async fn plugin_fail_catalog_install(
    operation_id: String,
    runtime: State<'_, PluginRuntime>,
    app: AppHandle,
) -> Result<OperationSnapshot, PluginCommandError> {
    let runtime = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || runtime.fail_health_check(&operation_id, &app))
        .await
        .map_err(join_error)?
}

#[tauri::command]
pub async fn plugin_get_operation(
    operation_id: String,
    runtime: State<'_, PluginRuntime>,
) -> Result<OperationSnapshot, PluginCommandError> {
    runtime.operation(&operation_id)
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;

    use super::{PluginRuntime, P0_TEST_PUBLIC_KEY};
    use crate::plugin::install::PluginStore;
    use crate::plugin::remote_install::{OperationSnapshot, OperationStage};

    fn operation(operation_id: &str, stage: OperationStage) -> OperationSnapshot {
        OperationSnapshot {
            operation_id: operation_id.to_string(),
            correlation_id: format!("correlation-{operation_id}"),
            app_id: "com.attson.codec".to_string(),
            name: "编码转换".to_string(),
            version: "1.0.0".to_string(),
            publisher_id: "com.attson".to_string(),
            publisher_name: "Attson".to_string(),
            publisher_key_id: "codec-key".to_string(),
            entry: None,
            permissions: vec!["host.theme.read".to_string()],
            stage,
            downloaded_bytes: 0,
            total_bytes: 1,
            error_code: None,
            message: None,
        }
    }

    #[test]
    fn p0_runtime_trusts_the_sdk_test_key() {
        let directory = tempfile::tempdir().unwrap();
        let runtime =
            PluginRuntime::p0(PluginStore::open(directory.path().join("plugins")).unwrap())
                .unwrap();
        let seed = [
            0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec,
            0x2c, 0xc4, 0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03,
            0x1c, 0xae, 0x7f, 0x60,
        ];

        assert_eq!(
            SigningKey::from_bytes(&seed).verifying_key().to_bytes(),
            P0_TEST_PUBLIC_KEY
        );
        assert!(runtime.trusted_keys.contains_key("attool-p0-test"));
    }

    #[test]
    fn rejects_a_second_active_install_for_the_same_application() {
        let directory = tempfile::tempdir().unwrap();
        let runtime =
            PluginRuntime::p0(PluginStore::open(directory.path().join("plugins")).unwrap())
                .unwrap();
        runtime
            .reserve_operation(operation("operation-1", OperationStage::Download))
            .unwrap();

        let error = runtime
            .reserve_operation(operation("operation-2", OperationStage::Download))
            .unwrap_err();

        assert_eq!(error.code, "PLUGIN_INSTALL_IN_PROGRESS");
        assert_eq!(error.correlation_id, "correlation-operation-2");
    }
}
