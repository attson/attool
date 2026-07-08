use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::time::{Duration, Instant};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyValue {
    pub key: String,
    pub value: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRequestSpec {
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub headers: Vec<KeyValue>,
    #[serde(default)]
    pub query_params: Vec<KeyValue>,
    #[serde(default)]
    pub body_type: String, // "none" | "json" | "form" | "text"
    #[serde(default)]
    pub body: String,
    pub timeout_seconds: Option<u32>,
    #[serde(default = "default_true")]
    pub follow_redirects: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpResponseInfo {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub body_bytes: u64,
    pub elapsed_ms: u128,
    pub final_url: String,
}

pub async fn send(spec: HttpRequestSpec) -> Result<HttpResponseInfo, String> {
    let method = Method::from_bytes(spec.method.trim().to_uppercase().as_bytes())
        .map_err(|error| format!("非法 HTTP 方法：{error}"))?;
    let timeout = Duration::from_secs(spec.timeout_seconds.unwrap_or(30).clamp(1, 300) as u64);

    let redirect = if spec.follow_redirects {
        reqwest::redirect::Policy::limited(10)
    } else {
        reqwest::redirect::Policy::none()
    };

    let client = reqwest::Client::builder()
        .timeout(timeout)
        .redirect(redirect)
        .build()
        .map_err(|error| format!("构造 HTTP 客户端失败：{error}"))?;

    // Base URL with additional query params
    let mut url = reqwest::Url::parse(spec.url.trim())
        .map_err(|error| format!("URL 非法：{error}"))?;
    for p in &spec.query_params {
        if !p.enabled || p.key.trim().is_empty() {
            continue;
        }
        url.query_pairs_mut().append_pair(&p.key, &p.value);
    }

    let mut headers = HeaderMap::new();
    for h in &spec.headers {
        if !h.enabled || h.key.trim().is_empty() {
            continue;
        }
        let name = HeaderName::from_str(&h.key)
            .map_err(|error| format!("非法 header 名 {}：{error}", h.key))?;
        let value = HeaderValue::from_str(&h.value)
            .map_err(|error| format!("非法 header 值 {}：{error}", h.value))?;
        headers.insert(name, value);
    }

    let mut request = client.request(method.clone(), url).headers(headers);

    match spec.body_type.trim() {
        "" | "none" => {}
        "json" => {
            request = request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(spec.body.clone());
        }
        "form" => {
            // Re-encode "k=v&k=v" so keys/values are properly percent-encoded.
            let mut ser = url::form_urlencoded::Serializer::new(String::new());
            for (k, v) in url::form_urlencoded::parse(spec.body.as_bytes()).into_owned() {
                ser.append_pair(&k, &v);
            }
            request = request
                .header(
                    reqwest::header::CONTENT_TYPE,
                    "application/x-www-form-urlencoded",
                )
                .body(ser.finish());
        }
        "text" | _ => {
            request = request.body(spec.body.clone());
        }
    }

    let start = Instant::now();
    let response = request
        .send()
        .await
        .map_err(|error| format!("请求失败：{error}"))?;
    let final_url = response.url().to_string();
    let status = response.status();
    let status_text = status.canonical_reason().unwrap_or("").to_string();
    let response_headers: Vec<(String, String)> = response
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("<binary>").to_string()))
        .collect();
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("读取响应体失败：{error}"))?;
    let elapsed = start.elapsed().as_millis();
    let body_bytes = bytes.len() as u64;
    let body = String::from_utf8_lossy(&bytes).into_owned();

    Ok(HttpResponseInfo {
        status: status.as_u16(),
        status_text,
        headers: response_headers,
        body,
        body_bytes,
        elapsed_ms: elapsed,
        final_url,
    })
}
