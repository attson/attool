// Task 4: Anthropic Claude provider.

use futures_util::StreamExt;
use tokio::sync::{mpsc, oneshot};

use super::{ChatDelta, ChatFinal, ChatRequest, ContentPart, ProviderError, ProviderModelInfo};

pub struct AnthropicProvider {
    pub base_url: String,
    pub api_key: String,
    pub client: reqwest::Client,
}

impl AnthropicProvider {
    pub async fn stream_chat(
        &self,
        req: ChatRequest,
        delta_tx: mpsc::UnboundedSender<ChatDelta>,
        mut cancel_rx: oneshot::Receiver<()>,
    ) -> Result<ChatFinal, ProviderError> {
        // system prompt 是顶层字段，不放进 messages 数组
        let messages: Vec<serde_json::Value> = req
            .messages
            .iter()
            .map(|m| {
                let parts: Vec<serde_json::Value> = m
                    .content
                    .iter()
                    .map(|p| match p {
                        ContentPart::Text(t) => serde_json::json!({"type":"text","text":t}),
                        ContentPart::Image { path, mime } => {
                            use base64::Engine;
                            let bytes =
                                std::fs::read(path.trim_start_matches("file://")).unwrap_or_default();
                            let data = base64::engine::general_purpose::STANDARD.encode(&bytes);
                            serde_json::json!({
                                "type": "image",
                                "source": {"type": "base64", "media_type": mime, "data": data},
                            })
                        }
                    })
                    .collect();
                serde_json::json!({"role": m.role, "content": parts})
            })
            .collect();

        // Anthropic 要求 max_tokens 必填，默认给 4096
        let mut body = serde_json::json!({
            "model": req.model_id,
            "messages": messages,
            "stream": true,
            "max_tokens": req.max_tokens.unwrap_or(4096),
        });
        if let Some(sp) = &req.system_prompt {
            if !sp.is_empty() {
                body["system"] = sp.clone().into();
            }
        }
        if let Some(t) = req.temperature {
            body["temperature"] = t.into();
        }

        let url = format!("{}/messages", self.base_url.trim_end_matches('/'));
        let resp = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
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
                        let mut event_type = "";
                        let mut data = String::new();
                        for line in event.lines() {
                            if let Some(t) = line.strip_prefix("event: ") {
                                event_type = t.trim();
                            } else if let Some(d) = line.strip_prefix("data: ") {
                                data.push_str(d.trim());
                            }
                        }
                        match parse_anthropic_event(event_type, &data) {
                            ParsedAnthropicEvent::Text(t) => {
                                let _ = delta_tx.send(ChatDelta::Text(t));
                            }
                            ParsedAnthropicEvent::InputTokens(n) => fin.prompt_tokens = Some(n),
                            ParsedAnthropicEvent::OutputTokens(n) => fin.completion_tokens = Some(n),
                            ParsedAnthropicEvent::Done => return Ok(fin),
                            ParsedAnthropicEvent::Ignore => {}
                        }
                    }
                }
            }
        }
        Ok(fin)
    }

    pub async fn list_models(&self) -> Result<Vec<ProviderModelInfo>, ProviderError> {
        // Anthropic 没有可枚举模型的公开接口，返回内置列表
        Ok(builtin_anthropic_models())
    }
}

pub(crate) enum ParsedAnthropicEvent {
    Text(String),
    InputTokens(i64),
    OutputTokens(i64),
    Done,
    Ignore,
}

pub(crate) fn parse_anthropic_event(event_type: &str, data: &str) -> ParsedAnthropicEvent {
    let v: serde_json::Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => return ParsedAnthropicEvent::Ignore,
    };
    match event_type {
        "content_block_delta" => {
            let delta = v.get("delta");
            let dtype = delta.and_then(|d| d.get("type")).and_then(|t| t.as_str()).unwrap_or("");
            if dtype == "text_delta" {
                if let Some(t) = delta.and_then(|d| d.get("text")).and_then(|t| t.as_str()) {
                    return ParsedAnthropicEvent::Text(t.to_string());
                }
            }
            ParsedAnthropicEvent::Ignore
        }
        "message_start" => v
            .get("message")
            .and_then(|m| m.get("usage"))
            .and_then(|u| u.get("input_tokens"))
            .and_then(|n| n.as_i64())
            .map(ParsedAnthropicEvent::InputTokens)
            .unwrap_or(ParsedAnthropicEvent::Ignore),
        "message_delta" => v
            .get("usage")
            .and_then(|u| u.get("output_tokens"))
            .and_then(|n| n.as_i64())
            .map(ParsedAnthropicEvent::OutputTokens)
            .unwrap_or(ParsedAnthropicEvent::Ignore),
        "message_stop" => ParsedAnthropicEvent::Done,
        _ => ParsedAnthropicEvent::Ignore,
    }
}

fn builtin_anthropic_models() -> Vec<ProviderModelInfo> {
    [
        "claude-opus-4-1-20250805",
        "claude-sonnet-4-5-20250929",
        "claude-3-5-haiku-20241022",
        "claude-3-5-sonnet-20241022",
        "claude-3-opus-20240229",
        "claude-3-haiku-20240307",
    ]
    .iter()
    .map(|id| ProviderModelInfo { id: (*id).into(), display_name: (*id).into() })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_delta_extracts_content() {
        let ev = parse_anthropic_event(
            "content_block_delta",
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"hi"}}"#,
        );
        match ev {
            ParsedAnthropicEvent::Text(t) => assert_eq!(t, "hi"),
            _ => panic!(),
        }
    }

    #[test]
    fn non_text_delta_ignored() {
        let ev = parse_anthropic_event(
            "content_block_delta",
            r#"{"delta":{"type":"input_json_delta","partial_json":"..."}}"#,
        );
        assert!(matches!(ev, ParsedAnthropicEvent::Ignore));
    }

    #[test]
    fn message_start_extracts_input_tokens() {
        let ev = parse_anthropic_event(
            "message_start",
            r#"{"type":"message_start","message":{"usage":{"input_tokens":12,"output_tokens":0}}}"#,
        );
        assert!(matches!(ev, ParsedAnthropicEvent::InputTokens(12)));
    }

    #[test]
    fn message_delta_extracts_output_tokens() {
        let ev = parse_anthropic_event(
            "message_delta",
            r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":50}}"#,
        );
        assert!(matches!(ev, ParsedAnthropicEvent::OutputTokens(50)));
    }

    #[test]
    fn message_stop_is_done() {
        assert!(matches!(parse_anthropic_event("message_stop", "{}"), ParsedAnthropicEvent::Done));
    }

    #[test]
    fn unknown_event_ignored() {
        assert!(matches!(parse_anthropic_event("ping", ""), ParsedAnthropicEvent::Ignore));
    }
}
