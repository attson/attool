use std::{collections::HashSet, path::Path};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::{fs::File, io::AsyncWriteExt};

use super::{
    catalog::{CatalogArtifact, CatalogInstallPlan, CatalogTargetArch, CatalogTargetOs},
    github_proxy::send_with_fallback,
    package::InspectedPackage,
};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum OperationStage {
    Download,
    Verify,
    Staging,
    HealthCheck,
    Activate,
    Complete,
    RolledBack,
    Error,
}

impl OperationStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Download => "download",
            Self::Verify => "verify",
            Self::Staging => "staging",
            Self::HealthCheck => "health-check",
            Self::Activate => "activate",
            Self::Complete => "complete",
            Self::RolledBack => "rolled-back",
            Self::Error => "error",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "download" => Self::Download,
            "verify" => Self::Verify,
            "staging" => Self::Staging,
            "health-check" => Self::HealthCheck,
            "activate" => Self::Activate,
            "complete" => Self::Complete,
            "rolled-back" => Self::RolledBack,
            "error" => Self::Error,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OperationSnapshot {
    #[serde(skip)]
    pub action: String,
    pub operation_id: String,
    pub correlation_id: String,
    pub app_id: String,
    pub name: String,
    pub version: String,
    pub publisher_id: String,
    pub publisher_name: String,
    pub publisher_key_id: String,
    pub entry: Option<String>,
    pub permissions: Vec<String>,
    pub stage: OperationStage,
    pub steps: Vec<String>,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub error_code: Option<String>,
    pub message: Option<String>,
    pub final_available_version: Option<String>,
}

impl OperationSnapshot {
    pub fn new(
        operation_id: String,
        correlation_id: String,
        action: &'static str,
        plan: &CatalogInstallPlan,
    ) -> Self {
        Self {
            action: action.to_string(),
            operation_id,
            correlation_id,
            app_id: plan.app_id.clone(),
            name: plan.name.clone(),
            version: plan.version.clone(),
            publisher_id: plan.publisher_id.clone(),
            publisher_name: plan.publisher_name.clone(),
            publisher_key_id: plan.publisher_key_id.clone(),
            entry: None,
            permissions: plan.permissions.clone(),
            stage: OperationStage::Download,
            steps: vec![OperationStage::Download.as_str().to_string()],
            downloaded_bytes: 0,
            total_bytes: plan.artifact.size,
            error_code: None,
            message: None,
            final_available_version: None,
        }
    }

    pub fn advance_to(&mut self, next: OperationStage) -> Result<(), RemoteInstallError> {
        if self.stage == next {
            return Ok(());
        }
        let allowed = matches!(
            (self.stage, next),
            (
                OperationStage::Download,
                OperationStage::Verify | OperationStage::Error
            ) | (
                OperationStage::Verify,
                OperationStage::Staging | OperationStage::Error
            ) | (
                OperationStage::Staging,
                OperationStage::HealthCheck | OperationStage::Error
            ) | (
                OperationStage::HealthCheck,
                OperationStage::Activate | OperationStage::Error
            ) | (
                OperationStage::Activate,
                OperationStage::Complete | OperationStage::Error
            ) | (
                OperationStage::Complete,
                OperationStage::RolledBack | OperationStage::Error
            )
        );
        if !allowed {
            return Err(RemoteInstallError::new(
                "PLUGIN_OPERATION_STATE_INVALID",
                format!(
                    "Plugin operation cannot move from {} to {}",
                    self.stage.as_str(),
                    next.as_str()
                ),
            ));
        }
        self.stage = next;
        self.steps.push(next.as_str().to_string());
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

pub fn permission_diff(current: &[String], target: &[String]) -> PermissionDiff {
    let current = current.iter().collect::<HashSet<_>>();
    let target = target.iter().collect::<HashSet<_>>();
    let mut added = target
        .difference(&current)
        .map(|permission| (*permission).clone())
        .collect::<Vec<_>>();
    let mut removed = current
        .difference(&target)
        .map(|permission| (*permission).clone())
        .collect::<Vec<_>>();
    added.sort();
    removed.sort();
    PermissionDiff { added, removed }
}

#[derive(Debug)]
pub struct RemoteInstallError {
    pub code: &'static str,
    pub message: String,
}

impl RemoteInstallError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for RemoteInstallError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for RemoteInstallError {}

pub fn validate_accepted_permissions(
    accepted: &[String],
    expected: &[String],
) -> Result<(), RemoteInstallError> {
    let accepted_set = accepted.iter().collect::<HashSet<_>>();
    let expected_set = expected.iter().collect::<HashSet<_>>();
    if accepted_set.len() != accepted.len()
        || expected_set.len() != expected.len()
        || accepted_set != expected_set
    {
        return Err(RemoteInstallError::new(
            "PLUGIN_PERMISSION_CONFIRMATION_INVALID",
            "Accepted permissions must exactly match the Catalog release",
        ));
    }
    Ok(())
}

pub fn validate_package_against_catalog(
    package: &InspectedPackage,
    plan: &CatalogInstallPlan,
) -> Result<(), RemoteInstallError> {
    let target_os = match plan.artifact.os {
        CatalogTargetOs::Macos => "macos",
        CatalogTargetOs::Windows => "windows",
        CatalogTargetOs::Linux => "linux",
    };
    let target_arch = match plan.artifact.arch {
        CatalogTargetArch::Aarch64 => "aarch64",
        CatalogTargetArch::X86_64 => "x86_64",
    };
    let matches = package.app_id == plan.app_id
        && package.name == plan.name
        && package.version == plan.version
        && package.publisher_id == plan.publisher_id
        && package.publisher_name == plan.publisher_name
        && package.publisher_key_id == plan.publisher_key_id
        && package.target_os == target_os
        && package.target_arch == target_arch
        && package.host_api_min == plan.host_api_min
        && package.host_api_max == plan.host_api_max;
    if !matches {
        return Err(RemoteInstallError::new(
            "PLUGIN_CATALOG_MANIFEST_MISMATCH",
            "Plugin manifest does not match the verified Catalog release",
        ));
    }
    validate_accepted_permissions(&package.permissions, &plan.permissions).map_err(|_| {
        RemoteInstallError::new(
            "PLUGIN_CATALOG_MANIFEST_MISMATCH",
            "Plugin permissions do not match the verified Catalog release",
        )
    })
}

pub async fn download_artifact(
    client: &reqwest::Client,
    artifact: &CatalogArtifact,
    destination: &Path,
    mut on_progress: impl FnMut(u64),
) -> Result<(), RemoteInstallError> {
    let result = async {
        let response = send_with_fallback(client, &artifact.url)
            .await
            .map_err(|error| {
                RemoteInstallError::new(
                    "PLUGIN_DOWNLOAD_HTTP_FAILED",
                    format!("Plugin download failed: {error}"),
                )
            })?;
        if !response.status().is_success() {
            return Err(RemoteInstallError::new(
                "PLUGIN_DOWNLOAD_HTTP_FAILED",
                format!("Plugin server returned HTTP {}", response.status().as_u16()),
            ));
        }
        if response
            .content_length()
            .map(|size| size != artifact.size)
            .unwrap_or(false)
        {
            return Err(RemoteInstallError::new(
                "PLUGIN_DOWNLOAD_SIZE_MISMATCH",
                "Plugin download size does not match the Catalog",
            ));
        }
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|error| {
                RemoteInstallError::new(
                    "PLUGIN_STAGING_FAILED",
                    format!("Cannot create plugin download directory: {error}"),
                )
            })?;
        }
        let mut file = File::create(destination).await.map_err(|error| {
            RemoteInstallError::new(
                "PLUGIN_STAGING_FAILED",
                format!("Cannot create plugin download: {error}"),
            )
        })?;
        let mut downloaded = 0_u64;
        let mut hash = Sha256::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|error| {
                RemoteInstallError::new(
                    "PLUGIN_DOWNLOAD_INTERRUPTED",
                    format!("Plugin download was interrupted: {error}"),
                )
            })?;
            downloaded = downloaded.checked_add(chunk.len() as u64).ok_or_else(|| {
                RemoteInstallError::new(
                    "PLUGIN_DOWNLOAD_SIZE_MISMATCH",
                    "Plugin download size overflowed",
                )
            })?;
            if downloaded > artifact.size {
                return Err(RemoteInstallError::new(
                    "PLUGIN_DOWNLOAD_SIZE_MISMATCH",
                    "Plugin download exceeds the Catalog size",
                ));
            }
            file.write_all(&chunk).await.map_err(|error| {
                RemoteInstallError::new(
                    "PLUGIN_STAGING_FAILED",
                    format!("Cannot write plugin download: {error}"),
                )
            })?;
            hash.update(&chunk);
            on_progress(downloaded);
        }
        file.flush().await.map_err(|error| {
            RemoteInstallError::new(
                "PLUGIN_STAGING_FAILED",
                format!("Cannot flush plugin download: {error}"),
            )
        })?;
        if downloaded != artifact.size {
            return Err(RemoteInstallError::new(
                "PLUGIN_DOWNLOAD_SIZE_MISMATCH",
                "Plugin download size does not match the Catalog",
            ));
        }
        if hex::encode(hash.finalize()) != artifact.sha256 {
            return Err(RemoteInstallError::new(
                "PLUGIN_DOWNLOAD_HASH_MISMATCH",
                "Plugin download hash does not match the Catalog",
            ));
        }
        Ok(())
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(destination).await;
    }
    result
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, HashMap},
        io::{Cursor, Write},
        path::Path,
    };

    use base64::Engine;
    use ed25519_dalek::{Signer, SigningKey};
    use sha2::{Digest, Sha256};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    use zip::{write::SimpleFileOptions, ZipWriter};

    use super::{
        download_artifact, permission_diff, validate_accepted_permissions,
        validate_package_against_catalog, OperationSnapshot, OperationStage,
    };
    use crate::plugin::{
        catalog::{CatalogArtifact, CatalogInstallPlan, CatalogTargetArch, CatalogTargetOs},
        install::PluginStore,
        package::InspectedPackage,
        package::{inspect_package, HostEnvironment},
    };

    async fn serve_once(body: Vec<u8>, content_length: usize) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 1024];
            let _ = stream.read(&mut request).await;
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {content_length}\r\nConnection: close\r\n\r\n"
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            stream.write_all(&body).await.unwrap();
        });
        format!("http://{address}/codec.atpkg")
    }

    fn artifact(url: String, body: &[u8]) -> CatalogArtifact {
        CatalogArtifact {
            os: CatalogTargetOs::Linux,
            arch: CatalogTargetArch::X86_64,
            url,
            file_name: "AT.Codec_1.0.0_linux_x86_64.atpkg".to_string(),
            size: body.len() as u64,
            sha256: hex::encode(Sha256::digest(body)),
        }
    }

    fn install_plan() -> CatalogInstallPlan {
        CatalogInstallPlan {
            app_id: "com.attson.codec".to_string(),
            name: "编码转换".to_string(),
            publisher_id: "com.attson".to_string(),
            publisher_name: "Attson".to_string(),
            version: "1.0.0".to_string(),
            publisher_key_id: "attson-codec-2026-01".to_string(),
            publisher_key: SigningKey::from_bytes(&[7; 32]).verifying_key(),
            host_api_min: "1.0.0".to_string(),
            host_api_max: "1.x".to_string(),
            permissions: vec!["host.theme.read".to_string()],
            artifact: artifact(
                "https://example.invalid/codec.atpkg".to_string(),
                b"package",
            ),
        }
    }

    #[test]
    fn calculates_added_and_removed_permissions_as_sets() {
        let current = vec![
            "host.theme.read".to_string(),
            "host.notification.show".to_string(),
        ];
        let target = vec!["host.file.save".to_string(), "host.theme.read".to_string()];

        let difference = permission_diff(&current, &target);

        assert_eq!(difference.added, vec!["host.file.save"]);
        assert_eq!(difference.removed, vec!["host.notification.show"]);
    }

    #[test]
    fn operation_stage_history_cannot_regress_after_completion() {
        let plan = install_plan();
        let mut operation = OperationSnapshot::new(
            "operation-1".to_string(),
            "correlation-1".to_string(),
            "install",
            &plan,
        );
        for stage in [
            OperationStage::Verify,
            OperationStage::Staging,
            OperationStage::HealthCheck,
            OperationStage::Activate,
            OperationStage::Complete,
        ] {
            operation.advance_to(stage).unwrap();
        }

        let error = operation.advance_to(OperationStage::Download).unwrap_err();

        assert_eq!(error.code, "PLUGIN_OPERATION_STATE_INVALID");
        assert_eq!(
            operation.steps,
            vec![
                "download",
                "verify",
                "staging",
                "health-check",
                "activate",
                "complete",
            ]
        );
    }

    fn inspected_package() -> InspectedPackage {
        InspectedPackage {
            app_id: "com.attson.codec".to_string(),
            name: "编码转换".to_string(),
            version: "1.0.0".to_string(),
            publisher_id: "com.attson".to_string(),
            publisher_name: "Attson".to_string(),
            publisher_key_id: "attson-codec-2026-01".to_string(),
            target_os: "linux".to_string(),
            target_arch: "x86_64".to_string(),
            host_api_min: "1.0.0".to_string(),
            host_api_max: "1.x".to_string(),
            entry: "ui/index.html".to_string(),
            permissions: vec!["host.theme.read".to_string()],
            native_service: None,
            files: BTreeMap::from([("ui/index.html".to_string(), "a".repeat(64))]),
        }
    }

    fn package_bytes(signing_key: &SigningKey, target_os: &str, target_arch: &str) -> Vec<u8> {
        let license = b"test license";
        let index = b"<!doctype html><script>/* no handshake */</script>";
        let manifest = serde_json::json!({
            "schemaVersion": 1,
            "appId": "com.attson.codec",
            "name": "编码转换",
            "version": "1.0.0",
            "publisher": {
                "id": "com.attson",
                "name": "Attson",
                "keyId": "attson-codec-2026-01"
            },
            "target": { "os": target_os, "arch": target_arch },
            "hostApi": { "min": "1.0.0", "max": "1.x" },
            "ui": { "entry": "ui/index.html" },
            "permissions": ["host.theme.read"],
            "files": {
                "LICENSE": hex::encode(Sha256::digest(license)),
                "ui/index.html": hex::encode(Sha256::digest(index))
            }
        });
        let manifest = serde_json::to_string(&manifest).unwrap();
        let signature = base64::engine::general_purpose::STANDARD
            .encode(signing_key.sign(manifest.as_bytes()).to_bytes());
        let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
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
        archive.finish().unwrap().into_inner()
    }

    fn current_target() -> (CatalogTargetOs, CatalogTargetArch) {
        let os = match std::env::consts::OS {
            "macos" => CatalogTargetOs::Macos,
            "windows" => CatalogTargetOs::Windows,
            _ => CatalogTargetOs::Linux,
        };
        let arch = match std::env::consts::ARCH {
            "aarch64" => CatalogTargetArch::Aarch64,
            _ => CatalogTargetArch::X86_64,
        };
        (os, arch)
    }

    #[test]
    fn requires_exact_permission_confirmation_and_catalog_manifest_identity() {
        let plan = install_plan();
        let package = inspected_package();

        validate_accepted_permissions(&package.permissions, &plan.permissions).unwrap();
        validate_package_against_catalog(&package, &plan).unwrap();

        let permission_error = validate_accepted_permissions(&[], &plan.permissions)
            .expect_err("missing permission confirmation must fail");
        assert_eq!(
            permission_error.code,
            "PLUGIN_PERMISSION_CONFIRMATION_INVALID"
        );

        let mut wrong_target = package.clone();
        wrong_target.target_arch = "aarch64".to_string();
        let mismatch = validate_package_against_catalog(&wrong_target, &plan)
            .expect_err("target mismatch must fail");
        assert_eq!(mismatch.code, "PLUGIN_CATALOG_MANIFEST_MISMATCH");
    }

    #[tokio::test]
    async fn streams_and_verifies_a_catalog_artifact() {
        let body = b"signed plugin package".to_vec();
        let url = serve_once(body.clone(), body.len()).await;
        let artifact = artifact(url, &body);
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("codec.atpkg");
        let mut progress = Vec::new();

        download_artifact(
            &reqwest::Client::new(),
            &artifact,
            &destination,
            |downloaded| progress.push(downloaded),
        )
        .await
        .unwrap();

        assert_eq!(tokio::fs::read(destination).await.unwrap(), body);
        assert_eq!(progress.last(), Some(&artifact.size));
    }

    #[tokio::test]
    async fn rejects_an_interrupted_response_and_removes_the_partial_file() {
        let body = b"partial".to_vec();
        let url = serve_once(body.clone(), body.len() + 20).await;
        let mut artifact = artifact(url, &body);
        artifact.size += 20;
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("codec.atpkg");

        let error = download_artifact(&reqwest::Client::new(), &artifact, &destination, |_| {})
            .await
            .expect_err("interrupted download must fail");

        assert_eq!(error.code, "PLUGIN_DOWNLOAD_INTERRUPTED");
        assert!(!Path::new(&destination).exists());
    }

    #[tokio::test]
    async fn rejects_catalog_size_and_hash_mismatches() {
        let body = b"complete package".to_vec();
        let directory = tempfile::tempdir().unwrap();

        let size_url = serve_once(body.clone(), body.len()).await;
        let mut wrong_size = artifact(size_url, &body);
        wrong_size.size += 1;
        let size_path = directory.path().join("wrong-size.atpkg");
        let error = download_artifact(&reqwest::Client::new(), &wrong_size, &size_path, |_| {})
            .await
            .expect_err("size mismatch must fail");
        assert_eq!(error.code, "PLUGIN_DOWNLOAD_SIZE_MISMATCH");
        assert!(!size_path.exists());

        let hash_url = serve_once(body.clone(), body.len()).await;
        let mut wrong_hash = artifact(hash_url, &body);
        wrong_hash.sha256 = "0".repeat(64);
        let hash_path = directory.path().join("wrong-hash.atpkg");
        let error = download_artifact(&reqwest::Client::new(), &wrong_hash, &hash_path, |_| {})
            .await
            .expect_err("hash mismatch must fail");
        assert_eq!(error.code, "PLUGIN_DOWNLOAD_HASH_MISMATCH");
        assert!(!hash_path.exists());
    }

    #[tokio::test]
    async fn downloaded_package_rejects_wrong_publisher_signature_and_target() {
        let signing_key = SigningKey::from_bytes(&[8; 32]);
        let environment = HostEnvironment::current("1.0.0");
        let directory = tempfile::tempdir().unwrap();

        let signed = package_bytes(&signing_key, &environment.os, &environment.arch);
        let signature_url = serve_once(signed.clone(), signed.len()).await;
        let signature_artifact = artifact(signature_url, &signed);
        let signature_path = directory.path().join("wrong-signature.atpkg");
        download_artifact(
            &reqwest::Client::new(),
            &signature_artifact,
            &signature_path,
            |_| {},
        )
        .await
        .unwrap();
        let wrong_key = SigningKey::from_bytes(&[9; 32]);
        let keys = HashMap::from([(
            "attson-codec-2026-01".to_string(),
            wrong_key.verifying_key(),
        )]);
        let error = inspect_package(&signature_path, &environment, &keys)
            .expect_err("Publisher signature mismatch must fail");
        assert_eq!(error.code, "PLUGIN_SIGNATURE_INVALID");

        let wrong_target = package_bytes(&signing_key, &environment.os, "unsupported-arch");
        let target_url = serve_once(wrong_target.clone(), wrong_target.len()).await;
        let target_artifact = artifact(target_url, &wrong_target);
        let target_path = directory.path().join("wrong-target.atpkg");
        download_artifact(
            &reqwest::Client::new(),
            &target_artifact,
            &target_path,
            |_| {},
        )
        .await
        .unwrap();
        let keys = HashMap::from([(
            "attson-codec-2026-01".to_string(),
            signing_key.verifying_key(),
        )]);
        let error = inspect_package(&target_path, &environment, &keys)
            .expect_err("target mismatch must fail");
        assert_eq!(error.code, "PLUGIN_TARGET_MISMATCH");
    }

    #[tokio::test]
    async fn failed_health_probe_discards_http_downloaded_staging() {
        let signing_key = SigningKey::from_bytes(&[10; 32]);
        let environment = HostEnvironment::current("1.0.0");
        let package = package_bytes(&signing_key, &environment.os, &environment.arch);
        let url = serve_once(package.clone(), package.len()).await;
        let mut plan = install_plan();
        let (os, arch) = current_target();
        plan.publisher_key = signing_key.verifying_key();
        plan.artifact = artifact(url, &package);
        plan.artifact.os = os;
        plan.artifact.arch = arch;
        let directory = tempfile::tempdir().unwrap();
        let store_root = directory.path().join("plugins");
        let store = PluginStore::open(store_root.clone()).unwrap();
        let download = store.catalog_download_path("operation-health", &plan.artifact.file_name);
        download_artifact(&reqwest::Client::new(), &plan.artifact, &download, |_| {})
            .await
            .unwrap();
        let keys = HashMap::from([(plan.publisher_key_id.clone(), plan.publisher_key)]);
        let prepared = store
            .prepare_install(&download, &environment, &keys, "operation-health")
            .unwrap();

        store.discard_prepared(prepared);
        store.remove_catalog_download("operation-health");

        assert!(store.list().unwrap().is_empty());
        assert!(!store_root.join("staging/operation-health").exists());
        assert!(!store_root.join("downloads/operation-health").exists());
    }
}
