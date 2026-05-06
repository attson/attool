use std::{fs, path::PathBuf};

use rusqlite::{params, Connection};

use super::models::{TemplateProject, TemplateSummary};

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
                "#,
            )
            .map_err(|error| format!("初始化模板数据库失败：{error}"))?;
        Ok(())
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
}
