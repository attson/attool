use std::{collections::HashSet, fs, path::PathBuf, sync::Arc, time::Duration};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use regex::Regex;
use rusqlite::{params, Connection, OptionalExtension};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const CATALOG_BASE_URL: &str =
    "https://raw.githubusercontent.com/attsontools/attool/main/catalog/v1";
const METADATA_LIMIT_BYTES: usize = 2 * 1024 * 1024;
const SIGNATURE_LIMIT_BYTES: usize = 1024;
const KNOWN_PERMISSIONS: [&str; 3] = [
    "host.theme.read",
    "host.file.save",
    "host.notification.show",
];

#[derive(Debug, Clone)]
pub struct CatalogError {
    pub code: &'static str,
    pub message: String,
}

impl CatalogError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for CatalogError {}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrustRoot {
    pub schema_version: u64,
    pub sequence: u64,
    pub issued_at: String,
    pub expires_at: String,
    pub catalog_keys: Vec<TrustKey>,
    pub publishers: Vec<CatalogPublisher>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrustKey {
    pub key_id: String,
    pub algorithm: String,
    pub public_key: String,
    pub status: KeyStatus,
    pub not_before: String,
    pub not_after: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum KeyStatus {
    Active,
    Retired,
    Revoked,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogPublisher {
    pub publisher_id: String,
    pub name: String,
    pub keys: Vec<TrustKey>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginCatalog {
    pub schema_version: u64,
    pub sequence: u64,
    pub generated_at: String,
    pub expires_at: String,
    pub signing_key_id: String,
    pub apps: Vec<PluginCatalogApp>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginCatalogApp {
    pub app_id: String,
    pub name: String,
    pub description: String,
    pub publisher_id: String,
    pub icon: String,
    pub channels: CatalogChannels,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogChannels {
    pub stable: CatalogRelease,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogRelease {
    pub version: String,
    pub published_at: String,
    pub notes: String,
    pub publisher_key_id: String,
    pub host_api: CatalogHostApi,
    pub permissions: Vec<String>,
    pub artifacts: Vec<CatalogArtifact>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogHostApi {
    pub min: String,
    pub max: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogArtifact {
    pub os: CatalogTargetOs,
    pub arch: CatalogTargetArch,
    pub url: String,
    pub file_name: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum CatalogTargetOs {
    Macos,
    Windows,
    Linux,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum CatalogTargetArch {
    Aarch64,
    #[serde(rename = "x86_64")]
    X86_64,
}

#[derive(Debug, Clone)]
pub struct VerifiedCatalog {
    pub root: TrustRoot,
    pub catalog: PluginCatalog,
    pub expired: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSnapshot {
    pub root_sequence: u64,
    pub catalog_sequence: u64,
    pub generated_at: String,
    pub expires_at: String,
    pub expired: bool,
    pub apps: Vec<CatalogAppSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogAppSnapshot {
    pub app_id: String,
    pub name: String,
    pub description: String,
    pub publisher_id: String,
    pub publisher_name: String,
    pub icon: String,
    pub latest_version: String,
    pub published_at: String,
    pub notes: String,
    pub publisher_key_id: String,
    pub host_api_min: String,
    pub host_api_max: String,
    pub permissions: Vec<String>,
    pub target: Option<CatalogArtifact>,
}

#[derive(Debug, Clone)]
pub struct CatalogInstallPlan {
    pub app_id: String,
    pub name: String,
    pub publisher_id: String,
    pub publisher_name: String,
    pub version: String,
    pub publisher_key_id: String,
    pub publisher_key: VerifyingKey,
    pub host_api_min: String,
    pub host_api_max: String,
    pub permissions: Vec<String>,
    pub artifact: CatalogArtifact,
}

pub fn snapshot_for_target(
    verified: &VerifiedCatalog,
    os: CatalogTargetOs,
    arch: CatalogTargetArch,
) -> CatalogSnapshot {
    let apps = verified
        .catalog
        .apps
        .iter()
        .map(|app| {
            let release = &app.channels.stable;
            let publisher_name = verified
                .root
                .publishers
                .iter()
                .find(|publisher| publisher.publisher_id == app.publisher_id)
                .map(|publisher| publisher.name.clone())
                .unwrap_or_else(|| app.publisher_id.clone());
            CatalogAppSnapshot {
                app_id: app.app_id.clone(),
                name: app.name.clone(),
                description: app.description.clone(),
                publisher_id: app.publisher_id.clone(),
                publisher_name,
                icon: app.icon.clone(),
                latest_version: release.version.clone(),
                published_at: release.published_at.clone(),
                notes: release.notes.clone(),
                publisher_key_id: release.publisher_key_id.clone(),
                host_api_min: release.host_api.min.clone(),
                host_api_max: release.host_api.max.clone(),
                permissions: release.permissions.clone(),
                target: release
                    .artifacts
                    .iter()
                    .find(|artifact| artifact.os == os && artifact.arch == arch)
                    .cloned(),
            }
        })
        .collect();
    CatalogSnapshot {
        root_sequence: verified.root.sequence,
        catalog_sequence: verified.catalog.sequence,
        generated_at: verified.catalog.generated_at.clone(),
        expires_at: verified.catalog.expires_at.clone(),
        expired: verified.expired,
        apps,
    }
}

#[derive(Debug, Clone)]
pub struct CatalogBundle {
    pub root_json: String,
    pub root_signature: String,
    pub catalog_json: String,
    pub catalog_signature: String,
}

#[derive(Debug, Clone)]
pub struct CatalogCache {
    database: PathBuf,
}

impl CatalogCache {
    pub fn open(database: PathBuf) -> Result<Self, CatalogError> {
        if let Some(parent) = database.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                CatalogError::new(
                    "CATALOG_CACHE_OPEN_FAILED",
                    format!("Cannot create Catalog cache directory: {error}"),
                )
            })?;
        }
        let cache = Self { database };
        cache
            .connection()?
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS catalog_cache (
                    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                    root_json TEXT NOT NULL,
                    root_signature TEXT NOT NULL,
                    catalog_json TEXT NOT NULL,
                    catalog_signature TEXT NOT NULL,
                    root_sequence INTEGER NOT NULL,
                    catalog_sequence INTEGER NOT NULL,
                    accepted_at TEXT NOT NULL
                );",
            )
            .map_err(|error| {
                CatalogError::new(
                    "CATALOG_CACHE_OPEN_FAILED",
                    format!("Cannot initialize Catalog cache: {error}"),
                )
            })?;
        Ok(cache)
    }

    pub fn accept(
        &self,
        verifier: &CatalogVerifier,
        bundle: CatalogBundle,
        now: DateTime<Utc>,
    ) -> Result<VerifiedCatalog, CatalogError> {
        let verified = self.verify_candidate(verifier, &bundle, now)?;
        self.store_verified(bundle, &verified, now)?;
        Ok(verified)
    }

    fn verify_candidate(
        &self,
        verifier: &CatalogVerifier,
        bundle: &CatalogBundle,
        now: DateTime<Utc>,
    ) -> Result<VerifiedCatalog, CatalogError> {
        let (highest_root_sequence, highest_catalog_sequence) = self.highest_sequences()?;
        verifier.verify(
            &bundle.root_json,
            &bundle.root_signature,
            &bundle.catalog_json,
            &bundle.catalog_signature,
            now,
            highest_root_sequence,
            highest_catalog_sequence,
            true,
        )
    }

    fn store_verified(
        &self,
        bundle: CatalogBundle,
        verified: &VerifiedCatalog,
        now: DateTime<Utc>,
    ) -> Result<(), CatalogError> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(cache_write_error)?;
        let affected = transaction
            .execute(
                "INSERT INTO catalog_cache (
                    singleton, root_json, root_signature, catalog_json, catalog_signature,
                    root_sequence, catalog_sequence, accepted_at
                 ) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(singleton) DO UPDATE SET
                    root_json = excluded.root_json,
                    root_signature = excluded.root_signature,
                    catalog_json = excluded.catalog_json,
                    catalog_signature = excluded.catalog_signature,
                    root_sequence = excluded.root_sequence,
                    catalog_sequence = excluded.catalog_sequence,
                    accepted_at = excluded.accepted_at
                 WHERE excluded.root_sequence >= catalog_cache.root_sequence
                   AND excluded.catalog_sequence >= catalog_cache.catalog_sequence",
                params![
                    bundle.root_json,
                    bundle.root_signature,
                    bundle.catalog_json,
                    bundle.catalog_signature,
                    verified.root.sequence,
                    verified.catalog.sequence,
                    now.to_rfc3339(),
                ],
            )
            .map_err(cache_write_error)?;
        if affected == 0 {
            return Err(CatalogError::new(
                "METADATA_SEQUENCE_ROLLBACK",
                "Catalog cache already contains newer metadata",
            ));
        }
        transaction.commit().map_err(cache_write_error)?;
        Ok(())
    }

    pub fn load(
        &self,
        verifier: &CatalogVerifier,
        now: DateTime<Utc>,
    ) -> Result<Option<VerifiedCatalog>, CatalogError> {
        let Some(bundle) = self.load_bundle()? else {
            return Ok(None);
        };
        verifier
            .verify(
                &bundle.root_json,
                &bundle.root_signature,
                &bundle.catalog_json,
                &bundle.catalog_signature,
                now,
                0,
                0,
                false,
            )
            .map(Some)
            .map_err(|error| {
                CatalogError::new(
                    "CATALOG_CACHE_INVALID",
                    format!(
                        "Cached Catalog failed verification ({}): {}",
                        error.code, error.message
                    ),
                )
            })
    }

    fn highest_sequences(&self) -> Result<(u64, u64), CatalogError> {
        self.connection()?
            .query_row(
                "SELECT root_sequence, catalog_sequence FROM catalog_cache WHERE singleton = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map(|value| value.unwrap_or((0, 0)))
            .map_err(cache_read_error)
    }

    fn load_bundle(&self) -> Result<Option<CatalogBundle>, CatalogError> {
        self.connection()?
            .query_row(
                "SELECT root_json, root_signature, catalog_json, catalog_signature
                 FROM catalog_cache WHERE singleton = 1",
                [],
                |row| {
                    Ok(CatalogBundle {
                        root_json: row.get(0)?,
                        root_signature: row.get(1)?,
                        catalog_json: row.get(2)?,
                        catalog_signature: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(cache_read_error)
    }

    fn connection(&self) -> Result<Connection, CatalogError> {
        Connection::open(&self.database).map_err(|error| {
            CatalogError::new(
                "CATALOG_CACHE_OPEN_FAILED",
                format!("Cannot open Catalog cache: {error}"),
            )
        })
    }
}

fn cache_read_error(error: rusqlite::Error) -> CatalogError {
    CatalogError::new(
        "CATALOG_CACHE_READ_FAILED",
        format!("Cannot read Catalog cache: {error}"),
    )
}

fn cache_write_error(error: rusqlite::Error) -> CatalogError {
    CatalogError::new(
        "CATALOG_CACHE_WRITE_FAILED",
        format!("Cannot update Catalog cache: {error}"),
    )
}

#[derive(Clone)]
pub struct CatalogRuntime {
    verifier: Option<CatalogVerifier>,
    cache: Arc<CatalogCache>,
    client: reqwest::Client,
    base_url: String,
}

impl CatalogRuntime {
    pub fn production(cache_path: PathBuf) -> Result<Self, CatalogError> {
        let public_key = include_str!(concat!(env!("OUT_DIR"), "/catalog_root_public_key.txt"));
        let verifier = if public_key.trim().is_empty() {
            None
        } else {
            Some(CatalogVerifier::from_base64(public_key.trim())?)
        };
        let client = reqwest::Client::builder()
            .user_agent(format!("attool/{}", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::limited(3))
            .build()
            .map_err(|error| {
                CatalogError::new(
                    "CATALOG_CLIENT_FAILED",
                    format!("Cannot create Catalog client: {error}"),
                )
            })?;
        Ok(Self {
            verifier,
            cache: Arc::new(CatalogCache::open(cache_path)?),
            client,
            base_url: CATALOG_BASE_URL.to_string(),
        })
    }

    pub async fn refresh(&self) -> Result<CatalogSnapshot, CatalogError> {
        let verifier = self.verifier.clone().ok_or_else(|| {
            CatalogError::new(
                "CATALOG_DISABLED",
                "Remote Catalog is disabled because no Root public key is configured",
            )
        })?;
        let root_url = format!("{}/root.json", self.base_url);
        let root_signature_url = format!("{}/root.json.sig", self.base_url);
        let catalog_url = format!("{}/catalog.json", self.base_url);
        let catalog_signature_url = format!("{}/catalog.json.sig", self.base_url);
        let root_json = fetch_text(
            &self.client,
            &root_url,
            METADATA_LIMIT_BYTES,
            "TRUST_ROOT_NETWORK_FAILED",
        );
        let root_signature = fetch_text(
            &self.client,
            &root_signature_url,
            SIGNATURE_LIMIT_BYTES,
            "TRUST_ROOT_NETWORK_FAILED",
        );
        let catalog_json = fetch_text(
            &self.client,
            &catalog_url,
            METADATA_LIMIT_BYTES,
            "CATALOG_NETWORK_FAILED",
        );
        let catalog_signature = fetch_text(
            &self.client,
            &catalog_signature_url,
            SIGNATURE_LIMIT_BYTES,
            "CATALOG_NETWORK_FAILED",
        );
        let (root_json, root_signature, catalog_json, catalog_signature) =
            tokio::try_join!(root_json, root_signature, catalog_json, catalog_signature)?;
        let bundle = CatalogBundle {
            root_json,
            root_signature,
            catalog_json,
            catalog_signature,
        };
        let cache = Arc::clone(&self.cache);
        let verified =
            tokio::task::spawn_blocking(move || cache.accept(&verifier, bundle, Utc::now()))
                .await
                .map_err(|error| {
                    CatalogError::new(
                        "CATALOG_WORKER_FAILED",
                        format!("Catalog verification worker failed: {error}"),
                    )
                })??;
        let (os, arch) = current_target()?;
        Ok(snapshot_for_target(&verified, os, arch))
    }

    pub async fn list(&self) -> Result<Option<CatalogSnapshot>, CatalogError> {
        let Some(verifier) = self.verifier.clone() else {
            return Ok(None);
        };
        let cache = Arc::clone(&self.cache);
        let verified = tokio::task::spawn_blocking(move || cache.load(&verifier, Utc::now()))
            .await
            .map_err(|error| {
                CatalogError::new(
                    "CATALOG_WORKER_FAILED",
                    format!("Catalog cache worker failed: {error}"),
                )
            })??;
        let Some(verified) = verified else {
            return Ok(None);
        };
        let (os, arch) = current_target()?;
        Ok(Some(snapshot_for_target(&verified, os, arch)))
    }

    pub async fn install_plan(&self, app_id: &str) -> Result<CatalogInstallPlan, CatalogError> {
        let verifier = self.verifier.clone().ok_or_else(|| {
            CatalogError::new(
                "CATALOG_DISABLED",
                "Remote Catalog is disabled because no Root public key is configured",
            )
        })?;
        let cache = Arc::clone(&self.cache);
        let verified = tokio::task::spawn_blocking(move || cache.load(&verifier, Utc::now()))
            .await
            .map_err(|error| {
                CatalogError::new(
                    "CATALOG_WORKER_FAILED",
                    format!("Catalog cache worker failed: {error}"),
                )
            })??
            .ok_or_else(|| {
                CatalogError::new("CATALOG_NOT_FOUND", "No verified Catalog is cached")
            })?;
        if verified.expired {
            return Err(CatalogError::new(
                "CATALOG_EXPIRED",
                "The cached Catalog is expired",
            ));
        }
        let (os, arch) = current_target()?;
        install_plan_from_verified(&verified, app_id, os, arch)
    }
}

fn install_plan_from_verified(
    verified: &VerifiedCatalog,
    app_id: &str,
    os: CatalogTargetOs,
    arch: CatalogTargetArch,
) -> Result<CatalogInstallPlan, CatalogError> {
    if app_id != "com.attson.codec" {
        return Err(CatalogError::new(
            "CATALOG_APP_NOT_FOUND",
            "The requested official application was not found",
        ));
    }
    let app = verified
        .catalog
        .apps
        .iter()
        .find(|app| app.app_id == app_id)
        .ok_or_else(|| {
            CatalogError::new(
                "CATALOG_APP_NOT_FOUND",
                "The requested official application was not found",
            )
        })?;
    let release = &app.channels.stable;
    let artifact = release
        .artifacts
        .iter()
        .find(|artifact| artifact.os == os && artifact.arch == arch)
        .cloned()
        .ok_or_else(|| {
            CatalogError::new(
                "CATALOG_TARGET_NOT_FOUND",
                "The Catalog has no artifact for this system",
            )
        })?;
    validate_official_artifact_url(&release.version, &artifact)?;
    let publisher = verified
        .root
        .publishers
        .iter()
        .find(|publisher| publisher.publisher_id == app.publisher_id)
        .ok_or_else(|| {
            CatalogError::new(
                "CATALOG_PUBLISHER_UNKNOWN",
                "The application publisher is not trusted",
            )
        })?;
    let key = publisher
        .keys
        .iter()
        .find(|key| key.key_id == release.publisher_key_id)
        .ok_or_else(|| {
            CatalogError::new(
                "CATALOG_PUBLISHER_KEY_UNKNOWN",
                "The application Publisher Key is not trusted",
            )
        })?;
    Ok(CatalogInstallPlan {
        app_id: app.app_id.clone(),
        name: app.name.clone(),
        publisher_id: app.publisher_id.clone(),
        publisher_name: publisher.name.clone(),
        version: release.version.clone(),
        publisher_key_id: release.publisher_key_id.clone(),
        publisher_key: decode_public_key(&key.public_key, "TRUST_ROOT_KEY_INVALID")?,
        host_api_min: release.host_api.min.clone(),
        host_api_max: release.host_api.max.clone(),
        permissions: release.permissions.clone(),
        artifact,
    })
}

fn validate_official_artifact_url(
    version: &str,
    artifact: &CatalogArtifact,
) -> Result<(), CatalogError> {
    let url = Url::parse(&artifact.url)
        .map_err(|_| CatalogError::new("CATALOG_ARTIFACT_INVALID", "Artifact URL is invalid"))?;
    let expected_path = format!(
        "/attsontools/atcodec/releases/download/v{version}/{}",
        artifact.file_name
    );
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != expected_path
    {
        return Err(CatalogError::new(
            "CATALOG_ARTIFACT_URL_UNTRUSTED",
            "Catalog artifact is not an immutable official Codec release asset",
        ));
    }
    Ok(())
}

async fn fetch_text(
    client: &reqwest::Client,
    url: &str,
    limit: usize,
    code: &'static str,
) -> Result<String, CatalogError> {
    use futures_util::StreamExt;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| CatalogError::new(code, format!("Catalog request failed: {error}")))?;
    if !response.status().is_success() {
        return Err(CatalogError::new(
            code,
            format!(
                "Catalog server returned HTTP {}",
                response.status().as_u16()
            ),
        ));
    }
    if response
        .content_length()
        .map(|length| length > limit as u64)
        .unwrap_or(false)
    {
        return Err(CatalogError::new(
            "CATALOG_RESPONSE_TOO_LARGE",
            "Catalog response exceeds its size limit",
        ));
    }
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| {
            CatalogError::new(code, format!("Catalog response was interrupted: {error}"))
        })?;
        if body.len() + chunk.len() > limit {
            return Err(CatalogError::new(
                "CATALOG_RESPONSE_TOO_LARGE",
                "Catalog response exceeds its size limit",
            ));
        }
        body.extend_from_slice(&chunk);
    }
    String::from_utf8(body)
        .map_err(|_| CatalogError::new("CATALOG_RESPONSE_INVALID", "Catalog response is not UTF-8"))
}

fn current_target() -> Result<(CatalogTargetOs, CatalogTargetArch), CatalogError> {
    let os = match std::env::consts::OS {
        "macos" => CatalogTargetOs::Macos,
        "windows" => CatalogTargetOs::Windows,
        "linux" => CatalogTargetOs::Linux,
        other => {
            return Err(CatalogError::new(
                "CATALOG_TARGET_UNSUPPORTED",
                format!("Unsupported operating system: {other}"),
            ))
        }
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => CatalogTargetArch::Aarch64,
        "x86_64" => CatalogTargetArch::X86_64,
        other => {
            return Err(CatalogError::new(
                "CATALOG_TARGET_UNSUPPORTED",
                format!("Unsupported architecture: {other}"),
            ))
        }
    };
    Ok((os, arch))
}

#[derive(Clone)]
pub struct CatalogVerifier {
    root_key: VerifyingKey,
}

impl CatalogVerifier {
    pub fn from_base64(public_key: &str) -> Result<Self, CatalogError> {
        Ok(Self {
            root_key: decode_public_key(public_key, "CATALOG_ROOT_KEY_INVALID")?,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn verify(
        &self,
        root_json: &str,
        root_signature: &str,
        catalog_json: &str,
        catalog_signature: &str,
        now: DateTime<Utc>,
        highest_root_sequence: u64,
        highest_catalog_sequence: u64,
        require_fresh: bool,
    ) -> Result<VerifiedCatalog, CatalogError> {
        let (root_value, root_canonical) = parse_canonical(root_json, "TRUST_ROOT_INVALID")?;
        require_schema_v1(&root_value, "TRUST_ROOT_SCHEMA_UNSUPPORTED")?;
        verify_signature(
            &self.root_key,
            &root_canonical,
            root_signature,
            "TRUST_ROOT_SIGNATURE_INVALID",
        )?;
        let root: TrustRoot = serde_json::from_value(root_value).map_err(|error| {
            CatalogError::new(
                "TRUST_ROOT_INVALID",
                format!("Root schema is invalid: {error}"),
            )
        })?;
        validate_root(&root)?;
        if root.sequence < highest_root_sequence {
            return Err(CatalogError::new(
                "METADATA_SEQUENCE_ROLLBACK",
                format!(
                    "Root sequence {} is below {}",
                    root.sequence, highest_root_sequence
                ),
            ));
        }

        let root_issued_at = parse_timestamp(&root.issued_at, "TRUST_ROOT_TIME_INVALID")?;
        let root_expires_at = parse_timestamp(&root.expires_at, "TRUST_ROOT_TIME_INVALID")?;
        let root_expired = now < root_issued_at || now >= root_expires_at;
        if require_fresh && root_expired {
            return Err(CatalogError::new(
                "TRUST_ROOT_EXPIRED",
                "Root is not currently valid",
            ));
        }

        let (catalog_value, catalog_canonical) = parse_canonical(catalog_json, "CATALOG_INVALID")?;
        require_schema_v1(&catalog_value, "CATALOG_SCHEMA_UNSUPPORTED")?;
        let catalog: PluginCatalog = serde_json::from_value(catalog_value).map_err(|error| {
            CatalogError::new(
                "CATALOG_INVALID",
                format!("Catalog schema is invalid: {error}"),
            )
        })?;
        validate_catalog(&catalog)?;
        if catalog.sequence < highest_catalog_sequence {
            return Err(CatalogError::new(
                "METADATA_SEQUENCE_ROLLBACK",
                format!(
                    "Catalog sequence {} is below {}",
                    catalog.sequence, highest_catalog_sequence
                ),
            ));
        }

        let generated_at = parse_timestamp(&catalog.generated_at, "CATALOG_TIME_INVALID")?;
        let catalog_expires_at = parse_timestamp(&catalog.expires_at, "CATALOG_TIME_INVALID")?;
        let catalog_expired = now < generated_at || now >= catalog_expires_at;
        if require_fresh && catalog_expired {
            return Err(CatalogError::new(
                "CATALOG_EXPIRED",
                "Catalog is not currently valid",
            ));
        }

        let signing_key = root
            .catalog_keys
            .iter()
            .find(|key| key.key_id == catalog.signing_key_id)
            .ok_or_else(|| {
                CatalogError::new("CATALOG_KEY_UNKNOWN", "Catalog signing key is not trusted")
            })?;
        if signing_key.status == KeyStatus::Revoked
            || !key_contains_time(signing_key, generated_at)?
        {
            return Err(CatalogError::new(
                "CATALOG_KEY_INACTIVE",
                "Catalog signing key cannot verify this Catalog",
            ));
        }
        verify_signature(
            &decode_public_key(&signing_key.public_key, "TRUST_ROOT_KEY_INVALID")?,
            &catalog_canonical,
            catalog_signature,
            "CATALOG_SIGNATURE_INVALID",
        )?;
        validate_publishers(&root, &catalog)?;

        Ok(VerifiedCatalog {
            root,
            catalog,
            expired: root_expired || catalog_expired,
        })
    }
}

fn parse_canonical(json: &str, code: &'static str) -> Result<(Value, Vec<u8>), CatalogError> {
    let value: Value = serde_json::from_str(json)
        .map_err(|error| CatalogError::new(code, format!("Metadata is not valid JSON: {error}")))?;
    if !value.is_object() {
        return Err(CatalogError::new(code, "Metadata must be an object"));
    }
    let canonical = serde_json::to_vec(&canonicalize(value.clone())).map_err(|error| {
        CatalogError::new(code, format!("Cannot canonicalize metadata: {error}"))
    })?;
    Ok((value, canonical))
}

fn canonicalize(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(canonicalize).collect()),
        Value::Object(values) => {
            let mut entries: Vec<_> = values.into_iter().collect();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            Value::Object(
                entries
                    .into_iter()
                    .map(|(key, value)| (key, canonicalize(value)))
                    .collect(),
            )
        }
        other => other,
    }
}

fn require_schema_v1(value: &Value, code: &'static str) -> Result<(), CatalogError> {
    if value.get("schemaVersion").and_then(Value::as_u64) != Some(1) {
        return Err(CatalogError::new(
            code,
            "Only schema version 1 is supported",
        ));
    }
    Ok(())
}

fn verify_signature(
    key: &VerifyingKey,
    message: &[u8],
    encoded_signature: &str,
    code: &'static str,
) -> Result<(), CatalogError> {
    let bytes = BASE64
        .decode(encoded_signature.trim())
        .map_err(|_| CatalogError::new(code, "Signature is not valid base64"))?;
    let signature = Signature::from_slice(&bytes)
        .map_err(|_| CatalogError::new(code, "Signature must contain 64 bytes"))?;
    key.verify(message, &signature)
        .map_err(|_| CatalogError::new(code, "Signature verification failed"))
}

fn decode_public_key(value: &str, code: &'static str) -> Result<VerifyingKey, CatalogError> {
    let bytes = BASE64
        .decode(value)
        .map_err(|_| CatalogError::new(code, "Ed25519 public key is not valid base64"))?;
    let raw: [u8; 32] = bytes
        .try_into()
        .map_err(|_| CatalogError::new(code, "Ed25519 public key must contain 32 bytes"))?;
    VerifyingKey::from_bytes(&raw)
        .map_err(|_| CatalogError::new(code, "Ed25519 public key is invalid"))
}

fn parse_timestamp(value: &str, code: &'static str) -> Result<DateTime<Utc>, CatalogError> {
    let pattern = Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$")
        .expect("timestamp pattern is valid");
    if !pattern.is_match(value) {
        return Err(CatalogError::new(code, "Timestamp must be UTC ISO-8601"));
    }
    value
        .parse::<DateTime<Utc>>()
        .map_err(|_| CatalogError::new(code, "Timestamp is not a valid calendar time"))
}

fn validate_root(root: &TrustRoot) -> Result<(), CatalogError> {
    if root.schema_version != 1
        || root.sequence == 0
        || root.sequence > MAX_SAFE_INTEGER
        || root.catalog_keys.is_empty()
        || root.publishers.is_empty()
    {
        return Err(CatalogError::new("TRUST_ROOT_INVALID", "Root is invalid"));
    }
    let issued_at = parse_timestamp(&root.issued_at, "TRUST_ROOT_TIME_INVALID")?;
    let expires_at = parse_timestamp(&root.expires_at, "TRUST_ROOT_TIME_INVALID")?;
    if issued_at >= expires_at {
        return Err(CatalogError::new(
            "TRUST_ROOT_TIME_INVALID",
            "Root validity window must increase",
        ));
    }

    let mut key_ids = HashSet::new();
    for key in &root.catalog_keys {
        validate_key(key)?;
        if !key_ids.insert(key.key_id.as_str()) {
            return Err(CatalogError::new(
                "TRUST_ROOT_DUPLICATE_KEY",
                "Root key IDs must be unique",
            ));
        }
    }
    let mut publisher_ids = HashSet::new();
    for publisher in &root.publishers {
        if publisher.publisher_id.is_empty()
            || publisher.name.trim().is_empty()
            || publisher.keys.is_empty()
            || !publisher_ids.insert(publisher.publisher_id.as_str())
        {
            return Err(CatalogError::new(
                "TRUST_ROOT_INVALID",
                "Publisher declaration is invalid",
            ));
        }
        for key in &publisher.keys {
            validate_key(key)?;
            if !key_ids.insert(key.key_id.as_str()) {
                return Err(CatalogError::new(
                    "TRUST_ROOT_DUPLICATE_KEY",
                    "Root key IDs must be unique",
                ));
            }
        }
    }
    Ok(())
}

fn validate_key(key: &TrustKey) -> Result<(), CatalogError> {
    if key.key_id.is_empty() || key.algorithm != "ed25519" {
        return Err(CatalogError::new(
            "TRUST_ROOT_KEY_INVALID",
            "Trust key identity or algorithm is invalid",
        ));
    }
    decode_public_key(&key.public_key, "TRUST_ROOT_KEY_INVALID")?;
    let not_before = parse_timestamp(&key.not_before, "TRUST_ROOT_TIME_INVALID")?;
    let not_after = parse_timestamp(&key.not_after, "TRUST_ROOT_TIME_INVALID")?;
    if not_before >= not_after {
        return Err(CatalogError::new(
            "TRUST_ROOT_TIME_INVALID",
            "Trust key validity window must increase",
        ));
    }
    Ok(())
}

fn validate_catalog(catalog: &PluginCatalog) -> Result<(), CatalogError> {
    if catalog.schema_version != 1
        || catalog.sequence == 0
        || catalog.sequence > MAX_SAFE_INTEGER
        || catalog.signing_key_id.is_empty()
        || catalog.apps.is_empty()
    {
        return Err(CatalogError::new("CATALOG_INVALID", "Catalog is invalid"));
    }
    let generated_at = parse_timestamp(&catalog.generated_at, "CATALOG_TIME_INVALID")?;
    let expires_at = parse_timestamp(&catalog.expires_at, "CATALOG_TIME_INVALID")?;
    if generated_at >= expires_at {
        return Err(CatalogError::new(
            "CATALOG_TIME_INVALID",
            "Catalog validity window must increase",
        ));
    }

    let app_id_pattern =
        Regex::new(r"^[a-z0-9]+(?:[.-][a-z0-9]+)+$").expect("app ID pattern is valid");
    let sha256_pattern = Regex::new(r"^[a-f0-9]{64}$").expect("SHA-256 pattern is valid");
    let mut app_ids = HashSet::new();
    for app in &catalog.apps {
        if !app_id_pattern.is_match(&app.app_id)
            || app.name.trim().is_empty()
            || app.publisher_id.is_empty()
            || app.icon.is_empty()
            || !app_ids.insert(app.app_id.as_str())
        {
            return Err(CatalogError::new(
                "CATALOG_APP_INVALID",
                "Catalog application is invalid",
            ));
        }
        validate_release(&app.channels.stable, &sha256_pattern)?;
    }
    Ok(())
}

fn validate_release(release: &CatalogRelease, sha256_pattern: &Regex) -> Result<(), CatalogError> {
    if Version::parse(&release.version).is_err()
        || release.publisher_key_id.is_empty()
        || release.host_api.max != "1.x"
        || Version::parse(&release.host_api.min)
            .map(|version| version.major != 1)
            .unwrap_or(true)
        || release.artifacts.is_empty()
    {
        return Err(CatalogError::new(
            "CATALOG_RELEASE_INVALID",
            "Catalog release is invalid",
        ));
    }
    parse_timestamp(&release.published_at, "CATALOG_TIME_INVALID")?;
    let mut permissions = HashSet::new();
    for permission in &release.permissions {
        if !KNOWN_PERMISSIONS.contains(&permission.as_str()) {
            return Err(CatalogError::new(
                "CATALOG_PERMISSION_UNKNOWN",
                format!("Unknown Catalog permission: {permission}"),
            ));
        }
        if !permissions.insert(permission.as_str()) {
            return Err(CatalogError::new(
                "CATALOG_RELEASE_INVALID",
                "Catalog release contains duplicate permissions",
            ));
        }
    }

    let mut targets = HashSet::new();
    for artifact in &release.artifacts {
        let url = Url::parse(&artifact.url).map_err(|_| {
            CatalogError::new("CATALOG_ARTIFACT_INVALID", "Artifact URL is invalid")
        })?;
        if url.scheme() != "https"
            || artifact.file_name.is_empty()
            || artifact.file_name == "."
            || artifact.file_name == ".."
            || artifact
                .file_name
                .chars()
                .any(|character| matches!(character, '\\' | '/' | '\0' | ':'))
            || artifact.size == 0
            || artifact.size > MAX_SAFE_INTEGER
            || !sha256_pattern.is_match(&artifact.sha256)
        {
            return Err(CatalogError::new(
                "CATALOG_ARTIFACT_INVALID",
                "Catalog artifact is invalid",
            ));
        }
        if !targets.insert((artifact.os, artifact.arch)) {
            return Err(CatalogError::new(
                "CATALOG_DUPLICATE_TARGET",
                "Catalog release contains duplicate targets",
            ));
        }
    }
    Ok(())
}

fn key_contains_time(key: &TrustKey, signed_at: DateTime<Utc>) -> Result<bool, CatalogError> {
    Ok(
        signed_at >= parse_timestamp(&key.not_before, "TRUST_ROOT_TIME_INVALID")?
            && signed_at <= parse_timestamp(&key.not_after, "TRUST_ROOT_TIME_INVALID")?,
    )
}

fn validate_publishers(root: &TrustRoot, catalog: &PluginCatalog) -> Result<(), CatalogError> {
    for app in &catalog.apps {
        let publisher = root
            .publishers
            .iter()
            .find(|publisher| publisher.publisher_id == app.publisher_id)
            .ok_or_else(|| {
                CatalogError::new(
                    "CATALOG_PUBLISHER_UNKNOWN",
                    format!("Publisher {} is not trusted", app.publisher_id),
                )
            })?;
        let release = &app.channels.stable;
        let publisher_key = publisher
            .keys
            .iter()
            .find(|key| key.key_id == release.publisher_key_id)
            .ok_or_else(|| {
                CatalogError::new(
                    "CATALOG_PUBLISHER_KEY_UNKNOWN",
                    format!("Publisher key {} is not trusted", release.publisher_key_id),
                )
            })?;
        let published_at = parse_timestamp(&release.published_at, "CATALOG_TIME_INVALID")?;
        if publisher_key.status == KeyStatus::Revoked
            || !key_contains_time(publisher_key, published_at)?
        {
            return Err(CatalogError::new(
                "CATALOG_PUBLISHER_KEY_INACTIVE",
                "Publisher key cannot verify this release",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{sync::mpsc, time::Duration};

    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
    use chrono::{TimeZone, Utc};
    use ed25519_dalek::{Signer, SigningKey};
    use serde_json::Value;
    use tempfile::tempdir;

    use super::{
        canonicalize, install_plan_from_verified, snapshot_for_target, CatalogBundle, CatalogCache,
        CatalogTargetArch, CatalogTargetOs, CatalogVerifier,
    };

    const ROOT_PUBLIC_KEY: &str = "pZQzAC2N9PEN86+KXe878Z5IKqzt3GPl91tmhDUR2B0=";
    const ROOT_SIGNATURE: &str =
        "6xr/t+I1UvVFB2q7vp2avUQPqX4cF6fqz1oIO/XhfPFuopggAbHWt2UOHXxTF73EWUI8+dcsxk9LZMxnj+pfAg==";
    const CATALOG_SIGNATURE: &str =
        "rPRz4GMQmvhJ0x0J5y+EUxikuRHOSPJ1Q/t3TH1EMOln5XR2ZrbPCkk9uqgn5s7TTW+Cf7RJxYSO9NK69K2vAA==";
    const ROOT_JSON: &str = r#"{
      "schemaVersion": 1,
      "sequence": 1,
      "issuedAt": "2026-11-10T00:00:00Z",
      "expiresAt": "2027-11-10T00:00:00Z",
      "catalogKeys": [{
        "keyId": "catalog-2026-01",
        "algorithm": "ed25519",
        "publicKey": "pZQzAC2N9PEN86+KXe878Z5IKqzt3GPl91tmhDUR2B0=",
        "status": "active",
        "notBefore": "2026-11-01T00:00:00Z",
        "notAfter": "2027-05-01T00:00:00Z"
      }],
      "publishers": [{
        "publisherId": "com.attson",
        "name": "Attson",
        "keys": [{
          "keyId": "attson-codec-2026-01",
          "algorithm": "ed25519",
          "publicKey": "IiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiI=",
          "status": "active",
          "notBefore": "2026-11-01T00:00:00Z",
          "notAfter": "2027-05-01T00:00:00Z"
        }]
      }]
    }"#;
    const CATALOG_JSON: &str = r#"{
      "schemaVersion": 1,
      "sequence": 18,
      "generatedAt": "2026-11-10T08:00:00Z",
      "expiresAt": "2026-11-17T08:00:00Z",
      "signingKeyId": "catalog-2026-01",
      "apps": [{
        "appId": "com.attson.codec",
        "name": "编码转换",
        "description": "Base64、URL、Unicode、Hex、Hash 与 JWT",
        "publisherId": "com.attson",
        "icon": "codec",
        "channels": {
          "stable": {
            "version": "1.1.0",
            "publishedAt": "2026-11-10T07:00:00Z",
            "notes": "新增一键复制转换结果",
            "publisherKeyId": "attson-codec-2026-01",
            "hostApi": { "min": "1.0.0", "max": "1.x" },
            "permissions": ["host.theme.read", "host.file.save", "host.notification.show"],
            "artifacts": [{
              "os": "linux",
              "arch": "x86_64",
              "url": "https://github.com/attsontools/atcodec/releases/download/v1.1.0/AT.Codec_1.1.0_linux_x86_64.atpkg",
              "fileName": "AT.Codec_1.1.0_linux_x86_64.atpkg",
              "size": 4096,
              "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            }]
          }
        }
      }]
    }"#;

    fn signing_key() -> SigningKey {
        SigningKey::from_bytes(&[
            0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x66, 0x4c, 0x47, 0x15, 0x45, 0x91, 0x88,
            0x0d, 0x45, 0x06, 0x2b, 0xf5, 0x6c, 0xf5, 0xef, 0x3a, 0x82, 0xc5, 0xb1, 0xd7, 0x87,
            0x54, 0x0c, 0x8e, 0x00,
        ])
    }

    fn sign_value(value: &Value) -> String {
        let canonical = serde_json::to_vec(&canonicalize(value.clone())).unwrap();
        BASE64.encode(signing_key().sign(&canonical).to_bytes())
    }

    fn bundle_with_sequences(root_sequence: u64, catalog_sequence: u64) -> CatalogBundle {
        let mut root: Value = serde_json::from_str(ROOT_JSON).unwrap();
        root["sequence"] = Value::from(root_sequence);
        let mut catalog: Value = serde_json::from_str(CATALOG_JSON).unwrap();
        catalog["sequence"] = Value::from(catalog_sequence);
        CatalogBundle {
            root_json: root.to_string(),
            root_signature: sign_value(&root),
            catalog_json: catalog.to_string(),
            catalog_signature: sign_value(&catalog),
        }
    }

    #[test]
    fn accepts_sdk_golden_trust_chain() {
        let verifier = CatalogVerifier::from_base64(ROOT_PUBLIC_KEY).unwrap();
        let verified = verifier
            .verify(
                ROOT_JSON,
                ROOT_SIGNATURE,
                CATALOG_JSON,
                CATALOG_SIGNATURE,
                Utc.with_ymd_and_hms(2026, 11, 11, 0, 0, 0).unwrap(),
                0,
                0,
                true,
            )
            .unwrap();

        assert_eq!(verified.root.sequence, 1);
        assert_eq!(verified.catalog.sequence, 18);
        assert_eq!(verified.catalog.apps[0].name, "编码转换");
    }

    #[test]
    fn rejects_rollback_and_marks_an_expired_cache() {
        let verifier = CatalogVerifier::from_base64(ROOT_PUBLIC_KEY).unwrap();
        let now = Utc.with_ymd_and_hms(2026, 11, 11, 0, 0, 0).unwrap();
        let rollback = verifier
            .verify(
                ROOT_JSON,
                ROOT_SIGNATURE,
                CATALOG_JSON,
                CATALOG_SIGNATURE,
                now,
                2,
                19,
                true,
            )
            .unwrap_err();
        assert_eq!(rollback.code, "METADATA_SEQUENCE_ROLLBACK");

        let expired_at = Utc.with_ymd_and_hms(2026, 11, 18, 0, 0, 0).unwrap();
        let expired = verifier
            .verify(
                ROOT_JSON,
                ROOT_SIGNATURE,
                CATALOG_JSON,
                CATALOG_SIGNATURE,
                expired_at,
                0,
                0,
                false,
            )
            .unwrap();
        assert!(expired.expired);
        assert_eq!(
            verifier
                .verify(
                    ROOT_JSON,
                    ROOT_SIGNATURE,
                    CATALOG_JSON,
                    CATALOG_SIGNATURE,
                    expired_at,
                    0,
                    0,
                    true,
                )
                .unwrap_err()
                .code,
            "CATALOG_EXPIRED"
        );
    }

    #[test]
    fn rejected_candidate_does_not_replace_the_cached_catalog() {
        let directory = tempdir().unwrap();
        let cache = CatalogCache::open(directory.path().join("catalog.sqlite3")).unwrap();
        let verifier = CatalogVerifier::from_base64(ROOT_PUBLIC_KEY).unwrap();
        let now = Utc.with_ymd_and_hms(2026, 11, 11, 0, 0, 0).unwrap();
        let valid = CatalogBundle {
            root_json: ROOT_JSON.to_string(),
            root_signature: ROOT_SIGNATURE.to_string(),
            catalog_json: CATALOG_JSON.to_string(),
            catalog_signature: CATALOG_SIGNATURE.to_string(),
        };

        cache.accept(&verifier, valid.clone(), now).unwrap();
        let mut invalid = valid;
        invalid.catalog_signature = ROOT_SIGNATURE.to_string();
        assert_eq!(
            cache.accept(&verifier, invalid, now).unwrap_err().code,
            "CATALOG_SIGNATURE_INVALID"
        );

        let cached = cache.load(&verifier, now).unwrap().unwrap();
        assert_eq!(cached.catalog.sequence, 18);
        assert_eq!(cached.catalog.apps[0].name, "编码转换");
    }

    #[test]
    fn concurrent_accept_cannot_replace_newer_sequences_with_stale_candidate() {
        let directory = tempdir().unwrap();
        let cache = CatalogCache::open(directory.path().join("catalog.sqlite3")).unwrap();
        let verifier = CatalogVerifier::from_base64(ROOT_PUBLIC_KEY).unwrap();
        let now = Utc.with_ymd_and_hms(2026, 11, 11, 0, 0, 0).unwrap();
        let stale_bundle = bundle_with_sequences(1, 18);
        let stale_verified = cache
            .verify_candidate(&verifier, &stale_bundle, now)
            .unwrap();
        let (newer_stored_tx, newer_stored_rx) = mpsc::channel();

        let newer_cache = cache.clone();
        let newer_verifier = verifier.clone();
        let newer = std::thread::spawn(move || {
            newer_cache
                .accept(&newer_verifier, bundle_with_sequences(2, 19), now)
                .unwrap();
            newer_stored_tx.send(()).unwrap();
        });
        newer_stored_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap();

        let stale_result = cache.store_verified(stale_bundle, &stale_verified, now);
        newer.join().unwrap();

        assert_eq!(stale_result.unwrap_err().code, "METADATA_SEQUENCE_ROLLBACK");
        let cached = cache.load(&verifier, now).unwrap().unwrap();
        assert_eq!(cached.root.sequence, 2);
        assert_eq!(cached.catalog.sequence, 19);
    }

    #[test]
    fn snapshot_selects_the_current_target_and_publisher() {
        let verifier = CatalogVerifier::from_base64(ROOT_PUBLIC_KEY).unwrap();
        let verified = verifier
            .verify(
                ROOT_JSON,
                ROOT_SIGNATURE,
                CATALOG_JSON,
                CATALOG_SIGNATURE,
                Utc.with_ymd_and_hms(2026, 11, 11, 0, 0, 0).unwrap(),
                0,
                0,
                true,
            )
            .unwrap();

        let snapshot =
            snapshot_for_target(&verified, CatalogTargetOs::Linux, CatalogTargetArch::X86_64);
        let codec = &snapshot.apps[0];
        assert_eq!(codec.name, "编码转换");
        assert_eq!(codec.publisher_name, "Attson");
        assert_eq!(codec.latest_version, "1.1.0");
        assert_eq!(codec.host_api_min, "1.0.0");
        assert_eq!(
            codec.target.as_ref().unwrap().sha256,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
    }

    #[test]
    fn install_plan_accepts_only_the_immutable_official_release_asset() {
        let verifier = CatalogVerifier::from_base64(ROOT_PUBLIC_KEY).unwrap();
        let verified = verifier
            .verify(
                ROOT_JSON,
                ROOT_SIGNATURE,
                CATALOG_JSON,
                CATALOG_SIGNATURE,
                Utc.with_ymd_and_hms(2026, 11, 11, 0, 0, 0).unwrap(),
                0,
                0,
                true,
            )
            .unwrap();

        let plan = install_plan_from_verified(
            &verified,
            "com.attson.codec",
            CatalogTargetOs::Linux,
            CatalogTargetArch::X86_64,
        )
        .unwrap();
        assert_eq!(plan.version, "1.1.0");
        assert_eq!(plan.publisher_key_id, "attson-codec-2026-01");

        let mut redirected = verified.clone();
        redirected.catalog.apps[0].channels.stable.artifacts[0].url =
            "https://example.com/latest/codec.atpkg".to_string();
        let error = install_plan_from_verified(
            &redirected,
            "com.attson.codec",
            CatalogTargetOs::Linux,
            CatalogTargetArch::X86_64,
        )
        .expect_err("non-release URL must fail");
        assert_eq!(error.code, "CATALOG_ARTIFACT_URL_UNTRUSTED");

        let mut alternate_endpoint = verified.clone();
        alternate_endpoint.catalog.apps[0].channels.stable.artifacts[0].url =
            "https://user@github.com:444/attsontools/atcodec/releases/download/v1.1.0/AT.Codec_1.1.0_linux_x86_64.atpkg".to_string();
        let error = install_plan_from_verified(
            &alternate_endpoint,
            "com.attson.codec",
            CatalogTargetOs::Linux,
            CatalogTargetArch::X86_64,
        )
        .expect_err("alternate endpoint components must fail");
        assert_eq!(error.code, "CATALOG_ARTIFACT_URL_UNTRUSTED");
    }

    #[test]
    fn maps_validly_signed_schema_and_key_policy_errors() {
        let verifier = CatalogVerifier::from_base64(ROOT_PUBLIC_KEY).unwrap();
        let now = Utc.with_ymd_and_hms(2026, 11, 11, 0, 0, 0).unwrap();
        let catalog: Value = serde_json::from_str(CATALOG_JSON).unwrap();

        let mut unknown_root: Value = serde_json::from_str(ROOT_JSON).unwrap();
        unknown_root["unexpected"] = Value::Bool(true);
        assert_eq!(
            verifier
                .verify(
                    &unknown_root.to_string(),
                    &sign_value(&unknown_root),
                    CATALOG_JSON,
                    CATALOG_SIGNATURE,
                    now,
                    0,
                    0,
                    true,
                )
                .unwrap_err()
                .code,
            "TRUST_ROOT_INVALID"
        );

        let mut retired_root: Value = serde_json::from_str(ROOT_JSON).unwrap();
        retired_root["catalogKeys"][0]["status"] = Value::String("retired".to_string());
        verifier
            .verify(
                &retired_root.to_string(),
                &sign_value(&retired_root),
                CATALOG_JSON,
                CATALOG_SIGNATURE,
                now,
                0,
                0,
                true,
            )
            .expect("retired key must verify a Catalog signed inside its validity window");

        let mut revoked_catalog_root: Value = serde_json::from_str(ROOT_JSON).unwrap();
        revoked_catalog_root["catalogKeys"][0]["status"] = Value::String("revoked".to_string());
        assert_eq!(
            verifier
                .verify(
                    &revoked_catalog_root.to_string(),
                    &sign_value(&revoked_catalog_root),
                    CATALOG_JSON,
                    CATALOG_SIGNATURE,
                    now,
                    0,
                    0,
                    true,
                )
                .unwrap_err()
                .code,
            "CATALOG_KEY_INACTIVE"
        );

        let mut revoked_root: Value = serde_json::from_str(ROOT_JSON).unwrap();
        revoked_root["publishers"][0]["keys"][0]["status"] = Value::String("revoked".to_string());
        assert_eq!(
            verifier
                .verify(
                    &revoked_root.to_string(),
                    &sign_value(&revoked_root),
                    &catalog.to_string(),
                    CATALOG_SIGNATURE,
                    now,
                    0,
                    0,
                    true,
                )
                .unwrap_err()
                .code,
            "CATALOG_PUBLISHER_KEY_INACTIVE"
        );
    }
}
