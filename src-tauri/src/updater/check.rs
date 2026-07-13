use std::time::Duration;

use semver::Version;

use super::models::{GithubRelease, ReleaseInfo};

const RELEASES_LATEST_URL: &str = "https://api.github.com/repos/attson/attool/releases/latest";

pub fn newer_than_current(tag: &str, current: &str) -> bool {
    let tag = tag.trim_start_matches('v');
    let current = current.trim_start_matches('v');
    match (Version::parse(tag), Version::parse(current)) {
        (Ok(t), Ok(c)) => t > c,
        _ => false,
    }
}

pub fn expected_asset_name(version: &str) -> Result<String, String> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    // GitHub releases 把文件名里的空格换成点，`AT Tool` → `AT.Tool`
    let stem = format!("AT.Tool_{version}");
    let name = match (os, arch) {
        ("macos", "aarch64") => format!("{stem}_arm64.app.tar.gz"),
        ("macos", "x86_64") => format!("{stem}_amd64.app.tar.gz"),
        ("windows", "x86_64") => format!("{stem}_amd64.exe.zip"),
        ("linux", "aarch64") => format!("{stem}_arm64.tar.gz"),
        ("linux", "x86_64") => format!("{stem}_amd64.tar.gz"),
        _ => return Err(format!("暂无 {os}/{arch} 的更新档案")),
    };
    Ok(name)
}

pub async fn fetch_latest_release() -> Result<GithubRelease, String> {
    let client = reqwest::Client::builder()
        .user_agent(format!("attool/{}", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|error| format!("构造 HTTP 客户端失败：{error}"))?;
    let resp = client
        .get(RELEASES_LATEST_URL)
        .send()
        .await
        .map_err(|error| format!("拉取 releases 失败：{error}"))?
        .error_for_status()
        .map_err(|error| format!("GitHub API 返回错误：{error}"))?;
    resp.json::<GithubRelease>()
        .await
        .map_err(|error| format!("解析 release JSON 失败：{error}"))
}

pub fn build_release_info(release: &GithubRelease) -> Result<ReleaseInfo, String> {
    let version = release.tag_name.trim_start_matches('v').to_string();
    let asset_name = expected_asset_name(&version)?;
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == asset_name)
        .ok_or_else(|| format!("release 中缺少 {asset_name} 资源"))?;
    Ok(ReleaseInfo {
        version,
        notes: release.body.clone().unwrap_or_default(),
        published_at: release.published_at.clone().unwrap_or_default(),
        asset_name: asset.name.clone(),
        asset_url: asset.browser_download_url.clone(),
        asset_size: asset.size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_strictly_greater() {
        assert!(newer_than_current("0.8.5", "0.8.4"));
        assert!(!newer_than_current("0.8.5", "0.8.5"));
        assert!(!newer_than_current("0.8.4", "0.8.5"));
    }

    #[test]
    fn v_prefix_tolerated() {
        assert!(newer_than_current("v0.9.0", "0.8.5"));
        assert!(newer_than_current("0.9.0", "v0.8.5"));
    }

    #[test]
    fn invalid_returns_false() {
        assert!(!newer_than_current("foo", "0.8.5"));
        assert!(!newer_than_current("0.8.5", "bar"));
    }

    #[test]
    fn parses_release_with_null_body_and_published_at() {
        // GitHub 在 release 未填描述 / 未 publish 时会把这两个字段序列化成 null。
        let json = r#"{
            "tag_name": "v0.9.0",
            "body": null,
            "published_at": null,
            "assets": []
        }"#;
        let release: GithubRelease =
            serde_json::from_str(json).expect("null body/published_at should decode");
        assert_eq!(release.tag_name, "v0.9.0");
        assert!(release.body.is_none());
        assert!(release.published_at.is_none());
    }
}
