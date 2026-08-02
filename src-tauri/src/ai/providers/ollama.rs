// Task 5: Ollama provider.

use futures_util::StreamExt;
use tokio::sync::{mpsc, oneshot};

use super::{ChatDelta, ChatFinal, ChatRequest, ContentPart, ProviderError, ProviderModelInfo};

pub struct OllamaProvider {
    pub base_url: String,
    pub client: reqwest::Client,
}

impl OllamaProvider {
    pub async fn stream_chat(
        &self,
        req: ChatRequest,
        delta_tx: mpsc::UnboundedSender<ChatDelta>,
        mut cancel_rx: oneshot::Receiver<()>,
    ) -> Result<ChatFinal, ProviderError> {
        use base64::Engine;

        let mut messages = Vec::new();
        if let Some(sp) = &req.system_prompt {
            if !sp.is_empty() {
                messages.push(serde_json::json!({"role": "system", "content": sp}));
            }
        }
        for m in &req.messages {
            // Ollama 没有多段 content 数组，文本拼接进 content，图片走顶层 images 字段。
            let mut text = String::new();
            let mut images: Vec<String> = Vec::new();
            for p in &m.content {
                match p {
                    ContentPart::Text(t) => text.push_str(t),
                    ContentPart::Image { path, .. } => {
                        let bytes = std::fs::read(path.trim_start_matches("file://")).unwrap_or_default();
                        images.push(base64::engine::general_purpose::STANDARD.encode(&bytes));
                    }
                }
            }
            let mut obj = serde_json::json!({"role": m.role, "content": text});
            if !images.is_empty() {
                obj["images"] = images.into();
            }
            messages.push(obj);
        }

        let mut options = serde_json::json!({});
        if let Some(t) = req.temperature {
            options["temperature"] = t.into();
        }
        if let Some(mt) = req.max_tokens {
            options["num_predict"] = mt.into();
        }

        let body = serde_json::json!({
            "model": req.model_id,
            "messages": messages,
            "stream": true,
            "options": options,
        });

        let url = format!("{}/api/chat", self.base_url.trim_end_matches('/'));
        let resp = self
            .client
            .post(&url)
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
                    // NDJSON: 每行一个 JSON 对象，按单个 '\n' 切分。
                    while let Some(pos) = buf.find('\n') {
                        let line: String = buf.drain(..pos + 1).collect();
                        match parse_ollama_line(&line) {
                            ParsedOllamaLine::Text(t) => {
                                let _ = delta_tx.send(ChatDelta::Text(t));
                            }
                            ParsedOllamaLine::Final { prompt, completion } => {
                                fin.prompt_tokens = Some(prompt);
                                fin.completion_tokens = Some(completion);
                                return Ok(fin);
                            }
                            ParsedOllamaLine::Ignore => {}
                        }
                    }
                }
            }
        }
        Ok(fin)
    }

    pub async fn list_models(&self) -> Result<Vec<ProviderModelInfo>, ProviderError> {
        let url = format!("{}/api/tags", self.base_url.trim_end_matches('/'));
        let resp = self
            .client
            .get(&url)
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
        Ok(v.get("models")
            .and_then(|m| m.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|it| {
                        let name = it.get("name")?.as_str()?.to_string();
                        Some(ProviderModelInfo { id: name.clone(), display_name: name })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }
}

pub(crate) enum ParsedOllamaLine {
    Text(String),
    Final { prompt: i64, completion: i64 },
    Ignore,
}

pub(crate) fn parse_ollama_line(line: &str) -> ParsedOllamaLine {
    let line = line.trim();
    if line.is_empty() {
        return ParsedOllamaLine::Ignore;
    }
    let v: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return ParsedOllamaLine::Ignore,
    };
    if v.get("done").and_then(|d| d.as_bool()).unwrap_or(false) {
        let prompt = v.get("prompt_eval_count").and_then(|n| n.as_i64()).unwrap_or(0);
        let completion = v.get("eval_count").and_then(|n| n.as_i64()).unwrap_or(0);
        return ParsedOllamaLine::Final { prompt, completion };
    }
    v.get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| ParsedOllamaLine::Text(s.to_string()))
        .unwrap_or(ParsedOllamaLine::Ignore)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_line_extracts_content() {
        let f = parse_ollama_line(r#"{"message":{"role":"assistant","content":"hi"},"done":false}"#);
        match f {
            ParsedOllamaLine::Text(t) => assert_eq!(t, "hi"),
            _ => panic!(),
        }
    }

    #[test]
    fn final_line_extracts_counts() {
        let f = parse_ollama_line(r#"{"done":true,"prompt_eval_count":10,"eval_count":20}"#);
        match f {
            ParsedOllamaLine::Final { prompt, completion } => {
                assert_eq!(prompt, 10);
                assert_eq!(completion, 20);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn empty_content_ignored() {
        assert!(matches!(
            parse_ollama_line(r#"{"message":{"content":""},"done":false}"#),
            ParsedOllamaLine::Ignore
        ));
    }

    #[test]
    fn blank_line_ignored() {
        assert!(matches!(parse_ollama_line(""), ParsedOllamaLine::Ignore));
    }
}
