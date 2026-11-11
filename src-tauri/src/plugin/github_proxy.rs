use url::Url;

pub fn fallback_url(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    if parsed.scheme() != "https"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !matches!(
            parsed.host_str(),
            Some("github.com" | "raw.githubusercontent.com")
        )
    {
        return None;
    }
    Some(format!("https://gh-proxy.com/{}", parsed.as_str()))
}

pub async fn send_with_fallback(
    client: &reqwest::Client,
    url: &str,
) -> Result<reqwest::Response, reqwest::Error> {
    match client.get(url).send().await {
        Ok(response) => Ok(response),
        Err(direct_error) => match fallback_url(url) {
            Some(fallback) => client.get(fallback).send().await,
            None => Err(direct_error),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::fallback_url;

    #[test]
    fn maps_official_github_download_urls_to_the_proxy() {
        let raw =
            "https://raw.githubusercontent.com/attsontools/attool/main/catalog/v1/catalog.json";
        assert_eq!(
            fallback_url(raw).as_deref(),
            Some("https://gh-proxy.com/https://raw.githubusercontent.com/attsontools/attool/main/catalog/v1/catalog.json")
        );

        let release = "https://github.com/attsontools/atcodec/releases/download/v0.1.0-alpha.1/AT.Codec_0.1.0-alpha.1_linux_x86_64.atpkg";
        assert_eq!(
            fallback_url(release).as_deref(),
            Some("https://gh-proxy.com/https://github.com/attsontools/atcodec/releases/download/v0.1.0-alpha.1/AT.Codec_0.1.0-alpha.1_linux_x86_64.atpkg")
        );
    }

    #[test]
    fn rejects_non_official_or_ambiguous_urls() {
        for url in [
            "http://github.com/attsontools/atcodec/releases/download/v1/codec.atpkg",
            "https://github.com.evil.example/attsontools/atcodec/releases/download/v1/codec.atpkg",
            "https://user@github.com/attsontools/atcodec/releases/download/v1/codec.atpkg",
            "https://github.com:444/attsontools/atcodec/releases/download/v1/codec.atpkg",
            "https://github.com/attsontools/atcodec/releases/download/v1/codec.atpkg?token=secret",
            "https://example.com/codec.atpkg",
        ] {
            assert_eq!(fallback_url(url), None, "unexpected proxy URL for {url}");
        }
    }
}
