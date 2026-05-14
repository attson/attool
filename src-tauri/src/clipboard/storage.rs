use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::PathBuf,
};

use chrono::Local;
use rusqlite::{params, Connection, OptionalExtension};

use super::models::{ClipboardHistoryItem, ClipboardItemKind};

#[derive(Clone, Debug)]
pub struct ClipboardStore {
    root_dir: PathBuf,
    db_path: PathBuf,
}

impl ClipboardStore {
    pub fn new(root_dir: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(root_dir.join("assets"))
            .map_err(|error| format!("创建剪贴板目录失败：{error}"))?;
        let store = Self {
            db_path: root_dir.join("clipboard_history.sqlite3"),
            root_dir,
        };
        store.init_database()?;
        Ok(store)
    }

    #[allow(dead_code)]
    pub fn root_dir(&self) -> &PathBuf {
        &self.root_dir
    }

    pub fn init_database(&self) -> Result<(), String> {
        let connection = Connection::open(&self.db_path)
            .map_err(|error| format!("打开剪贴板数据库失败：{error}"))?;
        connection
            .execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS clipboard_items (
                    id TEXT PRIMARY KEY,
                    kind TEXT NOT NULL,
                    preview TEXT NOT NULL,
                    content_text TEXT NOT NULL,
                    asset_path TEXT,
                    file_paths_json TEXT NOT NULL,
                    content_hash TEXT NOT NULL UNIQUE,
                    is_pinned INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL,
                    last_copied_at TEXT
                );

                CREATE INDEX IF NOT EXISTS idx_clipboard_items_created_at
                    ON clipboard_items(created_at DESC);

                CREATE INDEX IF NOT EXISTS idx_clipboard_items_kind
                    ON clipboard_items(kind);
                "#,
            )
            .map_err(|error| format!("初始化剪贴板数据库失败：{error}"))?;
        Ok(())
    }

    pub fn insert_text(&self, value: &str) -> Result<Option<ClipboardHistoryItem>, String> {
        let text = value.trim();
        if text.is_empty() {
            return Ok(None);
        }
        self.insert_item(
            ClipboardItemKind::Text,
            preview(text),
            text.to_string(),
            None,
            Vec::new(),
        )
    }

    pub fn insert_files(
        &self,
        file_paths: Vec<String>,
    ) -> Result<Option<ClipboardHistoryItem>, String> {
        let paths: Vec<String> = file_paths
            .into_iter()
            .map(|path| path.trim().to_string())
            .filter(|path| !path.is_empty())
            .collect();
        if paths.is_empty() {
            return Ok(None);
        }
        let content_text = paths.join("\n");
        let label = if paths.len() == 1 {
            "1 file".to_string()
        } else {
            format!("{} files", paths.len())
        };
        self.insert_item(ClipboardItemKind::Files, label, content_text, None, paths)
    }

    pub fn insert_image_bytes(
        &self,
        mime_type: &str,
        bytes: &[u8],
        width: u32,
        height: u32,
    ) -> Result<Option<ClipboardHistoryItem>, String> {
        if bytes.is_empty() {
            return Ok(None);
        }
        let hash = content_hash(&format!(
            "image:{mime_type}:{width}:{height}:{}",
            hash_bytes(bytes)
        ));
        if self.find_by_hash(&hash)?.is_some() {
            return Ok(None);
        }
        let id = format!("clip-{}", uuid::Uuid::new_v4().simple());
        let extension = if mime_type == "image/jpeg" {
            "jpg"
        } else {
            "png"
        };
        let asset_path = self
            .root_dir
            .join("assets")
            .join(format!("{id}.{extension}"));
        fs::write(&asset_path, bytes).map_err(|error| format!("保存剪贴板图片失败：{error}"))?;
        self.insert_item_with_hash(
            id,
            ClipboardItemKind::Image,
            format!("Image {width} x {height}"),
            String::new(),
            Some(asset_path.to_string_lossy().into_owned()),
            Vec::new(),
            hash,
        )
    }

    pub fn list_items(
        &self,
        kind: Option<ClipboardItemKind>,
        query: Option<&str>,
    ) -> Result<Vec<ClipboardHistoryItem>, String> {
        let connection = Connection::open(&self.db_path)
            .map_err(|error| format!("打开剪贴板数据库失败：{error}"))?;
        let query = query.unwrap_or("").trim().to_lowercase();
        let mut statement = connection
            .prepare(
                r#"
                SELECT id, kind, preview, content_text, asset_path, file_paths_json, is_pinned, created_at, last_copied_at
                FROM clipboard_items
                ORDER BY is_pinned DESC, created_at DESC, id DESC
                "#,
            )
            .map_err(|error| format!("读取剪贴板历史失败：{error}"))?;
        let rows = statement
            .query_map([], row_to_item)
            .map_err(|error| format!("读取剪贴板历史失败：{error}"))?;
        let mut items = Vec::new();
        for row in rows {
            let item = row.map_err(|error| format!("读取剪贴板历史失败：{error}"))?;
            if let Some(expected) = kind {
                if item.kind != expected {
                    continue;
                }
            }
            if !query.is_empty() {
                let haystack = format!(
                    "{}\n{}\n{}",
                    item.preview,
                    item.content_text,
                    item.file_paths.join("\n")
                )
                .to_lowercase();
                if !haystack.contains(&query) {
                    continue;
                }
            }
            items.push(item);
        }
        Ok(items)
    }

    pub fn set_pinned(&self, id: &str, is_pinned: bool) -> Result<(), String> {
        let connection = Connection::open(&self.db_path)
            .map_err(|error| format!("打开剪贴板数据库失败：{error}"))?;
        connection
            .execute(
                "UPDATE clipboard_items SET is_pinned = ?2 WHERE id = ?1",
                params![id, if is_pinned { 1 } else { 0 }],
            )
            .map_err(|error| format!("更新剪贴板收藏失败：{error}"))?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn touch_copied(&self, id: &str) -> Result<(), String> {
        let now = Local::now().to_rfc3339();
        let connection = Connection::open(&self.db_path)
            .map_err(|error| format!("打开剪贴板数据库失败：{error}"))?;
        connection
            .execute(
                "UPDATE clipboard_items SET last_copied_at = ?2 WHERE id = ?1",
                params![id, now],
            )
            .map_err(|error| format!("更新剪贴板使用时间失败：{error}"))?;
        Ok(())
    }

    pub fn delete_item(&self, id: &str) -> Result<(), String> {
        let connection = Connection::open(&self.db_path)
            .map_err(|error| format!("打开剪贴板数据库失败：{error}"))?;
        let asset_path: Option<String> = connection
            .query_row(
                "SELECT asset_path FROM clipboard_items WHERE id = ?1",
                params![id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map_err(|error| format!("读取剪贴板资源失败：{error}"))?
            .flatten();
        connection
            .execute("DELETE FROM clipboard_items WHERE id = ?1", params![id])
            .map_err(|error| format!("删除剪贴板历史失败：{error}"))?;
        if let Some(path) = asset_path {
            let _ = fs::remove_file(path);
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn clear_unpinned(&self) -> Result<(), String> {
        let items = self.list_items(None, None)?;
        for item in items.into_iter().filter(|item| !item.is_pinned) {
            self.delete_item(&item.id)?;
        }
        Ok(())
    }

    pub fn enforce_retention(&self, limit: usize) -> Result<(), String> {
        let items = self.list_items(None, None)?;
        let pinned_count = items.iter().filter(|item| item.is_pinned).count();
        let unpinned_limit = limit.saturating_sub(pinned_count);
        let unpinned: Vec<_> = items.into_iter().filter(|item| !item.is_pinned).collect();
        if unpinned.len() <= unpinned_limit {
            return Ok(());
        }
        for item in unpinned.into_iter().skip(unpinned_limit) {
            self.delete_item(&item.id)?;
        }
        Ok(())
    }

    fn find_by_hash(&self, hash: &str) -> Result<Option<String>, String> {
        let connection = Connection::open(&self.db_path)
            .map_err(|error| format!("打开剪贴板数据库失败：{error}"))?;
        connection
            .query_row(
                "SELECT id FROM clipboard_items WHERE content_hash = ?1",
                params![hash],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| format!("读取剪贴板历史失败：{error}"))
    }

    fn insert_item(
        &self,
        kind: ClipboardItemKind,
        preview: String,
        content_text: String,
        asset_path: Option<String>,
        file_paths: Vec<String>,
    ) -> Result<Option<ClipboardHistoryItem>, String> {
        let hash = content_hash(&format!(
            "{}:{}:{}",
            kind.as_str(),
            content_text,
            file_paths.join("\n")
        ));
        if self.find_by_hash(&hash)?.is_some() {
            return Ok(None);
        }
        self.insert_item_with_hash(
            format!("clip-{}", uuid::Uuid::new_v4().simple()),
            kind,
            preview,
            content_text,
            asset_path,
            file_paths,
            hash,
        )
    }

    fn insert_item_with_hash(
        &self,
        id: String,
        kind: ClipboardItemKind,
        preview: String,
        content_text: String,
        asset_path: Option<String>,
        file_paths: Vec<String>,
        hash: String,
    ) -> Result<Option<ClipboardHistoryItem>, String> {
        let created_at = Local::now().to_rfc3339();
        let file_paths_json = serde_json::to_string(&file_paths)
            .map_err(|error| format!("序列化文件路径失败：{error}"))?;
        let connection = Connection::open(&self.db_path)
            .map_err(|error| format!("打开剪贴板数据库失败：{error}"))?;
        connection
            .execute(
                r#"
                INSERT INTO clipboard_items (id, kind, preview, content_text, asset_path, file_paths_json, content_hash, is_pinned, created_at, last_copied_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, NULL)
                "#,
                params![id, kind.as_str(), preview, content_text, asset_path, file_paths_json, hash, created_at],
            )
            .map_err(|error| format!("保存剪贴板历史失败：{error}"))?;
        Ok(Some(ClipboardHistoryItem {
            id,
            kind,
            preview,
            content_text,
            asset_path: asset_path.clone(),
            asset_url: asset_path.map(|path| format!("asset://localhost/{path}")),
            file_paths,
            is_pinned: false,
            created_at,
            last_copied_at: None,
        }))
    }
}

fn row_to_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<ClipboardHistoryItem> {
    let kind_string: String = row.get(1)?;
    let file_paths_json: String = row.get(5)?;
    let asset_path: Option<String> = row.get(4)?;
    Ok(ClipboardHistoryItem {
        id: row.get(0)?,
        kind: ClipboardItemKind::from_db(&kind_string)
            .map_err(rusqlite::Error::InvalidParameterName)?,
        preview: row.get(2)?,
        content_text: row.get(3)?,
        asset_url: asset_path
            .as_ref()
            .map(|path| format!("asset://localhost/{path}")),
        asset_path,
        file_paths: serde_json::from_str(&file_paths_json).unwrap_or_default(),
        is_pinned: row.get::<_, i64>(6)? == 1,
        created_at: row.get(7)?,
        last_copied_at: row.get(8)?,
    })
}

fn preview(value: &str) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= 120 {
        return normalized;
    }
    format!("{}...", normalized.chars().take(117).collect::<String>())
}

fn content_hash(value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_store() -> ClipboardStore {
        let dir = std::env::temp_dir().join(format!(
            "attool-clipboard-test-{}",
            uuid::Uuid::new_v4().simple()
        ));
        ClipboardStore::new(dir).expect("store")
    }

    #[test]
    fn stores_lists_and_searches_items() {
        let store = temp_store();
        store
            .insert_text("hello rust clipboard")
            .expect("insert text");
        store
            .insert_files(vec![
                "/Users/attson/a.png".to_string(),
                "/Users/attson/b.psd".to_string(),
            ])
            .expect("insert files");

        let all = store.list_items(None, None).expect("list all");
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].kind, ClipboardItemKind::Files);

        let text = store
            .list_items(Some(ClipboardItemKind::Text), Some("rust"))
            .expect("search text");
        assert_eq!(text.len(), 1);
        assert_eq!(text[0].preview, "hello rust clipboard");
    }

    #[test]
    fn deduplicates_by_content_hash() {
        let store = temp_store();
        assert!(store.insert_text("same").expect("first").is_some());
        assert!(store.insert_text("same").expect("duplicate").is_none());
        assert_eq!(store.list_items(None, None).expect("list").len(), 1);
    }

    #[test]
    fn retention_keeps_pinned_items() {
        let store = temp_store();
        let pinned = store
            .insert_text("keep me")
            .expect("insert pinned")
            .expect("pinned item");
        store.set_pinned(&pinned.id, true).expect("pin");
        store.insert_text("drop 1").expect("insert");
        store.insert_text("drop 2").expect("insert");
        store.enforce_retention(2).expect("retention");

        let items = store.list_items(None, None).expect("list");
        assert_eq!(items.len(), 2);
        assert!(items
            .iter()
            .any(|item| item.id == pinned.id && item.is_pinned));
    }

    #[test]
    fn deleting_image_removes_asset_file() {
        let store = temp_store();
        let item = store
            .insert_image_bytes("image/png", &[1, 2, 3, 4], 10, 10)
            .expect("insert image")
            .expect("image item");
        let asset_path = item.asset_path.clone().expect("asset path");
        assert!(fs::metadata(&asset_path).is_ok());

        store.delete_item(&item.id).expect("delete");
        assert!(fs::metadata(&asset_path).is_err());
    }
}
