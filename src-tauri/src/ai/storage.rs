use std::{fs, path::PathBuf, sync::Mutex};

use rusqlite::{params, Connection, OptionalExtension};

use super::models::{MessageRow, ModelRow, ProviderRow, SessionRow};

pub(super) const MIGRATION_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS ai_providers (
  id           TEXT PRIMARY KEY,
  name         TEXT NOT NULL,
  kind         TEXT NOT NULL,
  base_url     TEXT NOT NULL,
  api_key      TEXT NOT NULL DEFAULT '',
  extra_json   TEXT NOT NULL DEFAULT '{}',
  sort_order   INTEGER NOT NULL DEFAULT 0,
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS ai_models (
  id             TEXT PRIMARY KEY,
  provider_id    TEXT NOT NULL REFERENCES ai_providers(id) ON DELETE CASCADE,
  model_id       TEXT NOT NULL,
  display_name   TEXT NOT NULL,
  capabilities   TEXT NOT NULL DEFAULT '["text"]',
  temperature    REAL,
  max_tokens     INTEGER,
  sort_order     INTEGER NOT NULL DEFAULT 0,
  created_at     INTEGER NOT NULL,
  updated_at     INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ai_models_provider ON ai_models(provider_id, sort_order);

CREATE TABLE IF NOT EXISTS ai_sessions (
  id               TEXT PRIMARY KEY,
  title            TEXT NOT NULL,
  system_prompt    TEXT NOT NULL DEFAULT '',
  current_model_id TEXT REFERENCES ai_models(id) ON DELETE SET NULL,
  created_at       INTEGER NOT NULL,
  updated_at       INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ai_sessions_updated ON ai_sessions(updated_at DESC);

CREATE TABLE IF NOT EXISTS ai_messages (
  id                TEXT PRIMARY KEY,
  session_id        TEXT NOT NULL REFERENCES ai_sessions(id) ON DELETE CASCADE,
  role              TEXT NOT NULL,
  content_json      TEXT NOT NULL,
  model_id          TEXT,
  status            TEXT NOT NULL,
  error_message     TEXT,
  prompt_tokens     INTEGER,
  completion_tokens INTEGER,
  created_at        INTEGER NOT NULL,
  finished_at       INTEGER
);
CREATE INDEX IF NOT EXISTS idx_ai_messages_session ON ai_messages(session_id, created_at);
"#;

pub struct AiStore {
    conn: Mutex<Connection>,
}

impl AiStore {
    pub fn new(root_dir: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&root_dir).map_err(|error| format!("创建 ai 目录失败：{error}"))?;
        let db_path = root_dir.join("ai.sqlite3");
        let connection =
            Connection::open(&db_path).map_err(|error| format!("打开 ai 数据库失败：{error}"))?;
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(|error| format!("启用外键约束失败：{error}"))?;
        connection
            .execute_batch(MIGRATION_SQL)
            .map_err(|error| format!("初始化 ai 数据库失败：{error}"))?;
        Ok(Self {
            conn: Mutex::new(connection),
        })
    }

    pub fn new_in_memory() -> Result<Self, String> {
        let connection = Connection::open_in_memory()
            .map_err(|error| format!("打开 ai 内存数据库失败：{error}"))?;
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(|error| format!("启用外键约束失败：{error}"))?;
        connection
            .execute_batch(MIGRATION_SQL)
            .map_err(|error| format!("初始化 ai 数据库失败：{error}"))?;
        Ok(Self {
            conn: Mutex::new(connection),
        })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("ai store mutex poisoned")
    }

    // ---- providers ----

    pub fn list_providers(&self) -> Result<Vec<ProviderRow>, String> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, kind, base_url, api_key, extra_json, sort_order, \
                        created_at, updated_at \
                 FROM ai_providers ORDER BY sort_order ASC, updated_at ASC",
            )
            .map_err(err_map)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(ProviderRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    kind: row.get(2)?,
                    base_url: row.get(3)?,
                    api_key: row.get(4)?,
                    extra_json: row.get(5)?,
                    sort_order: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            })
            .map_err(err_map)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err_map)?;
        Ok(rows)
    }

    pub fn upsert_provider(&self, row: ProviderRow) -> Result<ProviderRow, String> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO ai_providers \
               (id, name, kind, base_url, api_key, extra_json, sort_order, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
             ON CONFLICT(id) DO UPDATE SET \
               name=excluded.name, kind=excluded.kind, base_url=excluded.base_url, \
               api_key=excluded.api_key, extra_json=excluded.extra_json, \
               sort_order=excluded.sort_order, updated_at=excluded.updated_at",
            params![
                row.id,
                row.name,
                row.kind,
                row.base_url,
                row.api_key,
                row.extra_json,
                row.sort_order,
                row.created_at,
                row.updated_at,
            ],
        )
        .map_err(err_map)?;
        Ok(row)
    }

    pub fn delete_provider(&self, id: &str) -> Result<(), String> {
        let conn = self.conn();
        conn.execute("DELETE FROM ai_providers WHERE id = ?1", params![id])
            .map_err(err_map)?;
        Ok(())
    }

    // ---- models ----

    pub fn list_models(&self, provider_id: Option<&str>) -> Result<Vec<ModelRow>, String> {
        let conn = self.conn();
        let mut stmt;
        let rows;
        if let Some(provider_id) = provider_id {
            stmt = conn
                .prepare(
                    "SELECT id, provider_id, model_id, display_name, capabilities, \
                            temperature, max_tokens, sort_order, created_at, updated_at \
                     FROM ai_models WHERE provider_id = ?1 ORDER BY sort_order ASC",
                )
                .map_err(err_map)?;
            rows = stmt
                .query_map(params![provider_id], map_model_row)
                .map_err(err_map)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(err_map)?;
        } else {
            stmt = conn
                .prepare(
                    "SELECT id, provider_id, model_id, display_name, capabilities, \
                            temperature, max_tokens, sort_order, created_at, updated_at \
                     FROM ai_models ORDER BY provider_id ASC, sort_order ASC",
                )
                .map_err(err_map)?;
            rows = stmt
                .query_map([], map_model_row)
                .map_err(err_map)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(err_map)?;
        }
        Ok(rows)
    }

    pub fn get_model(&self, id: &str) -> Result<Option<ModelRow>, String> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, provider_id, model_id, display_name, capabilities, \
                    temperature, max_tokens, sort_order, created_at, updated_at \
             FROM ai_models WHERE id = ?1",
            params![id],
            map_model_row,
        )
        .optional()
        .map_err(err_map)
    }

    pub fn upsert_model(&self, row: ModelRow) -> Result<ModelRow, String> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO ai_models \
               (id, provider_id, model_id, display_name, capabilities, temperature, \
                max_tokens, sort_order, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) \
             ON CONFLICT(id) DO UPDATE SET \
               provider_id=excluded.provider_id, model_id=excluded.model_id, \
               display_name=excluded.display_name, capabilities=excluded.capabilities, \
               temperature=excluded.temperature, max_tokens=excluded.max_tokens, \
               sort_order=excluded.sort_order, updated_at=excluded.updated_at",
            params![
                row.id,
                row.provider_id,
                row.model_id,
                row.display_name,
                row.capabilities,
                row.temperature,
                row.max_tokens,
                row.sort_order,
                row.created_at,
                row.updated_at,
            ],
        )
        .map_err(err_map)?;
        Ok(row)
    }

    pub fn delete_model(&self, id: &str) -> Result<(), String> {
        let conn = self.conn();
        conn.execute("DELETE FROM ai_models WHERE id = ?1", params![id])
            .map_err(err_map)?;
        Ok(())
    }

    // ---- sessions ----

    pub fn list_sessions(&self, search: Option<&str>) -> Result<Vec<SessionRow>, String> {
        let conn = self.conn();
        let search = search.filter(|s| !s.is_empty());
        let mut stmt;
        let rows;
        if let Some(search) = search {
            stmt = conn
                .prepare(
                    "SELECT id, title, system_prompt, current_model_id, created_at, updated_at \
                     FROM ai_sessions WHERE title LIKE '%' || ?1 || '%' \
                     ORDER BY updated_at DESC",
                )
                .map_err(err_map)?;
            rows = stmt
                .query_map(params![search], map_session_row)
                .map_err(err_map)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(err_map)?;
        } else {
            stmt = conn
                .prepare(
                    "SELECT id, title, system_prompt, current_model_id, created_at, updated_at \
                     FROM ai_sessions ORDER BY updated_at DESC",
                )
                .map_err(err_map)?;
            rows = stmt
                .query_map([], map_session_row)
                .map_err(err_map)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(err_map)?;
        }
        Ok(rows)
    }

    pub fn get_session(&self, id: &str) -> Result<Option<SessionRow>, String> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, title, system_prompt, current_model_id, created_at, updated_at \
             FROM ai_sessions WHERE id = ?1",
            params![id],
            map_session_row,
        )
        .optional()
        .map_err(err_map)
    }

    pub fn create_session(&self, row: SessionRow) -> Result<SessionRow, String> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO ai_sessions \
               (id, title, system_prompt, current_model_id, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                row.id,
                row.title,
                row.system_prompt,
                row.current_model_id,
                row.created_at,
                row.updated_at,
            ],
        )
        .map_err(err_map)?;
        Ok(row)
    }

    pub fn update_session(
        &self,
        id: &str,
        title: Option<String>,
        system_prompt: Option<String>,
        current_model_id: Option<Option<String>>,
    ) -> Result<SessionRow, String> {
        let existing = self
            .get_session(id)?
            .ok_or_else(|| format!("会话不存在：{id}"))?;
        let title = title.unwrap_or(existing.title);
        let system_prompt = system_prompt.unwrap_or(existing.system_prompt);
        let current_model_id = current_model_id.unwrap_or(existing.current_model_id);
        let updated_at = now_ms();
        let conn = self.conn();
        conn.execute(
            "UPDATE ai_sessions SET title = ?1, system_prompt = ?2, current_model_id = ?3, \
             updated_at = ?4 WHERE id = ?5",
            params![title, system_prompt, current_model_id, updated_at, id],
        )
        .map_err(err_map)?;
        Ok(SessionRow {
            id: id.to_string(),
            title,
            system_prompt,
            current_model_id,
            created_at: existing.created_at,
            updated_at,
        })
    }

    pub fn touch_session(&self, id: &str) -> Result<(), String> {
        let conn = self.conn();
        conn.execute(
            "UPDATE ai_sessions SET updated_at = ?1 WHERE id = ?2",
            params![now_ms(), id],
        )
        .map_err(err_map)?;
        Ok(())
    }

    pub fn delete_session(&self, id: &str) -> Result<(), String> {
        let conn = self.conn();
        conn.execute("DELETE FROM ai_sessions WHERE id = ?1", params![id])
            .map_err(err_map)?;
        Ok(())
    }

    // ---- messages ----

    pub fn list_messages(&self, session_id: &str) -> Result<Vec<MessageRow>, String> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, session_id, role, content_json, model_id, status, error_message, \
                        prompt_tokens, completion_tokens, created_at, finished_at \
                 FROM ai_messages WHERE session_id = ?1 ORDER BY created_at ASC",
            )
            .map_err(err_map)?;
        let rows = stmt
            .query_map(params![session_id], map_message_row)
            .map_err(err_map)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err_map)?;
        Ok(rows)
    }

    pub fn get_message(&self, id: &str) -> Result<Option<MessageRow>, String> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, session_id, role, content_json, model_id, status, error_message, \
                    prompt_tokens, completion_tokens, created_at, finished_at \
             FROM ai_messages WHERE id = ?1",
            params![id],
            map_message_row,
        )
        .optional()
        .map_err(err_map)
    }

    pub fn insert_message(&self, row: MessageRow) -> Result<MessageRow, String> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO ai_messages \
               (id, session_id, role, content_json, model_id, status, error_message, \
                prompt_tokens, completion_tokens, created_at, finished_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                row.id,
                row.session_id,
                row.role,
                row.content_json,
                row.model_id,
                row.status,
                row.error_message,
                row.prompt_tokens,
                row.completion_tokens,
                row.created_at,
                row.finished_at,
            ],
        )
        .map_err(err_map)?;
        Ok(row)
    }

    pub fn update_message_status(
        &self,
        id: &str,
        status: &str,
        error_message: Option<String>,
        prompt_tokens: Option<i64>,
        completion_tokens: Option<i64>,
        finished_at: Option<i64>,
    ) -> Result<(), String> {
        let conn = self.conn();
        conn.execute(
            "UPDATE ai_messages SET status = ?1, error_message = ?2, prompt_tokens = ?3, \
             completion_tokens = ?4, finished_at = ?5 WHERE id = ?6",
            params![
                status,
                error_message,
                prompt_tokens,
                completion_tokens,
                finished_at,
                id,
            ],
        )
        .map_err(err_map)?;
        Ok(())
    }

    pub fn append_message_delta(&self, id: &str, delta_text: &str) -> Result<(), String> {
        let existing = self
            .get_message(id)?
            .ok_or_else(|| format!("消息不存在：{id}"))?;
        let mut segments: Vec<serde_json::Value> = serde_json::from_str(&existing.content_json)
            .map_err(|error| format!("解析 content_json 失败：{error}"))?;
        let text_segment = segments
            .iter_mut()
            .find(|segment| segment.get("type").and_then(|v| v.as_str()) == Some("text"));
        match text_segment {
            Some(segment) => {
                let current = segment
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                segment["text"] = serde_json::Value::String(current + delta_text);
            }
            None => {
                segments.push(serde_json::json!({ "type": "text", "text": delta_text }));
            }
        }
        let content_json =
            serde_json::to_string(&segments).map_err(|error| format!("序列化 content_json 失败：{error}"))?;
        let conn = self.conn();
        conn.execute(
            "UPDATE ai_messages SET content_json = ?1 WHERE id = ?2",
            params![content_json, id],
        )
        .map_err(err_map)?;
        Ok(())
    }
}

fn map_model_row(row: &rusqlite::Row) -> rusqlite::Result<ModelRow> {
    Ok(ModelRow {
        id: row.get(0)?,
        provider_id: row.get(1)?,
        model_id: row.get(2)?,
        display_name: row.get(3)?,
        capabilities: row.get(4)?,
        temperature: row.get(5)?,
        max_tokens: row.get(6)?,
        sort_order: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn map_session_row(row: &rusqlite::Row) -> rusqlite::Result<SessionRow> {
    Ok(SessionRow {
        id: row.get(0)?,
        title: row.get(1)?,
        system_prompt: row.get(2)?,
        current_model_id: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn map_message_row(row: &rusqlite::Row) -> rusqlite::Result<MessageRow> {
    Ok(MessageRow {
        id: row.get(0)?,
        session_id: row.get(1)?,
        role: row.get(2)?,
        content_json: row.get(3)?,
        model_id: row.get(4)?,
        status: row.get(5)?,
        error_message: row.get(6)?,
        prompt_tokens: row.get(7)?,
        completion_tokens: row.get(8)?,
        created_at: row.get(9)?,
        finished_at: row.get(10)?,
    })
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn err_map(error: rusqlite::Error) -> String {
    format!("ai 数据库错误：{error}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> i64 {
        1_700_000_000_000
    }

    fn provider(id: &str) -> ProviderRow {
        ProviderRow {
            id: id.into(),
            name: id.into(),
            kind: "openai".into(),
            base_url: "https://x/v1".into(),
            api_key: "sk".into(),
            extra_json: "{}".into(),
            sort_order: 0,
            created_at: now(),
            updated_at: now(),
        }
    }

    fn model(id: &str, provider_id: &str) -> ModelRow {
        ModelRow {
            id: id.into(),
            provider_id: provider_id.into(),
            model_id: "gpt-4o".into(),
            display_name: id.into(),
            capabilities: r#"["text"]"#.into(),
            temperature: None,
            max_tokens: None,
            sort_order: 0,
            created_at: now(),
            updated_at: now(),
        }
    }

    fn session(id: &str) -> SessionRow {
        SessionRow {
            id: id.into(),
            title: "t".into(),
            system_prompt: "".into(),
            current_model_id: None,
            created_at: now(),
            updated_at: now(),
        }
    }

    fn message(id: &str, session_id: &str, role: &str, content: &str) -> MessageRow {
        MessageRow {
            id: id.into(),
            session_id: session_id.into(),
            role: role.into(),
            content_json: format!(r#"[{{"type":"text","text":{:?}}}]"#, content),
            model_id: None,
            status: "done".into(),
            error_message: None,
            prompt_tokens: None,
            completion_tokens: None,
            created_at: now(),
            finished_at: Some(now()),
        }
    }

    #[test]
    fn provider_crud_roundtrip() {
        let s = AiStore::new_in_memory().unwrap();
        s.upsert_provider(provider("p1")).unwrap();
        assert_eq!(s.list_providers().unwrap().len(), 1);
        s.delete_provider("p1").unwrap();
        assert_eq!(s.list_providers().unwrap().len(), 0);
    }

    #[test]
    fn model_cascades_when_provider_deleted() {
        let s = AiStore::new_in_memory().unwrap();
        s.upsert_provider(provider("p1")).unwrap();
        s.upsert_model(model("m1", "p1")).unwrap();
        assert_eq!(s.list_models(Some("p1")).unwrap().len(), 1);
        s.delete_provider("p1").unwrap();
        assert_eq!(s.list_models(None).unwrap().len(), 0);
    }

    #[test]
    fn message_cascades_when_session_deleted() {
        let s = AiStore::new_in_memory().unwrap();
        s.create_session(session("s1")).unwrap();
        s.insert_message(message("m1", "s1", "user", "hi")).unwrap();
        assert_eq!(s.list_messages("s1").unwrap().len(), 1);
        s.delete_session("s1").unwrap();
        assert_eq!(s.list_messages("s1").unwrap().len(), 0);
    }

    #[test]
    fn session_current_model_set_null_on_model_delete() {
        let s = AiStore::new_in_memory().unwrap();
        s.upsert_provider(provider("p1")).unwrap();
        s.upsert_model(model("m1", "p1")).unwrap();
        let mut sess = session("s1");
        sess.current_model_id = Some("m1".into());
        s.create_session(sess).unwrap();
        s.delete_model("m1").unwrap();
        let got = s.get_session("s1").unwrap().unwrap();
        assert_eq!(got.current_model_id, None);
    }

    #[test]
    fn append_delta_updates_content_text_segment() {
        let s = AiStore::new_in_memory().unwrap();
        s.create_session(session("s1")).unwrap();
        let mut m = message("m1", "s1", "assistant", "");
        m.status = "streaming".into();
        s.insert_message(m).unwrap();
        s.append_message_delta("m1", "hello ").unwrap();
        s.append_message_delta("m1", "world").unwrap();
        let got = s.get_message("m1").unwrap().unwrap();
        assert!(got.content_json.contains("hello world"), "content = {}", got.content_json);
    }

    #[test]
    fn list_sessions_search_filters_by_title() {
        let s = AiStore::new_in_memory().unwrap();
        let mut a = session("s1");
        a.title = "foo".into();
        let mut b = session("s2");
        b.title = "bar".into();
        s.create_session(a).unwrap();
        s.create_session(b).unwrap();
        let hits = s.list_sessions(Some("fo")).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "s1");
    }

    #[test]
    fn migration_is_idempotent() {
        let s = AiStore::new_in_memory().unwrap();
        s.upsert_provider(provider("p1")).unwrap();
        // 再跑一次 DDL（模拟重启），已有数据保留
        s.conn.lock().unwrap().execute_batch(super::MIGRATION_SQL).unwrap();
        assert_eq!(s.list_providers().unwrap().len(), 1);
    }
}
