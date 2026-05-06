# Ecommerce Template Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first working ecommerce main-image template tool: import a layered PSD into a template draft, edit text/image/shape layers, load batch data, and export PNG images.

**Architecture:** Keep the existing Tauri + Vue app and add a focused ecommerce-template module on both sides. Rust owns persistence, batch import, export orchestration, and Tauri commands; a Python PSD bridge using `psd-tools` performs the first PSD structure import because current Rust PSD crates do not expose enough text-layer metadata for this feature. Frontend state is split into small composables/components so `src/App.vue` only routes tools and hosts pages.

**Tech Stack:** Vue 3, TypeScript, Naive UI, Tauri 2, Rust, rusqlite, image, serde, calamine, csv, uuid, a local Python helper with psd-tools, Vitest for frontend unit tests.

---

## Scope Check

The spec covers PSD import, editing, batch data, and PNG export. This plan keeps them in one MVP because each part is required for the user-visible closed loop, but tasks are sequenced so each commit is testable: data helpers first, backend storage/import, editor UI, batch import, export, then integration.

## File Structure

Create these focused frontend files:

- `src/types/ecommerceTemplate.ts` - shared TypeScript model for templates, layers, assets, batch rows, and export results.
- `src/utils/ecommerceTemplate.ts` - pure helpers for field extraction, layer traversal, filename generation, and batch validation.
- `src/utils/ecommerceTemplate.test.ts` - Vitest coverage for the pure helpers.
- `src/components/ecommerce/TemplateTool.vue` - top-level page for the new tool.
- `src/components/ecommerce/TemplateCanvas.vue` - visual editor canvas with selection, drag, and resize.
- `src/components/ecommerce/LayerTree.vue` - layer tree and selection controls.
- `src/components/ecommerce/LayerProperties.vue` - right-side inspector for selected text/image/shape/group layer.
- `src/components/ecommerce/BatchPanel.vue` - CSV/Excel/folder import preview and export controls.
- `src/components/ecommerce/templateDefaults.ts` - empty project defaults and sample field labels.

Modify these frontend files:

- `src/App.vue` - add the ecommerce template tool entry and route to `TemplateTool` while preserving current aria2 and logo flows.
- `src/styles.css` - add editor layout, canvas, selection, and panel styles.
- `package.json` and `package-lock.json` - add Vitest scripts and test dependency.

Create these backend files:

- `src-tauri/src/ecommerce/mod.rs` - module exports and Tauri command registration list.
- `src-tauri/src/ecommerce/models.rs` - Rust models matching frontend JSON.
- `src-tauri/src/ecommerce/storage.rs` - SQLite/template asset storage.
- `src-tauri/src/ecommerce/psd_bridge.rs` - runs the Python PSD bridge and normalizes its output.
- `src-tauri/src/ecommerce/batch.rs` - CSV/Excel/folder batch import.
- `src-tauri/src/ecommerce/render.rs` - renders template + data rows to PNG.
- `src-tauri/src/ecommerce/commands.rs` - Tauri command functions.
- `src-tauri/python/psd_template_bridge.py` - PSD parser powered by `psd-tools`.
- `src-tauri/tests/ecommerce_storage.rs` - integration tests for storage.
- `src-tauri/tests/ecommerce_batch.rs` - integration tests for batch parsing and validation.
- `src-tauri/tests/ecommerce_render.rs` - integration tests for PNG export.

Modify these backend files:

- `src-tauri/src/lib.rs` - register ecommerce module commands and initialize ecommerce storage alongside existing app state.
- `src-tauri/Cargo.toml` and `src-tauri/Cargo.lock` - add Rust dependencies.
- `src-tauri/capabilities/default.json` - allow new Tauri commands if capability permissions require explicit entries.
- `README.md` - document PSD naming rules and first-run Python dependency.

## Task 1: Frontend Types And Pure Helpers

**Files:**
- Create: `src/types/ecommerceTemplate.ts`
- Create: `src/utils/ecommerceTemplate.ts`
- Create: `src/utils/ecommerceTemplate.test.ts`
- Modify: `package.json`
- Modify: `package-lock.json`

- [ ] **Step 1: Add Vitest dependency and script**

Run:

```bash
npm install -D vitest
```

Edit `package.json` so `scripts` includes this exact entry:

```json
"test": "./node_modules/.bin/vitest run"
```

Expected: `package-lock.json` changes and `npm run test -- --runInBand` is not used because Vitest does not support Jest's `--runInBand` flag.

- [ ] **Step 2: Create shared TypeScript models**

Create `src/types/ecommerceTemplate.ts` with this content:

```ts
export type TemplateLayerType = 'text' | 'image' | 'shape' | 'group';
export type ImageFit = 'cover' | 'contain' | 'stretch';
export type ShapeKind = 'rect' | 'roundRect' | 'ellipse' | 'line';
export type TextAlign = 'left' | 'center' | 'right';

export type TemplateProject = {
  id: string;
  name: string;
  canvasWidth: number;
  canvasHeight: number;
  layers: TemplateLayer[];
  assets: TemplateAsset[];
  sourcePsdPath?: string;
  previewPath?: string;
  createdAt: string;
  updatedAt: string;
};

export type TemplateLayer = {
  id: string;
  name: string;
  type: TemplateLayerType;
  x: number;
  y: number;
  width: number;
  height: number;
  visible: boolean;
  opacity: number;
  rotation: number;
  bindingKey?: string;
  locked?: boolean;
  children?: TemplateLayer[];
  text?: TextLayerData;
  image?: ImageLayerData;
  shape?: ShapeLayerData;
};

export type TextLayerData = {
  text: string;
  fontFamily: string;
  fontSize: number;
  fontWeight: number | string;
  color: string;
  strokeColor?: string;
  strokeWidth?: number;
  letterSpacing?: number;
  lineHeight?: number;
  align?: TextAlign;
};

export type ImageLayerData = {
  assetId: string;
  fit: ImageFit;
  replaceable: boolean;
};

export type ShapeLayerData = {
  shape: ShapeKind;
  fill?: string;
  stroke?: string;
  strokeWidth?: number;
  radius?: number;
};

export type TemplateAsset = {
  id: string;
  name: string;
  path: string;
  sourceLayerId?: string;
  mimeType: string;
  width: number;
  height: number;
};

export type TemplateSummary = {
  id: string;
  name: string;
  canvasWidth: number;
  canvasHeight: number;
  previewPath?: string;
  updatedAt: string;
};

export type BatchRow = {
  id: string;
  index: number;
  values: Record<string, string>;
};

export type BatchDataPreview = {
  fields: string[];
  rows: BatchRow[];
  unusedFields: string[];
  missingFields: string[];
};

export type ExportRequest = {
  templateId: string;
  outputDir: string;
  rows: BatchRow[];
};

export type ExportFailure = {
  rowIndex: number;
  field?: string;
  message: string;
};

export type ExportResult = {
  total: number;
  succeeded: number;
  outputs: string[];
  failed: ExportFailure[];
};
```

- [ ] **Step 3: Write failing helper tests**

Create `src/utils/ecommerceTemplate.test.ts` with this content:

```ts
import { describe, expect, it } from 'vitest';
import type { TemplateLayer } from '../types/ecommerceTemplate';
import {
  collectBindingKeys,
  extractBindingKey,
  flattenLayers,
  makeExportFileName,
  validateBatchFields
} from './ecommerceTemplate';

const layers: TemplateLayer[] = [
  {
    id: 'group-1',
    name: '右下角标',
    type: 'group',
    x: 10,
    y: 20,
    width: 300,
    height: 80,
    visible: true,
    opacity: 1,
    rotation: 0,
    children: [
      {
        id: 'text-1',
        name: '{{title}} 大标题',
        type: 'text',
        x: 12,
        y: 24,
        width: 250,
        height: 40,
        visible: true,
        opacity: 1,
        rotation: 0,
        bindingKey: 'title',
        text: {
          text: '便携小沙发',
          fontFamily: 'STHupo',
          fontSize: 48,
          fontWeight: 700,
          color: '#ffffff'
        }
      }
    ]
  },
  {
    id: 'image-1',
    name: '{{product_image}} 商品图',
    type: 'image',
    x: 0,
    y: 0,
    width: 1000,
    height: 1000,
    visible: true,
    opacity: 1,
    rotation: 0,
    bindingKey: 'product_image',
    image: { assetId: 'asset-1', fit: 'cover', replaceable: true }
  }
];

describe('ecommerceTemplate helpers', () => {
  it('extracts binding keys from PSD-style layer names', () => {
    expect(extractBindingKey('{{title}} 大标题')).toBe('title');
    expect(extractBindingKey('prefix {{selling_point_1}} 卖点')).toBe('selling_point_1');
    expect(extractBindingKey('商品图')).toBeUndefined();
  });

  it('flattens nested layers in paint order', () => {
    expect(flattenLayers(layers).map((layer) => layer.id)).toEqual(['group-1', 'text-1', 'image-1']);
  });

  it('collects unique binding keys from nested layers', () => {
    expect(collectBindingKeys(layers)).toEqual(['title', 'product_image']);
  });

  it('reports missing and unused batch fields', () => {
    expect(validateBatchFields(['title', 'product_image'], ['title', 'price'])).toEqual({
      missingFields: ['product_image'],
      unusedFields: ['price']
    });
  });

  it('generates safe PNG filenames', () => {
    expect(makeExportFileName({ title: '双人黑色/北欧风' }, 0)).toBe('双人黑色_北欧风.png');
    expect(makeExportFileName({ name: 'sku 88' }, 4)).toBe('sku_88.png');
    expect(makeExportFileName({}, 8)).toBe('009.png');
  });
});
```

- [ ] **Step 4: Run tests and verify failure**

Run:

```bash
npm run test -- src/utils/ecommerceTemplate.test.ts
```

Expected: FAIL with an import error for `./ecommerceTemplate` or missing exported helper functions.

- [ ] **Step 5: Implement helper functions**

Create `src/utils/ecommerceTemplate.ts` with this content:

```ts
import type { TemplateLayer } from '../types/ecommerceTemplate';

const BINDING_PATTERN = /\{\{\s*([a-zA-Z][a-zA-Z0-9_]*)\s*\}\}/;

export function extractBindingKey(layerName: string): string | undefined {
  return layerName.match(BINDING_PATTERN)?.[1];
}

export function flattenLayers(layers: TemplateLayer[]): TemplateLayer[] {
  const flattened: TemplateLayer[] = [];
  const visit = (items: TemplateLayer[]) => {
    for (const layer of items) {
      flattened.push(layer);
      if (layer.children?.length) {
        visit(layer.children);
      }
    }
  };
  visit(layers);
  return flattened;
}

export function collectBindingKeys(layers: TemplateLayer[]): string[] {
  const keys: string[] = [];
  for (const layer of flattenLayers(layers)) {
    if (layer.bindingKey && !keys.includes(layer.bindingKey)) {
      keys.push(layer.bindingKey);
    }
  }
  return keys;
}

export function validateBatchFields(requiredFields: string[], incomingFields: string[]) {
  return {
    missingFields: requiredFields.filter((field) => !incomingFields.includes(field)),
    unusedFields: incomingFields.filter((field) => !requiredFields.includes(field))
  };
}

export function makeExportFileName(values: Record<string, string>, rowIndex: number): string {
  const rawName = values.name || values.title || String(rowIndex + 1).padStart(3, '0');
  const safeName = rawName
    .trim()
    .replace(/[\\/:*?"<>|]+/g, '_')
    .replace(/\s+/g, '_')
    .replace(/^\.+$/, '')
    .slice(0, 80);

  return `${safeName || String(rowIndex + 1).padStart(3, '0')}.png`;
}
```

- [ ] **Step 6: Run tests and commit**

Run:

```bash
npm run test -- src/utils/ecommerceTemplate.test.ts
npm run build
```

Expected: both commands PASS.

Commit:

```bash
git add package.json package-lock.json src/types/ecommerceTemplate.ts src/utils/ecommerceTemplate.ts src/utils/ecommerceTemplate.test.ts
git commit -m "feat: add ecommerce template types"
```

## Task 2: Backend Models And Template Storage

**Files:**
- Create: `src-tauri/src/ecommerce/mod.rs`
- Create: `src-tauri/src/ecommerce/models.rs`
- Create: `src-tauri/src/ecommerce/storage.rs`
- Create: `src-tauri/tests/ecommerce_storage.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`

- [ ] **Step 1: Add backend dependencies**

Edit `src-tauri/Cargo.toml` dependencies to include:

```toml
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
```

Run:

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: PASS and `src-tauri/Cargo.lock` updates.

- [ ] **Step 2: Create backend data models**

Create `src-tauri/src/ecommerce/models.rs` with this content:

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateProject {
    pub id: String,
    pub name: String,
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub layers: Vec<TemplateLayer>,
    pub assets: Vec<TemplateAsset>,
    pub source_psd_path: Option<String>,
    pub preview_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateLayer {
    pub id: String,
    pub name: String,
    pub r#type: TemplateLayerType,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub visible: bool,
    pub opacity: f32,
    pub rotation: f32,
    pub binding_key: Option<String>,
    pub locked: Option<bool>,
    pub children: Option<Vec<TemplateLayer>>,
    pub text: Option<TextLayerData>,
    pub image: Option<ImageLayerData>,
    pub shape: Option<ShapeLayerData>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TemplateLayerType {
    Text,
    Image,
    Shape,
    Group,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextLayerData {
    pub text: String,
    pub font_family: String,
    pub font_size: f32,
    pub font_weight: serde_json::Value,
    pub color: String,
    pub stroke_color: Option<String>,
    pub stroke_width: Option<f32>,
    pub letter_spacing: Option<f32>,
    pub line_height: Option<f32>,
    pub align: Option<TextAlign>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageLayerData {
    pub asset_id: String,
    pub fit: ImageFit,
    pub replaceable: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ImageFit {
    Cover,
    Contain,
    Stretch,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShapeLayerData {
    pub shape: ShapeKind,
    pub fill: Option<String>,
    pub stroke: Option<String>,
    pub stroke_width: Option<f32>,
    pub radius: Option<f32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ShapeKind {
    Rect,
    RoundRect,
    Ellipse,
    Line,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateAsset {
    pub id: String,
    pub name: String,
    pub path: String,
    pub source_layer_id: Option<String>,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateSummary {
    pub id: String,
    pub name: String,
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub preview_path: Option<String>,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchRow {
    pub id: String,
    pub index: usize,
    pub values: std::collections::HashMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchDataPreview {
    pub fields: Vec<String>,
    pub rows: Vec<BatchRow>,
    pub unused_fields: Vec<String>,
    pub missing_fields: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub template_id: String,
    pub output_dir: String,
    pub rows: Vec<BatchRow>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportFailure {
    pub row_index: usize,
    pub field: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub total: usize,
    pub succeeded: usize,
    pub outputs: Vec<String>,
    pub failed: Vec<ExportFailure>,
}
```

- [ ] **Step 3: Write failing storage integration test**

Create `src-tauri/tests/ecommerce_storage.rs` with this content:

```rust
use std::fs;

use attool_lib::ecommerce::{models::*, storage::EcommerceStore};

fn sample_project() -> TemplateProject {
    TemplateProject {
        id: "tpl-test".to_string(),
        name: "双人椅主图".to_string(),
        canvas_width: 1000,
        canvas_height: 1000,
        layers: vec![TemplateLayer {
            id: "layer-title".to_string(),
            name: "{{title}} 大标题".to_string(),
            r#type: TemplateLayerType::Text,
            x: 10.0,
            y: 900.0,
            width: 980.0,
            height: 80.0,
            visible: true,
            opacity: 1.0,
            rotation: 0.0,
            binding_key: Some("title".to_string()),
            locked: Some(false),
            children: None,
            text: Some(TextLayerData {
                text: "便携小沙发".to_string(),
                font_family: "STHupo".to_string(),
                font_size: 48.0,
                font_weight: serde_json::json!(700),
                color: "#ffffff".to_string(),
                stroke_color: None,
                stroke_width: None,
                letter_spacing: None,
                line_height: None,
                align: Some(TextAlign::Center),
            }),
            image: None,
            shape: None,
        }],
        assets: vec![],
        source_psd_path: Some("/tmp/source.psd".to_string()),
        preview_path: None,
        created_at: "2026-05-07 00:00:00".to_string(),
        updated_at: "2026-05-07 00:00:00".to_string(),
    }
}

#[test]
fn saves_lists_and_loads_templates() {
    let root = std::env::temp_dir().join(format!("attool-store-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let store = EcommerceStore::new(root.clone()).unwrap();

    let saved = store.save_template(sample_project()).unwrap();
    assert_eq!(saved.name, "双人椅主图");

    let summaries = store.list_templates().unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, "tpl-test");

    let loaded = store.load_template("tpl-test").unwrap();
    assert_eq!(loaded.layers[0].binding_key.as_deref(), Some("title"));

    fs::remove_dir_all(root).unwrap();
}
```

- [ ] **Step 4: Run storage test and verify failure**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test ecommerce_storage
```

Expected: FAIL because `attool_lib::ecommerce` does not exist.

- [ ] **Step 5: Implement module and storage**

Create `src-tauri/src/ecommerce/mod.rs` with this content:

```rust
pub mod models;
pub mod storage;

pub use storage::EcommerceStore;
```

Modify `src-tauri/src/lib.rs` near the top to expose the module:

```rust
pub mod ecommerce;
```

Create `src-tauri/src/ecommerce/storage.rs` with this content:

```rust
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
```

- [ ] **Step 6: Run storage test and commit**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test ecommerce_storage
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: both commands PASS.

Commit:

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/lib.rs src-tauri/src/ecommerce/mod.rs src-tauri/src/ecommerce/models.rs src-tauri/src/ecommerce/storage.rs src-tauri/tests/ecommerce_storage.rs
git commit -m "feat: add ecommerce template storage"
```

## Task 3: PSD Bridge Import MVP

**Files:**
- Create: `src-tauri/python/psd_template_bridge.py`
- Create: `src-tauri/src/ecommerce/psd_bridge.rs`
- Create: `src-tauri/tests/ecommerce_psd_bridge.rs`
- Modify: `src-tauri/src/ecommerce/mod.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `README.md`

- [ ] **Step 1: Add Rust dependency for temporary directories in tests**

Edit `src-tauri/Cargo.toml` dependencies:

```toml
tempfile = "3"
```

Run:

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 2: Create Python PSD bridge script**

Create `src-tauri/python/psd_template_bridge.py` with this content:

```python
#!/usr/bin/env python3
import argparse
import json
import re
import sys
import uuid
from datetime import datetime
from pathlib import Path

from psd_tools import PSDImage

BINDING_RE = re.compile(r"\{\{\s*([a-zA-Z][a-zA-Z0-9_]*)\s*\}\}")
SUGGESTED_BINDINGS = {
    "商品图": "product_image",
    "LOGO": "logo",
    "大标题": "title",
    "右下角标题": "install_note",
    "户外/家用/店铺/办公": "usage_text",
    "提带设计 单手提拿": "feature_text",
}


def now_text():
    return datetime.now().strftime("%Y-%m-%d %H:%M:%S")


def binding_for(name):
    match = BINDING_RE.search(name)
    if match:
        return match.group(1)
    return SUGGESTED_BINDINGS.get(name)


def rgba_to_hex(values, fallback="#111111"):
    if not values or len(values) < 4:
        return fallback
    rgb = values[1:4]
    return "#" + "".join(f"{max(0, min(255, round(channel * 255))):02x}" for channel in rgb)


def first_text_style(layer):
    result = {
        "fontFamily": "PingFang SC",
        "fontSize": max(12, layer.bbox.height or 24),
        "fontWeight": 700,
        "color": "#111111",
        "align": "left",
    }
    try:
        engine = layer.engine_dict
        resources = layer.resource_dict
        font_set = resources.get("FontSet", [])
        run_array = engine.get("StyleRun", {}).get("RunArray", [])
        if run_array:
            data = run_array[0].get("StyleSheet", {}).get("StyleSheetData", {})
            font_index = data.get("Font")
            if isinstance(font_index, int) and font_index < len(font_set):
                result["fontFamily"] = str(font_set[font_index].get("Name", result["fontFamily"]))
            if "FontSize" in data:
                result["fontSize"] = float(data["FontSize"])
            if "FillColor" in data:
                result["color"] = rgba_to_hex(data["FillColor"].get("Values"))
    except Exception:
        pass
    return result


def save_layer_png(layer, asset_dir):
    image = layer.composite()
    if image is None:
        return None
    asset_id = f"asset-{uuid.uuid4().hex}"
    output = asset_dir / f"{asset_id}.png"
    image.save(output)
    return {
        "id": asset_id,
        "name": layer.name,
        "path": str(output),
        "sourceLayerId": None,
        "mimeType": "image/png",
        "width": image.width,
        "height": image.height,
    }


def layer_to_template(layer, asset_dir):
    bbox = layer.bbox
    layer_id = f"layer-{uuid.uuid4().hex}"
    base = {
        "id": layer_id,
        "name": layer.name,
        "type": "group" if layer.is_group() else "image",
        "x": float(bbox.x1),
        "y": float(bbox.y1),
        "width": float(max(1, bbox.width)),
        "height": float(max(1, bbox.height)),
        "visible": bool(layer.visible),
        "opacity": float(getattr(layer, "opacity", 255)) / 255.0,
        "rotation": 0,
        "bindingKey": binding_for(layer.name),
        "locked": False,
    }

    assets = []
    if layer.is_group():
        children = []
        for child in layer:
            child_layer, child_assets = layer_to_template(child, asset_dir)
            children.append(child_layer)
            assets.extend(child_assets)
        base["children"] = children
        return base, assets

    if layer.kind == "type":
        style = first_text_style(layer)
        base["type"] = "text"
        base["text"] = {
            "text": layer.text or "",
            "fontFamily": style["fontFamily"],
            "fontSize": style["fontSize"],
            "fontWeight": style["fontWeight"],
            "color": style["color"],
            "align": style["align"],
        }
        return base, assets

    asset = save_layer_png(layer, asset_dir)
    if asset:
        asset["sourceLayerId"] = layer_id
        assets.append(asset)
        base["type"] = "image"
        base["image"] = {
            "assetId": asset["id"],
            "fit": "stretch",
            "replaceable": bool(base.get("bindingKey")) or layer.kind in ("smartobject", "pixel"),
        }
        return base, assets

    base["type"] = "shape"
    base["shape"] = {"shape": "rect", "fill": "rgba(0,0,0,0)", "strokeWidth": 0}
    return base, assets


def import_psd(psd_path, output_dir):
    psd = PSDImage.open(psd_path)
    if str(psd.color_mode).split(".")[-1] != "RGB" or psd.depth != 8:
        raise ValueError("当前只支持 RGB / 8-bit PSD")

    template_id = f"tpl-{uuid.uuid4().hex}"
    template_dir = output_dir / template_id
    asset_dir = template_dir / "assets"
    asset_dir.mkdir(parents=True, exist_ok=True)

    preview_path = template_dir / "preview.png"
    psd.composite().save(preview_path)

    layers = []
    assets = []
    for layer in psd:
        converted, layer_assets = layer_to_template(layer, asset_dir)
        layers.append(converted)
        assets.extend(layer_assets)

    timestamp = now_text()
    return {
        "id": template_id,
        "name": Path(psd_path).stem,
        "canvasWidth": psd.width,
        "canvasHeight": psd.height,
        "layers": layers,
        "assets": assets,
        "sourcePsdPath": str(psd_path),
        "previewPath": str(preview_path),
        "createdAt": timestamp,
        "updatedAt": timestamp,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--psd", required=True)
    parser.add_argument("--output-dir", required=True)
    args = parser.parse_args()
    result = import_psd(Path(args.psd), Path(args.output_dir))
    print(json.dumps(result, ensure_ascii=False))


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(json.dumps({"error": str(exc)}, ensure_ascii=False), file=sys.stderr)
        sys.exit(1)
```

- [ ] **Step 3: Create psd bridge Rust wrapper**

Create `src-tauri/src/ecommerce/psd_bridge.rs` with this content:

```rust
use std::{path::Path, process::Command};

use super::models::TemplateProject;

pub fn import_psd_with_bridge(
    bridge_script: &Path,
    psd_path: &Path,
    output_dir: &Path,
) -> Result<TemplateProject, String> {
    if !psd_path.is_file() {
        return Err("PSD 文件不存在".to_string());
    }
    if psd_path.extension().and_then(|value| value.to_str()) != Some("psd") {
        return Err("请选择 PSD 文件".to_string());
    }

    std::fs::create_dir_all(output_dir).map_err(|error| format!("创建模板输出目录失败：{error}"))?;
    let output = Command::new("python3")
        .arg(bridge_script)
        .arg("--psd")
        .arg(psd_path)
        .arg("--output-dir")
        .arg(output_dir)
        .output()
        .map_err(|error| format!("启动 PSD 解析器失败：{error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("PSD 解析失败：{}", stderr.trim()));
    }

    serde_json::from_slice::<TemplateProject>(&output.stdout)
        .map_err(|error| format!("读取 PSD 解析结果失败：{error}"))
}
```

Modify `src-tauri/src/ecommerce/mod.rs`:

```rust
pub mod models;
pub mod psd_bridge;
pub mod storage;

pub use storage::EcommerceStore;
```

- [ ] **Step 4: Write bridge smoke test using the user's PSD**

Create `src-tauri/tests/ecommerce_psd_bridge.rs` with this content:

```rust
use std::path::PathBuf;

use attool_lib::ecommerce::psd_bridge::import_psd_with_bridge;

#[test]
fn imports_user_psd_when_available() {
    let psd_path = PathBuf::from("/Users/attson/Documents/店铺/双人椅/双人主图2+活动 - 4链接 拷贝 2.psd");
    if !psd_path.exists() {
        eprintln!("skipping user PSD smoke test because file is not present");
        return;
    }

    let bridge = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("python/psd_template_bridge.py");
    let output = tempfile::tempdir().unwrap();
    let project = import_psd_with_bridge(&bridge, &psd_path, output.path()).unwrap();

    assert_eq!(project.canvas_width, 1000);
    assert_eq!(project.canvas_height, 1000);
    assert!(project.layers.len() >= 10);
    assert!(project.assets.iter().any(|asset| asset.name.contains("LOGO")));
}
```

- [ ] **Step 5: Install bridge Python dependency locally and run test**

Run:

```bash
python3 -m pip install --user psd-tools
cargo test --manifest-path src-tauri/Cargo.toml --test ecommerce_psd_bridge
```

Expected: PASS on this machine. If `psd-tools` is already installed, pip prints `Requirement already satisfied` and the cargo test still passes.

- [ ] **Step 6: Document PSD bridge dependency and commit**

Add this section to `README.md` under local dependencies:

```md
### 电商 PSD 模板导入

PSD 导入首版通过本机 Python 解析 PSD 图层结构，需要安装 psd-tools：

```bash
python3 -m pip install --user psd-tools
```

PSD 图层名可用 `{{field_name}}` 标记批量替换字段，例如 `{{product_image}} 商品图`、`{{title}} 大标题`、`{{bottom_title}} 底部文案`。
```

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test ecommerce_psd_bridge
```

Expected: PASS.

Commit:

```bash
git add README.md src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/python/psd_template_bridge.py src-tauri/src/ecommerce/mod.rs src-tauri/src/ecommerce/psd_bridge.rs src-tauri/tests/ecommerce_psd_bridge.rs
git commit -m "feat: import ecommerce templates from psd"
```

## Task 4: Backend Commands For Templates

**Files:**
- Create: `src-tauri/src/ecommerce/commands.rs`
- Modify: `src-tauri/src/ecommerce/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create command module**

Create `src-tauri/src/ecommerce/commands.rs` with this content:

```rust
use std::path::PathBuf;

use tauri::{AppHandle, Manager, State};

use super::{
    models::{TemplateProject, TemplateSummary},
    psd_bridge::import_psd_with_bridge,
    storage::EcommerceStore,
};

#[tauri::command]
pub async fn import_psd_template(
    psd_path: String,
    app: AppHandle,
    store: State<'_, EcommerceStore>,
) -> Result<TemplateProject, String> {
    let bridge = app
        .path()
        .resource_dir()
        .ok()
        .map(|dir| dir.join("python/psd_template_bridge.py"))
        .filter(|path| path.exists())
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("python/psd_template_bridge.py"));
    let project = import_psd_with_bridge(&bridge, PathBuf::from(psd_path).as_path(), store.root_dir())?;
    store.save_template(project)
}

#[tauri::command]
pub async fn list_ecommerce_templates(
    store: State<'_, EcommerceStore>,
) -> Result<Vec<TemplateSummary>, String> {
    store.list_templates()
}

#[tauri::command]
pub async fn load_ecommerce_template(
    id: String,
    store: State<'_, EcommerceStore>,
) -> Result<TemplateProject, String> {
    store.load_template(&id)
}

#[tauri::command]
pub async fn save_ecommerce_template(
    project: TemplateProject,
    store: State<'_, EcommerceStore>,
) -> Result<TemplateProject, String> {
    store.save_template(project)
}
```

Modify `src-tauri/src/ecommerce/mod.rs`:

```rust
pub mod commands;
pub mod models;
pub mod psd_bridge;
pub mod storage;

pub use storage::EcommerceStore;
```

- [ ] **Step 2: Register store and commands in Tauri app**

Modify `src-tauri/src/lib.rs` imports to include the store:

```rust
use ecommerce::EcommerceStore;
```

In `setup`, after the existing download state is created and managed, add:

```rust
let ecommerce_dir = app
    .path()
    .app_data_dir()
    .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?
    .join("ecommerce");
let ecommerce_store = EcommerceStore::new(ecommerce_dir)
    .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
app.manage(ecommerce_store);
```

Add these command names to `tauri::generate_handler!`:

```rust
ecommerce::commands::import_psd_template,
ecommerce::commands::list_ecommerce_templates,
ecommerce::commands::load_ecommerce_template,
ecommerce::commands::save_ecommerce_template
```

- [ ] **Step 3: Run backend checks and commit**

Run:

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml --test ecommerce_storage
cargo test --manifest-path src-tauri/Cargo.toml --test ecommerce_psd_bridge
```

Expected: all commands PASS.

Commit:

```bash
git add src-tauri/src/lib.rs src-tauri/src/ecommerce/mod.rs src-tauri/src/ecommerce/commands.rs
git commit -m "feat: expose ecommerce template commands"
```

## Task 5: Editor UI Shell

**Files:**
- Create: `src/components/ecommerce/templateDefaults.ts`
- Create: `src/components/ecommerce/TemplateTool.vue`
- Create: `src/components/ecommerce/LayerTree.vue`
- Create: `src/components/ecommerce/TemplateCanvas.vue`
- Create: `src/components/ecommerce/LayerProperties.vue`
- Modify: `src/App.vue`
- Modify: `src/styles.css`

- [ ] **Step 1: Create defaults module**

Create `src/components/ecommerce/templateDefaults.ts` with this content:

```ts
import type { TemplateProject } from '../../types/ecommerceTemplate';

export function createEmptyTemplateProject(): TemplateProject {
  const timestamp = new Date().toLocaleString();
  return {
    id: `tpl-${crypto.randomUUID()}`,
    name: '未命名主图模板',
    canvasWidth: 1000,
    canvasHeight: 1000,
    layers: [],
    assets: [],
    createdAt: timestamp,
    updatedAt: timestamp
  };
}
```

- [ ] **Step 2: Create layer tree component**

Create `src/components/ecommerce/LayerTree.vue` with this content:

```vue
<script setup lang="ts">
import { NEllipsis, NEmpty, NTag } from 'naive-ui';
import type { TemplateLayer } from '../../types/ecommerceTemplate';

const props = defineProps<{
  layers: TemplateLayer[];
  selectedLayerId: string | null;
}>();

const emit = defineEmits<{
  select: [layerId: string];
}>();

function renderType(layer: TemplateLayer) {
  const labels = { text: '文字', image: '图片', shape: '形状', group: '组合' };
  return labels[layer.type];
}
</script>

<template>
  <div class="template-layer-tree">
    <n-empty v-if="props.layers.length === 0" description="还没有图层" />
    <template v-for="layer in props.layers" :key="layer.id">
      <button
        type="button"
        :class="['template-layer-item', { active: layer.id === props.selectedLayerId }]"
        @click="emit('select', layer.id)"
      >
        <n-ellipsis :tooltip="false">{{ layer.name }}</n-ellipsis>
        <n-tag size="small" round>{{ renderType(layer) }}</n-tag>
      </button>
      <button
        v-for="child in layer.children ?? []"
        :key="child.id"
        type="button"
        :class="['template-layer-item', 'child', { active: child.id === props.selectedLayerId }]"
        @click="emit('select', child.id)"
      >
        <n-ellipsis :tooltip="false">{{ child.name }}</n-ellipsis>
        <n-tag size="small" round>{{ renderType(child) }}</n-tag>
      </button>
    </template>
  </div>
</template>
```

- [ ] **Step 3: Create canvas component**

Create `src/components/ecommerce/TemplateCanvas.vue` with this content:

```vue
<script setup lang="ts">
import { computed } from 'vue';
import { convertFileSrc } from '@tauri-apps/api/core';
import type { TemplateAsset, TemplateLayer } from '../../types/ecommerceTemplate';
import { flattenLayers } from '../../utils/ecommerceTemplate';

const props = defineProps<{
  canvasWidth: number;
  canvasHeight: number;
  layers: TemplateLayer[];
  assets: TemplateAsset[];
  selectedLayerId: string | null;
}>();

const emit = defineEmits<{
  select: [layerId: string];
}>();

const flatLayers = computed(() => flattenLayers(props.layers).filter((layer) => layer.type !== 'group' && layer.visible));
const canvasStyle = computed(() => ({ aspectRatio: `${props.canvasWidth} / ${props.canvasHeight}` }));

function layerStyle(layer: TemplateLayer) {
  return {
    left: `${(layer.x / props.canvasWidth) * 100}%`,
    top: `${(layer.y / props.canvasHeight) * 100}%`,
    width: `${(layer.width / props.canvasWidth) * 100}%`,
    height: `${(layer.height / props.canvasHeight) * 100}%`,
    opacity: layer.opacity,
    transform: `rotate(${layer.rotation}deg)`
  };
}

function assetSrc(layer: TemplateLayer) {
  const asset = props.assets.find((item) => item.id === layer.image?.assetId);
  return asset ? convertFileSrc(asset.path) : '';
}
</script>

<template>
  <div class="template-canvas-wrap">
    <div class="template-canvas" :style="canvasStyle">
      <button
        v-for="layer in flatLayers"
        :key="layer.id"
        type="button"
        :class="['template-canvas-layer', layer.type, { selected: layer.id === selectedLayerId }]"
        :style="layerStyle(layer)"
        @click.stop="emit('select', layer.id)"
      >
        <span v-if="layer.type === 'text'" class="template-text-layer" :style="{ color: layer.text?.color, fontSize: `${layer.text?.fontSize ?? 24}px`, fontFamily: layer.text?.fontFamily }">
          {{ layer.text?.text }}
        </span>
        <img v-else-if="layer.type === 'image' && assetSrc(layer)" :src="assetSrc(layer)" alt="模板图片图层" draggable="false" />
        <span v-else-if="layer.type === 'shape'" class="template-shape-layer" :style="{ background: layer.shape?.fill, borderColor: layer.shape?.stroke, borderWidth: `${layer.shape?.strokeWidth ?? 0}px`, borderRadius: `${layer.shape?.radius ?? 0}px` }" />
      </button>
    </div>
  </div>
</template>
```

- [ ] **Step 4: Create property panel component**

Create `src/components/ecommerce/LayerProperties.vue` with this content:

```vue
<script setup lang="ts">
import { computed } from 'vue';
import { NEmpty, NForm, NFormItem, NInput, NInputNumber, NSelect } from 'naive-ui';
import type { TemplateLayer } from '../../types/ecommerceTemplate';

const props = defineProps<{ layer: TemplateLayer | null }>();
const emit = defineEmits<{ update: [layer: TemplateLayer] }>();

const selected = computed(() => props.layer);
const fitOptions = [
  { label: '覆盖', value: 'cover' },
  { label: '完整显示', value: 'contain' },
  { label: '拉伸', value: 'stretch' }
];

function patch(values: Partial<TemplateLayer>) {
  if (!props.layer) return;
  emit('update', { ...props.layer, ...values });
}
</script>

<template>
  <n-empty v-if="!selected" description="请选择一个图层" />
  <n-form v-else label-placement="top" size="small">
    <n-form-item label="图层名">
      <n-input :value="selected.name" @update:value="patch({ name: $event })" />
    </n-form-item>
    <n-form-item label="绑定字段">
      <n-input :value="selected.bindingKey" placeholder="例如 title" @update:value="patch({ bindingKey: $event || undefined })" />
    </n-form-item>
    <n-form-item label="X / Y / 宽 / 高">
      <div class="template-prop-grid">
        <n-input-number :value="selected.x" @update:value="patch({ x: $event ?? 0 })" />
        <n-input-number :value="selected.y" @update:value="patch({ y: $event ?? 0 })" />
        <n-input-number :value="selected.width" :min="1" @update:value="patch({ width: $event ?? 1 })" />
        <n-input-number :value="selected.height" :min="1" @update:value="patch({ height: $event ?? 1 })" />
      </div>
    </n-form-item>
    <template v-if="selected.type === 'text' && selected.text">
      <n-form-item label="文字内容">
        <n-input :value="selected.text.text" type="textarea" @update:value="patch({ text: { ...selected.text!, text: $event } })" />
      </n-form-item>
      <n-form-item label="字号">
        <n-input-number :value="selected.text.fontSize" :min="1" @update:value="patch({ text: { ...selected.text!, fontSize: $event ?? 12 } })" />
      </n-form-item>
      <n-form-item label="颜色">
        <n-input :value="selected.text.color" @update:value="patch({ text: { ...selected.text!, color: $event } })" />
      </n-form-item>
    </template>
    <template v-if="selected.type === 'image' && selected.image">
      <n-form-item label="裁剪方式">
        <n-select :value="selected.image.fit" :options="fitOptions" @update:value="patch({ image: { ...selected.image!, fit: $event } })" />
      </n-form-item>
    </template>
  </n-form>
</template>
```

- [ ] **Step 5: Create top-level template tool**

Create `src/components/ecommerce/TemplateTool.vue` with this content:

```vue
<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { NAlert, NButton, NCard, NGrid, NGridItem, NPageHeader, NSpace, NTag } from 'naive-ui';
import type { TemplateLayer, TemplateProject, TemplateSummary } from '../../types/ecommerceTemplate';
import { flattenLayers } from '../../utils/ecommerceTemplate';
import LayerProperties from './LayerProperties.vue';
import LayerTree from './LayerTree.vue';
import TemplateCanvas from './TemplateCanvas.vue';
import { createEmptyTemplateProject } from './templateDefaults';

const templates = ref<TemplateSummary[]>([]);
const project = ref<TemplateProject>(createEmptyTemplateProject());
const selectedLayerId = ref<string | null>(null);
const notice = ref('');
const importing = ref(false);
const saving = ref(false);

const selectedLayer = computed(() => flattenLayers(project.value.layers).find((layer) => layer.id === selectedLayerId.value) ?? null);

onMounted(loadTemplateList);

async function loadTemplateList() {
  templates.value = await invoke<TemplateSummary[]>('list_ecommerce_templates');
}

async function importPsd() {
  notice.value = '';
  importing.value = true;
  try {
    const selected = await open({ multiple: false, filters: [{ name: 'PSD', extensions: ['psd'] }] });
    if (typeof selected === 'string') {
      project.value = await invoke<TemplateProject>('import_psd_template', { psdPath: selected });
      selectedLayerId.value = project.value.layers[0]?.id ?? null;
      await loadTemplateList();
    }
  } catch (error) {
    notice.value = String(error);
  } finally {
    importing.value = false;
  }
}

async function saveTemplate() {
  notice.value = '';
  saving.value = true;
  try {
    project.value = await invoke<TemplateProject>('save_ecommerce_template', { project: project.value });
    await loadTemplateList();
  } catch (error) {
    notice.value = String(error);
  } finally {
    saving.value = false;
  }
}

function selectLayer(layerId: string) {
  selectedLayerId.value = layerId;
}

function updateLayer(updated: TemplateLayer) {
  const replace = (layers: TemplateLayer[]): TemplateLayer[] =>
    layers.map((layer) => {
      if (layer.id === updated.id) return updated;
      if (layer.children) return { ...layer, children: replace(layer.children) };
      return layer;
    });
  project.value = { ...project.value, layers: replace(project.value.layers), updatedAt: new Date().toLocaleString() };
}
</script>

<template>
  <n-space vertical :size="16">
    <n-page-header subtitle="导入 PSD 生成模板草稿，绑定字段后批量导出 PNG 主图。">
      <template #title>电商主图模板</template>
      <template #extra>
        <n-space>
          <n-tag round>{{ project.canvasWidth }}x{{ project.canvasHeight }}</n-tag>
          <n-button secondary :loading="saving" @click="saveTemplate">保存模板</n-button>
          <n-button type="primary" :loading="importing" @click="importPsd">导入 PSD</n-button>
        </n-space>
      </template>
    </n-page-header>

    <n-alert v-if="notice" type="error" :bordered="false">{{ notice }}</n-alert>

    <n-grid responsive="screen" cols="1 l:5" :x-gap="12" :y-gap="12">
      <n-grid-item span="1">
        <n-card title="图层" size="small" :bordered="false" class="panel-card template-editor-panel">
          <LayerTree :layers="project.layers" :selected-layer-id="selectedLayerId" @select="selectLayer" />
        </n-card>
      </n-grid-item>
      <n-grid-item span="1 l:3">
        <n-card title="画布" size="small" :bordered="false" class="panel-card template-canvas-card">
          <TemplateCanvas :canvas-width="project.canvasWidth" :canvas-height="project.canvasHeight" :layers="project.layers" :assets="project.assets" :selected-layer-id="selectedLayerId" @select="selectLayer" />
        </n-card>
      </n-grid-item>
      <n-grid-item span="1">
        <n-card title="属性" size="small" :bordered="false" class="panel-card template-editor-panel">
          <LayerProperties :layer="selectedLayer" @update="updateLayer" />
        </n-card>
      </n-grid-item>
    </n-grid>
  </n-space>
</template>
```

- [ ] **Step 6: Wire tool into App.vue**

Modify `src/App.vue`:

Add import:

```ts
import TemplateTool from './components/ecommerce/TemplateTool.vue';
```

Add a tool entry to `tools`:

```ts
{
  id: 'template',
  name: '主图模板',
  description: 'PSD 导入、字段替换、批量生成主图',
  badge: 'New',
  active: true
}
```

Add a template branch before the existing image branch:

```vue
<template v-else-if="selectedTool.id === 'template'">
  <TemplateTool />
</template>
```

- [ ] **Step 7: Add editor styles**

Append to `src/styles.css`:

```css
.template-editor-panel .n-card__content {
  max-height: 72vh;
  overflow: auto;
}

.template-layer-tree {
  display: grid;
  gap: 8px;
}

.template-layer-item {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 8px;
  align-items: center;
  border: 1px solid rgba(23, 33, 27, 0.08);
  border-radius: 12px;
  background: rgba(255, 250, 238, 0.68);
  color: var(--ink);
  padding: 8px 10px;
  text-align: left;
}

.template-layer-item.child {
  margin-left: 18px;
}

.template-layer-item.active {
  border-color: rgba(86, 113, 93, 0.42);
  background: rgba(86, 113, 93, 0.14);
}

.template-canvas-wrap {
  display: grid;
  place-items: center;
  min-height: 68vh;
  overflow: auto;
}

.template-canvas {
  position: relative;
  width: min(68vh, 100%);
  max-width: 100%;
  border: 1px solid rgba(23, 33, 27, 0.16);
  background: #fff;
  box-shadow: 0 18px 50px rgba(23, 33, 27, 0.14);
}

.template-canvas-layer {
  position: absolute;
  overflow: hidden;
  border: 1px solid transparent;
  background: transparent;
  padding: 0;
  text-align: left;
}

.template-canvas-layer.selected {
  border-color: #2f8f46;
  box-shadow: 0 0 0 2px rgba(47, 143, 70, 0.22);
}

.template-canvas-layer img,
.template-shape-layer {
  display: block;
  width: 100%;
  height: 100%;
}

.template-text-layer {
  display: block;
  white-space: pre-wrap;
  line-height: 1.1;
}

.template-prop-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}
```

- [ ] **Step 8: Run frontend verification and commit**

Run:

```bash
npm run test -- src/utils/ecommerceTemplate.test.ts
npm run build
```

Expected: both commands PASS.

Commit:

```bash
git add src/App.vue src/styles.css src/components/ecommerce src/types/ecommerceTemplate.ts src/utils/ecommerceTemplate.ts src/utils/ecommerceTemplate.test.ts
git commit -m "feat: add ecommerce template editor shell"
```

## Task 6: Canvas Drag And Resize Editing

**Files:**
- Modify: `src/components/ecommerce/TemplateCanvas.vue`
- Modify: `src/components/ecommerce/TemplateTool.vue`
- Modify: `src/styles.css`

- [ ] **Step 1: Extend TemplateCanvas events**

Modify `TemplateCanvas.vue` to emit layer updates:

```ts
const emit = defineEmits<{
  select: [layerId: string];
  update: [layer: TemplateLayer];
}>();
```

Add pointer state and functions inside `<script setup>`:

```ts
const interaction = ref<null | {
  mode: 'move' | 'resize';
  layer: TemplateLayer;
  startX: number;
  startY: number;
  startLayerX: number;
  startLayerY: number;
  startWidth: number;
  startHeight: number;
}>(null);

function startMove(event: PointerEvent, layer: TemplateLayer) {
  emit('select', layer.id);
  interaction.value = {
    mode: 'move',
    layer,
    startX: event.clientX,
    startY: event.clientY,
    startLayerX: layer.x,
    startLayerY: layer.y,
    startWidth: layer.width,
    startHeight: layer.height
  };
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}

function startResize(event: PointerEvent, layer: TemplateLayer) {
  event.stopPropagation();
  interaction.value = {
    mode: 'resize',
    layer,
    startX: event.clientX,
    startY: event.clientY,
    startLayerX: layer.x,
    startLayerY: layer.y,
    startWidth: layer.width,
    startHeight: layer.height
  };
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}

function movePointer(event: PointerEvent) {
  if (!interaction.value) return;
  const canvas = (event.currentTarget as HTMLElement).closest('.template-canvas');
  if (!canvas) return;
  const rect = canvas.getBoundingClientRect();
  const dx = ((event.clientX - interaction.value.startX) / rect.width) * props.canvasWidth;
  const dy = ((event.clientY - interaction.value.startY) / rect.height) * props.canvasHeight;
  const source = interaction.value.layer;
  if (interaction.value.mode === 'move') {
    emit('update', {
      ...source,
      x: Math.max(0, Math.min(props.canvasWidth - source.width, interaction.value.startLayerX + dx)),
      y: Math.max(0, Math.min(props.canvasHeight - source.height, interaction.value.startLayerY + dy))
    });
  } else {
    emit('update', {
      ...source,
      width: Math.max(8, Math.min(props.canvasWidth - source.x, interaction.value.startWidth + dx)),
      height: Math.max(8, Math.min(props.canvasHeight - source.y, interaction.value.startHeight + dy))
    });
  }
}

function stopPointer(event: PointerEvent) {
  interaction.value = null;
  try {
    (event.target as HTMLElement).releasePointerCapture(event.pointerId);
  } catch {
    // The browser can release capture before pointerup during fast drags.
  }
}
```

Update the layer button template:

```vue
@pointerdown="startMove($event, layer)"
@pointermove="movePointer"
@pointerup="stopPointer"
@pointercancel="stopPointer"
```

Add a resize handle inside each selected layer:

```vue
<span v-if="layer.id === selectedLayerId" class="template-resize-handle" @pointerdown="startResize($event, layer)" />
```

- [ ] **Step 2: Wire update event in TemplateTool**

Modify `TemplateTool.vue` canvas usage:

```vue
<TemplateCanvas
  :canvas-width="project.canvasWidth"
  :canvas-height="project.canvasHeight"
  :layers="project.layers"
  :assets="project.assets"
  :selected-layer-id="selectedLayerId"
  @select="selectLayer"
  @update="updateLayer"
/>
```

- [ ] **Step 3: Add resize handle styles**

Append to `src/styles.css`:

```css
.template-resize-handle {
  position: absolute;
  right: -6px;
  bottom: -6px;
  width: 12px;
  height: 12px;
  border: 2px solid #fff;
  border-radius: 50%;
  background: var(--moss);
  box-shadow: 0 2px 8px rgba(23, 33, 27, 0.28);
  cursor: nwse-resize;
}
```

- [ ] **Step 4: Verify and commit**

Run:

```bash
npm run build
```

Expected: PASS.

Manual check with `npm run tauri:dev`: import the sample PSD, drag the `大标题` layer, resize it, and confirm the right-side X/Y/W/H values update.

Commit:

```bash
git add src/components/ecommerce/TemplateCanvas.vue src/components/ecommerce/TemplateTool.vue src/styles.css
git commit -m "feat: support template canvas drag editing"
```

## Task 7: Batch Import Backend

**Files:**
- Create: `src-tauri/src/ecommerce/batch.rs`
- Create: `src-tauri/tests/ecommerce_batch.rs`
- Modify: `src-tauri/src/ecommerce/mod.rs`
- Modify: `src-tauri/src/ecommerce/commands.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`

- [ ] **Step 1: Add batch dependencies**

Edit `src-tauri/Cargo.toml` dependencies:

```toml
calamine = "0.34"
csv = "1"
```

Run:

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 2: Write failing batch tests**

Create `src-tauri/tests/ecommerce_batch.rs` with this content:

```rust
use std::{collections::HashMap, fs};

use attool_lib::ecommerce::{batch::{batch_from_folder, read_batch_table, validate_batch_fields}, models::BatchDataPreview};

#[test]
fn reads_csv_batch_table() {
    let dir = tempfile::tempdir().unwrap();
    let csv_path = dir.path().join("batch.csv");
    fs::write(&csv_path, "title,product_image\n双人椅,/tmp/a.png\n").unwrap();

    let preview = read_batch_table(&csv_path, &["title".to_string(), "product_image".to_string()]).unwrap();
    assert_eq!(preview.fields, vec!["title", "product_image"]);
    assert_eq!(preview.rows[0].values.get("title").unwrap(), "双人椅");
    assert!(preview.missing_fields.is_empty());
}

#[test]
fn reports_missing_and_unused_fields() {
    let result = validate_batch_fields(&["title".to_string(), "product_image".to_string()], &["title".to_string(), "price".to_string()]);
    assert_eq!(result.missing_fields, vec!["product_image"]);
    assert_eq!(result.unused_fields, vec!["price"]);
}

#[test]
fn creates_batch_from_image_folder() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.png"), b"not-real-image-but-valid-path").unwrap();
    fs::write(dir.path().join("notes.txt"), b"skip").unwrap();

    let preview = batch_from_folder(dir.path(), "product_image").unwrap();
    assert_eq!(preview.fields, vec!["product_image", "name"]);
    assert_eq!(preview.rows.len(), 1);
    assert_eq!(preview.rows[0].values.get("name").unwrap(), "a");
}
```

- [ ] **Step 3: Run batch tests and verify failure**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test ecommerce_batch
```

Expected: FAIL because `ecommerce::batch` does not exist.

- [ ] **Step 4: Implement batch module**

Create `src-tauri/src/ecommerce/batch.rs` with this content:

```rust
use std::{collections::HashMap, path::Path};

use calamine::{open_workbook_auto, Reader};
use uuid::Uuid;

use super::models::{BatchDataPreview, BatchRow};

pub fn validate_batch_fields(required: &[String], incoming: &[String]) -> BatchDataPreview {
    BatchDataPreview {
        fields: incoming.to_vec(),
        rows: Vec::new(),
        missing_fields: required.iter().filter(|field| !incoming.contains(field)).cloned().collect(),
        unused_fields: incoming.iter().filter(|field| !required.contains(field)).cloned().collect(),
    }
}

pub fn read_batch_table(path: &Path, required_fields: &[String]) -> Result<BatchDataPreview, String> {
    match path.extension().and_then(|value| value.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("csv") => read_csv(path, required_fields),
        Some("xlsx") | Some("xls") => read_excel(path, required_fields),
        _ => Err("请选择 CSV、XLS 或 XLSX 表格".to_string()),
    }
}

fn read_csv(path: &Path, required_fields: &[String]) -> Result<BatchDataPreview, String> {
    let mut reader = csv::Reader::from_path(path).map_err(|error| format!("读取 CSV 失败：{error}"))?;
    let fields: Vec<String> = reader
        .headers()
        .map_err(|error| format!("读取 CSV 表头失败：{error}"))?
        .iter()
        .map(str::to_string)
        .collect();
    let mut rows = Vec::new();
    for (index, record) in reader.records().enumerate() {
        let record = record.map_err(|error| format!("读取 CSV 第 {} 行失败：{error}", index + 2))?;
        let mut values = HashMap::new();
        for (field, value) in fields.iter().zip(record.iter()) {
            values.insert(field.clone(), value.to_string());
        }
        rows.push(BatchRow { id: Uuid::new_v4().to_string(), index, values });
    }
    Ok(with_field_validation(fields, rows, required_fields))
}

fn read_excel(path: &Path, required_fields: &[String]) -> Result<BatchDataPreview, String> {
    let mut workbook = open_workbook_auto(path).map_err(|error| format!("读取 Excel 失败：{error}"))?;
    let sheet_name = workbook.sheet_names().first().cloned().ok_or_else(|| "Excel 没有工作表".to_string())?;
    let range = workbook.worksheet_range(&sheet_name).map_err(|error| format!("读取 Excel 工作表失败：{error}"))?;
    let mut rows_iter = range.rows();
    let header = rows_iter.next().ok_or_else(|| "Excel 表格为空".to_string())?;
    let fields: Vec<String> = header.iter().map(|cell| cell.to_string()).collect();
    let mut rows = Vec::new();
    for (index, row) in rows_iter.enumerate() {
        let mut values = HashMap::new();
        for (field, cell) in fields.iter().zip(row.iter()) {
            values.insert(field.clone(), cell.to_string());
        }
        rows.push(BatchRow { id: Uuid::new_v4().to_string(), index, values });
    }
    Ok(with_field_validation(fields, rows, required_fields))
}

pub fn batch_from_folder(folder: &Path, image_binding_key: &str) -> Result<BatchDataPreview, String> {
    if !folder.is_dir() {
        return Err("请选择图片文件夹".to_string());
    }
    let mut paths: Vec<_> = std::fs::read_dir(folder)
        .map_err(|error| format!("读取图片文件夹失败：{error}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| is_supported_image(path))
        .collect();
    paths.sort();

    let mut rows = Vec::new();
    for (index, path) in paths.into_iter().enumerate() {
        let mut values = HashMap::new();
        values.insert(image_binding_key.to_string(), path.to_string_lossy().into_owned());
        values.insert(
            "name".to_string(),
            path.file_stem().and_then(|value| value.to_str()).unwrap_or("image").to_string(),
        );
        rows.push(BatchRow { id: Uuid::new_v4().to_string(), index, values });
    }

    Ok(BatchDataPreview {
        fields: vec![image_binding_key.to_string(), "name".to_string()],
        rows,
        missing_fields: Vec::new(),
        unused_fields: Vec::new(),
    })
}

fn with_field_validation(fields: Vec<String>, rows: Vec<BatchRow>, required_fields: &[String]) -> BatchDataPreview {
    let validation = validate_batch_fields(required_fields, &fields);
    BatchDataPreview { fields, rows, missing_fields: validation.missing_fields, unused_fields: validation.unused_fields }
}

fn is_supported_image(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()).map(str::to_ascii_lowercase).as_deref(),
        Some("png" | "jpg" | "jpeg" | "webp")
    )
}
```

Modify `src-tauri/src/ecommerce/mod.rs`:

```rust
pub mod batch;
pub mod commands;
pub mod models;
pub mod psd_bridge;
pub mod storage;

pub use storage::EcommerceStore;
```

- [ ] **Step 5: Add Tauri batch commands**

Add to `src-tauri/src/ecommerce/commands.rs`:

```rust
use super::batch::{batch_from_folder, read_batch_table};
use super::models::BatchDataPreview;

#[tauri::command]
pub async fn import_batch_table(
    path: String,
    required_fields: Vec<String>,
) -> Result<BatchDataPreview, String> {
    read_batch_table(PathBuf::from(path).as_path(), &required_fields)
}

#[tauri::command]
pub async fn create_batch_from_folder(
    folder_path: String,
    image_binding_key: String,
) -> Result<BatchDataPreview, String> {
    batch_from_folder(PathBuf::from(folder_path).as_path(), &image_binding_key)
}
```

Register commands in `src-tauri/src/lib.rs`:

```rust
ecommerce::commands::import_batch_table,
ecommerce::commands::create_batch_from_folder
```

- [ ] **Step 6: Run tests and commit**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test ecommerce_batch
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: both commands PASS.

Commit:

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/lib.rs src-tauri/src/ecommerce/mod.rs src-tauri/src/ecommerce/batch.rs src-tauri/src/ecommerce/commands.rs src-tauri/tests/ecommerce_batch.rs
git commit -m "feat: add ecommerce batch import"
```

## Task 8: Batch Panel UI

**Files:**
- Create: `src/components/ecommerce/BatchPanel.vue`
- Modify: `src/components/ecommerce/TemplateTool.vue`
- Modify: `src/styles.css`

- [ ] **Step 1: Create batch panel component**

Create `src/components/ecommerce/BatchPanel.vue` with this content:

```vue
<script setup lang="ts">
import { computed, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { NAlert, NButton, NDataTable, NEmpty, NSpace, NTag } from 'naive-ui';
import type { BatchDataPreview, BatchRow, ExportResult } from '../../types/ecommerceTemplate';

const props = defineProps<{
  templateId: string;
  requiredFields: string[];
}>();

const rows = ref<BatchRow[]>([]);
const preview = ref<BatchDataPreview | null>(null);
const outputDir = ref('');
const notice = ref('');
const importing = ref(false);
const exporting = ref(false);
const result = ref<ExportResult | null>(null);

const columns = computed(() =>
  (preview.value?.fields ?? []).slice(0, 8).map((field) => ({ title: field, key: field, render: (row: BatchRow) => row.values[field] ?? '' }))
);

async function importTable() {
  notice.value = '';
  importing.value = true;
  try {
    const selected = await open({ multiple: false, filters: [{ name: 'Table', extensions: ['csv', 'xlsx', 'xls'] }] });
    if (typeof selected === 'string') {
      preview.value = await invoke<BatchDataPreview>('import_batch_table', { path: selected, requiredFields: props.requiredFields });
      rows.value = preview.value.rows;
    }
  } catch (error) {
    notice.value = String(error);
  } finally {
    importing.value = false;
  }
}

async function importFolder() {
  notice.value = '';
  importing.value = true;
  try {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === 'string') {
      preview.value = await invoke<BatchDataPreview>('create_batch_from_folder', { folderPath: selected, imageBindingKey: 'product_image' });
      rows.value = preview.value.rows;
    }
  } catch (error) {
    notice.value = String(error);
  } finally {
    importing.value = false;
  }
}

async function chooseOutputDir() {
  const selected = await open({ directory: true, multiple: false, defaultPath: outputDir.value || undefined });
  if (typeof selected === 'string') outputDir.value = selected;
}

async function exportRows() {
  notice.value = '';
  result.value = null;
  if (!outputDir.value || rows.value.length === 0) {
    notice.value = '请选择输出目录并导入批量数据。';
    return;
  }
  exporting.value = true;
  try {
    result.value = await invoke<ExportResult>('export_ecommerce_images', {
      request: { templateId: props.templateId, outputDir: outputDir.value, rows: rows.value }
    });
  } catch (error) {
    notice.value = String(error);
  } finally {
    exporting.value = false;
  }
}
</script>

<template>
  <n-space vertical :size="12">
    <n-space>
      <n-button secondary :loading="importing" @click="importTable">导入表格</n-button>
      <n-button secondary :loading="importing" @click="importFolder">图片文件夹模式</n-button>
      <n-button secondary @click="chooseOutputDir">选择输出目录</n-button>
      <n-button type="primary" :loading="exporting" @click="exportRows">导出 PNG</n-button>
    </n-space>

    <n-alert v-if="notice" type="error" :bordered="false">{{ notice }}</n-alert>
    <n-alert v-if="preview?.missingFields.length" type="warning" :bordered="false">
      缺失字段：{{ preview.missingFields.join(', ') }}。缺失字段会使用模板默认值。
    </n-alert>
    <n-alert v-if="preview?.unusedFields.length" type="info" :bordered="false">
      未使用字段：{{ preview.unusedFields.join(', ') }}。
    </n-alert>

    <n-empty v-if="rows.length === 0" description="还没有批量数据" />
    <n-data-table v-else size="small" :columns="columns" :data="rows" :pagination="{ pageSize: 6 }" />

    <n-alert v-if="result" type="success" :bordered="false">
      共 {{ result.total }} 张，成功 {{ result.succeeded }} 张，失败 {{ result.failed.length }} 张。
    </n-alert>
    <n-space v-if="outputDir" align="center">
      <n-tag round>输出目录</n-tag><span class="template-output-dir">{{ outputDir }}</span>
    </n-space>
  </n-space>
</template>
```

- [ ] **Step 2: Add batch panel to TemplateTool**

Modify `TemplateTool.vue` imports:

```ts
import { collectBindingKeys } from '../../utils/ecommerceTemplate';
import BatchPanel from './BatchPanel.vue';
```

Add computed required fields:

```ts
const requiredFields = computed(() => collectBindingKeys(project.value.layers));
```

Add under the editor grid:

```vue
<n-card title="批量生成" size="small" :bordered="false" class="panel-card">
  <BatchPanel :template-id="project.id" :required-fields="requiredFields" />
</n-card>
```

- [ ] **Step 3: Add output directory style**

Append to `src/styles.css`:

```css
.template-output-dir {
  color: var(--muted);
  word-break: break-all;
}
```

- [ ] **Step 4: Verify and commit**

Run:

```bash
npm run build
```

Expected: PASS.

Commit:

```bash
git add src/components/ecommerce/BatchPanel.vue src/components/ecommerce/TemplateTool.vue src/styles.css
git commit -m "feat: add ecommerce batch panel"
```

## Task 9: PNG Export Renderer

**Files:**
- Create: `src-tauri/src/ecommerce/render.rs`
- Create: `src-tauri/tests/ecommerce_render.rs`
- Modify: `src-tauri/src/ecommerce/mod.rs`
- Modify: `src-tauri/src/ecommerce/commands.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`

- [ ] **Step 1: Write failing render test**

Create `src-tauri/tests/ecommerce_render.rs` with this content:

```rust
use std::{collections::HashMap, fs};

use attool_lib::ecommerce::{
    models::{BatchRow, ExportRequest, ImageLayerData, ImageFit, TemplateAsset, TemplateLayer, TemplateLayerType, TemplateProject},
    render::export_images,
    storage::EcommerceStore,
};

fn sample_project(asset_path: String) -> TemplateProject {
    TemplateProject {
        id: "tpl-render".to_string(),
        name: "render".to_string(),
        canvas_width: 120,
        canvas_height: 120,
        layers: vec![TemplateLayer {
            id: "image".to_string(),
            name: "{{product_image}} 商品图".to_string(),
            r#type: TemplateLayerType::Image,
            x: 0.0,
            y: 0.0,
            width: 120.0,
            height: 120.0,
            visible: true,
            opacity: 1.0,
            rotation: 0.0,
            binding_key: Some("product_image".to_string()),
            locked: Some(false),
            children: None,
            text: None,
            image: Some(ImageLayerData { asset_id: "asset".to_string(), fit: ImageFit::Stretch, replaceable: true }),
            shape: None,
        }],
        assets: vec![TemplateAsset { id: "asset".to_string(), name: "asset.png".to_string(), path: asset_path, source_layer_id: None, mime_type: "image/png".to_string(), width: 120, height: 120 }],
        source_psd_path: None,
        preview_path: None,
        created_at: "2026-05-07 00:00:00".to_string(),
        updated_at: "2026-05-07 00:00:00".to_string(),
    }
}

#[test]
fn exports_png_for_batch_row() {
    let dir = tempfile::tempdir().unwrap();
    let input_path = dir.path().join("input.png");
    image::RgbaImage::from_pixel(120, 120, image::Rgba([255, 0, 0, 255])).save(&input_path).unwrap();

    let store = EcommerceStore::new(dir.path().join("store")).unwrap();
    store.save_template(sample_project(input_path.to_string_lossy().into_owned())).unwrap();

    let output_dir = dir.path().join("out");
    let mut values = HashMap::new();
    values.insert("product_image".to_string(), input_path.to_string_lossy().into_owned());
    values.insert("name".to_string(), "red-chair".to_string());
    let request = ExportRequest {
        template_id: "tpl-render".to_string(),
        output_dir: output_dir.to_string_lossy().into_owned(),
        rows: vec![BatchRow { id: "row-1".to_string(), index: 0, values }],
    };

    let result = export_images(&store, request).unwrap();
    assert_eq!(result.succeeded, 1);
    assert!(fs::metadata(output_dir.join("red-chair.png")).unwrap().is_file());
}
```

- [ ] **Step 2: Run render test and verify failure**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test ecommerce_render
```

Expected: FAIL because `ecommerce::render` does not exist.

- [ ] **Step 3: Implement image-only renderer with shape/text hooks**

Create `src-tauri/src/ecommerce/render.rs` with this content:

```rust
use std::{path::{Path, PathBuf}};

use image::{imageops, DynamicImage, Rgba, RgbaImage};

use super::{models::*, storage::EcommerceStore};

pub fn export_images(store: &EcommerceStore, request: ExportRequest) -> Result<ExportResult, String> {
    let template = store.load_template(&request.template_id)?;
    let output_dir = PathBuf::from(request.output_dir.trim());
    if output_dir.as_os_str().is_empty() {
        return Err("请选择输出目录".to_string());
    }
    std::fs::create_dir_all(&output_dir).map_err(|error| format!("创建输出目录失败：{error}"))?;

    let mut outputs = Vec::new();
    let mut failed = Vec::new();
    for row in &request.rows {
        match render_row(&template, row, &output_dir) {
            Ok(path) => outputs.push(path.to_string_lossy().into_owned()),
            Err(message) => failed.push(ExportFailure { row_index: row.index, field: None, message }),
        }
    }

    Ok(ExportResult { total: request.rows.len(), succeeded: outputs.len(), outputs, failed })
}

fn render_row(template: &TemplateProject, row: &BatchRow, output_dir: &Path) -> Result<PathBuf, String> {
    let mut canvas = RgbaImage::from_pixel(template.canvas_width, template.canvas_height, Rgba([255, 255, 255, 255]));
    for layer in flatten_layers(&template.layers) {
        if !layer.visible || matches!(layer.r#type, TemplateLayerType::Group) {
            continue;
        }
        match layer.r#type {
            TemplateLayerType::Image => draw_image_layer(&mut canvas, template, layer, row)?,
            TemplateLayerType::Shape => draw_shape_layer(&mut canvas, layer),
            TemplateLayerType::Text => draw_text_layer_noop(&mut canvas, layer),
            TemplateLayerType::Group => {}
        }
    }
    let output_path = output_dir.join(make_export_file_name(&row.values, row.index));
    DynamicImage::ImageRgba8(canvas).save(&output_path).map_err(|error| format!("保存 PNG 失败：{error}"))?;
    Ok(output_path)
}

fn draw_image_layer(canvas: &mut RgbaImage, template: &TemplateProject, layer: &TemplateLayer, row: &BatchRow) -> Result<(), String> {
    let image_data = layer.image.as_ref().ok_or_else(|| format!("图片图层缺少素材：{}", layer.name))?;
    let source_path = layer
        .binding_key
        .as_ref()
        .and_then(|key| row.values.get(key))
        .filter(|path| !path.trim().is_empty())
        .cloned()
        .or_else(|| template.assets.iter().find(|asset| asset.id == image_data.asset_id).map(|asset| asset.path.clone()))
        .ok_or_else(|| format!("图片图层没有可用路径：{}", layer.name))?;
    let image = image::open(&source_path).map_err(|error| format!("读取图片失败 {source_path}：{error}"))?;
    let resized = image.resize_exact(layer.width.max(1.0) as u32, layer.height.max(1.0) as u32, imageops::FilterType::Lanczos3).to_rgba8();
    imageops::overlay(canvas, &resized, layer.x.round() as i64, layer.y.round() as i64);
    Ok(())
}

fn draw_shape_layer(canvas: &mut RgbaImage, layer: &TemplateLayer) {
    let color = parse_hex(layer.shape.as_ref().and_then(|shape| shape.fill.as_deref()).unwrap_or("#000000"));
    let min_x = layer.x.max(0.0) as u32;
    let min_y = layer.y.max(0.0) as u32;
    let max_x = (layer.x + layer.width).max(0.0) as u32;
    let max_y = (layer.y + layer.height).max(0.0) as u32;
    for y in min_y..max_y.min(canvas.height()) {
        for x in min_x..max_x.min(canvas.width()) {
            canvas.put_pixel(x, y, color);
        }
    }
}

fn draw_text_layer_noop(_canvas: &mut RgbaImage, _layer: &TemplateLayer) {
    // The next task replaces this no-op with system font rendering.
}

fn flatten_layers(layers: &[TemplateLayer]) -> Vec<&TemplateLayer> {
    let mut result = Vec::new();
    for layer in layers {
        result.push(layer);
        if let Some(children) = &layer.children {
            result.extend(flatten_layers(children));
        }
    }
    result
}

fn parse_hex(value: &str) -> Rgba<u8> {
    let hex = value.trim_start_matches('#');
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
        return Rgba([r, g, b, 255]);
    }
    Rgba([0, 0, 0, 255])
}

fn make_export_file_name(values: &std::collections::HashMap<String, String>, row_index: usize) -> String {
    let raw = values.get("name").or_else(|| values.get("title")).cloned().unwrap_or_else(|| format!("{:03}", row_index + 1));
    let safe: String = raw
        .chars()
        .map(|ch| if matches!(ch, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || ch.is_whitespace() { '_' } else { ch })
        .collect();
    format!("{}.png", if safe.trim().is_empty() { format!("{:03}", row_index + 1) } else { safe })
}
```

Modify `src-tauri/src/ecommerce/mod.rs`:

```rust
pub mod batch;
pub mod commands;
pub mod models;
pub mod psd_bridge;
pub mod render;
pub mod storage;

pub use storage::EcommerceStore;
```

- [ ] **Step 4: Add export command**

Add to `src-tauri/src/ecommerce/commands.rs`:

```rust
use super::render::export_images;
use super::models::{ExportRequest, ExportResult};

#[tauri::command]
pub async fn export_ecommerce_images(
    request: ExportRequest,
    store: State<'_, EcommerceStore>,
) -> Result<ExportResult, String> {
    export_images(&store, request)
}
```

Register in `src-tauri/src/lib.rs`:

```rust
ecommerce::commands::export_ecommerce_images
```

- [ ] **Step 5: Run tests and commit**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test ecommerce_render
cargo check --manifest-path src-tauri/Cargo.toml
npm run build
```

Expected: all commands PASS.

Commit:

```bash
git add src-tauri/src/lib.rs src-tauri/src/ecommerce/mod.rs src-tauri/src/ecommerce/render.rs src-tauri/src/ecommerce/commands.rs src-tauri/tests/ecommerce_render.rs
git commit -m "feat: export ecommerce template pngs"
```


## Task 10: Text Rendering In PNG Export

**Files:**
- Modify: `src-tauri/src/ecommerce/render.rs`
- Modify: `src-tauri/tests/ecommerce_render.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`

- [ ] **Step 1: Add text rendering dependencies**

Edit `src-tauri/Cargo.toml` dependencies:

```toml
ab_glyph = "0.2"
fontdb = "0.23"
imageproc = "0.26"
```

Run:

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 2: Extend render test with a text layer**

Modify `src-tauri/tests/ecommerce_render.rs` so `sample_project` adds this second layer after the image layer:

```rust
TemplateLayer {
    id: "title".to_string(),
    name: "{{title}} 大标题".to_string(),
    r#type: TemplateLayerType::Text,
    x: 8.0,
    y: 48.0,
    width: 110.0,
    height: 40.0,
    visible: true,
    opacity: 1.0,
    rotation: 0.0,
    binding_key: Some("title".to_string()),
    locked: Some(false),
    children: None,
    text: Some(TextLayerData {
        text: "默认标题".to_string(),
        font_family: "PingFang SC".to_string(),
        font_size: 18.0,
        font_weight: serde_json::json!(700),
        color: "#ffffff".to_string(),
        stroke_color: None,
        stroke_width: None,
        letter_spacing: None,
        line_height: None,
        align: Some(TextAlign::Left),
    }),
    image: None,
    shape: None,
}
```

Add a title value to the row:

```rust
values.insert("title".to_string(), "批量标题".to_string());
```

Add this assertion after the file existence assertion:

```rust
let exported = image::open(output_dir.join("red-chair.png")).unwrap().to_rgba8();
let non_red_pixels = exported.pixels().filter(|pixel| pixel.0 != [255, 0, 0, 255]).count();
assert!(non_red_pixels > 0, "text rendering should change pixels over the red image");
```

- [ ] **Step 3: Run render test and verify failure**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test ecommerce_render
```

Expected: FAIL because `draw_text_layer_noop` does not draw text.

- [ ] **Step 4: Implement font loading and text drawing**

Modify `src-tauri/src/ecommerce/render.rs` imports:

```rust
use ab_glyph::{FontArc, PxScale};
use imageproc::drawing::draw_text_mut;
```

Change `render_row` to pass row values into text drawing by replacing:

```rust
TemplateLayerType::Text => draw_text_layer_noop(&mut canvas, layer),
```

with:

```rust
TemplateLayerType::Text => draw_text_layer(&mut canvas, layer, row),
```

Replace `draw_text_layer_noop` with this complete implementation:

```rust
fn draw_text_layer(canvas: &mut RgbaImage, layer: &TemplateLayer, row: &BatchRow) {
    let Some(text_data) = &layer.text else {
        return;
    };
    let Some(font) = load_font(&text_data.font_family).or_else(|| load_font("PingFang SC")).or_else(default_font) else {
        return;
    };
    let text = layer
        .binding_key
        .as_ref()
        .and_then(|key| row.values.get(key))
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| text_data.text.clone());
    let color = parse_hex(&text_data.color);
    draw_text_mut(
        canvas,
        color,
        layer.x.round() as i32,
        layer.y.round() as i32,
        PxScale::from(text_data.font_size.max(1.0)),
        &font,
        &text,
    );
}

fn load_font(preferred_family: &str) -> Option<FontArc> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let query = fontdb::Query {
        families: &[fontdb::Family::Name(preferred_family)],
        ..fontdb::Query::default()
    };
    let id = db.query(&query)?;
    let face = db.face(id)?;
    let bytes = match &face.source {
        fontdb::Source::File(path) => std::fs::read(path).ok()?,
        fontdb::Source::Binary(data) => data.as_ref().as_ref().to_vec(),
        fontdb::Source::SharedFile(path, _) => std::fs::read(path).ok()?,
    };
    FontArc::try_from_vec(bytes).ok()
}

fn default_font() -> Option<FontArc> {
    for family in ["PingFang SC", "Heiti SC", "Arial Unicode MS", "Noto Sans CJK SC", "Microsoft YaHei"] {
        if let Some(font) = load_font(family) {
            return Some(font);
        }
    }
    None
}
```

- [ ] **Step 5: Run render tests and commit**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test ecommerce_render
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: both commands PASS.

Commit:

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/ecommerce/render.rs src-tauri/tests/ecommerce_render.rs
git commit -m "feat: render ecommerce template text"
```

## Task 11: End-To-End Validation And Documentation

**Files:**
- Modify: `README.md`
- Modify: `docs/superpowers/specs/2026-05-07-ecommerce-template-design.md` only if implementation decisions changed from the approved design

- [ ] **Step 1: Add README usage section**

Append this section to `README.md`:

```md
## 电商主图模板工具

首版支持从分层 PSD 导入主图模板草稿，编辑图层字段，并用表格或图片文件夹批量导出 PNG。

推荐 PSD 图层命名：

- `{{product_image}} 商品图`
- `{{logo}} LOGO`
- `{{title}} 大标题`
- `{{subtitle}} 副标题`
- `{{selling_point_1}} 卖点 1`
- `{{bottom_title}} 底部文案`

批量表格要求：

- 第一行是字段名。
- 图片字段填写本地图片路径。
- 每一行导出一张 PNG。
- 缺失字段使用模板默认值。
```

- [ ] **Step 2: Run full verification**

Run:

```bash
npm run test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: all commands PASS.

- [ ] **Step 3: Manual end-to-end smoke test**

Run:

```bash
npm run tauri:dev
```

Manual expected result:

1. Open “主图模板”.
2. Import `/Users/attson/Documents/店铺/双人椅/双人主图2+活动 - 4链接 拷贝 2.psd`.
3. Confirm the editor shows a `1000x1000` canvas and layer tree.
4. Select the title layer and change its text in the property panel.
5. Drag the title layer on the canvas.
6. Save the template.
7. Import a CSV with fields `name,title,product_image`.
8. Choose an output folder.
9. Export PNG.
10. Confirm at least one PNG exists in the output folder.

- [ ] **Step 4: Commit final docs**

Commit:

```bash
git add README.md docs/superpowers/specs/2026-05-07-ecommerce-template-design.md
git commit -m "docs: document ecommerce template workflow"
```

## Self-Review Notes

- Spec coverage: PSD import is covered by Tasks 3-4; editor UI by Tasks 5-6; batch table/folder modes by Tasks 7-8; image/shape PNG export by Task 9; text PNG export by Task 10; documentation and validation by Task 11.
- Type consistency: frontend and backend use `TemplateProject`, `TemplateLayer`, `BatchRow`, `ExportRequest`, and `ExportResult` with camelCase JSON across the Tauri boundary.
- Dirty worktree warning: before executing this plan, inspect `git status --short`. Existing modifications in `src/App.vue`, `src/styles.css`, and `src-tauri/*` predate this plan and must be preserved rather than reverted.
