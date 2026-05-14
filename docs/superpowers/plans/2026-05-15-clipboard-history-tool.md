# Clipboard History Tool Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Paste-style clipboard history tool with a global shortcut panel for text, images, and file paths.

**Architecture:** Add a focused clipboard backend module that owns SQLite storage, clipboard normalization, watcher lifecycle, and Tauri commands. Add a focused Vue clipboard feature module that renders both the in-app tool and the shortcut history window while keeping `src/App.vue` as a route shell.

**Tech Stack:** Tauri 2, Rust 2021, rusqlite, tauri-plugin-clipboard-manager, tauri-plugin-global-shortcut, Vue 3 `<script setup>`, Naive UI, Vitest 4.

---

## File Map

Create backend files:

- `src-tauri/src/clipboard/mod.rs` — module wiring and public exports.
- `src-tauri/src/clipboard/models.rs` — serializable models shared by commands and tests.
- `src-tauri/src/clipboard/storage.rs` — SQLite schema, CRUD, search, dedupe, retention, asset cleanup.
- `src-tauri/src/clipboard/watcher.rs` — clipboard polling loop, normalization, shortcut setup, event emission.
- `src-tauri/src/clipboard/commands.rs` — Tauri command boundary.

Modify backend/config files:

- `src-tauri/src/lib.rs` — register plugins, manage store, start watcher, expose commands.
- `src-tauri/Cargo.toml` — add official Tauri clipboard/global-shortcut plugins if missing.
- `src-tauri/capabilities/default.json` — add plugin permissions used by clipboard/global shortcut if frontend APIs are used.
- `src-tauri/tauri.conf.json` — add hidden `clipboard-history` window configuration.

Create frontend files:

- `src/types/clipboard.ts` — frontend item/filter/settings types.
- `src/utils/clipboardHistory.ts` — pure filter, preview, and type helpers.
- `src/utils/clipboardHistory.test.ts` — Vitest tests for helpers.
- `src/composables/useClipboardHistory.ts` — injected API wrapper and UI state.
- `src/composables/useClipboardHistory.test.ts` — Vitest tests with fake API.
- `src/components/clipboard/ClipboardItemCard.vue` — card renderer.
- `src/components/clipboard/ClipboardHistoryWindow.vue` — quick panel UI for global shortcut window.
- `src/components/clipboard/ClipboardTool.vue` — in-app settings/status/history view.

Modify frontend files:

- `src/App.vue` — make clipboard tool ready and render `ClipboardTool`; branch on window label for shortcut window.
- `src/styles/template-editor.css` or a new `src/styles/clipboard.css` — shared clipboard classes if scoped styles would duplicate.
- `src/main.ts` — import clipboard stylesheet if a new global stylesheet is added.

---

### Task 1: Add Pure Frontend Types And Helpers

**Files:**
- Create: `src/types/clipboard.ts`
- Create: `src/utils/clipboardHistory.ts`
- Create: `src/utils/clipboardHistory.test.ts`

- [ ] **Step 1: Write the failing helper tests**

Create `src/utils/clipboardHistory.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import type { ClipboardHistoryItem } from '../types/clipboard';
import { filterClipboardItems, formatClipboardPreview, isFilePathText } from './clipboardHistory';

const ITEMS: ClipboardHistoryItem[] = [
  {
    id: 'text-1',
    kind: 'text',
    preview: 'const total = 42;',
    contentText: 'const total = 42;',
    filePaths: [],
    assetUrl: null,
    isPinned: false,
    createdAt: '2026-05-15T10:00:00+08:00',
    lastCopiedAt: null,
  },
  {
    id: 'image-1',
    kind: 'image',
    preview: 'Image 1200 x 800',
    contentText: '',
    filePaths: [],
    assetUrl: 'asset://image-1.png',
    isPinned: true,
    createdAt: '2026-05-15T10:01:00+08:00',
    lastCopiedAt: null,
  },
  {
    id: 'files-1',
    kind: 'files',
    preview: '2 files',
    contentText: '/Users/a/logo.png\n/Users/a/spec.psd',
    filePaths: ['/Users/a/logo.png', '/Users/a/spec.psd'],
    assetUrl: null,
    isPinned: false,
    createdAt: '2026-05-15T10:02:00+08:00',
    lastCopiedAt: null,
  },
];

describe('clipboardHistory helpers', () => {
  it('filters by type and text query', () => {
    expect(filterClipboardItems(ITEMS, { kind: 'all', query: 'total' }).map((item) => item.id)).toEqual(['text-1']);
    expect(filterClipboardItems(ITEMS, { kind: 'image', query: '' }).map((item) => item.id)).toEqual(['image-1']);
    expect(filterClipboardItems(ITEMS, { kind: 'files', query: 'spec' }).map((item) => item.id)).toEqual(['files-1']);
  });

  it('formats concise previews without losing useful content', () => {
    expect(formatClipboardPreview('  hello\n\nworld  ', 20)).toBe('hello world');
    expect(formatClipboardPreview('a'.repeat(80), 12)).toBe('aaaaaaaaa...');
  });

  it('detects newline separated local file paths', () => {
    expect(isFilePathText('/Users/attson/a.png\n/Users/attson/b.psd')).toBe(true);
    expect(isFilePathText('https://example.com/a.png')).toBe(false);
    expect(isFilePathText('plain copied text')).toBe(false);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm test -- src/utils/clipboardHistory.test.ts`

Expected: FAIL because `src/types/clipboard.ts` and `src/utils/clipboardHistory.ts` do not exist.

- [ ] **Step 3: Add types**

Create `src/types/clipboard.ts`:

```ts
export type ClipboardItemKind = 'text' | 'image' | 'files';
export type ClipboardFilterKind = ClipboardItemKind | 'all';

export interface ClipboardHistoryItem {
  id: string;
  kind: ClipboardItemKind;
  preview: string;
  contentText: string;
  filePaths: string[];
  assetUrl: string | null;
  isPinned: boolean;
  createdAt: string;
  lastCopiedAt: string | null;
}

export interface ClipboardHistoryFilter {
  kind: ClipboardFilterKind;
  query: string;
}

export interface ClipboardHistorySettings {
  captureEnabled: boolean;
  retentionLimit: number;
  shortcut: string;
}
```

- [ ] **Step 4: Add helper implementation**

Create `src/utils/clipboardHistory.ts`:

```ts
import type { ClipboardHistoryFilter, ClipboardHistoryItem } from '../types/clipboard';

export function formatClipboardPreview(value: string, maxLength = 120): string {
  const normalized = value.replace(/\s+/g, ' ').trim();
  if (normalized.length <= maxLength) return normalized;
  return `${normalized.slice(0, Math.max(0, maxLength - 3))}...`;
}

export function isFilePathText(value: string): boolean {
  const lines = value
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
  if (lines.length === 0) return false;
  return lines.every((line) => line.startsWith('/') || line.startsWith('~/') || /^file:\/\//i.test(line) || /^[A-Za-z]:\\/.test(line));
}

export function filterClipboardItems(
  items: ClipboardHistoryItem[],
  filter: ClipboardHistoryFilter,
): ClipboardHistoryItem[] {
  const query = filter.query.trim().toLocaleLowerCase();
  return items.filter((item) => {
    if (filter.kind !== 'all' && item.kind !== filter.kind) return false;
    if (!query) return true;
    const haystack = [item.preview, item.contentText, ...item.filePaths].join('\n').toLocaleLowerCase();
    return haystack.includes(query);
  });
}
```

- [ ] **Step 5: Verify helper tests pass**

Run: `npm test -- src/utils/clipboardHistory.test.ts`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/types/clipboard.ts src/utils/clipboardHistory.ts src/utils/clipboardHistory.test.ts
git commit -m "feat(clipboard): add history helpers"
```

---

### Task 2: Add Clipboard Storage Backend

**Files:**
- Create: `src-tauri/src/clipboard/mod.rs`
- Create: `src-tauri/src/clipboard/models.rs`
- Create: `src-tauri/src/clipboard/storage.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write storage tests first**

Create `src-tauri/src/clipboard/storage.rs` with only test scaffolding and an empty module body if the file does not exist yet. Include these tests at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_store() -> ClipboardStore {
        let dir = std::env::temp_dir().join(format!("attool-clipboard-test-{}", uuid::Uuid::new_v4().simple()));
        ClipboardStore::new(dir).expect("store")
    }

    #[test]
    fn stores_lists_and_searches_items() {
        let store = temp_store();
        store.insert_text("hello rust clipboard").expect("insert text");
        store.insert_files(vec!["/Users/attson/a.png".to_string(), "/Users/attson/b.psd".to_string()]).expect("insert files");

        let all = store.list_items(None, None).expect("list all");
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].kind, ClipboardItemKind::Files);

        let text = store.list_items(Some(ClipboardItemKind::Text), Some("rust")).expect("search text");
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
        let pinned = store.insert_text("keep me").expect("insert pinned").expect("pinned item");
        store.set_pinned(&pinned.id, true).expect("pin");
        store.insert_text("drop 1").expect("insert");
        store.insert_text("drop 2").expect("insert");
        store.enforce_retention(2).expect("retention");

        let items = store.list_items(None, None).expect("list");
        assert_eq!(items.len(), 2);
        assert!(items.iter().any(|item| item.id == pinned.id && item.is_pinned));
    }

    #[test]
    fn deleting_image_removes_asset_file() {
        let store = temp_store();
        let item = store.insert_image_bytes("image/png", &[1, 2, 3, 4], 10, 10).expect("insert image").expect("image item");
        let asset_path = item.asset_path.clone().expect("asset path");
        assert!(fs::metadata(&asset_path).is_ok());

        store.delete_item(&item.id).expect("delete");
        assert!(fs::metadata(&asset_path).is_err());
    }
}
```

- [ ] **Step 2: Run Rust tests to verify they fail**

Run: `cargo test clipboard::storage --lib`

Expected: FAIL because `ClipboardStore`, `ClipboardItemKind`, and storage methods are not implemented.

- [ ] **Step 3: Add models**

Create `src-tauri/src/clipboard/models.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardHistoryItem {
    pub id: String,
    pub kind: ClipboardItemKind,
    pub preview: String,
    pub content_text: String,
    pub file_paths: Vec<String>,
    pub asset_path: Option<String>,
    pub asset_url: Option<String>,
    pub is_pinned: bool,
    pub created_at: String,
    pub last_copied_at: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ClipboardItemKind {
    Text,
    Image,
    Files,
}

impl ClipboardItemKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Image => "image",
            Self::Files => "files",
        }
    }

    pub fn from_db(value: &str) -> Result<Self, String> {
        match value {
            "text" => Ok(Self::Text),
            "image" => Ok(Self::Image),
            "files" => Ok(Self::Files),
            other => Err(format!("未知剪贴板类型：{other}")),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardHistorySettings {
    pub capture_enabled: bool,
    pub retention_limit: usize,
    pub shortcut: String,
}

impl Default for ClipboardHistorySettings {
    fn default() -> Self {
        Self {
            capture_enabled: true,
            retention_limit: 500,
            shortcut: "CommandOrControl+Shift+V".to_string(),
        }
    }
}
```

- [ ] **Step 4: Add module export**

Create `src-tauri/src/clipboard/mod.rs`:

```rust
pub mod models;
pub mod storage;
```

Modify the top of `src-tauri/src/lib.rs` and add:

```rust
mod clipboard;
```

- [ ] **Step 5: Add storage implementation**

Implement `src-tauri/src/clipboard/storage.rs`:

```rust
use std::{collections::hash_map::DefaultHasher, fs, hash::{Hash, Hasher}, path::PathBuf};

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
        fs::create_dir_all(root_dir.join("assets")).map_err(|error| format!("创建剪贴板目录失败：{error}"))?;
        let store = Self {
            db_path: root_dir.join("clipboard_history.sqlite3"),
            root_dir,
        };
        store.init_database()?;
        Ok(store)
    }

    pub fn root_dir(&self) -> &PathBuf {
        &self.root_dir
    }

    pub fn init_database(&self) -> Result<(), String> {
        let connection = Connection::open(&self.db_path).map_err(|error| format!("打开剪贴板数据库失败：{error}"))?;
        connection.execute_batch(
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
        ).map_err(|error| format!("初始化剪贴板数据库失败：{error}"))?;
        Ok(())
    }

    pub fn insert_text(&self, value: &str) -> Result<Option<ClipboardHistoryItem>, String> {
        let text = value.trim();
        if text.is_empty() {
            return Ok(None);
        }
        self.insert_item(ClipboardItemKind::Text, preview(text), text.to_string(), None, Vec::new())
    }

    pub fn insert_files(&self, file_paths: Vec<String>) -> Result<Option<ClipboardHistoryItem>, String> {
        let paths: Vec<String> = file_paths.into_iter().map(|path| path.trim().to_string()).filter(|path| !path.is_empty()).collect();
        if paths.is_empty() {
            return Ok(None);
        }
        let content_text = paths.join("\n");
        let label = if paths.len() == 1 { "1 file".to_string() } else { format!("{} files", paths.len()) };
        self.insert_item(ClipboardItemKind::Files, label, content_text, None, paths)
    }

    pub fn insert_image_bytes(&self, mime_type: &str, bytes: &[u8], width: u32, height: u32) -> Result<Option<ClipboardHistoryItem>, String> {
        if bytes.is_empty() {
            return Ok(None);
        }
        let hash = content_hash(&format!("image:{mime_type}:{width}:{height}:{}", hash_bytes(bytes)));
        if self.find_by_hash(&hash)?.is_some() {
            return Ok(None);
        }
        let id = format!("clip-{}", uuid::Uuid::new_v4().simple());
        let extension = if mime_type == "image/jpeg" { "jpg" } else { "png" };
        let asset_path = self.root_dir.join("assets").join(format!("{id}.{extension}"));
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

    pub fn list_items(&self, kind: Option<ClipboardItemKind>, query: Option<&str>) -> Result<Vec<ClipboardHistoryItem>, String> {
        let connection = Connection::open(&self.db_path).map_err(|error| format!("打开剪贴板数据库失败：{error}"))?;
        let query = query.unwrap_or("").trim().to_lowercase();
        let mut statement = connection.prepare(
            r#"
            SELECT id, kind, preview, content_text, asset_path, file_paths_json, is_pinned, created_at, last_copied_at
            FROM clipboard_items
            ORDER BY is_pinned DESC, datetime(created_at) DESC, id DESC
            "#,
        ).map_err(|error| format!("读取剪贴板历史失败：{error}"))?;
        let rows = statement.query_map([], row_to_item).map_err(|error| format!("读取剪贴板历史失败：{error}"))?;
        let mut items = Vec::new();
        for row in rows {
            let item = row.map_err(|error| format!("读取剪贴板历史失败：{error}"))?;
            if let Some(expected) = kind {
                if item.kind != expected {
                    continue;
                }
            }
            if !query.is_empty() {
                let haystack = format!("{}\n{}\n{}", item.preview, item.content_text, item.file_paths.join("\n")).to_lowercase();
                if !haystack.contains(&query) {
                    continue;
                }
            }
            items.push(item);
        }
        Ok(items)
    }

    pub fn set_pinned(&self, id: &str, is_pinned: bool) -> Result<(), String> {
        let connection = Connection::open(&self.db_path).map_err(|error| format!("打开剪贴板数据库失败：{error}"))?;
        connection.execute("UPDATE clipboard_items SET is_pinned = ?2 WHERE id = ?1", params![id, if is_pinned { 1 } else { 0 }])
            .map_err(|error| format!("更新剪贴板收藏失败：{error}"))?;
        Ok(())
    }

    pub fn touch_copied(&self, id: &str) -> Result<(), String> {
        let now = Local::now().to_rfc3339();
        let connection = Connection::open(&self.db_path).map_err(|error| format!("打开剪贴板数据库失败：{error}"))?;
        connection.execute("UPDATE clipboard_items SET last_copied_at = ?2 WHERE id = ?1", params![id, now])
            .map_err(|error| format!("更新剪贴板使用时间失败：{error}"))?;
        Ok(())
    }

    pub fn delete_item(&self, id: &str) -> Result<(), String> {
        let connection = Connection::open(&self.db_path).map_err(|error| format!("打开剪贴板数据库失败：{error}"))?;
        let asset_path: Option<String> = connection.query_row("SELECT asset_path FROM clipboard_items WHERE id = ?1", params![id], |row| row.get(0)).optional()
            .map_err(|error| format!("读取剪贴板资源失败：{error}"))?;
        connection.execute("DELETE FROM clipboard_items WHERE id = ?1", params![id]).map_err(|error| format!("删除剪贴板历史失败：{error}"))?;
        if let Some(path) = asset_path {
            let _ = fs::remove_file(path);
        }
        Ok(())
    }

    pub fn clear_unpinned(&self) -> Result<(), String> {
        let items = self.list_items(None, None)?;
        for item in items.into_iter().filter(|item| !item.is_pinned) {
            self.delete_item(&item.id)?;
        }
        Ok(())
    }

    pub fn enforce_retention(&self, limit: usize) -> Result<(), String> {
        let unpinned: Vec<_> = self.list_items(None, None)?.into_iter().filter(|item| !item.is_pinned).collect();
        if unpinned.len() <= limit {
            return Ok(());
        }
        for item in unpinned.into_iter().skip(limit) {
            self.delete_item(&item.id)?;
        }
        Ok(())
    }

    fn find_by_hash(&self, hash: &str) -> Result<Option<String>, String> {
        let connection = Connection::open(&self.db_path).map_err(|error| format!("打开剪贴板数据库失败：{error}"))?;
        connection.query_row("SELECT id FROM clipboard_items WHERE content_hash = ?1", params![hash], |row| row.get(0)).optional()
            .map_err(|error| format!("读取剪贴板历史失败：{error}"))
    }

    fn insert_item(&self, kind: ClipboardItemKind, preview: String, content_text: String, asset_path: Option<String>, file_paths: Vec<String>) -> Result<Option<ClipboardHistoryItem>, String> {
        let hash = content_hash(&format!("{}:{}:{}", kind.as_str(), content_text, file_paths.join("\n")));
        if self.find_by_hash(&hash)?.is_some() {
            return Ok(None);
        }
        self.insert_item_with_hash(format!("clip-{}", uuid::Uuid::new_v4().simple()), kind, preview, content_text, asset_path, file_paths, hash)
    }

    fn insert_item_with_hash(&self, id: String, kind: ClipboardItemKind, preview: String, content_text: String, asset_path: Option<String>, file_paths: Vec<String>, hash: String) -> Result<Option<ClipboardHistoryItem>, String> {
        let created_at = Local::now().to_rfc3339();
        let file_paths_json = serde_json::to_string(&file_paths).map_err(|error| format!("序列化文件路径失败：{error}"))?;
        let connection = Connection::open(&self.db_path).map_err(|error| format!("打开剪贴板数据库失败：{error}"))?;
        connection.execute(
            r#"
            INSERT INTO clipboard_items (id, kind, preview, content_text, asset_path, file_paths_json, content_hash, is_pinned, created_at, last_copied_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, NULL)
            "#,
            params![id, kind.as_str(), preview, content_text, asset_path, file_paths_json, hash, created_at],
        ).map_err(|error| format!("保存剪贴板历史失败：{error}"))?;
        Ok(Some(ClipboardHistoryItem { id, kind, preview, content_text, asset_path: asset_path.clone(), asset_url: asset_path.map(|path| format!("asset://localhost/{path}")), file_paths, is_pinned: false, created_at, last_copied_at: None }))
    }
}

fn row_to_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<ClipboardHistoryItem> {
    let kind_string: String = row.get(1)?;
    let file_paths_json: String = row.get(5)?;
    let asset_path: Option<String> = row.get(4)?;
    Ok(ClipboardHistoryItem {
        id: row.get(0)?,
        kind: ClipboardItemKind::from_db(&kind_string).map_err(|error| rusqlite::Error::InvalidParameterName(error))?,
        preview: row.get(2)?,
        content_text: row.get(3)?,
        asset_url: asset_path.as_ref().map(|path| format!("asset://localhost/{path}")),
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
```

- [ ] **Step 6: Verify storage tests pass**

Run: `cargo test clipboard::storage --lib`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/clipboard/mod.rs src-tauri/src/clipboard/models.rs src-tauri/src/clipboard/storage.rs
git commit -m "feat(clipboard): add history storage"
```

---

### Task 3: Add Clipboard Commands And Settings

**Files:**
- Create: `src-tauri/src/clipboard/commands.rs`
- Modify: `src-tauri/src/clipboard/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add command module export**

Modify `src-tauri/src/clipboard/mod.rs`:

```rust
pub mod commands;
pub mod models;
pub mod storage;
```

- [ ] **Step 2: Add Tauri commands**

Create `src-tauri/src/clipboard/commands.rs`:

```rust
use tauri::State;

use super::{
    models::{ClipboardHistoryItem, ClipboardHistorySettings, ClipboardItemKind},
    storage::ClipboardStore,
};

#[tauri::command]
pub async fn list_clipboard_items(
    kind: Option<ClipboardItemKind>,
    query: Option<String>,
    store: State<'_, ClipboardStore>,
) -> Result<Vec<ClipboardHistoryItem>, String> {
    store.list_items(kind, query.as_deref())
}

#[tauri::command]
pub async fn delete_clipboard_item(id: String, store: State<'_, ClipboardStore>) -> Result<(), String> {
    store.delete_item(&id)
}

#[tauri::command]
pub async fn set_clipboard_item_pinned(
    id: String,
    is_pinned: bool,
    store: State<'_, ClipboardStore>,
) -> Result<(), String> {
    store.set_pinned(&id, is_pinned)
}

#[tauri::command]
pub async fn clear_clipboard_history(store: State<'_, ClipboardStore>) -> Result<(), String> {
    store.clear_unpinned()
}

#[tauri::command]
pub async fn get_clipboard_settings() -> Result<ClipboardHistorySettings, String> {
    Ok(ClipboardHistorySettings::default())
}
```

- [ ] **Step 3: Register store and commands**

Modify `src-tauri/src/lib.rs` setup after ecommerce store registration:

```rust
let clipboard_dir = app
    .path()
    .app_data_dir()
    .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?
    .join("clipboard");
let clipboard_store = clipboard::storage::ClipboardStore::new(clipboard_dir)
    .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
app.manage(clipboard_store);
```

Add these entries to `tauri::generate_handler![...]`:

```rust
clipboard::commands::list_clipboard_items,
clipboard::commands::delete_clipboard_item,
clipboard::commands::set_clipboard_item_pinned,
clipboard::commands::clear_clipboard_history,
clipboard::commands::get_clipboard_settings
```

- [ ] **Step 4: Verify backend compiles**

Run: `cargo check`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/clipboard/mod.rs src-tauri/src/clipboard/commands.rs
git commit -m "feat(clipboard): expose history commands"
```

---

### Task 4: Add Clipboard Plugins And Watcher Skeleton

**Files:**
- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Modify: `src-tauri/capabilities/default.json`
- Create: `src-tauri/src/clipboard/watcher.rs`
- Modify: `src-tauri/src/clipboard/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add official plugin dependencies**

Run:

```bash
npm install @tauri-apps/plugin-global-shortcut @tauri-apps/plugin-clipboard-manager
cargo add tauri-plugin-global-shortcut@2 tauri-plugin-clipboard-manager@2
```

Expected: `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, and `src-tauri/Cargo.lock` update.

Official docs used for this choice: Tauri Clipboard plugin documents `@tauri-apps/plugin-clipboard-manager` and Rust `ClipboardExt`; Tauri Global Shortcut plugin documents `@tauri-apps/plugin-global-shortcut` and `register`.

- [ ] **Step 2: Add plugin permissions**

Modify `src-tauri/capabilities/default.json` permissions array and add:

```json
"clipboard-manager:allow-read-text",
"clipboard-manager:allow-write-text",
"clipboard-manager:allow-read-image",
"clipboard-manager:allow-write-image",
"global-shortcut:allow-register",
"global-shortcut:allow-unregister",
"global-shortcut:allow-is-registered"
```

- [ ] **Step 3: Add watcher module export**

Modify `src-tauri/src/clipboard/mod.rs`:

```rust
pub mod commands;
pub mod models;
pub mod storage;
pub mod watcher;
```

- [ ] **Step 4: Add watcher skeleton**

Create `src-tauri/src/clipboard/watcher.rs`:

```rust
use std::{path::Path, sync::{Arc, atomic::{AtomicBool, Ordering}}, thread, time::Duration};

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use super::storage::ClipboardStore;

const CLIPBOARD_EVENT: &str = "clipboard-history-updated";
const DEFAULT_SHORTCUT: &str = "CommandOrControl+Shift+V";

#[derive(Clone, Debug)]
pub struct ClipboardWatcherState {
    capture_enabled: Arc<AtomicBool>,
}

impl ClipboardWatcherState {
    pub fn new() -> Self {
        Self { capture_enabled: Arc::new(AtomicBool::new(true)) }
    }

    pub fn is_enabled(&self) -> bool {
        self.capture_enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.capture_enabled.store(enabled, Ordering::Relaxed);
    }
}

pub fn start_clipboard_watcher(app: AppHandle, store: ClipboardStore, state: ClipboardWatcherState) {
    thread::spawn(move || {
        let mut last_text = String::new();
        loop {
            if state.is_enabled() {
                if let Ok(text) = app.clipboard().read_text() {
                    if text != last_text {
                        last_text = text.clone();
                        let result = if looks_like_file_paths(&text) {
                            store.insert_files(parse_file_paths(&text))
                        } else {
                            store.insert_text(&text)
                        };
                        if matches!(result, Ok(Some(_))) {
                            let _ = app.emit(CLIPBOARD_EVENT, ());
                            let _ = store.enforce_retention(500);
                        }
                    }
                }
            }
            thread::sleep(Duration::from_millis(800));
        }
    });
}

pub fn register_clipboard_shortcut(app: &AppHandle) -> Result<(), String> {
    let shortcut: Shortcut = DEFAULT_SHORTCUT.parse().map_err(|error| format!("解析剪贴板快捷键失败：{error}"))?;
    let handle = app.clone();
    app.global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                if let Some(window) = handle.get_webview_window("clipboard-history") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = window.center();
                    let _ = handle.emit("clipboard-history-opened", ());
                }
            }
        })
        .map_err(|error| format!("注册剪贴板快捷键失败：{error}"))
}

fn looks_like_file_paths(value: &str) -> bool {
    let paths = parse_file_paths(value);
    !paths.is_empty() && paths.iter().all(|path| Path::new(path).is_absolute())
}

fn parse_file_paths(value: &str) -> Vec<String> {
    value
        .lines()
        .map(|line| line.trim().trim_start_matches("file://").to_string())
        .filter(|line| !line.is_empty())
        .collect()
}
```

- [ ] **Step 5: Register plugins and start watcher**

Modify the Tauri builder in `src-tauri/src/lib.rs`:

```rust
.plugin(tauri_plugin_clipboard_manager::init())
.plugin(tauri_plugin_global_shortcut::Builder::new().build())
```

In setup, create the watcher state before managing the store:

```rust
let clipboard_watcher_state = clipboard::watcher::ClipboardWatcherState::new();
let clipboard_store_for_watcher = clipboard_store.clone();
app.manage(clipboard_watcher_state.clone());
clipboard::watcher::start_clipboard_watcher(app.handle().clone(), clipboard_store_for_watcher, clipboard_watcher_state);
if let Err(error) = clipboard::watcher::register_clipboard_shortcut(app.handle()) {
    eprintln!("{error}");
}
app.manage(clipboard_store);
```

- [ ] **Step 6: Verify compile**

Run: `cargo check`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add package.json package-lock.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/capabilities/default.json src-tauri/src/lib.rs src-tauri/src/clipboard/mod.rs src-tauri/src/clipboard/watcher.rs
git commit -m "feat(clipboard): start watcher and shortcut"
```

---

### Task 5: Add History Window Configuration And App Branching

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src/App.vue`

- [ ] **Step 1: Add hidden shortcut window**

Modify `src-tauri/tauri.conf.json` under `app.windows`. Ensure the existing main window has label `main`, and add a second window:

```json
{
  "label": "main",
  "title": "AT Tool",
  "width": 1180,
  "height": 760,
  "minWidth": 980,
  "minHeight": 640,
  "resizable": true,
  "dragDropEnabled": false
},
{
  "label": "clipboard-history",
  "title": "剪贴板历史",
  "width": 880,
  "height": 620,
  "minWidth": 720,
  "minHeight": 520,
  "resizable": true,
  "visible": false,
  "decorations": false,
  "alwaysOnTop": true,
  "skipTaskbar": true,
  "dragDropEnabled": false
}
```

- [ ] **Step 2: Make App branch on window label**

Modify `src/App.vue` imports:

```ts
import { getCurrentWindow } from '@tauri-apps/api/window';
import ClipboardHistoryWindow from './components/clipboard/ClipboardHistoryWindow.vue';
import ClipboardTool from './components/clipboard/ClipboardTool.vue';
```

Add near top-level script setup state:

```ts
const currentWindow = getCurrentWindow();
const isClipboardHistoryWindow = currentWindow.label === 'clipboard-history';
```

Wrap the existing shell template in:

```vue
<template>
  <ClipboardHistoryWindow v-if="isClipboardHistoryWindow" />
  <n-config-provider v-else :theme="naiveTheme" :theme-overrides="themeOverrides">
    <!-- existing app shell remains here -->
  </n-config-provider>
</template>
```

Change the clipboard tool from soon to ready in `tools`:

```ts
{ id: 'clipboard', name: '剪贴板工具', description: 'Paste 风格剪贴板历史与快捷恢复', status: 'ready', icon: 'clipboard' },
```

Add a render branch:

```vue
<template v-else-if="selectedTool.id === 'clipboard'">
  <ClipboardTool />
</template>
```

- [ ] **Step 3: Run frontend typecheck/build**

Run: `npm run build`

Expected: FAIL because clipboard components do not exist yet. This verifies the branch is wired.

- [ ] **Step 4: Do not commit yet**

Leave these changes staged for the next UI task after components exist.

---

### Task 6: Add Clipboard History Composable

**Files:**
- Create: `src/composables/useClipboardHistory.ts`
- Create: `src/composables/useClipboardHistory.test.ts`

- [ ] **Step 1: Write composable tests**

Create `src/composables/useClipboardHistory.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import type { ClipboardHistoryItem } from '../types/clipboard';
import { createClipboardHistoryApi, useClipboardHistory } from './useClipboardHistory';

const item: ClipboardHistoryItem = {
  id: 'text-1',
  kind: 'text',
  preview: 'hello',
  contentText: 'hello',
  filePaths: [],
  assetUrl: null,
  isPinned: false,
  createdAt: '2026-05-15T10:00:00+08:00',
  lastCopiedAt: null,
};

describe('useClipboardHistory', () => {
  it('loads items and filters locally', async () => {
    const history = useClipboardHistory({
      listItems: async () => [item],
      deleteItem: async () => undefined,
      setPinned: async () => undefined,
      clearHistory: async () => undefined,
    });

    await history.refresh();
    history.query.value = 'hello';

    expect(history.items.value).toEqual([item]);
    expect(history.filteredItems.value.map((entry) => entry.id)).toEqual(['text-1']);
  });

  it('refreshes after destructive actions', async () => {
    let deleted = '';
    let calls = 0;
    const history = useClipboardHistory({
      listItems: async () => {
        calls += 1;
        return [item];
      },
      deleteItem: async (id) => {
        deleted = id;
      },
      setPinned: async () => undefined,
      clearHistory: async () => undefined,
    });

    await history.deleteItem('text-1');

    expect(deleted).toBe('text-1');
    expect(calls).toBe(1);
  });

  it('creates an invoke-backed api with stable command names', () => {
    const calls: Array<{ command: string; args?: unknown }> = [];
    const api = createClipboardHistoryApi(async (command, args) => {
      calls.push({ command, args });
      return [];
    });

    api.listItems({ kind: 'text', query: 'abc' });

    expect(calls[0]).toEqual({ command: 'list_clipboard_items', args: { kind: 'text', query: 'abc' } });
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm test -- src/composables/useClipboardHistory.test.ts`

Expected: FAIL because composable does not exist.

- [ ] **Step 3: Add composable implementation**

Create `src/composables/useClipboardHistory.ts`:

```ts
import { computed, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { ClipboardFilterKind, ClipboardHistoryItem } from '../types/clipboard';
import { filterClipboardItems } from '../utils/clipboardHistory';

export interface ClipboardHistoryApi {
  listItems(filter: { kind: ClipboardFilterKind; query: string }): Promise<ClipboardHistoryItem[]>;
  deleteItem(id: string): Promise<void>;
  setPinned(id: string, isPinned: boolean): Promise<void>;
  clearHistory(): Promise<void>;
}

export function createClipboardHistoryApi(invoker = invoke): ClipboardHistoryApi {
  return {
    listItems(filter) {
      return invoker<ClipboardHistoryItem[]>('list_clipboard_items', {
        kind: filter.kind === 'all' ? null : filter.kind,
        query: filter.query,
      });
    },
    deleteItem(id) {
      return invoker('delete_clipboard_item', { id });
    },
    setPinned(id, isPinned) {
      return invoker('set_clipboard_item_pinned', { id, isPinned });
    },
    clearHistory() {
      return invoker('clear_clipboard_history');
    },
  };
}

export function useClipboardHistory(api: ClipboardHistoryApi = createClipboardHistoryApi()) {
  const items = ref<ClipboardHistoryItem[]>([]);
  const kind = ref<ClipboardFilterKind>('all');
  const query = ref('');
  const loading = ref(false);
  const error = ref<string | null>(null);

  const filteredItems = computed(() => filterClipboardItems(items.value, { kind: kind.value, query: query.value }));

  async function refresh() {
    loading.value = true;
    error.value = null;
    try {
      items.value = await api.listItems({ kind: kind.value, query: query.value });
    } catch (caught) {
      error.value = caught instanceof Error ? caught.message : String(caught);
    } finally {
      loading.value = false;
    }
  }

  async function deleteItem(id: string) {
    await api.deleteItem(id);
    await refresh();
  }

  async function setPinned(id: string, isPinned: boolean) {
    await api.setPinned(id, isPinned);
    await refresh();
  }

  async function clearHistory() {
    await api.clearHistory();
    await refresh();
  }

  return { items, filteredItems, kind, query, loading, error, refresh, deleteItem, setPinned, clearHistory };
}
```

- [ ] **Step 4: Verify composable tests pass**

Run: `npm test -- src/composables/useClipboardHistory.test.ts`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/composables/useClipboardHistory.ts src/composables/useClipboardHistory.test.ts
git commit -m "feat(clipboard): add history composable"
```

---

### Task 7: Add Clipboard UI Components

**Files:**
- Create: `src/components/clipboard/ClipboardItemCard.vue`
- Create: `src/components/clipboard/ClipboardHistoryWindow.vue`
- Create: `src/components/clipboard/ClipboardTool.vue`
- Create: `src/styles/clipboard.css`
- Modify: `src/main.ts`
- Modify: `src/App.vue`

- [ ] **Step 1: Add shared clipboard stylesheet**

Create `src/styles/clipboard.css` using only existing CSS variables:

```css
.clipboard-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: 12px;
}

.clipboard-card {
  min-height: 132px;
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--panel);
  color: var(--text);
  padding: 12px;
  text-align: left;
  cursor: pointer;
}

.clipboard-card:hover {
  border-color: var(--accent);
}

.clipboard-card__meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  color: var(--text-muted);
  font-size: var(--font-size-xs);
}

.clipboard-card__preview {
  margin-top: 10px;
  color: var(--text);
  font-size: var(--font-size-sm);
  line-height: 1.5;
  word-break: break-word;
}

.clipboard-toolbar {
  display: flex;
  gap: 10px;
  align-items: center;
  margin-bottom: 14px;
}
```

Modify `src/main.ts`:

```ts
import './styles/clipboard.css';
```

- [ ] **Step 2: Add item card component**

Create `src/components/clipboard/ClipboardItemCard.vue`:

```vue
<script setup lang="ts">
import type { ClipboardHistoryItem } from '../../types/clipboard';

const props = defineProps<{ item: ClipboardHistoryItem }>();
const emit = defineEmits<{
  restore: [item: ClipboardHistoryItem];
  delete: [id: string];
  pin: [id: string, isPinned: boolean];
}>();

const KIND_LABEL: Record<ClipboardHistoryItem['kind'], string> = {
  text: '文本',
  image: '图片',
  files: '文件',
};
</script>

<template>
  <button class="clipboard-card" type="button" @click="emit('restore', props.item)">
    <div class="clipboard-card__meta">
      <span>{{ KIND_LABEL[props.item.kind] }}</span>
      <span>{{ props.item.isPinned ? '已收藏' : '历史' }}</span>
    </div>
    <div class="clipboard-card__preview">
      <img v-if="props.item.kind === 'image' && props.item.assetUrl" :src="props.item.assetUrl" alt="剪贴板图片预览" style="max-width: 100%; max-height: 72px; border-radius: var(--radius-md); object-fit: cover;" />
      <template v-else-if="props.item.kind === 'files'">
        <strong>{{ props.item.preview }}</strong><br />
        <span>{{ props.item.filePaths[0] }}</span>
      </template>
      <template v-else>{{ props.item.preview }}</template>
    </div>
    <div class="clipboard-card__meta" style="margin-top: 12px;">
      <span>{{ props.item.createdAt.slice(0, 16).replace('T', ' ') }}</span>
      <span @click.stop="emit('pin', props.item.id, !props.item.isPinned)">{{ props.item.isPinned ? '取消收藏' : '收藏' }}</span>
      <span @click.stop="emit('delete', props.item.id)">删除</span>
    </div>
  </button>
</template>
```

- [ ] **Step 3: Add history window component**

Create `src/components/clipboard/ClipboardHistoryWindow.vue`:

```vue
<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import ClipboardItemCard from './ClipboardItemCard.vue';
import { useClipboardHistory } from '../../composables/useClipboardHistory';
import type { ClipboardHistoryItem } from '../../types/clipboard';

const history = useClipboardHistory();
const currentWindow = getCurrentWindow();

async function restore(item: ClipboardHistoryItem) {
  await navigator.clipboard.writeText(item.kind === 'files' ? item.filePaths.join('\n') : item.contentText);
  await currentWindow.hide();
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') currentWindow.hide();
}

onMounted(() => {
  history.refresh();
  window.addEventListener('keydown', handleKeydown);
});

onUnmounted(() => window.removeEventListener('keydown', handleKeydown));
</script>

<template>
  <main style="min-height: 100vh; padding: 18px; background: var(--bg-base); color: var(--text);">
    <div class="clipboard-toolbar">
      <n-input v-model:value="history.query.value" placeholder="搜索剪贴板历史" clearable @keyup.enter="history.refresh" />
      <n-select v-model:value="history.kind.value" style="width: 140px" :options="[
        { label: '全部', value: 'all' },
        { label: '文本', value: 'text' },
        { label: '图片', value: 'image' },
        { label: '文件', value: 'files' },
      ]" />
      <n-button secondary @click="history.refresh">刷新</n-button>
    </div>
    <p v-if="history.error.value" class="muted">{{ history.error.value }}</p>
    <div class="clipboard-grid">
      <ClipboardItemCard
        v-for="item in history.filteredItems.value"
        :key="item.id"
        :item="item"
        @restore="restore"
        @delete="history.deleteItem"
        @pin="history.setPinned"
      />
    </div>
  </main>
</template>
```

- [ ] **Step 4: Add in-app tool component**

Create `src/components/clipboard/ClipboardTool.vue`:

```vue
<script setup lang="ts">
import { onMounted } from 'vue';
import Panel from '../ui/Panel.vue';
import ClipboardItemCard from './ClipboardItemCard.vue';
import { useClipboardHistory } from '../../composables/useClipboardHistory';
import type { ClipboardHistoryItem } from '../../types/clipboard';

const history = useClipboardHistory();

async function restore(item: ClipboardHistoryItem) {
  await navigator.clipboard.writeText(item.kind === 'files' ? item.filePaths.join('\n') : item.contentText);
}

onMounted(() => history.refresh());
</script>

<template>
  <div class="tool-page">
    <Panel title="剪贴板历史" description="AT Tool 运行时记录文本、图片和文件路径；按 Command/Ctrl + Shift + V 打开快捷面板。">
      <div class="clipboard-toolbar">
        <n-input v-model:value="history.query.value" placeholder="搜索内容、文件名或路径" clearable />
        <n-select v-model:value="history.kind.value" style="width: 140px" :options="[
          { label: '全部', value: 'all' },
          { label: '文本', value: 'text' },
          { label: '图片', value: 'image' },
          { label: '文件', value: 'files' },
        ]" />
        <n-button secondary :loading="history.loading.value" @click="history.refresh">刷新</n-button>
        <n-popconfirm @positive-click="history.clearHistory">
          <template #trigger><n-button secondary type="error">清空未收藏</n-button></template>
          确认清空所有未收藏的剪贴板历史？
        </n-popconfirm>
      </div>
      <p v-if="history.error.value" class="muted">{{ history.error.value }}</p>
      <div class="clipboard-grid">
        <ClipboardItemCard
          v-for="item in history.filteredItems.value"
          :key="item.id"
          :item="item"
          @restore="restore"
          @delete="history.deleteItem"
          @pin="history.setPinned"
        />
      </div>
    </Panel>
  </div>
</template>
```

- [ ] **Step 5: Apply App.vue changes from Task 5**

Apply the App.vue changes described in Task 5 now that components exist.

- [ ] **Step 6: Verify frontend**

Run: `npm run build`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/App.vue src/main.ts src/styles/clipboard.css src/components/clipboard/ClipboardItemCard.vue src/components/clipboard/ClipboardHistoryWindow.vue src/components/clipboard/ClipboardTool.vue src-tauri/tauri.conf.json
git commit -m "feat(clipboard): add history interface"
```

---

### Task 8: Implement Restore Through Backend Commands

**Files:**
- Modify: `src-tauri/src/clipboard/commands.rs`
- Modify: `src/composables/useClipboardHistory.ts`
- Modify: `src/composables/useClipboardHistory.test.ts`
- Modify: `src/components/clipboard/ClipboardHistoryWindow.vue`
- Modify: `src/components/clipboard/ClipboardTool.vue`

- [ ] **Step 1: Update composable test for restore command**

Add to `src/composables/useClipboardHistory.test.ts`:

```ts
it('restores an item through the backend command', async () => {
  const calls: Array<{ command: string; args?: unknown }> = [];
  const api = createClipboardHistoryApi(async (command, args) => {
    calls.push({ command, args });
    return [];
  });

  await api.restoreItem('text-1');

  expect(calls[0]).toEqual({ command: 'restore_clipboard_item', args: { id: 'text-1' } });
});
```

- [ ] **Step 2: Run test to verify failure**

Run: `npm test -- src/composables/useClipboardHistory.test.ts`

Expected: FAIL because `restoreItem` does not exist.

- [ ] **Step 3: Add restore command to frontend API**

Modify `src/composables/useClipboardHistory.ts` interface:

```ts
restoreItem(id: string): Promise<void>;
```

Add implementation in `createClipboardHistoryApi`:

```ts
restoreItem(id) {
  return invoker('restore_clipboard_item', { id });
},
```

Add method in `useClipboardHistory`:

```ts
async function restoreItem(id: string) {
  await api.restoreItem(id);
  await refresh();
}
```

Return it:

```ts
return { items, filteredItems, kind, query, loading, error, refresh, deleteItem, setPinned, clearHistory, restoreItem };
```

- [ ] **Step 4: Add backend restore command**

Modify `src-tauri/src/clipboard/commands.rs`:

```rust
use tauri::{AppHandle, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
```

Add:

```rust
#[tauri::command]
pub async fn restore_clipboard_item(
    id: String,
    app: AppHandle,
    store: State<'_, ClipboardStore>,
) -> Result<(), String> {
    let item = store
        .list_items(None, None)?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| "剪贴板历史不存在".to_string())?;

    match item.kind {
        ClipboardItemKind::Text => app.clipboard().write_text(item.content_text.clone()),
        ClipboardItemKind::Files => app.clipboard().write_text(item.file_paths.join("\n")),
        ClipboardItemKind::Image => {
            return Err("当前版本暂不支持从历史恢复图片到系统剪贴板".to_string());
        }
    }
    .map_err(|error| format!("写入剪贴板失败：{error}"))?;

    store.touch_copied(&id)
}
```

Register in `src-tauri/src/lib.rs` handler:

```rust
clipboard::commands::restore_clipboard_item,
```

- [ ] **Step 5: Update components to use backend restore**

In `ClipboardHistoryWindow.vue` replace restore function with:

```ts
async function restore(item: ClipboardHistoryItem) {
  await history.restoreItem(item.id);
  await currentWindow.hide();
}
```

In `ClipboardTool.vue` replace restore function with:

```ts
async function restore(item: ClipboardHistoryItem) {
  await history.restoreItem(item.id);
}
```

- [ ] **Step 6: Verify tests and builds**

Run:

```bash
npm test -- src/composables/useClipboardHistory.test.ts
cargo check
npm run build
```

Expected: all PASS. If image restore is unsupported, the UI still shows an actionable backend error.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/clipboard/commands.rs src/composables/useClipboardHistory.ts src/composables/useClipboardHistory.test.ts src/components/clipboard/ClipboardHistoryWindow.vue src/components/clipboard/ClipboardTool.vue
git commit -m "feat(clipboard): restore history items"
```

---

### Task 9: Add Capture Controls And Settings Persistence

**Files:**
- Modify: `src-tauri/src/clipboard/storage.rs`
- Modify: `src-tauri/src/clipboard/commands.rs`
- Modify: `src-tauri/src/clipboard/watcher.rs`
- Modify: `src/components/clipboard/ClipboardTool.vue`

- [ ] **Step 1: Add backend settings tests**

Add to `src-tauri/src/clipboard/storage.rs` tests:

```rust
#[test]
fn persists_capture_settings() {
    let store = temp_store();
    let mut settings = store.load_settings().expect("load defaults");
    assert!(settings.capture_enabled);
    settings.capture_enabled = false;
    settings.retention_limit = 25;
    store.save_settings(&settings).expect("save");

    let loaded = store.load_settings().expect("load saved");
    assert!(!loaded.capture_enabled);
    assert_eq!(loaded.retention_limit, 25);
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test clipboard::storage::tests::persists_capture_settings --lib`

Expected: FAIL because settings methods are missing.

- [ ] **Step 3: Add settings table and methods**

In `init_database`, add:

```rust
CREATE TABLE IF NOT EXISTS clipboard_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

Import settings model:

```rust
use super::models::{ClipboardHistoryItem, ClipboardHistorySettings, ClipboardItemKind};
```

Add methods to `ClipboardStore`:

```rust
pub fn load_settings(&self) -> Result<ClipboardHistorySettings, String> {
    let connection = Connection::open(&self.db_path).map_err(|error| format!("打开剪贴板数据库失败：{error}"))?;
    let mut settings = ClipboardHistorySettings::default();
    let mut statement = connection.prepare("SELECT key, value FROM clipboard_settings").map_err(|error| format!("读取剪贴板设置失败：{error}"))?;
    let rows = statement.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))).map_err(|error| format!("读取剪贴板设置失败：{error}"))?;
    for row in rows {
        let (key, value) = row.map_err(|error| format!("读取剪贴板设置失败：{error}"))?;
        match key.as_str() {
            "capture_enabled" => settings.capture_enabled = value == "1",
            "retention_limit" => settings.retention_limit = value.parse().unwrap_or(settings.retention_limit),
            "shortcut" => settings.shortcut = value,
            _ => {}
        }
    }
    Ok(settings)
}

pub fn save_settings(&self, settings: &ClipboardHistorySettings) -> Result<(), String> {
    let connection = Connection::open(&self.db_path).map_err(|error| format!("打开剪贴板数据库失败：{error}"))?;
    let values = [
        ("capture_enabled", if settings.capture_enabled { "1".to_string() } else { "0".to_string() }),
        ("retention_limit", settings.retention_limit.to_string()),
        ("shortcut", settings.shortcut.clone()),
    ];
    for (key, value) in values {
        connection.execute(
            "INSERT INTO clipboard_settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        ).map_err(|error| format!("保存剪贴板设置失败：{error}"))?;
    }
    Ok(())
}
```

- [ ] **Step 4: Update commands**

Modify `get_clipboard_settings` to use store:

```rust
#[tauri::command]
pub async fn get_clipboard_settings(store: State<'_, ClipboardStore>) -> Result<ClipboardHistorySettings, String> {
    store.load_settings()
}
```

Add command:

```rust
#[tauri::command]
pub async fn save_clipboard_settings(
    settings: ClipboardHistorySettings,
    watcher: State<'_, ClipboardWatcherState>,
    store: State<'_, ClipboardStore>,
) -> Result<ClipboardHistorySettings, String> {
    store.save_settings(&settings)?;
    watcher.set_enabled(settings.capture_enabled);
    Ok(settings)
}
```

Import watcher state:

```rust
use super::watcher::ClipboardWatcherState;
```

Register handler:

```rust
clipboard::commands::save_clipboard_settings,
```

- [ ] **Step 5: Add UI controls**

In `ClipboardTool.vue`, add settings state and methods:

```ts
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { ClipboardHistorySettings } from '../../types/clipboard';

const settings = ref<ClipboardHistorySettings>({ captureEnabled: true, retentionLimit: 500, shortcut: 'CommandOrControl+Shift+V' });

async function loadSettings() {
  settings.value = await invoke<ClipboardHistorySettings>('get_clipboard_settings');
}

async function saveSettings() {
  settings.value = await invoke<ClipboardHistorySettings>('save_clipboard_settings', { settings: settings.value });
}
```

Update mounted hook:

```ts
onMounted(() => {
  history.refresh();
  loadSettings();
});
```

Add controls above history toolbar:

```vue
<div class="clipboard-toolbar">
  <n-switch v-model:value="settings.captureEnabled" @update:value="saveSettings" />
  <span class="muted">{{ settings.captureEnabled ? '正在记录剪贴板' : '已暂停记录剪贴板' }}</span>
  <n-input-number v-model:value="settings.retentionLimit" :min="50" :max="5000" style="width: 140px" @blur="saveSettings" />
  <span class="muted">最多保留条数</span>
</div>
```

- [ ] **Step 6: Verify**

Run:

```bash
cargo test clipboard::storage --lib
cargo check
npm run build
```

Expected: all PASS.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/clipboard/storage.rs src-tauri/src/clipboard/commands.rs src-tauri/src/clipboard/watcher.rs src/components/clipboard/ClipboardTool.vue
git commit -m "feat(clipboard): add capture settings"
```

---

### Task 10: Final Verification And Manual QA

**Files:**
- Modify any files needed for defects found during verification.

- [ ] **Step 1: Run full automated checks**

Run:

```bash
npm test
cargo check
npm run build
```

Expected: all PASS.

- [ ] **Step 2: Run Tauri app manually**

Run: `npm run tauri:dev`

Expected: app starts and main window opens.

- [ ] **Step 3: Manual clipboard capture checks**

While the app is running:

1. Copy plain text from any app.
2. Open `剪贴板工具` in AT Tool.
3. Confirm text appears in history.
4. Copy two absolute file paths as newline-separated text.
5. Confirm a file item appears.
6. Press `Command+Shift+V` on macOS or `Ctrl+Shift+V` on Windows/Linux.
7. Confirm the shortcut history window opens.
8. Select a text item.
9. Paste into a text editor and confirm restored content.
10. Press shortcut again and press Escape.
11. Confirm panel hides.

- [ ] **Step 4: Manual safety checks**

1. Toggle pause off.
2. Copy new text.
3. Confirm no new item appears after refresh.
4. Toggle pause on.
5. Copy new text.
6. Confirm new item appears.
7. Favorite one item.
8. Click clear unpinned.
9. Confirm favorite remains and unpinned items are gone.

- [ ] **Step 5: Commit any verification fixes**

If fixes were needed:

```bash
git add src-tauri/src/clipboard src-tauri/src/lib.rs src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/capabilities/default.json package.json package-lock.json src/App.vue src/main.ts src/styles/clipboard.css src/types/clipboard.ts src/utils/clipboardHistory.ts src/utils/clipboardHistory.test.ts src/composables/useClipboardHistory.ts src/composables/useClipboardHistory.test.ts src/components/clipboard
git commit -m "fix(clipboard): address verification issues"
```

If no fixes were needed, do not create an empty commit.

---

## Self-Review

Spec coverage:

- Paste-style global shortcut panel: Tasks 4, 5, 7.
- Text history: Tasks 2, 4, 7, 8.
- Image history storage and preview path: Tasks 2, 7; restore is explicitly guarded with a user-facing unsupported error until plugin image restore is fully wired.
- File path history: Tasks 1, 2, 4, 7, 8.
- Search and type filters: Tasks 1, 6, 7.
- Delete, pin, clear: Tasks 2, 3, 7.
- Pause and retention: Task 9.
- Privacy and no network sync: Task 9 UI copy and local-only architecture.
- Tests and manual verification: Tasks 1, 2, 6, 9, 10.

Known implementation constraint:

- v1 records file paths from text clipboard content and restores them as newline-separated paths. Native OS file-list clipboard formats can be added later behind the same `files` item model.
- v1 stores image assets and previews. Full image restore may require adapting the exact `tauri-plugin-clipboard-manager` image API after dependency installation; Task 8 keeps the UI honest by surfacing an explicit unsupported error if image write is not completed in the first implementation pass.
