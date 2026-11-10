use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::File,
    io::Read,
    path::Path,
};

use base64::Engine;
use ed25519_dalek::{Signature, VerifyingKey};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use zip::ZipArchive;

const MAX_MANIFEST_BYTES: u64 = 256 * 1024;
const MAX_SIGNATURE_BYTES: u64 = 1024;
const MAX_FILE_BYTES: u64 = 100 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct HostEnvironment {
    pub os: String,
    pub arch: String,
    pub host_api_version: Version,
}

impl HostEnvironment {
    pub fn current(host_api_version: &str) -> Self {
        Self {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            host_api_version: Version::parse(host_api_version).expect("valid host API version"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectedPackage {
    pub app_id: String,
    pub name: String,
    pub version: String,
    pub publisher_id: String,
    pub publisher_name: String,
    pub publisher_key_id: String,
    pub target_os: String,
    pub target_arch: String,
    pub host_api_min: String,
    pub host_api_max: String,
    pub entry: String,
    pub permissions: Vec<String>,
}

#[derive(Debug)]
pub struct PackageError {
    pub code: &'static str,
    pub message: String,
}

impl PackageError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for PackageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for PackageError {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    app_id: String,
    name: String,
    version: String,
    publisher: Publisher,
    target: Target,
    host_api: HostApi,
    ui: Ui,
    permissions: Vec<String>,
    files: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Publisher {
    id: String,
    name: String,
    key_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Target {
    os: String,
    arch: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HostApi {
    min: String,
    max: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ui {
    entry: String,
}

pub fn inspect_package(
    path: &Path,
    environment: &HostEnvironment,
    trusted_keys: &HashMap<String, VerifyingKey>,
) -> Result<InspectedPackage, PackageError> {
    let file = File::open(path).map_err(|error| {
        PackageError::new(
            "PLUGIN_PACKAGE_READ_FAILED",
            format!("Cannot open package: {error}"),
        )
    })?;
    let mut archive = ZipArchive::new(file).map_err(|error| {
        PackageError::new(
            "PLUGIN_PACKAGE_INVALID",
            format!("Invalid package archive: {error}"),
        )
    })?;
    let manifest_bytes = read_entry(&mut archive, "manifest.json", MAX_MANIFEST_BYTES)?;
    let signature_bytes = read_entry(&mut archive, "manifest.sig", MAX_SIGNATURE_BYTES)?;
    let manifest_value: Value = serde_json::from_slice(&manifest_bytes).map_err(|error| {
        PackageError::new(
            "PLUGIN_MANIFEST_INVALID",
            format!("Invalid manifest JSON: {error}"),
        )
    })?;
    let manifest: Manifest = serde_json::from_value(manifest_value.clone()).map_err(|error| {
        PackageError::new(
            "PLUGIN_MANIFEST_INVALID",
            format!("Invalid manifest: {error}"),
        )
    })?;

    verify_archive_layout(&mut archive, &manifest)?;
    validate_manifest(&manifest, environment)?;
    verify_signature(&manifest, &manifest_value, &signature_bytes, trusted_keys)?;
    verify_files(&mut archive, &manifest)?;

    Ok(InspectedPackage {
        app_id: manifest.app_id,
        name: manifest.name,
        version: manifest.version,
        publisher_id: manifest.publisher.id,
        publisher_name: manifest.publisher.name,
        publisher_key_id: manifest.publisher.key_id,
        target_os: manifest.target.os,
        target_arch: manifest.target.arch,
        host_api_min: manifest.host_api.min,
        host_api_max: manifest.host_api.max,
        entry: manifest.ui.entry,
        permissions: manifest.permissions,
    })
}

fn verify_archive_layout(
    archive: &mut ZipArchive<File>,
    manifest: &Manifest,
) -> Result<(), PackageError> {
    let mut allowed = manifest
        .files
        .keys()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    allowed.insert("manifest.json");
    allowed.insert("manifest.sig");
    let mut seen = HashSet::new();

    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| {
            PackageError::new(
                "PLUGIN_PACKAGE_INVALID",
                format!("Cannot inspect package entry: {error}"),
            )
        })?;
        let name = entry.name().to_string();
        if entry.is_dir() || validate_package_path(&name).is_err() {
            return Err(PackageError::new(
                "PLUGIN_FILE_PATH_INVALID",
                format!("Invalid package entry: {name}"),
            ));
        }
        if !seen.insert(name.clone()) {
            return Err(PackageError::new(
                "PLUGIN_FILE_DUPLICATE",
                format!("Duplicate package entry: {name}"),
            ));
        }
        if !allowed.contains(name.as_str()) {
            return Err(PackageError::new(
                "PLUGIN_FILE_UNDECLARED",
                format!("Undeclared package file: {name}"),
            ));
        }
    }
    Ok(())
}

fn validate_manifest(
    manifest: &Manifest,
    environment: &HostEnvironment,
) -> Result<(), PackageError> {
    if manifest.schema_version != 1 {
        return Err(PackageError::new(
            "PLUGIN_MANIFEST_SCHEMA_UNSUPPORTED",
            "Only manifest schema version 1 is supported",
        ));
    }
    let app_id_pattern =
        regex::Regex::new(r"^[a-z0-9]+(?:[.-][a-z0-9]+)+$").expect("app ID pattern is valid");
    if !app_id_pattern.is_match(&manifest.app_id) {
        return Err(PackageError::new(
            "PLUGIN_APP_ID_INVALID",
            "Package appId is invalid",
        ));
    }
    if manifest.permissions.iter().any(|permission| {
        !matches!(
            permission.as_str(),
            "host.theme.read" | "host.file.save" | "host.notification.show"
        )
    }) {
        return Err(PackageError::new(
            "PLUGIN_PERMISSION_UNKNOWN",
            "Package requests an unknown permission",
        ));
    }
    if manifest.target.os != environment.os || manifest.target.arch != environment.arch {
        return Err(PackageError::new(
            "PLUGIN_TARGET_MISMATCH",
            format!(
                "Package target {}/{} does not match host {}/{}",
                manifest.target.os, manifest.target.arch, environment.os, environment.arch
            ),
        ));
    }
    let host_min = Version::parse(&manifest.host_api.min).map_err(|_| {
        PackageError::new(
            "PLUGIN_HOST_API_UNSUPPORTED",
            "Invalid minimum Host API version",
        )
    })?;
    if manifest.host_api.max != "1.x"
        || host_min.major != 1
        || environment.host_api_version.major != 1
        || environment.host_api_version < host_min
    {
        return Err(PackageError::new(
            "PLUGIN_HOST_API_UNSUPPORTED",
            "Package requires an unsupported Host API version",
        ));
    }
    if !manifest.files.contains_key(&manifest.ui.entry) {
        return Err(PackageError::new(
            "PLUGIN_ENTRY_MISSING",
            "UI entry is not declared in manifest files",
        ));
    }
    Ok(())
}

fn verify_signature(
    manifest: &Manifest,
    manifest_value: &Value,
    signature_bytes: &[u8],
    trusted_keys: &HashMap<String, VerifyingKey>,
) -> Result<(), PackageError> {
    let key = trusted_keys
        .get(&manifest.publisher.key_id)
        .ok_or_else(|| {
            PackageError::new(
                "PLUGIN_PUBLISHER_UNTRUSTED",
                "Package publisher key is not trusted",
            )
        })?;
    let signature_text = std::str::from_utf8(signature_bytes).map_err(|_| {
        PackageError::new("PLUGIN_SIGNATURE_INVALID", "Package signature is not UTF-8")
    })?;
    let signature_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature_text.trim())
        .map_err(|_| {
            PackageError::new(
                "PLUGIN_SIGNATURE_INVALID",
                "Package signature is not valid base64",
            )
        })?;
    let signature = Signature::from_slice(&signature_bytes).map_err(|_| {
        PackageError::new(
            "PLUGIN_SIGNATURE_INVALID",
            "Package signature has an invalid length",
        )
    })?;
    let canonical = canonical_json(manifest_value).map_err(|error| {
        PackageError::new(
            "PLUGIN_MANIFEST_INVALID",
            format!("Cannot canonicalize manifest: {error}"),
        )
    })?;
    key.verify_strict(canonical.as_bytes(), &signature)
        .map_err(|_| PackageError::new("PLUGIN_SIGNATURE_INVALID", "Package signature is invalid"))
}

fn verify_files(archive: &mut ZipArchive<File>, manifest: &Manifest) -> Result<(), PackageError> {
    for (path, expected_hash) in &manifest.files {
        validate_package_path(path)?;
        let bytes = read_entry(archive, path, MAX_FILE_BYTES)?;
        let actual_hash = hex::encode(Sha256::digest(&bytes));
        if &actual_hash != expected_hash {
            return Err(PackageError::new(
                "PLUGIN_FILE_HASH_MISMATCH",
                format!("Package file hash does not match: {path}"),
            ));
        }
    }
    Ok(())
}

fn validate_package_path(path: &str) -> Result<(), PackageError> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(PackageError::new(
            "PLUGIN_FILE_PATH_INVALID",
            format!("Unsafe package path: {path}"),
        ));
    }
    Ok(())
}

fn read_entry(
    archive: &mut ZipArchive<File>,
    name: &str,
    max_bytes: u64,
) -> Result<Vec<u8>, PackageError> {
    let mut entry = archive.by_name(name).map_err(|_| {
        PackageError::new(
            "PLUGIN_FILE_MISSING",
            format!("Package file is missing: {name}"),
        )
    })?;
    if entry.is_dir() || entry.size() > max_bytes {
        return Err(PackageError::new(
            "PLUGIN_FILE_INVALID",
            format!("Package file is invalid or too large: {name}"),
        ));
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut bytes).map_err(|error| {
        PackageError::new(
            "PLUGIN_PACKAGE_READ_FAILED",
            format!("Cannot read package file {name}: {error}"),
        )
    })?;
    Ok(bytes)
}

fn canonical_json(value: &Value) -> Result<String, serde_json::Error> {
    match value {
        Value::Object(object) => {
            let sorted = object
                .iter()
                .map(|(key, value)| (key.clone(), canonical_value(value)))
                .collect::<BTreeMap<_, _>>();
            serde_json::to_string(&sorted)
        }
        _ => serde_json::to_string(&canonical_value(value)),
    }
}

fn canonical_value(value: &Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.iter().map(canonical_value).collect()),
        Value::Object(object) => {
            let sorted = object
                .iter()
                .map(|(key, value)| (key.clone(), canonical_value(value)))
                .collect::<BTreeMap<_, _>>();
            serde_json::to_value(sorted).expect("JSON value is serializable")
        }
        value => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        fs::{File, OpenOptions},
        io::Write,
    };

    use base64::Engine;
    use ed25519_dalek::{Signer, SigningKey};
    use sha2::{Digest, Sha256};
    use zip::{write::SimpleFileOptions, ZipWriter};

    use super::{inspect_package, HostEnvironment};

    const TEST_KEY_SEED: [u8; 32] = [
        0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c,
        0xc4, 0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae,
        0x7f, 0x60,
    ];

    fn sha256(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    fn package_with_index(
        archived_index: &[u8],
        hashed_index: &[u8],
        app_id: &str,
        permissions_json: &str,
    ) -> (tempfile::TempDir, std::path::PathBuf, SigningKey) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("codec.atpkg");
        let signing_key = SigningKey::from_bytes(&TEST_KEY_SEED);
        let license = b"P0 test license";
        let manifest = format!(
            concat!(
                "{{\"appId\":\"{}\",",
                "\"files\":{{\"LICENSE\":\"{}\",\"ui/index.html\":\"{}\"}},",
                "\"hostApi\":{{\"max\":\"1.x\",\"min\":\"1.0.0\"}},",
                "\"name\":\"AT Codec\",",
                "\"permissions\":{},",
                "\"publisher\":{{\"id\":\"com.attson\",\"keyId\":\"attool-p0-test\",\"name\":\"Attson\"}},",
                "\"schemaVersion\":1,",
                "\"target\":{{\"arch\":\"{}\",\"os\":\"{}\"}},",
                "\"ui\":{{\"entry\":\"ui/index.html\"}},",
                "\"version\":\"0.1.0-alpha.0\"}}"
            ),
            app_id,
            sha256(license),
            sha256(hashed_index),
            permissions_json,
            std::env::consts::ARCH,
            std::env::consts::OS
        );
        let signature = signing_key.sign(manifest.as_bytes());
        let signature = base64::engine::general_purpose::STANDARD.encode(signature.to_bytes());

        let file = File::create(&path).unwrap();
        let mut archive = ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        for (name, bytes) in [
            ("manifest.json", manifest.as_bytes()),
            ("manifest.sig", signature.as_bytes()),
            ("LICENSE", license.as_slice()),
            ("ui/index.html", archived_index),
        ] {
            archive.start_file(name, options).unwrap();
            archive.write_all(bytes).unwrap();
        }
        archive.finish().unwrap();

        (directory, path, signing_key)
    }

    fn valid_package() -> (tempfile::TempDir, std::path::PathBuf, SigningKey) {
        let index = b"<!doctype html><title>Codec</title>";
        package_with_index(index, index, "com.attson.codec", "[\"host.theme.read\"]")
    }

    #[test]
    fn inspects_a_valid_signed_package() {
        let (_directory, path, signing_key) = valid_package();
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);
        let environment = HostEnvironment::current("1.0.0");

        let package = inspect_package(&path, &environment, &keys).unwrap();

        assert_eq!(package.app_id, "com.attson.codec");
        assert_eq!(package.version, "0.1.0-alpha.0");
        assert_eq!(package.entry, "ui/index.html");
        assert_eq!(package.permissions, vec!["host.theme.read"]);
    }

    #[test]
    fn rejects_an_invalid_signature() {
        let (_directory, path, _signing_key) = valid_package();
        let different_key = SigningKey::from_bytes(&[7; 32]);
        let keys = HashMap::from([("attool-p0-test".to_string(), different_key.verifying_key())]);

        let error = inspect_package(&path, &HostEnvironment::current("1.0.0"), &keys)
            .expect_err("signature must fail");

        assert_eq!(error.code, "PLUGIN_SIGNATURE_INVALID");
    }

    #[test]
    fn rejects_a_modified_package_file() {
        let original = b"<!doctype html><title>Codec</title>";
        let tampered = b"<!doctype html><title>Tampered</title>";
        let (_directory, path, signing_key) = package_with_index(
            tampered,
            original,
            "com.attson.codec",
            "[\"host.theme.read\"]",
        );
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);

        let error = inspect_package(&path, &HostEnvironment::current("1.0.0"), &keys)
            .expect_err("hash must fail");

        assert_eq!(error.code, "PLUGIN_FILE_HASH_MISMATCH");
    }

    #[test]
    fn rejects_a_package_for_another_target() {
        let (_directory, path, signing_key) = valid_package();
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);
        let mut environment = HostEnvironment::current("1.0.0");
        environment.arch = "unsupported-test-arch".to_string();

        let error = inspect_package(&path, &environment, &keys).expect_err("target must fail");

        assert_eq!(error.code, "PLUGIN_TARGET_MISMATCH");
    }

    #[test]
    fn rejects_an_unsupported_host_api_major() {
        let (_directory, path, signing_key) = valid_package();
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);

        let error = inspect_package(&path, &HostEnvironment::current("2.0.0"), &keys)
            .expect_err("Host API must fail");

        assert_eq!(error.code, "PLUGIN_HOST_API_UNSUPPORTED");
    }

    #[test]
    fn rejects_a_signed_manifest_with_an_invalid_app_id() {
        let index = b"<!doctype html><title>Codec</title>";
        let (_directory, path, signing_key) =
            package_with_index(index, index, "../codec", "[\"host.theme.read\"]");
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);

        let error = inspect_package(&path, &HostEnvironment::current("1.0.0"), &keys)
            .expect_err("appId must fail");

        assert_eq!(error.code, "PLUGIN_APP_ID_INVALID");
    }

    #[test]
    fn rejects_a_signed_manifest_with_an_unknown_permission() {
        let index = b"<!doctype html><title>Codec</title>";
        let (_directory, path, signing_key) =
            package_with_index(index, index, "com.attson.codec", "[\"host.process.spawn\"]");
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);

        let error = inspect_package(&path, &HostEnvironment::current("1.0.0"), &keys)
            .expect_err("permission must fail");

        assert_eq!(error.code, "PLUGIN_PERMISSION_UNKNOWN");
    }

    #[test]
    fn rejects_an_undeclared_archive_file() {
        let (_directory, path, signing_key) = valid_package();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        let mut archive = ZipWriter::new_append(file).unwrap();
        archive
            .start_file("hidden/payload.js", SimpleFileOptions::default())
            .unwrap();
        archive.write_all(b"unexpected").unwrap();
        archive.finish().unwrap();
        let keys = HashMap::from([("attool-p0-test".to_string(), signing_key.verifying_key())]);

        let error = inspect_package(&path, &HostEnvironment::current("1.0.0"), &keys)
            .expect_err("undeclared file must fail");

        assert_eq!(error.code, "PLUGIN_FILE_UNDECLARED");
    }
}
