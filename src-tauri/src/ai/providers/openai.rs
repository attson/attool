// Task 3: OpenAI-compatible provider.

use futures_util::StreamExt;
use tokio::sync::{mpsc, oneshot};

use super::{ChatDelta, ChatFinal, ChatRequest, ContentPart, ProviderError, ProviderModelInfo};

pub struct OpenAiProvider {
    pub base_url: String,
    pub api_key: String,
    pub client: reqwest::Client,
}

impl OpenAiProvider {
    pub async fn stream_chat(
        &self,
        req: ChatRequest,
        delta_tx: mpsc::UnboundedSender<ChatDelta>,
        mut cancel_rx: oneshot::Receiver<()>,
    ) -> Result<ChatFinal, ProviderError> {
        // 构造 messages(含 system + 附件转 image_url data URL)
        let mut msgs = Vec::new();
        if let Some(sp) = &req.system_prompt {
            if !sp.is_empty() {
                msgs.push(serde_json::json!({"role": "system", "content": sp}));
            }
        }
        for m in &req.messages {
            let parts: Vec<serde_json::Value> = m
                .content
                .iter()
                .map(|p| match p {
                    ContentPart::Text(t) => serde_json::json!({"type":"text","text":t}),
                    ContentPart::Image { path, mime } => {
                        let data = crate::ai::providers::read_and_encode_image(path, mime)
                            .unwrap_or_else(|_| String::new());
                        serde_json::json!({"type":"image_url","image_url":{"url":data}})
                    }
                })
                .collect();
            // 若只有一段 text 就退回字符串形式(老 OpenAI 模型不支持数组)
            let content = if parts.len() == 1 && parts[0]["type"] == "text" {
                parts[0]["text"].clone()
            } else {
                serde_json::Value::Array(parts)
            };
            msgs.push(serde_json::json!({"role": m.role, "content": content}));
        }

        let mut body = serde_json::json!({
            "model": req.model_id,
            "messages": msgs,
            "stream": true,
            "stream_options": {"include_usage": true},
        });
        if let Some(t) = req.temperature {
            body["temperature"] = t.into();
        }
        if let Some(mt) = req.max_tokens {
            body["max_tokens"] = mt.into();
        }

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .header("accept", "text/event-stream")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default().chars().take(4096).collect();
            return Err(ProviderError::Http { status, body: text });
        }

        let mut stream = resp.bytes_stream();
        let mut buf = String::new();
        let mut fin = ChatFinal::default();
        loop {
            tokio::select! {
                _ = &mut cancel_rx => return Ok(fin),
                chunk = stream.next() => {
                    let Some(chunk) = chunk else { break };
                    let bytes = chunk.map_err(|e| ProviderError::Network(e.to_string()))?;
                    buf.push_str(&String::from_utf8_lossy(&bytes));
                    while let Some(pos) = buf.find("\n\n") {
                        let event: String = buf.drain(..pos + 2).collect();
                        for line in event.lines() {
                            match parse_sse_frame(line) {
                                ParsedOpenAiFrame::Text(t) => { let _ = delta_tx.send(ChatDelta::Text(t)); }
                                ParsedOpenAiFrame::Usage { prompt, completion } => {
                                    fin.prompt_tokens = Some(prompt);
                                    fin.completion_tokens = Some(completion);
                                }
                                ParsedOpenAiFrame::Done => return Ok(fin),
                                ParsedOpenAiFrame::Ignore => {}
                            }
                        }
                    }
                }
            }
        }
        Ok(fin)
    }

    pub async fn list_models(&self) -> Result<Vec<ProviderModelInfo>, ProviderError> {
        let url = format!("{}/models", self.base_url.trim_end_matches('/'));
        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(ProviderError::Http {
                status: resp.status().as_u16(),
                body: resp.text().await.unwrap_or_default().chars().take(4096).collect(),
            });
        }
        let v: serde_json::Value = resp.json().await.map_err(|e| ProviderError::Parse(e.to_string()))?;
        Ok(v.get("data")
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|it| {
                        let id = it.get("id")?.as_str()?.to_string();
                        Some(ProviderModelInfo { id: id.clone(), display_name: id })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }
}

pub(crate) enum ParsedOpenAiFrame {
    Text(String),
    Usage { prompt: i64, completion: i64 },
    Done,
    Ignore,
}

pub(crate) fn parse_sse_frame(line: &str) -> ParsedOpenAiFrame {
    let line = line.trim_end();
    let Some(payload) = line.strip_prefix("data: ").or_else(|| line.strip_prefix("data:")) else {
        return ParsedOpenAiFrame::Ignore;
    };
    let payload = payload.trim();
    if payload.is_empty() {
        return ParsedOpenAiFrame::Ignore;
    }
    if payload == "[DONE]" {
        return ParsedOpenAiFrame::Done;
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) else {
        return ParsedOpenAiFrame::Ignore;
    };
    let text = v
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("delta"))
        .and_then(|d| d.get("content"))
        .and_then(|c| c.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let usage = v.get("usage").and_then(|u| {
        let p = u.get("prompt_tokens")?.as_i64()?;
        let c = u.get("completion_tokens")?.as_i64()?;
        Some((p, c))
    });
    match (text, usage) {
        (Some(t), _) => ParsedOpenAiFrame::Text(t),
        (None, Some((p, c))) => ParsedOpenAiFrame::Usage { prompt: p, completion: c },
        _ => ParsedOpenAiFrame::Ignore,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_content_delta_frame() {
        let f = parse_sse_frame(r#"data: {"choices":[{"delta":{"content":"hello"}}]}"#);
        match f {
            ParsedOpenAiFrame::Text(t) => assert_eq!(t, "hello"),
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn parses_done_sentinel() {
        assert!(matches!(parse_sse_frame("data: [DONE]"), ParsedOpenAiFrame::Done));
    }

    #[test]
    fn parses_usage_frame() {
        let f = parse_sse_frame(
            r#"data: {"choices":[{"delta":{}}],"usage":{"prompt_tokens":3,"completion_tokens":7}}"#,
        );
        match f {
            ParsedOpenAiFrame::Usage { prompt, completion } => {
                assert_eq!(prompt, 3);
                assert_eq!(completion, 7);
            }
            _ => panic!("expected Usage"),
        }
    }

    #[test]
    fn ignores_comment_and_empty() {
        assert!(matches!(parse_sse_frame(""), ParsedOpenAiFrame::Ignore));
        assert!(matches!(parse_sse_frame(":ping"), ParsedOpenAiFrame::Ignore));
    }

    #[test]
    fn empty_delta_content_ignored() {
        let f = parse_sse_frame(r#"data: {"choices":[{"delta":{"role":"assistant"}}]}"#);
        assert!(matches!(f, ParsedOpenAiFrame::Ignore));
    }
}
