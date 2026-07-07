use serde::Serialize;
use std::time::Duration;

const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
    AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BiliVideoInfo {
    pub title: String,
    pub cover: Option<String>,
    pub uploader: Option<String>,
    pub bvid: String,
    pub duration: Option<i64>,
    pub video_url: Option<String>,
    pub audio_url: Option<String>,
    pub quality_note: Option<String>,
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
    let client = build_client()?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("请求失败：{error}"))?;
    Ok(response.url().to_string())
}

pub fn extract_bvid(url: &str) -> Option<String> {
    // Pattern: /video/BVxxxxxxxxxx  (BV + 10 chars alphanumeric)
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"/(BV[0-9A-Za-z]{10})").unwrap());
    re.captures(url)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

pub async fn extract_video(url: &str) -> Result<BiliVideoInfo, String> {
    let resolved = resolve_short_url(url).await?;
    let bvid = extract_bvid(&resolved).ok_or_else(|| "未能从 URL 提取 BVID".to_string())?;

    let client = build_client()?;

    // Basic info via /x/web-interface/view
    let view_url = format!("https://api.bilibili.com/x/web-interface/view?bvid={bvid}");
    let view_json: serde_json::Value = client
        .get(&view_url)
        .header("Referer", "https://www.bilibili.com/")
        .send()
        .await
        .map_err(|error| format!("请求 view API 失败：{error}"))?
        .json()
        .await
        .map_err(|error| format!("解析 view JSON 失败：{error}"))?;

    let code = view_json.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
    if code != 0 {
        let msg = view_json
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("未知错误");
        return Err(format!("B 站 view API 返回：{msg}（可能笔记不存在或需登录）"));
    }

    let data = view_json
        .get("data")
        .ok_or_else(|| "view API 无 data 字段".to_string())?;
    let title = data
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let cover = data.get("pic").and_then(|v| v.as_str()).map(str::to_string);
    let uploader = data
        .get("owner")
        .and_then(|o| o.get("name"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let duration = data.get("duration").and_then(|v| v.as_i64());
    let cid = data.get("cid").and_then(|v| v.as_i64());

    let mut video_url: Option<String> = None;
    let mut audio_url: Option<String> = None;
    let mut quality_note: Option<String> = None;
    if let Some(cid_v) = cid {
        // Try playurl without login (limited to lower qualities)
        // qn=80 = 1080P (needs login), qn=64 = 720P, qn=32 = 480P, qn=16 = 360P
        let playurl = format!(
            "https://api.bilibili.com/x/player/playurl?bvid={bvid}&cid={cid_v}&qn=64&fnval=16&fnver=0&fourk=0"
        );
        if let Ok(resp) = client
            .get(&playurl)
            .header("Referer", "https://www.bilibili.com/")
            .send()
            .await
        {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                if json.get("code").and_then(|v| v.as_i64()) == Some(0) {
                    let data = json.get("data");
                    // DASH format: data.dash.video[0].baseUrl + data.dash.audio[0].baseUrl
                    if let Some(dash) = data.and_then(|d| d.get("dash")) {
                        if let Some(v) = dash
                            .get("video")
                            .and_then(|a| a.get(0))
                            .and_then(|e| e.get("baseUrl").or_else(|| e.get("base_url")))
                            .and_then(|v| v.as_str())
                        {
                            video_url = Some(v.to_string());
                        }
                        if let Some(a) = dash
                            .get("audio")
                            .and_then(|a| a.get(0))
                            .and_then(|e| e.get("baseUrl").or_else(|| e.get("base_url")))
                            .and_then(|v| v.as_str())
                        {
                            audio_url = Some(a.to_string());
                        }
                        quality_note = Some(
                            "DASH 分离流：视频与音频分开下载，需自行合流（ffmpeg -c copy）"
                                .to_string(),
                        );
                    } else if let Some(durl) = data.and_then(|d| d.get("durl")) {
                        if let Some(v) = durl
                            .as_array()
                            .and_then(|arr| arr.get(0))
                            .and_then(|entry| entry.get("url"))
                            .and_then(|v| v.as_str())
                        {
                            video_url = Some(v.to_string());
                            quality_note = Some("FLV 单流（无需合流）".to_string());
                        }
                    }
                }
            }
        }
    }

    if video_url.is_none() {
        quality_note = Some(
            "未能获取播放地址，可能需要登录或视频受限。可用 复制页面 + 浏览器打开".to_string(),
        );
    }

    Ok(BiliVideoInfo {
        title,
        cover,
        uploader,
        bvid,
        duration,
        video_url,
        audio_url,
        quality_note,
    })
}
