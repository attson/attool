use std::path::Path;
use std::str::FromStr;
use std::time::{Duration, Instant};

use base64::Engine;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::multipart;
use reqwest::Method;

use super::cancel::HttpCancelState;
use super::models::{HttpRequestSpec, HttpResponseInfo};

pub async fn send(
    spec: HttpRequestSpec,
    cancel_token_id: Option<String>,
    cancel_state: Option<&HttpCancelState>,
) -> Result<HttpResponseInfo, String> {
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
        .danger_accept_invalid_certs(!spec.verify_ssl)
        .build()
        .map_err(|error| format!("构造 HTTP 客户端失败：{error}"))?;

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

    // Auth 优先 headers.insert（如果用户已经手填 Authorization，spec.auth 覆盖）
    match spec.auth.r#type.as_str() {
        "bearer" if !spec.auth.bearer_token.is_empty() => {
            if let Ok(value) =
                HeaderValue::from_str(&format!("Bearer {}", spec.auth.bearer_token))
            {
                headers.insert(reqwest::header::AUTHORIZATION, value);
            }
        }
        "basic" => {
            let raw = format!("{}:{}", spec.auth.basic_user, spec.auth.basic_pass);
            let encoded = base64::engine::general_purpose::STANDARD.encode(raw.as_bytes());
            if let Ok(value) = HeaderValue::from_str(&format!("Basic {encoded}")) {
                headers.insert(reqwest::header::AUTHORIZATION, value);
            }
        }
        _ => {}
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
        "multipart" => {
            let mut form = multipart::Form::new();
            for field in &spec.multipart_fields {
                if !field.enabled || field.key.trim().is_empty() {
                    continue;
                }
                match field.kind.as_str() {
                    "file" => {
                        let path = Path::new(&field.value);
                        let filename = path
                            .file_name()
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_else(|| field.key.clone());
                        let bytes = tokio::fs::read(path).await.map_err(|error| {
                            format!("读取文件 {} 失败：{error}", field.value)
                        })?;
                        let part = multipart::Part::bytes(bytes).file_name(filename);
                        form = form.part(field.key.clone(), part);
                    }
                    _ => {
                        form = form.text(field.key.clone(), field.value.clone());
                    }
                }
            }
            request = request.multipart(form);
        }
        _ => {
            request = request.body(spec.body.clone());
        }
    }

    let start = Instant::now();
    let cancel_rx = cancel_token_id
        .as_ref()
        .and_then(|id| cancel_state.map(|state| (id.clone(), state.register(id.clone()))));

    let response_result = if let Some((_, rx)) = cancel_rx {
        let cancel_id = cancel_token_id.clone();
        tokio::select! {
            r = request.send() => {
                if let (Some(id), Some(state)) = (cancel_id.as_ref(), cancel_state) {
                    state.unregister(id);
                }
                r
            }
            _ = rx => {
                if let (Some(id), Some(state)) = (cancel_id.as_ref(), cancel_state) {
                    state.unregister(id);
                }
                return Err("请求已取消".to_string());
            }
        }
    } else {
        request.send().await
    };

    let response = response_result.map_err(|error| format!("请求失败：{error}"))?;
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
