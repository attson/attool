use serde::Serialize;
use std::time::Duration;

const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
    AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XhsNoteInfo {
    pub title: String,
    pub note_type: String, // "video" | "images" | "unknown"
    pub cover: Option<String>,
    pub video_url: Option<String>,
    pub image_urls: Vec<String>,
}

fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(UA)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|error| format!("构造 HTTP 客户端失败：{error}"))
}

pub async fn resolve_short_url(url: &str) -> Result<String, String> {
    if !(url.starts_with("https://xhslink.com/")
        || url.starts_with("http://xhslink.com/")
        || url.starts_with("https://www.xiaohongshu.com/")
        || url.starts_with("http://www.xiaohongshu.com/"))
    {
        return Err("仅支持 xhslink.com 短链或 xiaohongshu.com 长链".to_string());
    }
    let client = build_client()?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("请求失败：{error}"))?;
    Ok(response.url().to_string())
}

pub async fn extract_note(url: &str) -> Result<XhsNoteInfo, String> {
    let resolved = resolve_short_url(url).await?;
    let client = build_client()?;
    let response = client
        .get(&resolved)
        .header("Accept-Language", "zh-CN,zh;q=0.9")
        .send()
        .await
        .map_err(|error| format!("请求笔记页失败：{error}"))?;
    if !response.status().is_success() {
        return Err(format!("笔记页 HTTP {}", response.status()));
    }
    let body = response
        .text()
        .await
        .map_err(|error| format!("读取笔记页失败：{error}"))?;

    let payload = extract_initial_state(&body)?;
    parse_note(&payload)
}

/// Extract the window.__INITIAL_STATE__=... JSON assigned in <script>.
fn extract_initial_state(html: &str) -> Result<serde_json::Value, String> {
    // The typical pattern is: window.__INITIAL_STATE__={...JSON...}</script>
    let marker = "window.__INITIAL_STATE__=";
    let start = html
        .find(marker)
        .ok_or_else(|| "未在页面中找到 __INITIAL_STATE__，可能页面需要登录或反爬".to_string())?
        + marker.len();

    // Scan JSON braces to find the closing brace of the top-level object.
    let bytes = html.as_bytes();
    if start >= bytes.len() || bytes[start] != b'{' {
        return Err("__INITIAL_STATE__ 不是 JSON 对象开头".to_string());
    }
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;
    let mut end = start;
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if escape {
            escape = false;
            continue;
        }
        if b == b'\\' && in_string {
            escape = true;
            continue;
        }
        if b == b'"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }
        if b == b'{' {
            depth += 1;
        } else if b == b'}' {
            depth -= 1;
            if depth == 0 {
                end = i + 1;
                break;
            }
        }
    }
    if end == start {
        return Err("未找到 __INITIAL_STATE__ 的 JSON 结束位置".to_string());
    }

    let raw = &html[start..end];
    // xhs 有时用 `undefined`，替换为 null 便于 serde_json 解析
    let cleaned = raw.replace(":undefined", ":null");
    serde_json::from_str(&cleaned).map_err(|error| format!("解析 __INITIAL_STATE__ JSON 失败：{error}"))
}

fn parse_note(root: &serde_json::Value) -> Result<XhsNoteInfo, String> {
    // Typical path: root.note.noteDetailMap.<id>.note
    let note = root
        .get("note")
        .and_then(|v| v.get("noteDetailMap"))
        .and_then(|map| {
            if let Some(obj) = map.as_object() {
                obj.values().find_map(|entry| entry.get("note"))
            } else {
                None
            }
        })
        .ok_or_else(|| "笔记数据结构不匹配，可能是登录墙或改版".to_string())?;

    let title = note
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let note_type = note
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let cover = note
        .get("imageList")
        .and_then(|arr| arr.get(0))
        .and_then(|img| img.get("urlDefault").or_else(|| img.get("url")))
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let mut video_url = None;
    if note_type == "video" {
        if let Some(v) = note
            .get("video")
            .and_then(|v| v.get("media"))
            .and_then(|m| m.get("stream"))
            .and_then(|s| s.get("h264"))
            .and_then(|arr| arr.get(0))
            .and_then(|entry| entry.get("masterUrl").or_else(|| entry.get("backupUrls").and_then(|a| a.get(0))))
            .and_then(|v| v.as_str())
        {
            video_url = Some(v.to_string());
        }
    }

    let mut image_urls: Vec<String> = Vec::new();
    if note_type == "normal" || note_type == "images" || video_url.is_none() {
        if let Some(arr) = note.get("imageList").and_then(|v| v.as_array()) {
            for img in arr {
                if let Some(u) = img
                    .get("urlDefault")
                    .or_else(|| img.get("url"))
                    .and_then(|v| v.as_str())
                {
                    image_urls.push(u.to_string());
                }
            }
        }
    }

    Ok(XhsNoteInfo {
        title,
        note_type,
        cover,
        video_url,
        image_urls,
    })
}
