use std::{fs, path::PathBuf, sync::Mutex};

use rusqlite::{params, Connection};

use super::models::{
    HttpCollectionFolderRow, HttpCollectionRequestRow, HttpCollectionRow, HttpEnvRow,
    HttpEnvVarRow, HttpHistoryRow, HttpTabRow,
};

const HISTORY_MAX_ROWS: i64 = 500;
const HISTORY_MAX_AGE_MS: i64 = 30 * 24 * 60 * 60 * 1000;

pub struct HttpStore {
    conn: Mutex<Connection>,
}

impl HttpStore {
    pub fn new(root_dir: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&root_dir).map_err(|error| format!("创建 http 目录失败：{error}"))?;
        let db_path = root_dir.join("http.sqlite3");
        let connection =
            Connection::open(&db_path).map_err(|error| format!("打开 http 数据库失败：{error}"))?;
        connection
            .execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS http_tabs (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    order_index INTEGER NOT NULL,
                    is_active INTEGER NOT NULL DEFAULT 0,
                    spec_json TEXT NOT NULL,
                    updated_at INTEGER NOT NULL,
                    kind TEXT NOT NULL DEFAULT 'http'
                );

                CREATE TABLE IF NOT EXISTS http_history (
                    id TEXT PRIMARY KEY,
                    method TEXT NOT NULL,
                    url TEXT NOT NULL,
                    status INTEGER,
                    elapsed_ms INTEGER,
                    body_bytes INTEGER,
                    spec_json TEXT NOT NULL,
                    resp_summary TEXT,
                    created_at INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_http_history_created
                    ON http_history(created_at DESC);

                CREATE TABLE IF NOT EXISTS http_envs (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    is_active INTEGER NOT NULL DEFAULT 0,
                    order_index INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );

                CREATE TABLE IF NOT EXISTS http_env_vars (
                    id TEXT PRIMARY KEY,
                    env_id TEXT NOT NULL,
                    key TEXT NOT NULL,
                    value TEXT NOT NULL,
                    enabled INTEGER NOT NULL DEFAULT 1,
                    order_index INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_http_env_vars_env
                    ON http_env_vars(env_id);

                CREATE TABLE IF NOT EXISTS http_collections (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    order_index INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );

                CREATE TABLE IF NOT EXISTS http_collection_folders (
                    id TEXT PRIMARY KEY,
                    collection_id TEXT NOT NULL,
                    parent_id TEXT,
                    name TEXT NOT NULL,
                    order_index INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_http_collection_folders_collection
                    ON http_collection_folders(collection_id);

                CREATE TABLE IF NOT EXISTS http_collection_requests (
                    id TEXT PRIMARY KEY,
                    collection_id TEXT NOT NULL,
                    folder_id TEXT,
                    name TEXT NOT NULL,
                    method TEXT NOT NULL,
                    spec_json TEXT NOT NULL,
                    order_index INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_http_collection_requests_collection
                    ON http_collection_requests(collection_id);
                CREATE INDEX IF NOT EXISTS idx_http_collection_requests_folder
                    ON http_collection_requests(folder_id);
                "#,
            )
            .map_err(|error| format!("初始化 http 数据库失败：{error}"))?;
        // 兼容旧库：老 schema 没有 kind 列，动态补上
        let has_kind = {
            let mut stmt = connection
                .prepare("PRAGMA table_info(http_tabs)")
                .map_err(|error| format!("检查 http_tabs schema 失败：{error}"))?;
            let mut found = false;
            let mut rows = stmt
                .query([])
                .map_err(|error| format!("检查 http_tabs schema 失败：{error}"))?;
            while let Some(row) = rows
                .next()
                .map_err(|error| format!("检查 http_tabs schema 失败：{error}"))?
            {
                let name: String = row
                    .get(1)
                    .map_err(|error| format!("读取列名失败：{error}"))?;
                if name == "kind" {
                    found = true;
                    break;
                }
            }
            found
        };
        if !has_kind {
            connection
                .execute(
                    "ALTER TABLE http_tabs ADD COLUMN kind TEXT NOT NULL DEFAULT 'http'",
                    [],
                )
                .map_err(|error| format!("迁移 http_tabs.kind 失败：{error}"))?;
        }
        Ok(Self {
            conn: Mutex::new(connection),
        })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("http store mutex poisoned")
    }

    // ---- tabs ----

    pub fn list_tabs(&self) -> Result<Vec<HttpTabRow>, String> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, title, order_index, is_active, spec_json, updated_at, kind \
                 FROM http_tabs ORDER BY order_index ASC, updated_at ASC",
            )
            .map_err(err_map)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(HttpTabRow {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    order_index: row.get(2)?,
                    is_active: row.get::<_, i64>(3)? != 0,
                    spec_json: row.get(4)?,
                    updated_at: row.get(5)?,
                    kind: row.get(6)?,
                })
            })
            .map_err(err_map)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err_map)?;
        Ok(rows)
    }

    pub fn upsert_tab(&self, row: HttpTabRow) -> Result<(), String> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO http_tabs (id, title, order_index, is_active, spec_json, updated_at, kind) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
             ON CONFLICT(id) DO UPDATE SET \
               title=excluded.title, order_index=excluded.order_index, \
               is_active=excluded.is_active, spec_json=excluded.spec_json, \
               updated_at=excluded.updated_at, kind=excluded.kind",
            params![
                row.id,
                row.title,
                row.order_index,
                row.is_active as i64,
                row.spec_json,
                row.updated_at,
                row.kind,
            ],
        )
        .map_err(err_map)?;
        Ok(())
    }

    pub fn delete_tab(&self, id: &str) -> Result<(), String> {
        let conn = self.conn();
        conn.execute("DELETE FROM http_tabs WHERE id = ?1", params![id])
            .map_err(err_map)?;
        Ok(())
    }

    pub fn set_active_tab(&self, id: &str) -> Result<(), String> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(err_map)?;
        tx.execute("UPDATE http_tabs SET is_active = 0 WHERE is_active = 1", [])
            .map_err(err_map)?;
        tx.execute(
            "UPDATE http_tabs SET is_active = 1 WHERE id = ?1",
            params![id],
        )
        .map_err(err_map)?;
        tx.commit().map_err(err_map)?;
        Ok(())
    }

    // ---- history ----

    pub fn list_history(&self, limit: u32) -> Result<Vec<HttpHistoryRow>, String> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, method, url, status, elapsed_ms, body_bytes, spec_json, \
                        resp_summary, created_at \
                 FROM http_history ORDER BY created_at DESC LIMIT ?1",
            )
            .map_err(err_map)?;
        let rows = stmt
            .query_map([limit as i64], |row| {
                Ok(HttpHistoryRow {
                    id: row.get(0)?,
                    method: row.get(1)?,
                    url: row.get(2)?,
                    status: row.get(3)?,
                    elapsed_ms: row.get(4)?,
                    body_bytes: row.get(5)?,
                    spec_json: row.get(6)?,
                    resp_summary: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })
            .map_err(err_map)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err_map)?;
        Ok(rows)
    }

    pub fn insert_history(&self, row: HttpHistoryRow) -> Result<(), String> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO http_history (id, method, url, status, elapsed_ms, body_bytes, \
                                       spec_json, resp_summary, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                row.id,
                row.method,
                row.url,
                row.status,
                row.elapsed_ms,
                row.body_bytes,
                row.spec_json,
                row.resp_summary,
                row.created_at,
            ],
        )
        .map_err(err_map)?;
        Ok(())
    }

    pub fn delete_history(&self, id: &str) -> Result<(), String> {
        let conn = self.conn();
        conn.execute("DELETE FROM http_history WHERE id = ?1", params![id])
            .map_err(err_map)?;
        Ok(())
    }

    pub fn clear_history(&self) -> Result<(), String> {
        let conn = self.conn();
        conn.execute("DELETE FROM http_history", [])
            .map_err(err_map)?;
        Ok(())
    }

    pub fn cleanup_history(&self, now_ms: i64) -> Result<(), String> {
        let cutoff = now_ms - HISTORY_MAX_AGE_MS;
        let conn = self.conn();
        conn.execute(
            "DELETE FROM http_history WHERE created_at < ?1",
            params![cutoff],
        )
        .map_err(err_map)?;
        conn.execute(
            "DELETE FROM http_history WHERE id NOT IN \
             (SELECT id FROM http_history ORDER BY created_at DESC LIMIT ?1)",
            params![HISTORY_MAX_ROWS],
        )
        .map_err(err_map)?;
        Ok(())
    }

    // ---- envs ----

    pub fn list_envs(&self) -> Result<Vec<HttpEnvRow>, String> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, is_active, order_index, updated_at FROM http_envs \
                 ORDER BY order_index ASC, updated_at ASC",
            )
            .map_err(err_map)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(HttpEnvRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    is_active: row.get::<_, i64>(2)? != 0,
                    order_index: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })
            .map_err(err_map)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err_map)?;
        Ok(rows)
    }

    pub fn upsert_env(&self, row: HttpEnvRow) -> Result<(), String> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO http_envs (id, name, is_active, order_index, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT(id) DO UPDATE SET \
               name=excluded.name, is_active=excluded.is_active, \
               order_index=excluded.order_index, updated_at=excluded.updated_at",
            params![
                row.id,
                row.name,
                row.is_active as i64,
                row.order_index,
                row.updated_at,
            ],
        )
        .map_err(err_map)?;
        Ok(())
    }

    pub fn delete_env(&self, id: &str) -> Result<(), String> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(err_map)?;
        tx.execute("DELETE FROM http_env_vars WHERE env_id = ?1", params![id])
            .map_err(err_map)?;
        tx.execute("DELETE FROM http_envs WHERE id = ?1", params![id])
            .map_err(err_map)?;
        tx.commit().map_err(err_map)?;
        Ok(())
    }

    pub fn set_active_env(&self, id: &str) -> Result<(), String> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(err_map)?;
        tx.execute("UPDATE http_envs SET is_active = 0 WHERE is_active = 1", [])
            .map_err(err_map)?;
        tx.execute(
            "UPDATE http_envs SET is_active = 1 WHERE id = ?1",
            params![id],
        )
        .map_err(err_map)?;
        tx.commit().map_err(err_map)?;
        Ok(())
    }

    // ---- env vars ----

    pub fn list_env_vars(&self, env_id: &str) -> Result<Vec<HttpEnvVarRow>, String> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, env_id, key, value, enabled, order_index, updated_at \
                 FROM http_env_vars WHERE env_id = ?1 \
                 ORDER BY order_index ASC, updated_at ASC",
            )
            .map_err(err_map)?;
        let rows = stmt
            .query_map([env_id], |row| {
                Ok(HttpEnvVarRow {
                    id: row.get(0)?,
                    env_id: row.get(1)?,
                    key: row.get(2)?,
                    value: row.get(3)?,
                    enabled: row.get::<_, i64>(4)? != 0,
                    order_index: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })
            .map_err(err_map)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err_map)?;
        Ok(rows)
    }

    pub fn upsert_env_var(&self, row: HttpEnvVarRow) -> Result<(), String> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO http_env_vars (id, env_id, key, value, enabled, order_index, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
             ON CONFLICT(id) DO UPDATE SET \
               env_id=excluded.env_id, key=excluded.key, value=excluded.value, \
               enabled=excluded.enabled, order_index=excluded.order_index, \
               updated_at=excluded.updated_at",
            params![
                row.id,
                row.env_id,
                row.key,
                row.value,
                row.enabled as i64,
                row.order_index,
                row.updated_at,
            ],
        )
        .map_err(err_map)?;
        Ok(())
    }

    pub fn delete_env_var(&self, id: &str) -> Result<(), String> {
        let conn = self.conn();
        conn.execute("DELETE FROM http_env_vars WHERE id = ?1", params![id])
            .map_err(err_map)?;
        Ok(())
    }

    // ---- collections ----

    pub fn list_collections(&self) -> Result<Vec<HttpCollectionRow>, String> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, order_index, updated_at FROM http_collections \
                 ORDER BY order_index ASC, updated_at ASC",
            )
            .map_err(err_map)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(HttpCollectionRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    order_index: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            })
            .map_err(err_map)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err_map)?;
        Ok(rows)
    }

    pub fn list_collection_folders(&self) -> Result<Vec<HttpCollectionFolderRow>, String> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, collection_id, parent_id, name, order_index, updated_at \
                 FROM http_collection_folders ORDER BY collection_id ASC, order_index ASC, updated_at ASC",
            )
            .map_err(err_map)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(HttpCollectionFolderRow {
                    id: row.get(0)?,
                    collection_id: row.get(1)?,
                    parent_id: row.get(2)?,
                    name: row.get(3)?,
                    order_index: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })
            .map_err(err_map)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err_map)?;
        Ok(rows)
    }

    pub fn list_collection_requests(&self) -> Result<Vec<HttpCollectionRequestRow>, String> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, collection_id, folder_id, name, method, spec_json, order_index, updated_at \
                 FROM http_collection_requests ORDER BY collection_id ASC, folder_id ASC, order_index ASC, updated_at ASC",
            )
            .map_err(err_map)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(HttpCollectionRequestRow {
                    id: row.get(0)?,
                    collection_id: row.get(1)?,
                    folder_id: row.get(2)?,
                    name: row.get(3)?,
                    method: row.get(4)?,
                    spec_json: row.get(5)?,
                    order_index: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            })
            .map_err(err_map)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err_map)?;
        Ok(rows)
    }

    pub fn upsert_collection(&self, row: HttpCollectionRow) -> Result<(), String> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO http_collections (id, name, order_index, updated_at) VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(id) DO UPDATE SET name=excluded.name, order_index=excluded.order_index, updated_at=excluded.updated_at",
            params![row.id, row.name, row.order_index, row.updated_at],
        )
        .map_err(err_map)?;
        Ok(())
    }

    pub fn upsert_collection_folder(&self, row: HttpCollectionFolderRow) -> Result<(), String> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO http_collection_folders (id, collection_id, parent_id, name, order_index, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
             ON CONFLICT(id) DO UPDATE SET collection_id=excluded.collection_id, parent_id=excluded.parent_id, \
               name=excluded.name, order_index=excluded.order_index, updated_at=excluded.updated_at",
            params![
                row.id,
                row.collection_id,
                row.parent_id,
                row.name,
                row.order_index,
                row.updated_at,
            ],
        )
        .map_err(err_map)?;
        Ok(())
    }

    pub fn upsert_collection_request(&self, row: HttpCollectionRequestRow) -> Result<(), String> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO http_collection_requests (id, collection_id, folder_id, name, method, spec_json, order_index, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
             ON CONFLICT(id) DO UPDATE SET collection_id=excluded.collection_id, folder_id=excluded.folder_id, \
               name=excluded.name, method=excluded.method, spec_json=excluded.spec_json, \
               order_index=excluded.order_index, updated_at=excluded.updated_at",
            params![
                row.id,
                row.collection_id,
                row.folder_id,
                row.name,
                row.method,
                row.spec_json,
                row.order_index,
                row.updated_at,
            ],
        )
        .map_err(err_map)?;
        Ok(())
    }

    pub fn delete_collection(&self, id: &str) -> Result<(), String> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(err_map)?;
        tx.execute(
            "DELETE FROM http_collection_requests WHERE collection_id = ?1",
            params![id],
        )
        .map_err(err_map)?;
        tx.execute(
            "DELETE FROM http_collection_folders WHERE collection_id = ?1",
            params![id],
        )
        .map_err(err_map)?;
        tx.execute("DELETE FROM http_collections WHERE id = ?1", params![id])
            .map_err(err_map)?;
        tx.commit().map_err(err_map)?;
        Ok(())
    }

    pub fn delete_collection_request(&self, id: &str) -> Result<(), String> {
        let conn = self.conn();
        conn.execute(
            "DELETE FROM http_collection_requests WHERE id = ?1",
            params![id],
        )
        .map_err(err_map)?;
        Ok(())
    }
}

fn err_map(error: rusqlite::Error) -> String {
    format!("http 数据库错误：{error}")
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::http::models::{HttpCollectionFolderRow, HttpCollectionRequestRow, HttpCollectionRow};

    fn make_store() -> HttpStore {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("attool-http-store-test-{stamp}"));
        HttpStore::new(dir).expect("store")
    }

    #[test]
    fn collection_rows_roundtrip_and_delete_cascade() {
        let store = make_store();

        store
            .upsert_collection(HttpCollectionRow {
                id: "c1".into(),
                name: "Admin API".into(),
                order_index: 0,
                updated_at: 1,
            })
            .expect("upsert collection");
        store
            .upsert_collection_folder(HttpCollectionFolderRow {
                id: "f1".into(),
                collection_id: "c1".into(),
                parent_id: None,
                name: "users".into(),
                order_index: 0,
                updated_at: 1,
            })
            .expect("upsert folder");
        store
            .upsert_collection_request(HttpCollectionRequestRow {
                id: "r1".into(),
                collection_id: "c1".into(),
                folder_id: Some("f1".into()),
                name: "GET users".into(),
                method: "GET".into(),
                spec_json: r#"{"method":"GET","url":"/users"}"#.into(),
                order_index: 0,
                updated_at: 1,
            })
            .expect("upsert request");

        assert_eq!(store.list_collections().expect("collections").len(), 1);
        assert_eq!(store.list_collection_folders().expect("folders").len(), 1);
        assert_eq!(store.list_collection_requests().expect("requests").len(), 1);

        store.delete_collection("c1").expect("delete collection");
        assert!(store.list_collections().expect("collections").is_empty());
        assert!(store.list_collection_folders().expect("folders").is_empty());
        assert!(store.list_collection_requests().expect("requests").is_empty());
    }
}
