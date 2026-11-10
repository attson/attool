use percent_encoding::percent_decode_str;
use tauri::http::{header, Method, Request, Response, StatusCode};

use super::commands::{PluginCommandError, PluginRuntime};

const PLUGIN_CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; media-src 'self'; connect-src 'none'; object-src 'none'; frame-ancestors tauri://localhost http://tauri.localhost https://tauri.localhost http://127.0.0.1:1420; base-uri 'none'; form-action 'none'";

#[derive(Debug, PartialEq, Eq)]
struct PluginUri {
    app_id: String,
    relative_path: String,
}

pub fn handle_plugin_request(
    runtime: &PluginRuntime,
    request: Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    if request.method() != Method::GET && request.method() != Method::HEAD {
        return error_response(
            StatusCode::METHOD_NOT_ALLOWED,
            "PLUGIN_ASSET_METHOD_NOT_ALLOWED",
        );
    }
    let parsed = match parse_plugin_uri(&request.uri().to_string()) {
        Ok(parsed) => parsed,
        Err(code) => return error_response(StatusCode::BAD_REQUEST, code),
    };
    match runtime.read_asset(&parsed.app_id, &parsed.relative_path) {
        Ok(asset) => {
            let body = if request.method() == Method::HEAD {
                Vec::new()
            } else {
                asset.bytes
            };
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, asset.content_type)
                .header(header::CONTENT_SECURITY_POLICY, PLUGIN_CSP)
                .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
                .header(header::CACHE_CONTROL, "no-store")
                .body(body)
                .expect("valid plugin asset response")
        }
        Err(error) => error_response(status_for_error(&error), &error.code),
    }
}

pub fn unavailable_response() -> Response<Vec<u8>> {
    error_response(
        StatusCode::SERVICE_UNAVAILABLE,
        "PLUGIN_RUNTIME_UNAVAILABLE",
    )
}

fn parse_plugin_uri(value: &str) -> Result<PluginUri, &'static str> {
    let rest = value
        .strip_prefix("attool-plugin://")
        .ok_or("PLUGIN_ASSET_URL_INVALID")?;
    let without_fragment = rest.split('#').next().unwrap_or(rest);
    let without_query = without_fragment
        .split('?')
        .next()
        .unwrap_or(without_fragment);
    let (app_id, raw_path) = without_query
        .split_once('/')
        .ok_or("PLUGIN_ASSET_PATH_INVALID")?;
    if !is_valid_app_id(app_id) {
        return Err("PLUGIN_ASSET_APP_ID_INVALID");
    }
    let relative_path = percent_decode_str(raw_path)
        .decode_utf8()
        .map_err(|_| "PLUGIN_ASSET_PATH_INVALID")?
        .into_owned();
    if !is_safe_relative_path(&relative_path) {
        return Err("PLUGIN_ASSET_PATH_INVALID");
    }
    Ok(PluginUri {
        app_id: app_id.to_string(),
        relative_path,
    })
}

fn is_valid_app_id(value: &str) -> bool {
    let mut has_separator = false;
    for segment in value.split(['.', '-']) {
        if segment.is_empty()
            || !segment
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        {
            return false;
        }
        has_separator = true;
    }
    has_separator && (value.contains('.') || value.contains('-'))
}

fn is_safe_relative_path(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('/')
        && !value.contains('\\')
        && value
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

fn status_for_error(error: &PluginCommandError) -> StatusCode {
    match error.code.as_str() {
        "PLUGIN_DISABLED" => StatusCode::FORBIDDEN,
        "PLUGIN_NOT_FOUND" | "PLUGIN_ASSET_NOT_FOUND" | "PLUGIN_ASSET_NOT_DECLARED" => {
            StatusCode::NOT_FOUND
        }
        "PLUGIN_ASSET_PATH_INVALID" => StatusCode::BAD_REQUEST,
        "PLUGIN_ASSET_HASH_MISMATCH" => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn error_response(status: StatusCode, code: &str) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::CONTENT_SECURITY_POLICY, "default-src 'none'")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(header::CACHE_CONTROL, "no-store")
        .body(code.as_bytes().to_vec())
        .expect("valid plugin error response")
}

#[cfg(test)]
mod tests {
    use super::parse_plugin_uri;

    #[test]
    fn parses_per_application_origins_and_rejects_traversal() {
        let parsed =
            parse_plugin_uri("attool-plugin://com.attson.codec/ui/index.html?revision=1").unwrap();
        assert_eq!(parsed.app_id, "com.attson.codec");
        assert_eq!(parsed.relative_path, "ui/index.html");

        assert!(parse_plugin_uri("attool-plugin://com.attson.codec/ui/%2e%2e/secret").is_err());
        assert!(parse_plugin_uri("attool-plugin://../ui/index.html").is_err());
        assert!(parse_plugin_uri("https://com.attson.codec/ui/index.html").is_err());
    }
}
