use serde::Deserialize;
use std::time::Duration;

const BODY_LIMIT_BYTES: usize = 5 * 1024 * 1024;
const REQUEST_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyValuePair {
    pub key: String,
    pub value: String,
}

#[tauri::command]
pub async fn fetch_openapi_url(
    url: String,
    headers: Vec<KeyValuePair>,
) -> Result<String, String> {
    fetch_impl(&url, &headers).await
}

async fn fetch_impl(url: &str, headers: &[KeyValuePair]) -> Result<String, String> {
    let parsed = reqwest::Url::parse(url).map_err(|e| format!("URL 无法解析：{e}"))?;
    match parsed.scheme() {
        "http" | "https" => {}
        other => return Err(format!("不支持的 scheme：{other}（只允许 http/https）")),
    }

    let mut header_map = reqwest::header::HeaderMap::new();
    for h in headers {
        if h.key.trim().is_empty() {
            continue;
        }
        let name = reqwest::header::HeaderName::from_bytes(h.key.as_bytes())
            .map_err(|e| format!("非法 header key `{k}`：{e}", k = h.key))?;
        let value = reqwest::header::HeaderValue::from_str(&h.value)
            .map_err(|e| format!("非法 header value（key=`{k}`）：{e}", k = h.key))?;
        header_map.insert(name, value);
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败：{e}"))?;

    let resp = client
        .get(parsed)
        .headers(header_map)
        .send()
        .await
        .map_err(|e| format!("请求失败：{e}"))?;

    let status = resp.status();
    if !status.is_success() {
        let snippet = read_body_snippet(resp, 500).await.unwrap_or_default();
        return Err(format!("HTTP {}: {snippet}", status.as_u16()));
    }

    if let Some(len) = resp.content_length() {
        if (len as usize) > BODY_LIMIT_BYTES {
            return Err(format!(
                "响应体超过 {} MB 上限（Content-Length={len}）",
                BODY_LIMIT_BYTES / 1024 / 1024
            ));
        }
    }

    let bytes = read_body_capped(resp, BODY_LIMIT_BYTES).await?;
    String::from_utf8(bytes).map_err(|e| format!("响应体不是合法 UTF-8：{e}"))
}

async fn read_body_capped(
    mut resp: reqwest::Response,
    cap: usize,
) -> Result<Vec<u8>, String> {
    use futures_util::StreamExt;
    let mut buf = Vec::<u8>::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("读取响应流失败：{e}"))?;
        if buf.len() + chunk.len() > cap {
            return Err(format!(
                "响应体超过 {} MB 上限",
                cap / 1024 / 1024
            ));
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

async fn read_body_snippet(resp: reqwest::Response, max_chars: usize) -> Option<String> {
    let bytes = read_body_capped(resp, BODY_LIMIT_BYTES).await.ok()?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    Some(text.chars().take(max_chars).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rejects_non_http_scheme() {
        let err = fetch_impl("file:///etc/passwd", &[]).await.unwrap_err();
        assert!(err.contains("scheme"), "got: {err}");
    }

    #[tokio::test]
    async fn rejects_bad_url() {
        let err = fetch_impl("not a url", &[]).await.unwrap_err();
        assert!(err.contains("URL 无法解析"), "got: {err}");
    }
}
