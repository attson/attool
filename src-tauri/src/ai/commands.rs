use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use super::models::{MessageRow, ModelRow, ProviderRow, SessionRow};
use super::providers::anthropic::AnthropicProvider;
use super::providers::ollama::OllamaProvider;
use super::providers::openai::OpenAiProvider;
use super::providers::{ChatDelta, ChatMessage, ChatRequest, ContentPart, ProviderModelInfo};
use super::session::{AiCancelHandle, AiSessionState};
use super::storage::AiStore;

/// 前端投递内容片段的 wire 格式；用 `to_content_part` 转成 provider 层的 `ContentPart`。
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UiContentPart {
    Text { text: String },
    Image { path: String, mime: String },
}

fn to_content_part(part: UiContentPart) -> ContentPart {
    match part {
        UiContentPart::Text { text } => ContentPart::Text(text),
        UiContentPart::Image { path, mime } => ContentPart::Image { path, mime },
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub session: SessionRow,
    pub messages: Vec<MessageRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigExport {
    pub version: i32,
    pub providers: Vec<ProviderRow>,
    pub models: Vec<ModelRow>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub providers_upserted: i32,
    pub models_upserted: i32,
}

/// Provider 派发枚举：把三家 provider 的具体类型收敛到一处，
/// 让 `ai_send` / `ai_retry` / `ai_fetch_provider_models` 只需 match 一次。
pub enum Provider {
    OpenAi(OpenAiProvider),
    Anthropic(AnthropicProvider),
    Ollama(OllamaProvider),
}

pub fn build_provider(row: &ProviderRow, client: reqwest::Client) -> Result<Provider, String> {
    match row.kind.as_str() {
        "openai" => Ok(Provider::OpenAi(OpenAiProvider {
            base_url: row.base_url.clone(),
            api_key: row.api_key.clone(),
            client,
        })),
        "anthropic" => Ok(Provider::Anthropic(AnthropicProvider {
            base_url: row.base_url.clone(),
            api_key: row.api_key.clone(),
            client,
        })),
        "ollama" => Ok(Provider::Ollama(OllamaProvider {
            base_url: row.base_url.clone(),
            client,
        })),
        k => Err(format!("未知 provider kind: {k}")),
    }
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn content_json_to_parts(content_json: &str) -> Result<Vec<ContentPart>, String> {
    let values: Vec<serde_json::Value> =
        serde_json::from_str(content_json).map_err(|error| format!("解析消息内容失败：{error}"))?;
    let mut parts = Vec::with_capacity(values.len());
    for value in values {
        match value.get("type").and_then(|t| t.as_str()) {
            Some("text") => {
                let text = value.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string();
                parts.push(ContentPart::Text(text));
            }
            Some("image") => {
                let path = value.get("path").and_then(|t| t.as_str()).unwrap_or("").to_string();
                let mime = value.get("mime").and_then(|t| t.as_str()).unwrap_or("").to_string();
                parts.push(ContentPart::Image { path, mime });
            }
            _ => {}
        }
    }
    Ok(parts)
}

fn content_parts_to_json(parts: &[ContentPart]) -> Result<String, String> {
    let values: Vec<serde_json::Value> = parts
        .iter()
        .map(|part| match part {
            ContentPart::Text(text) => serde_json::json!({"type": "text", "text": text}),
            ContentPart::Image { path, mime } => {
                serde_json::json!({"type": "image", "path": path, "mime": mime})
            }
        })
        .collect();
    serde_json::to_string(&values).map_err(|error| format!("序列化消息内容失败：{error}"))
}

fn extract_text_preview(content_json: &str) -> String {
    content_json_to_parts(content_json)
        .unwrap_or_default()
        .into_iter()
        .map(|part| match part {
            ContentPart::Text(text) => text,
            ContentPart::Image { .. } => "[image]".to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 根据 `session_id` 下已完成/流式中的 user/assistant 消息重建对话上下文，
/// 排除 `exclude_ids`（当前正在写入的 assistant 消息，以及 retry 时的旧消息）。
fn build_chat_messages(
    store: &AiStore,
    session_id: &str,
    exclude_ids: &[String],
) -> Result<Vec<ChatMessage>, String> {
    let history = store.list_messages(session_id)?;
    let mut messages = Vec::new();
    for m in history {
        if exclude_ids.iter().any(|id| id == &m.id) {
            continue;
        }
        if !(m.role == "user" || m.role == "assistant") {
            continue;
        }
        if !(m.status == "done" || m.status == "streaming") {
            continue;
        }
        let parts = content_json_to_parts(&m.content_json)?;
        messages.push(ChatMessage { role: m.role, content: parts });
    }
    Ok(messages)
}

/// 找到 model 对应的 provider row；集中处理"模型/服务商不存在"的错误信息。
fn resolve_provider_for_model(store: &AiStore, model: &ModelRow) -> Result<ProviderRow, String> {
    store
        .list_providers()?
        .into_iter()
        .find(|p| p.id == model.provider_id)
        .ok_or_else(|| format!("provider 不存在：{}", model.provider_id))
}

#[allow(clippy::too_many_arguments)]
fn spawn_chat_stream(
    store: Arc<AiStore>,
    session_state: Arc<AiSessionState>,
    client: reqwest::Client,
    app: AppHandle,
    session_id: String,
    assistant_id: String,
    exclude_ids: Vec<String>,
    system_prompt: String,
    model_id: String,
    temperature: Option<f64>,
    max_tokens: Option<i64>,
    provider_row: ProviderRow,
) -> Result<(), String> {
    let messages = build_chat_messages(&store, &session_id, &exclude_ids)?;
    let req = ChatRequest {
        model_id,
        messages,
        system_prompt: if system_prompt.is_empty() { None } else { Some(system_prompt) },
        temperature,
        max_tokens,
    };
    let provider = build_provider(&provider_row, client)?;

    let (delta_tx, mut delta_rx) = tokio::sync::mpsc::unbounded_channel::<ChatDelta>();
    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
    let cancelled = Arc::new(AtomicBool::new(false));
    session_state.insert(
        assistant_id.clone(),
        AiCancelHandle { tx: cancel_tx, cancelled: cancelled.clone() },
    );

    // 转发任务：把 provider 吐出的增量文本落库并广播给前端。持有 JoinHandle
    // 是因为主任务必须等它把 channel 排空、所有 delta 都落库/emit 完，才能
    // 发 done 事件——否则前端按“收到 done 就停止监听”处理时会截断正文。
    let store_fwd = store.clone();
    let app_fwd = app.clone();
    let msg_id_fwd = assistant_id.clone();
    let forwarder = tokio::spawn(async move {
        while let Some(ChatDelta::Text(t)) = delta_rx.recv().await {
            let _ = store_fwd.append_message_delta(&msg_id_fwd, &t);
            let _ = app_fwd.emit(
                &format!("ai-chat-delta-{msg_id_fwd}"),
                serde_json::json!({ "text": t }),
            );
        }
    });

    // 主任务：跑 stream_chat，收尾时把 done/error/cancelled 落库并广播。
    let store_final = store.clone();
    let app_final = app.clone();
    let msg_id_final = assistant_id.clone();
    let session_state_final = session_state.clone();
    let cancelled_final = cancelled.clone();
    tokio::spawn(async move {
        let result = match provider {
            Provider::OpenAi(p) => p.stream_chat(req, delta_tx, cancel_rx).await,
            Provider::Anthropic(p) => p.stream_chat(req, delta_tx, cancel_rx).await,
            Provider::Ollama(p) => p.stream_chat(req, delta_tx, cancel_rx).await,
        };
        // stream_chat 消费了 delta_tx（其内部持有的发送端在函数返回时被
        // drop），所以此刻 channel 的发送端已经没有别的持有者了；等待
        // forwarder 退出即可保证所有已入队的 delta 都已处理完毕。
        let _ = forwarder.await;
        let now = now_ms();
        let (status, err_msg, prompt_tok, completion_tok) = if cancelled_final.load(Ordering::SeqCst)
        {
            ("cancelled", None, None, None)
        } else {
            match &result {
                Ok(fin) => ("done", None, fin.prompt_tokens, fin.completion_tokens),
                Err(e) => ("error", Some(e.to_string()), None, None),
            }
        };
        let _ = store_final.update_message_status(
            &msg_id_final,
            status,
            err_msg.clone(),
            prompt_tok,
            completion_tok,
            Some(now),
        );
        let mut payload = serde_json::json!({ "status": status });
        if let Some(p) = prompt_tok {
            payload["promptTokens"] = p.into();
        }
        if let Some(c) = completion_tok {
            payload["completionTokens"] = c.into();
        }
        if let Some(e) = err_msg {
            payload["error"] = e.into();
        }
        let _ = app_final.emit(&format!("ai-chat-done-{msg_id_final}"), payload);
        session_state_final.remove(&msg_id_final);
    });

    Ok(())
}

/// `spawn_chat_stream` can fail before any task is spawned (e.g. unknown
/// provider `kind`, or a bad `content_json` row). At that point the
/// assistant placeholder is already sitting in the DB as `status="streaming"`
/// with no cancel handle and no finalizer task — nothing will ever move it
/// out of that state. Call this from every `spawn_chat_stream` call site's
/// error branch so the row (and the frontend listening for it) converges.
fn finalize_message_error(store: &AiStore, message_id: &str, error: &str) {
    let _ = store.update_message_status(
        message_id,
        "error",
        Some(error.to_string()),
        None,
        None,
        Some(now_ms()),
    );
}

// ---- providers ----

#[tauri::command]
pub fn ai_list_providers(store: State<'_, Arc<AiStore>>) -> Result<Vec<ProviderRow>, String> {
    store.list_providers()
}

const KNOWN_PROVIDER_KINDS: [&str; 3] = ["openai", "anthropic", "ollama"];

fn validate_provider_kind(kind: &str) -> Result<(), String> {
    if KNOWN_PROVIDER_KINDS.contains(&kind) {
        Ok(())
    } else {
        Err(format!("未知 provider kind: {kind}"))
    }
}

#[tauri::command]
pub fn ai_upsert_provider(
    provider: ProviderRow,
    store: State<'_, Arc<AiStore>>,
) -> Result<ProviderRow, String> {
    // 提前拒绝未知 kind：否则要等到某个会话真正 ai_send 时才会在
    // build_provider 里失败，把失败面从"保存配置时"推迟到"聊天时"。
    validate_provider_kind(&provider.kind)?;
    store.upsert_provider(provider)
}

#[tauri::command]
pub fn ai_delete_provider(id: String, store: State<'_, Arc<AiStore>>) -> Result<(), String> {
    store.delete_provider(&id)
}

// ---- models ----

#[tauri::command]
pub fn ai_list_models(
    provider_id: Option<String>,
    store: State<'_, Arc<AiStore>>,
) -> Result<Vec<ModelRow>, String> {
    store.list_models(provider_id.as_deref())
}

#[tauri::command]
pub fn ai_upsert_model(model: ModelRow, store: State<'_, Arc<AiStore>>) -> Result<ModelRow, String> {
    store.upsert_model(model)
}

#[tauri::command]
pub fn ai_delete_model(id: String, store: State<'_, Arc<AiStore>>) -> Result<(), String> {
    store.delete_model(&id)
}

#[tauri::command]
pub async fn ai_fetch_provider_models(
    provider_id: String,
    store: State<'_, Arc<AiStore>>,
    client: State<'_, reqwest::Client>,
) -> Result<Vec<ProviderModelInfo>, String> {
    let row = store
        .list_providers()?
        .into_iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| format!("provider 不存在：{provider_id}"))?;
    let provider = build_provider(&row, client.inner().clone())?;
    let result = match provider {
        Provider::OpenAi(p) => p.list_models().await,
        Provider::Anthropic(p) => p.list_models().await,
        Provider::Ollama(p) => p.list_models().await,
    };
    result.map_err(|error| error.to_string())
}

// ---- sessions ----

#[tauri::command]
pub fn ai_list_sessions(
    search: Option<String>,
    store: State<'_, Arc<AiStore>>,
) -> Result<Vec<SessionRow>, String> {
    store.list_sessions(search.as_deref())
}

#[tauri::command]
pub fn ai_create_session(
    title: Option<String>,
    model_id: Option<String>,
    store: State<'_, Arc<AiStore>>,
) -> Result<SessionRow, String> {
    let now = now_ms();
    let row = SessionRow {
        id: uuid::Uuid::new_v4().to_string(),
        title: title.unwrap_or_else(|| "新会话".to_string()),
        system_prompt: String::new(),
        current_model_id: model_id,
        created_at: now,
        updated_at: now,
    };
    store.create_session(row)
}

#[tauri::command]
pub fn ai_get_session(id: String, store: State<'_, Arc<AiStore>>) -> Result<SessionSummary, String> {
    let session = store.get_session(&id)?.ok_or_else(|| format!("会话不存在：{id}"))?;
    let messages = store.list_messages(&id)?;
    Ok(SessionSummary { session, messages })
}

#[tauri::command]
pub fn ai_update_session(
    id: String,
    title: Option<String>,
    system_prompt: Option<String>,
    model_id: Option<String>,
    store: State<'_, Arc<AiStore>>,
) -> Result<SessionRow, String> {
    // model_id=None 表示"不修改"；storage 层用 Option<Option<String>> 区分
    // "不修改" 与 "清空"，本命令暂不暴露清空能力（前端可用专门的 clear 语义再扩展）。
    store.update_session(&id, title, system_prompt, model_id.map(Some))
}

#[tauri::command]
pub fn ai_delete_session(id: String, store: State<'_, Arc<AiStore>>) -> Result<(), String> {
    store.delete_session(&id)
}

// ---- chat ----

#[tauri::command]
pub async fn ai_send(
    session_id: String,
    user_content: Vec<UiContentPart>,
    store: State<'_, Arc<AiStore>>,
    session_state: State<'_, Arc<AiSessionState>>,
    client: State<'_, reqwest::Client>,
    app: AppHandle,
) -> Result<String, String> {
    let session = store
        .get_session(&session_id)?
        .ok_or_else(|| format!("会话不存在：{session_id}"))?;
    let model_id = session
        .current_model_id
        .clone()
        .ok_or_else(|| "会话未选择模型".to_string())?;
    let model = store
        .get_model(&model_id)?
        .ok_or_else(|| format!("模型不存在：{model_id}"))?;
    let provider_row = resolve_provider_for_model(&store, &model)?;

    let now = now_ms();
    let user_id = uuid::Uuid::new_v4().to_string();
    let parts: Vec<ContentPart> = user_content.into_iter().map(to_content_part).collect();
    let content_json = content_parts_to_json(&parts)?;
    store.insert_message(MessageRow {
        id: user_id,
        session_id: session_id.clone(),
        role: "user".into(),
        content_json,
        model_id: None,
        status: "done".into(),
        error_message: None,
        prompt_tokens: None,
        completion_tokens: None,
        created_at: now,
        finished_at: Some(now),
    })?;

    let assistant_id = uuid::Uuid::new_v4().to_string();
    store.insert_message(MessageRow {
        id: assistant_id.clone(),
        session_id: session_id.clone(),
        role: "assistant".into(),
        content_json: "[]".into(),
        model_id: Some(model.id.clone()),
        status: "streaming".into(),
        error_message: None,
        prompt_tokens: None,
        completion_tokens: None,
        created_at: now_ms(),
        finished_at: None,
    })?;
    store.touch_session(&session_id)?;

    if let Err(error) = spawn_chat_stream(
        store.inner().clone(),
        session_state.inner().clone(),
        client.inner().clone(),
        app.clone(),
        session_id,
        assistant_id.clone(),
        vec![assistant_id.clone()],
        session.system_prompt,
        model.model_id,
        model.temperature,
        model.max_tokens,
        provider_row,
    ) {
        // 没有任何后台任务被启动，assistant 行会永远卡在 streaming——必须
        // 在这里就地收尾，否则前端等不到 done 事件，用户看到的是永久转圈。
        finalize_message_error(&store, &assistant_id, &error);
        let _ = app.emit(
            &format!("ai-chat-done-{assistant_id}"),
            serde_json::json!({ "status": "error", "error": error }),
        );
        return Err(error);
    }

    Ok(assistant_id)
}

#[tauri::command]
pub fn ai_cancel(
    assistant_message_id: String,
    session_state: State<'_, Arc<AiSessionState>>,
) -> Result<bool, String> {
    Ok(session_state.cancel(&assistant_message_id))
}

#[tauri::command]
pub async fn ai_retry(
    assistant_message_id: String,
    store: State<'_, Arc<AiStore>>,
    session_state: State<'_, Arc<AiSessionState>>,
    client: State<'_, reqwest::Client>,
    app: AppHandle,
) -> Result<String, String> {
    let old = store
        .get_message(&assistant_message_id)?
        .ok_or_else(|| format!("消息不存在：{assistant_message_id}"))?;
    let session = store
        .get_session(&old.session_id)?
        .ok_or_else(|| format!("会话不存在：{}", old.session_id))?;
    let model_id = session
        .current_model_id
        .clone()
        .ok_or_else(|| "会话未选择模型".to_string())?;
    let model = store
        .get_model(&model_id)?
        .ok_or_else(|| format!("模型不存在：{model_id}"))?;
    let provider_row = resolve_provider_for_model(&store, &model)?;

    let new_id = uuid::Uuid::new_v4().to_string();
    store.insert_message(MessageRow {
        id: new_id.clone(),
        session_id: old.session_id.clone(),
        role: "assistant".into(),
        content_json: "[]".into(),
        model_id: Some(model.id.clone()),
        status: "streaming".into(),
        error_message: None,
        prompt_tokens: None,
        completion_tokens: None,
        created_at: now_ms(),
        finished_at: None,
    })?;
    store.touch_session(&old.session_id)?;

    if let Err(error) = spawn_chat_stream(
        store.inner().clone(),
        session_state.inner().clone(),
        client.inner().clone(),
        app.clone(),
        old.session_id,
        new_id.clone(),
        // 新消息本身，以及被重试的旧 assistant 消息都不应进入上下文。
        vec![new_id.clone(), old.id],
        session.system_prompt,
        model.model_id,
        model.temperature,
        model.max_tokens,
        provider_row,
    ) {
        finalize_message_error(&store, &new_id, &error);
        let _ = app.emit(
            &format!("ai-chat-done-{new_id}"),
            serde_json::json!({ "status": "error", "error": error }),
        );
        return Err(error);
    }

    Ok(new_id)
}

// ---- export / import ----

#[tauri::command]
pub fn ai_export_session(
    session_id: String,
    format: String,
    store: State<'_, Arc<AiStore>>,
) -> Result<String, String> {
    let session = store
        .get_session(&session_id)?
        .ok_or_else(|| format!("会话不存在：{session_id}"))?;
    let messages = store.list_messages(&session_id)?;
    match format.as_str() {
        "json" => {
            let summary = SessionSummary { session, messages };
            serde_json::to_string_pretty(&summary).map_err(|error| format!("导出失败：{error}"))
        }
        "markdown" => {
            let mut out = format!("# {}\n\n> {}\n\n---\n\n", session.title, session.system_prompt);
            for m in messages {
                let text = extract_text_preview(&m.content_json);
                if m.role == "assistant" {
                    let model = m.model_id.clone().unwrap_or_default();
                    let tokens = match (m.prompt_tokens, m.completion_tokens) {
                        (Some(p), Some(c)) => format!(" · {p}t→{c}t"),
                        _ => String::new(),
                    };
                    out.push_str(&format!("**assistant** *({model}{tokens})*：\n\n{text}\n\n"));
                } else {
                    out.push_str(&format!("**{}**：\n\n{text}\n\n", m.role));
                }
            }
            Ok(out)
        }
        other => Err(format!("未知导出格式：{other}")),
    }
}

#[tauri::command]
pub fn ai_export_config(
    include_keys: bool,
    store: State<'_, Arc<AiStore>>,
) -> Result<String, String> {
    let mut providers = store.list_providers()?;
    if !include_keys {
        for p in providers.iter_mut() {
            p.api_key = String::new();
        }
    }
    let models = store.list_models(None)?;
    let export = ConfigExport { version: 1, providers, models };
    serde_json::to_string_pretty(&export).map_err(|error| format!("导出配置失败：{error}"))
}

#[tauri::command]
pub fn ai_import_config(
    json: String,
    store: State<'_, Arc<AiStore>>,
) -> Result<ImportSummary, String> {
    let parsed: ConfigExport =
        serde_json::from_str(&json).map_err(|error| format!("解析配置失败：{error}"))?;
    let existing_providers = store.list_providers()?;

    let mut providers_upserted = 0i32;
    for mut p in parsed.providers {
        if p.api_key.is_empty() {
            if let Some(existing) = existing_providers.iter().find(|e| e.id == p.id) {
                if !existing.api_key.is_empty() {
                    p.api_key = existing.api_key.clone();
                }
            }
        }
        store.upsert_provider(p)?;
        providers_upserted += 1;
    }

    let mut models_upserted = 0i32;
    for m in parsed.models {
        store.upsert_model(m)?;
        models_upserted += 1;
    }

    Ok(ImportSummary { providers_upserted, models_upserted })
}

#[tauri::command]
pub fn ai_save_attachment(bytes: Vec<u8>, mime: String, app: AppHandle) -> Result<String, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join("ai-attachments");
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let ext = mime.split('/').nth(1).unwrap_or("bin");
    let name = format!("{}.{ext}", uuid::Uuid::new_v4());
    let path = dir.join(&name);
    std::fs::write(&path, &bytes).map_err(|error| error.to_string())?;
    Ok(format!("file://{}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider_row(kind: &str) -> ProviderRow {
        ProviderRow {
            id: "p1".into(),
            name: "x".into(),
            kind: kind.into(),
            base_url: "http://x".into(),
            api_key: "k".into(),
            extra_json: "{}".into(),
            sort_order: 0,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn build_provider_dispatches_kind() {
        let client = reqwest::Client::new();
        let row = provider_row("openai");
        let p = build_provider(&row, client).unwrap();
        assert!(matches!(p, Provider::OpenAi(_)));
    }

    #[test]
    fn build_provider_rejects_unknown_kind() {
        let client = reqwest::Client::new();
        let row = provider_row("gemini");
        assert!(build_provider(&row, client).is_err());
    }

    #[test]
    fn content_json_roundtrip_text_and_image() {
        let parts = vec![
            ContentPart::Text("hi".into()),
            ContentPart::Image { path: "/a.png".into(), mime: "image/png".into() },
        ];
        let json = content_parts_to_json(&parts).unwrap();
        let back = content_json_to_parts(&json).unwrap();
        assert_eq!(back.len(), 2);
        match &back[0] {
            ContentPart::Text(t) => assert_eq!(t, "hi"),
            _ => panic!("expected text"),
        }
        match &back[1] {
            ContentPart::Image { path, mime } => {
                assert_eq!(path, "/a.png");
                assert_eq!(mime, "image/png");
            }
            _ => panic!("expected image"),
        }
    }

    #[test]
    fn validate_provider_kind_rejects_unknown() {
        assert!(validate_provider_kind("openai").is_ok());
        assert!(validate_provider_kind("anthropic").is_ok());
        assert!(validate_provider_kind("ollama").is_ok());
        assert!(validate_provider_kind("gemini").is_err());
    }

    /// Regression test for the "stuck streaming forever" bug: if
    /// `spawn_chat_stream` fails before any task is
    /// spawned, the assistant placeholder must be moved out of
    /// `status="streaming"` — otherwise no cancel handle and no finalizer
    /// task exist, and the row (and any frontend awaiting its done event)
    /// never converges.
    #[test]
    fn finalize_message_error_moves_streaming_row_to_error() {
        let store = AiStore::new_in_memory().unwrap();
        store
            .create_session(SessionRow {
                id: "s1".into(),
                title: "t".into(),
                system_prompt: "".into(),
                current_model_id: None,
                created_at: 0,
                updated_at: 0,
            })
            .unwrap();
        store
            .insert_message(MessageRow {
                id: "m1".into(),
                session_id: "s1".into(),
                role: "assistant".into(),
                content_json: "[]".into(),
                model_id: None,
                status: "streaming".into(),
                error_message: None,
                prompt_tokens: None,
                completion_tokens: None,
                created_at: 0,
                finished_at: None,
            })
            .unwrap();

        finalize_message_error(&store, "m1", "未知 provider kind: gemini");

        let got = store.get_message("m1").unwrap().unwrap();
        assert_eq!(got.status, "error");
        assert_eq!(got.error_message.as_deref(), Some("未知 provider kind: gemini"));
        assert!(got.finished_at.is_some());
    }
}
