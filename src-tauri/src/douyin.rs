use percent_encoding::percent_decode_str;
use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use std::sync::OnceLock;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DouyinVideoInfo {
    pub mp4_url: String,
    pub title: String,
    pub has_watermark: bool,
}

fn render_data_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"<script id="RENDER_DATA" type="application/json">([^<]+)</script>"#,
        )
        .expect("RENDER_DATA regex")
    })
}

fn mp4_fallback_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"https?://[^\s"'\\]+?\.mp4[^\s"'\\]*"#).expect("mp4 regex"))
}

fn video_id_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"/video/(\d+)").expect("video id regex"))
}

pub fn extract_video_id(url: &str) -> Option<String> {
    video_id_re()
        .captures(url)
        .map(|cap| cap[1].to_string())
}

/// Extract the JSON payload from an iesdouyin share page. Handles two shapes:
///   1. Modern (2025+): `window._ROUTER_DATA = { ... };` (Modern.js SSR)
///   2. Legacy: `<script id="RENDER_DATA" type="application/json">%7B...%7D</script>`
pub fn extract_payload_json(html: &str) -> Result<String, String> {
    if let Some(json) = extract_router_data(html) {
        return Ok(json);
    }
    if let Some(cap) = render_data_re().captures(html) {
        let decoded = percent_decode_str(&cap[1])
            .decode_utf8()
            .map_err(|error| format!("响应内容解码失败：{error}"))?
            .into_owned();
        return Ok(decoded);
    }
    Err("页面结构不识别（可能触发风控）".to_string())
}

fn extract_router_data(html: &str) -> Option<String> {
    let anchor = html.find("window._ROUTER_DATA")?;
    let after_anchor = &html[anchor..];
    let eq_pos = after_anchor.find('=')?;
    let mut rest = after_anchor[eq_pos + 1..].trim_start();
    // 有的实现前面还有可选 `!`，保守起见跳过任何非 `{` 前缀（例如 spaces / `!` / `!function...`）
    while !rest.starts_with('{') {
        let (_, tail) = rest.split_at(rest.char_indices().nth(1)?.0);
        rest = tail.trim_start();
        if rest.is_empty() {
            return None;
        }
    }
    let json_str = take_balanced_object(rest)?;
    Some(json_str.to_string())
}

fn take_balanced_object(input: &str) -> Option<&str> {
    let bytes = input.as_bytes();
    if bytes.first().copied()? != b'{' {
        return None;
    }
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (i, &b) in bytes.iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&input[..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn parse_render_data(json_text: &str) -> Result<Value, String> {
    serde_json::from_str::<Value>(json_text)
        .map_err(|error| format!("响应数据解析失败：{error}"))
}

/// Find a mp4 URL inside a parsed payload. Walks known 2025 Modern.js paths first,
/// then legacy paths, then falls back to a regex over the JSON dump.
pub fn find_mp4_url(root: &Value) -> Option<String> {
    // 2025 Modern.js SSR: loaderData.<page-route-key>.videoInfoRes.item_list[0].video.play_addr.url_list[0]
    if let Some(loader) = root.get("loaderData").and_then(|v| v.as_object()) {
        for (_route_key, page) in loader {
            if let Some(url) = extract_from_item_list(page) {
                return Some(url);
            }
        }
    }
    // 2025 fallback: 直接在 root 上尝试 videoInfoRes.item_list（若结构升级）
    if let Some(url) = extract_from_item_list(root) {
        return Some(url);
    }
    // 老结构 A: app.videoDetail
    if let Some(url) = pick_string_by_path(root, &["app", "videoDetail", "video", "playApi"]) {
        return Some(url);
    }
    if let Some(url) = pick_first_string_in_array(
        root,
        &["app", "videoDetail", "video", "playAddr"],
        "src",
    ) {
        return Some(url);
    }
    // 老结构 B: aweme_detail
    if let Some(url) =
        pick_string_by_path(root, &["aweme_detail", "video", "play_addr", "url_list", "0"])
    {
        return Some(url);
    }
    // 兜底：正则从 JSON dump 里抓第一个 .mp4
    let dumped = root.to_string();
    mp4_fallback_re().find(&dumped).map(|m| m.as_str().to_string())
}

fn extract_from_item_list(node: &Value) -> Option<String> {
    let item_list = node
        .get("videoInfoRes")?
        .get("item_list")
        .and_then(|v| v.as_array())?;
    for item in item_list {
        if let Some(url) = item
            .get("video")
            .and_then(|v| v.get("play_addr"))
            .and_then(|v| v.get("url_list"))
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
        {
            return Some(url.to_string());
        }
    }
    None
}

pub fn find_title(root: &Value, video_id: &str) -> String {
    // 2025 Modern.js
    if let Some(loader) = root.get("loaderData").and_then(|v| v.as_object()) {
        for (_k, page) in loader {
            if let Some(desc) = extract_desc_from_item_list(page) {
                let trimmed = desc.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }
    }
    if let Some(desc) = extract_desc_from_item_list(root) {
        let trimmed = desc.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    // 老结构
    for path in [
        &["app", "videoDetail", "desc"][..],
        &["aweme_detail", "desc"][..],
    ] {
        if let Some(s) = pick_string_by_path(root, path) {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    format!("douyin_{video_id}")
}

fn extract_desc_from_item_list(node: &Value) -> Option<String> {
    let item_list = node
        .get("videoInfoRes")?
        .get("item_list")
        .and_then(|v| v.as_array())?;
    for item in item_list {
        if let Some(desc) = item.get("desc").and_then(|v| v.as_str()) {
            if !desc.trim().is_empty() {
                return Some(desc.to_string());
            }
        }
    }
    None
}

pub fn derive_watermark_removed_url(url: &str) -> (String, bool) {
    if url.contains("playwm") {
        let removed = url.replace("playwm", "play");
        (removed, false)
    } else if url.contains("/play/") || url.contains("/play?") {
        (url.to_string(), false)
    } else {
        (url.to_string(), true)
    }
}

fn pick_string_by_path(root: &Value, path: &[&str]) -> Option<String> {
    let mut current = root;
    for seg in path {
        current = match seg.parse::<usize>() {
            Ok(idx) => current.get(idx)?,
            Err(_) => current.get(*seg)?,
        };
    }
    current.as_str().map(|s| s.to_string())
}

fn pick_first_string_in_array(root: &Value, path_to_array: &[&str], field: &str) -> Option<String> {
    let mut current = root;
    for seg in path_to_array {
        current = current.get(*seg)?;
    }
    let array = current.as_array()?;
    for item in array {
        if let Some(v) = item.get(field).and_then(|f| f.as_str()) {
            return Some(v.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_video_id_from_canonical_url() {
        assert_eq!(
            extract_video_id("https://www.douyin.com/video/7650383172730932515"),
            Some("7650383172730932515".to_string())
        );
    }

    #[test]
    fn returns_none_when_no_video_id_present() {
        assert!(extract_video_id("https://www.douyin.com/user/xxx").is_none());
    }

    #[test]
    fn extracts_payload_json_from_router_data_modern_shape() {
        let html = r#"<html><body><script>
window._ROUTER_DATA = {"loaderData":{"video_(id)/page":{"videoInfoRes":{"item_list":[{"video":{"play_addr":{"url_list":["https://aweme.snssdk.com/aweme/v1/playwm/?video_id=abc"]}},"desc":"hello"}]}}}};
</script></body></html>"#;
        let json = extract_payload_json(html).unwrap();
        assert!(json.starts_with('{'));
        let parsed = parse_render_data(&json).unwrap();
        assert_eq!(
            find_mp4_url(&parsed),
            Some("https://aweme.snssdk.com/aweme/v1/playwm/?video_id=abc".to_string())
        );
        assert_eq!(find_title(&parsed, "1"), "hello");
    }

    #[test]
    fn extracts_payload_json_from_legacy_render_data_percent_encoded() {
        let html = r#"<script id="RENDER_DATA" type="application/json">%7B%22a%22%3A1%7D</script>"#;
        let raw = extract_payload_json(html).unwrap();
        assert_eq!(raw, r#"{"a":1}"#);
    }

    #[test]
    fn missing_payload_reports_captcha_like_error() {
        let html = "<html><body>slider captcha here</body></html>";
        let err = extract_payload_json(html).unwrap_err();
        assert!(err.contains("页面结构"));
    }

    #[test]
    fn take_balanced_object_handles_nested_and_strings() {
        let s = r#"{"a":{"b":"}"},"c":1}trailing;"#;
        assert_eq!(take_balanced_object(s), Some(r#"{"a":{"b":"}"},"c":1}"#));
    }

    #[test]
    fn take_balanced_object_handles_escaped_quotes_in_string() {
        let s = r#"{"a":"\"nested\"","b":2}garbage"#;
        assert_eq!(
            take_balanced_object(s),
            Some(r#"{"a":"\"nested\"","b":2}"#)
        );
    }

    #[test]
    fn finds_mp4_url_via_modern_router_data_item_list() {
        let root = json!({
            "loaderData": {
                "video_(id)/page": {
                    "videoInfoRes": {
                        "item_list": [{
                            "video": {
                                "play_addr": {
                                    "url_list": [
                                        "https://aweme.snssdk.com/aweme/v1/playwm/?video_id=xxx"
                                    ]
                                }
                            }
                        }]
                    }
                }
            }
        });
        assert_eq!(
            find_mp4_url(&root),
            Some(
                "https://aweme.snssdk.com/aweme/v1/playwm/?video_id=xxx".to_string()
            )
        );
    }

    #[test]
    fn finds_mp4_url_via_legacy_play_api_path() {
        let root = json!({
            "app": {
                "videoDetail": {
                    "video": {
                        "playApi": "https://aweme.snssdk.com/play/xxx.mp4"
                    }
                }
            }
        });
        assert_eq!(
            find_mp4_url(&root),
            Some("https://aweme.snssdk.com/play/xxx.mp4".to_string())
        );
    }

    #[test]
    fn finds_mp4_url_via_legacy_play_addr_url_list_path() {
        let root = json!({
            "aweme_detail": {
                "video": {
                    "play_addr": {
                        "url_list": ["https://cdn.douyin.com/play/xxx.mp4"]
                    }
                }
            }
        });
        assert_eq!(
            find_mp4_url(&root),
            Some("https://cdn.douyin.com/play/xxx.mp4".to_string())
        );
    }

    #[test]
    fn falls_back_to_mp4_regex_when_no_known_path_matches() {
        let root = json!({
            "unknown": {
                "structure": "https://example.com/playwm/zzz.mp4?token=abc"
            }
        });
        assert_eq!(
            find_mp4_url(&root),
            Some("https://example.com/playwm/zzz.mp4?token=abc".to_string())
        );
    }

    #[test]
    fn returns_none_when_no_mp4_anywhere() {
        let root = json!({
            "loaderData": { "video_(id)/page": null }
        });
        assert!(find_mp4_url(&root).is_none());
    }

    #[test]
    fn finds_title_via_modern_router_data_item_list_desc() {
        let root = json!({
            "loaderData": {
                "video_(id)/page": {
                    "videoInfoRes": {
                        "item_list": [{"desc": "  新版描述  "}]
                    }
                }
            }
        });
        assert_eq!(find_title(&root, "999"), "新版描述");
    }

    #[test]
    fn finds_title_from_legacy_app_video_detail_desc() {
        let root = json!({
            "app": {
                "videoDetail": {
                    "desc": "  老结构标题  "
                }
            }
        });
        assert_eq!(find_title(&root, "123"), "老结构标题");
    }

    #[test]
    fn finds_title_from_legacy_aweme_detail_desc() {
        let root = json!({
            "aweme_detail": { "desc": "aweme 描述" }
        });
        assert_eq!(find_title(&root, "123"), "aweme 描述");
    }

    #[test]
    fn title_falls_back_when_desc_empty_or_missing() {
        assert_eq!(find_title(&json!({}), "999"), "douyin_999");
        assert_eq!(
            find_title(
                &json!({"app": {"videoDetail": {"desc": "   "}}}),
                "42"
            ),
            "douyin_42"
        );
    }

    #[test]
    fn derives_watermark_removed_url_via_playwm_swap() {
        let (url, has_wm) =
            derive_watermark_removed_url("https://aweme.snssdk.com/aweme/v1/playwm/?video_id=abc");
        assert_eq!(url, "https://aweme.snssdk.com/aweme/v1/play/?video_id=abc");
        assert!(!has_wm);
    }

    #[test]
    fn keeps_url_when_already_no_watermark_path() {
        let (url, has_wm) =
            derive_watermark_removed_url("https://cdn.douyin.com/play/abc.mp4");
        assert_eq!(url, "https://cdn.douyin.com/play/abc.mp4");
        assert!(!has_wm);
    }

    #[test]
    fn marks_watermark_for_unrecognizable_url() {
        let (url, has_wm) = derive_watermark_removed_url("https://example.com/video.mp4");
        assert_eq!(url, "https://example.com/video.mp4");
        assert!(has_wm);
    }
}
