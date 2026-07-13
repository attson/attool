use base64::Engine;
use ed25519_dalek::Signature;
use sha2::{Digest, Sha256};

use super::keys::verify_public_key;

pub fn verify_checksums_signature(sums_bytes: &[u8], sig_b64: &str) -> Result<(), String> {
    let pub_key = verify_public_key()?;
    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(sig_b64.trim())
        .map_err(|error| format!("SHA256SUMS.sig base64 解码失败：{error}"))?;
    let sig = Signature::from_slice(&sig_bytes)
        .map_err(|error| format!("SHA256SUMS.sig 长度非法：{error}"))?;
    pub_key
        .verify_strict(sums_bytes, &sig)
        .map_err(|error| format!("SHA256SUMS 签名校验失败：{error}"))?;
    Ok(())
}

pub fn lookup_expected_sha256(sums: &str, filename: &str) -> Option<String> {
    for line in sums.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut it = trimmed.split_whitespace();
        let hash = it.next()?;
        let name = it.next()?;
        if name == filename {
            return Some(hash.to_string());
        }
    }
    None
}

pub fn compute_sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_matches_exact_name() {
        let sums = "aaa  file1.tar.gz\nbbb  file2.dmg\n";
        assert_eq!(
            lookup_expected_sha256(sums, "file1.tar.gz"),
            Some("aaa".to_string())
        );
        assert_eq!(lookup_expected_sha256(sums, "file2.dmg"), Some("bbb".to_string()));
    }

    #[test]
    fn lookup_returns_none_on_miss() {
        let sums = "aaa  file1.tar.gz\n";
        assert!(lookup_expected_sha256(sums, "nope").is_none());
    }

    #[test]
    fn lookup_skips_empty_and_comment_lines() {
        let sums = "\n# comment\naaa  file.tar.gz\n";
        assert_eq!(
            lookup_expected_sha256(sums, "file.tar.gz"),
            Some("aaa".to_string())
        );
    }

    #[test]
    fn sha256_matches_known_vector() {
        // sha256("hello") = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
        assert_eq!(
            compute_sha256_hex(b"hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }
}
