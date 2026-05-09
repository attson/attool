use std::{fs, path::PathBuf};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use rusqlite::{params, Connection};

use super::models::{TemplateAsset, TemplateProject, TemplateSummary};

#[derive(Clone, Debug)]
pub struct EcommerceStore {
    root_dir: PathBuf,
    db_path: PathBuf,
}

impl EcommerceStore {
    pub fn new(root_dir: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(root_dir.join("templates"))
            .map_err(|error| format!("创建模板目录失败：{error}"))?;
        let db_path = root_dir.join("ecommerce_templates.sqlite3");
        let store = Self { root_dir, db_path };
        store.init_database()?;
        Ok(store)
    }

    pub fn root_dir(&self) -> &PathBuf {
        &self.root_dir
    }

    pub fn template_dir(&self, id: &str) -> PathBuf {
        self.root_dir.join("templates").join(id)
    }

    pub fn batch_cache_dir(&self) -> PathBuf {
        self.root_dir.join("batch_cache")
    }

    pub fn save_batch_outputs(&self, file_paths: &[String], target_dir: &str) -> Result<usize, String> {
        let target = PathBuf::from(target_dir.trim());
        if target.as_os_str().is_empty() {
            return Err("请选择输出目录".to_string());
        }
        fs::create_dir_all(&target)
            .map_err(|error| format!("创建输出目录失败：{error}"))?;
        let mut saved = 0usize;
        for source in file_paths {
            let src = PathBuf::from(source);
            if !src.is_file() {
                continue;
            }
            let original = src
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("output.png")
                .to_string();
            let mut destination = target.join(&original);
            if destination.exists() {
                let stem = src
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or("output")
                    .to_string();
                let extension = src
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or("png")
                    .to_string();
                for index in 1..1000 {
                    let candidate = target.join(format!("{stem}_{index:03}.{extension}"));
                    if !candidate.exists() {
                        destination = candidate;
                        break;
                    }
                }
            }
            fs::copy(&src, &destination)
                .map_err(|error| format!("拷贝失败 {}: {error}", src.display()))?;
            saved += 1;
        }
        Ok(saved)
    }

    pub fn init_database(&self) -> Result<(), String> {
        let connection = Connection::open(&self.db_path)
            .map_err(|error| format!("打开模板数据库失败：{error}"))?;
        connection
            .execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS ecommerce_templates (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    canvas_width INTEGER NOT NULL,
                    canvas_height INTEGER NOT NULL,
                    preview_path TEXT,
                    project_json TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE INDEX IF NOT EXISTS idx_ecommerce_templates_updated_at
                    ON ecommerce_templates(updated_at DESC);

                CREATE TABLE IF NOT EXISTS ecommerce_assets (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    mime_type TEXT NOT NULL,
                    width INTEGER NOT NULL,
                    height INTEGER NOT NULL,
                    data_base64 TEXT NOT NULL,
                    created_at TEXT NOT NULL
                );

                CREATE INDEX IF NOT EXISTS idx_ecommerce_assets_created_at
                    ON ecommerce_assets(created_at DESC);
                "#,
            )
            .map_err(|error| format!("初始化模板数据库失败：{error}"))?;
        Ok(())
    }

    pub fn save_asset(
        &self,
        name: String,
        mime_type: String,
        bytes: Vec<u8>,
    ) -> Result<TemplateAsset, String> {
        if bytes.is_empty() {
            return Err("图片为空".to_string());
        }
        let image = image::load_from_memory(&bytes)
            .map_err(|error| format!("图片格式不支持：{error}"))?;
        let asset_id = format!("asset-{}", uuid::Uuid::new_v4().simple());
        let resolved_mime = if mime_type.trim().is_empty() {
            guess_mime_type(&name)
        } else {
            mime_type
        };
        let encoded = BASE64_STANDARD.encode(&bytes);
        let data_url = format!("data:{};base64,{}", resolved_mime, encoded);
        let created_at = chrono::Local::now().to_rfc3339();
        let connection = Connection::open(&self.db_path)
            .map_err(|error| format!("打开模板数据库失败：{error}"))?;
        connection
            .execute(
                r#"
                INSERT INTO ecommerce_assets (id, name, mime_type, width, height, data_base64, created_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                "#,
                params![
                    asset_id,
                    name,
                    resolved_mime,
                    image.width() as i64,
                    image.height() as i64,
                    encoded,
                    created_at
                ],
            )
            .map_err(|error| format!("保存素材失败：{error}"))?;
        Ok(TemplateAsset {
            id: asset_id,
            name,
            data_url,
            source_layer_id: None,
            mime_type: resolved_mime,
            width: image.width(),
            height: image.height(),
            created_at,
        })
    }

    pub fn list_assets(&self) -> Result<Vec<TemplateAsset>, String> {
        let connection = Connection::open(&self.db_path)
            .map_err(|error| format!("打开模板数据库失败：{error}"))?;
        let mut statement = connection
            .prepare(
                r#"
                SELECT id, name, mime_type, width, height, data_base64, created_at
                FROM ecommerce_assets
                ORDER BY datetime(created_at) DESC, id DESC
                "#,
            )
            .map_err(|error| format!("读取素材失败：{error}"))?;
        let rows = statement
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let mime_type: String = row.get(2)?;
                let width: i64 = row.get(3)?;
                let height: i64 = row.get(4)?;
                let data_base64: String = row.get(5)?;
                let created_at: String = row.get(6)?;
                let data_url = format!("data:{};base64,{}", mime_type, data_base64);
                Ok(TemplateAsset {
                    id,
                    name,
                    data_url,
                    source_layer_id: None,
                    mime_type,
                    width: width as u32,
                    height: height as u32,
                    created_at,
                })
            })
            .map_err(|error| format!("读取素材失败：{error}"))?;
        let mut assets = Vec::new();
        for row in rows {
            assets.push(row.map_err(|error| format!("读取素材失败：{error}"))?);
        }
        Ok(assets)
    }

    pub fn delete_asset(&self, id: &str) -> Result<(), String> {
        let connection = Connection::open(&self.db_path)
            .map_err(|error| format!("打开模板数据库失败：{error}"))?;
        connection
            .execute("DELETE FROM ecommerce_assets WHERE id = ?1", params![id])
            .map_err(|error| format!("删除素材失败：{error}"))?;
        Ok(())
    }

    pub fn load_asset_bytes(&self, id: &str) -> Result<Vec<u8>, String> {
        let connection = Connection::open(&self.db_path)
            .map_err(|error| format!("打开模板数据库失败：{error}"))?;
        let data_base64: String = connection
            .query_row(
                "SELECT data_base64 FROM ecommerce_assets WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .map_err(|error| format!("未找到素材 {id}：{error}"))?;
        BASE64_STANDARD
            .decode(data_base64.as_bytes())
            .map_err(|error| format!("解码素材失败 {id}：{error}"))
    }

    pub fn save_template(&self, project: TemplateProject) -> Result<TemplateProject, String> {
        fs::create_dir_all(self.template_dir(&project.id).join("assets"))
            .map_err(|error| format!("创建模板素材目录失败：{error}"))?;
        let json = serde_json::to_string(&project)
            .map_err(|error| format!("序列化模板失败：{error}"))?;
        let connection = Connection::open(&self.db_path)
            .map_err(|error| format!("打开模板数据库失败：{error}"))?;
        connection
            .execute(
                r#"
                INSERT INTO ecommerce_templates (
                    id, name, canvas_width, canvas_height, preview_path, project_json, created_at, updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    canvas_width = excluded.canvas_width,
                    canvas_height = excluded.canvas_height,
                    preview_path = excluded.preview_path,
                    project_json = excluded.project_json,
                    updated_at = excluded.updated_at
                "#,
                params![
                    project.id,
                    project.name,
                    project.canvas_width,
                    project.canvas_height,
                    project.preview_path,
                    json,
                    project.created_at,
                    project.updated_at
                ],
            )
            .map_err(|error| format!("保存模板失败：{error}"))?;
        Ok(project)
    }

    pub fn list_templates(&self) -> Result<Vec<TemplateSummary>, String> {
        let connection = Connection::open(&self.db_path)
            .map_err(|error| format!("打开模板数据库失败：{error}"))?;
        let mut statement = connection
            .prepare(
                r#"
                SELECT id, name, canvas_width, canvas_height, preview_path, updated_at
                FROM ecommerce_templates
                ORDER BY datetime(updated_at) DESC, id DESC
                "#,
            )
            .map_err(|error| format!("读取模板列表失败：{error}"))?;
        let rows = statement
            .query_map([], |row| {
                Ok(TemplateSummary {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    canvas_width: row.get::<_, i64>(2)? as u32,
                    canvas_height: row.get::<_, i64>(3)? as u32,
                    preview_path: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })
            .map_err(|error| format!("读取模板列表失败：{error}"))?;
        let mut summaries = Vec::new();
        for row in rows {
            summaries.push(row.map_err(|error| format!("读取模板列表失败：{error}"))?);
        }
        Ok(summaries)
    }

    pub fn load_template(&self, id: &str) -> Result<TemplateProject, String> {
        let connection = Connection::open(&self.db_path)
            .map_err(|error| format!("打开模板数据库失败：{error}"))?;
        let json: String = connection
            .query_row(
                "SELECT project_json FROM ecommerce_templates WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .map_err(|error| format!("未找到模板：{error}"))?;
        serde_json::from_str(&json).map_err(|error| format!("解析模板失败：{error}"))
    }

    pub fn delete_template(&self, id: &str) -> Result<(), String> {
        let connection = Connection::open(&self.db_path)
            .map_err(|error| format!("打开模板数据库失败：{error}"))?;
        connection
            .execute("DELETE FROM ecommerce_templates WHERE id = ?1", params![id])
            .map_err(|error| format!("删除模板失败：{error}"))?;
        let dir = self.template_dir(id);
        if dir.exists() {
            let _ = fs::remove_dir_all(&dir);
        }
        Ok(())
    }

    pub fn rename_template(&self, id: &str, name: &str) -> Result<TemplateProject, String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("模板名称不能为空".to_string());
        }
        let mut project = self.load_template(id)?;
        project.name = trimmed.to_string();
        project.updated_at = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let json = serde_json::to_string(&project)
            .map_err(|error| format!("序列化模板失败：{error}"))?;
        let connection = Connection::open(&self.db_path)
            .map_err(|error| format!("打开模板数据库失败：{error}"))?;
        connection
            .execute(
                r#"
                UPDATE ecommerce_templates
                SET name = ?2, project_json = ?3, updated_at = ?4
                WHERE id = ?1
                "#,
                params![id, project.name, json, project.updated_at],
            )
            .map_err(|error| format!("重命名模板失败：{error}"))?;
        Ok(project)
    }
}

fn guess_mime_type(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg".to_string()
    } else if lower.ends_with(".webp") {
        "image/webp".to_string()
    } else if lower.ends_with(".gif") {
        "image/gif".to_string()
    } else {
        "image/png".to_string()
    }
}
