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

pub fn extract_render_data_from_html(html: &str) -> Result<String, String> {
    let cap = render_data_re()
        .captures(html)
        .ok_or_else(|| "页面结构不识别（可能触发风控）".to_string())?;
    let encoded = &cap[1];
    let decoded = percent_decode_str(encoded)
        .decode_utf8()
        .map_err(|error| format!("响应内容解码失败：{error}"))?
        .into_owned();
    Ok(decoded)
}

pub fn parse_render_data(json_text: &str) -> Result<Value, String> {
    serde_json::from_str::<Value>(json_text)
        .map_err(|error| format!("响应数据解析失败：{error}"))
}

pub fn find_mp4_url(root: &Value) -> Option<String> {
    let candidates = [
        &["app", "videoDetail", "video", "playApi"][..],
        &["aweme_detail", "video", "play_addr", "url_list", "0"][..],
    ];
    for path in candidates {
        if let Some(url) = pick_string_by_path(root, path) {
            return Some(url);
        }
    }
    if let Some(url) = pick_first_string_in_array(root, &["app", "videoDetail", "video", "playAddr"], "src") {
        return Some(url);
    }
    let dumped = root.to_string();
    if let Some(m) = mp4_fallback_re().find(&dumped) {
        return Some(m.as_str().to_string());
    }
    None
}

pub fn find_title(root: &Value, video_id: &str) -> String {
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

pub fn derive_watermark_removed_url(url: &str) -> (String, bool) {
    if url.contains("playwm") {
        let removed = url.replace("playwm", "play");
        (removed, false)
    } else if url.contains("/play/") {
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
    fn extracts_render_data_from_normal_html() {
        let html = r#"<html><head></head><body>
<script id="RENDER_DATA" type="application/json">%7B%22a%22%3A1%7D</script>
</body></html>"#;
        let raw = extract_render_data_from_html(html).unwrap();
        assert_eq!(raw, r#"{"a":1}"#);
    }

    #[test]
    fn extracts_render_data_decodes_utf8() {
        let html = r#"<script id="RENDER_DATA" type="application/json">%7B%22desc%22%3A%22%E6%B5%8B%E8%AF%95%22%7D</script>"#;
        let raw = extract_render_data_from_html(html).unwrap();
        assert_eq!(raw, r#"{"desc":"测试"}"#);
    }

    #[test]
    fn missing_render_data_reports_captcha_like_error() {
        let html = "<html><body>slider captcha here</body></html>";
        let err = extract_render_data_from_html(html).unwrap_err();
        assert!(err.contains("页面结构"));
    }

    #[test]
    fn finds_mp4_url_via_play_api_path() {
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
    fn finds_mp4_url_via_play_addr_url_list_path() {
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
    fn finds_mp4_url_via_play_addr_array_src_field() {
        let root = json!({
            "app": {
                "videoDetail": {
                    "video": {
                        "playAddr": [
                            {"src": "https://cdn.douyin.com/play/aaa.mp4"}
                        ]
                    }
                }
            }
        });
        assert_eq!(
            find_mp4_url(&root),
            Some("https://cdn.douyin.com/play/aaa.mp4".to_string())
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
            "app": {
                "videoDetail": null
            }
        });
        assert!(find_mp4_url(&root).is_none());
    }

    #[test]
    fn finds_title_from_app_video_detail_desc() {
        let root = json!({
            "app": {
                "videoDetail": {
                    "desc": "  测试标题  "
                }
            }
        });
        assert_eq!(find_title(&root, "123"), "测试标题");
    }

    #[test]
    fn finds_title_from_aweme_detail_desc() {
        let root = json!({
            "aweme_detail": { "desc": "老结构标题" }
        });
        assert_eq!(find_title(&root, "123"), "老结构标题");
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
            derive_watermark_removed_url("https://cdn.douyin.com/playwm/abc.mp4?x=1");
        assert_eq!(url, "https://cdn.douyin.com/play/abc.mp4?x=1");
        assert!(!has_wm);
    }

    #[test]
    fn keeps_url_when_already_no_watermark() {
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
