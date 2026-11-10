use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use ed25519_dalek::VerifyingKey;
use serde::Serialize;
use tauri::State;
use uuid::Uuid;

use super::{
    catalog::{CatalogError, CatalogRuntime, CatalogSnapshot},
    install::{InstallError, InstalledPlugin, PluginAsset, PluginHistoryRecord, PluginStore},
    package::{inspect_package, HostEnvironment, InspectedPackage, PackageError},
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
}

impl PluginRuntime {
    pub fn p0(store: PluginStore) -> Result<Self, String> {
        let key = VerifyingKey::from_bytes(&P0_TEST_PUBLIC_KEY)
            .map_err(|error| format!("invalid P0 plugin public key: {error}"))?;
        Ok(Self {
            store: Arc::new(Mutex::new(store)),
            environment: HostEnvironment::current("1.0.0"),
            trusted_keys: Arc::new(HashMap::from([("attool-p0-test".to_string(), key)])),
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

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;

    use super::{PluginRuntime, P0_TEST_PUBLIC_KEY};
    use crate::plugin::install::PluginStore;

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
}
