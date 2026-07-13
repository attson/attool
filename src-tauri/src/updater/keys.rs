use ed25519_dalek::pkcs8::DecodePublicKey;
use ed25519_dalek::VerifyingKey;

const VERIFY_PUBLIC_KEY_PEM: &str =
    include_str!(concat!(env!("OUT_DIR"), "/verify_public_key.pem"));

pub fn is_enabled() -> bool {
    !VERIFY_PUBLIC_KEY_PEM.trim().is_empty()
}

pub fn verify_public_key() -> Result<VerifyingKey, String> {
    let pem = VERIFY_PUBLIC_KEY_PEM.trim();
    if pem.is_empty() {
        return Err("公钥未配置（当前构建禁用了更新）".to_string());
    }
    VerifyingKey::from_public_key_pem(pem)
        .map_err(|error| format!("解析公钥 PEM 失败：{error}"))
}
