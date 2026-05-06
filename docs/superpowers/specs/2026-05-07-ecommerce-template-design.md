# 电商主图模板工具设计

## 背景

AT Tool 当前是一个 Tauri + Vue + Naive UI 桌面工具集合，已有下载工具和电商图片处理入口。新工具面向电商运营主图制作，目标是把现有 PSD 主图资产转换成可批量替换商品图和文案的模板，而不是做完整的 Photoshop 替代品。

用户提供的样例 PSD：

- 路径：`/Users/attson/Documents/店铺/双人椅/双人主图2+活动 - 4链接 拷贝 2.psd`
- 尺寸：`1000x1000`
- 模式：RGB，8-bit
- 大小：约 `3.4MB`
- 解析结果：顶层图层 12 个；总图层/子图层约 21 个；包含文字层 6 个、形状层 10 个、图片层 4 个、智能对象 1 个、图层组 1 个

该 PSD 有清晰分层，适合做“PSD 导入生成模板草稿”。导入后用户可以在画布编辑器中微调，并保存为正式模板用于批量生成 PNG 主图。

## 目标

首版实现完整闭环：

1. 导入规范分层 PSD，生成可编辑模板草稿。
2. 在画布中拖拽、缩放、选择图层，并通过属性面板精调。
3. 通过 PSD 图层命名规则绑定批量替换字段。
4. 支持 CSV/Excel 表格批量数据和图片文件夹快速模式。
5. 批量导出 PNG 主图，失败行不影响其他行。

默认主图尺寸面向通用电商方图，优先支持 `800x800` 和 `1000x1000`，首个样例和首版主路径使用 `1000x1000`。

## 非目标

首版不做以下能力：

- 不做完整 Photoshop 替代。
- 不保证 100% 还原复杂 PSD 样式。
- 不做在线图库或云端模板库。
- 不做 JPG/WebP 导出，先只导出 PNG。
- 不做高级排版规则，例如自动避让商品主体、智能换行压缩、自动视觉评分。
- 不做成品图自动拆图层；只支持 PSD 分层导入。
- 不强还原复杂混合模式、复杂蒙版、阴影、斜面浮雕、文字变形等 Photoshop 效果。

## 产品方案

首版采用“PSD 导入优先，但只做读取结构和栅格化兜底”的方案：

- PSD 导入后读取图层结构，生成文字、图片、形状、组合图层。
- 同时生成 PSD 合成预览，方便对比还原度。
- 对无法结构化还原的图层，导出为透明 PNG 图片图层作为兜底。
- 用户在编辑器中检查字段、调整位置和样式，然后保存模板。
- 批量数据按字段替换模板中的文字和图片，最终导出 PNG。

## 用户工作流

### PSD 导入建模板

1. 进入“电商主图模板”工具。
2. 选择 PSD 文件。
3. 后端解析 PSD，导出素材，生成模板草稿。
4. 前端打开模板编辑器，显示画布、图层树、字段列表和属性面板。
5. 用户检查字段绑定和视觉还原度。
6. 用户微调图层位置、尺寸、文字、颜色、替换字段。
7. 保存模板。

### 表格批量生成

1. 打开已保存模板。
2. 导入 CSV/Excel 数据。
3. 第一行作为字段名，匹配模板图层的 `bindingKey`。
4. 每一行对应一张主图。
5. 图片字段填写本地图片路径，例如 `product_image`、`logo`。
6. 文字字段填写文案，例如 `title`、`subtitle`、`bottom_title`。
7. 选择输出目录。
8. 批量导出 PNG。
9. 显示成功/失败列表。

### 文件夹快速模式

1. 打开已保存模板。
2. 选择商品图文件夹。
3. 每张图片自动生成一行数据。
4. 默认绑定到 `product_image` 字段。
5. 文件名或序号作为输出名。
6. 其他字段使用模板默认值，后续可在表格预览中修改。
7. 批量导出 PNG。

## UI 结构

编辑器采用三栏布局。

### 左侧：图层和素材

- 图层树
  - 显示 PSD 图层顺序、分组、显隐、锁定状态。
  - 支持选择图层。
  - 支持上移、下移、置顶、置底。
- 素材区
  - 显示从 PSD 导出的图片素材。
  - 支持后续添加本地图片素材。
- 字段列表
  - 显示模板需要的数据字段，例如 `product_image`、`title`、`bottom_title`。
  - 标记字段类型：文字、图片。
  - 标记字段是否已在批量数据中匹配。

### 中间：画布

- 默认 `1000x1000` 主图画布。
- 支持缩放适配窗口。
- 支持点击选择图层。
- 支持拖拽移动图层。
- 支持拖动控制点缩放图片、形状和文字框。
- 首版可以先不做旋转和复杂对齐线，但数据结构保留 `rotation` 字段。

### 右侧：属性面板

- 通用属性
  - 图层名
  - 绑定字段
  - `x`、`y`、`width`、`height`
  - 透明度
  - 显隐
- 文字属性
  - 文本内容
  - 字体
  - 字号
  - 字重
  - 颜色
  - 描边颜色
  - 描边宽度
  - 行高
  - 字距
  - 对齐方式
- 图片属性
  - 素材选择
  - 替换字段
  - 裁剪方式：`cover`、`contain`、`stretch`
- 形状属性
  - 形状类型
  - 填充色
  - 描边色
  - 描边宽度
  - 圆角
- 组合属性
  - 子图层数量
  - 整体移动

## 数据结构

### TemplateProject

```ts
type TemplateProject = {
  id: string;
  name: string;
  canvasWidth: number;
  canvasHeight: number;
  layers: TemplateLayer[];
  assets: TemplateAsset[];
  sourcePsdPath?: string;
  createdAt: string;
  updatedAt: string;
};
```

### TemplateLayer

```ts
type TemplateLayer = {
  id: string;
  name: string;
  type: 'text' | 'image' | 'shape' | 'group';
  x: number;
  y: number;
  width: number;
  height: number;
  visible: boolean;
  opacity: number;
  rotation: number;
  bindingKey?: string;
  children?: TemplateLayer[];
  text?: TextLayerData;
  image?: ImageLayerData;
  shape?: ShapeLayerData;
};
```

### TextLayerData

```ts
type TextLayerData = {
  text: string;
  fontFamily: string;
  fontSize: number;
  fontWeight: number | string;
  color: string;
  strokeColor?: string;
  strokeWidth?: number;
  letterSpacing?: number;
  lineHeight?: number;
  align?: 'left' | 'center' | 'right';
};
```

### ImageLayerData

```ts
type ImageLayerData = {
  assetId: string;
  fit: 'cover' | 'contain' | 'stretch';
  replaceable: boolean;
};
```

### ShapeLayerData

```ts
type ShapeLayerData = {
  shape: 'rect' | 'roundRect' | 'ellipse' | 'line';
  fill?: string;
  stroke?: string;
  strokeWidth?: number;
  radius?: number;
};
```

### TemplateAsset

```ts
type TemplateAsset = {
  id: string;
  name: string;
  path: string;
  sourceLayerId?: string;
  mimeType: string;
  width: number;
  height: number;
};
```

## PSD 导入规则

### 画布

- 读取 PSD 宽高。
- 首版默认支持 `800x800` 和 `1000x1000`。
- 首版支持 RGB / 8-bit。
- 不支持的颜色模式给出明确提示。

### 图层转换

- `type` 文字层
  - 转成 `text` 图层。
  - 读取文本、位置、字号、颜色、字体名。
  - 记录字体名，本机缺失时用默认中文字体替代。
- `smartobject`
  - 转成 `image` 图层。
  - 导出内嵌图片作为 asset。
  - 默认可替换。
- `pixel`
  - 转成 `image` 图层。
  - 导出当前图层透明 PNG。
  - 根据图层名判断是否默认可替换。
- `shape`
  - 优先转成基础 `shape` 图层。
  - 复杂形状或解析失败时导出透明 PNG，转成 `image` 兜底。
- `group`
  - 转成 `group` 图层。
  - 保留子图层层级和可见性。

### 字段绑定

首版使用 PSD 图层命名规则标记批量替换字段：

- `{{product_image}} 商品图`
- `{{logo}} LOGO`
- `{{title}} 大标题`
- `{{subtitle}} 副标题`
- `{{selling_point_1}} 卖点 1`
- `{{bottom_title}} 底部文案`

规则：

- 图层名包含 `{{field}}` 时，提取 `field` 为 `bindingKey`。
- 表格字段名直接匹配 `bindingKey`。
- 没有命名规则时，允许根据常见中文图层名做建议映射，例如 `商品图` 建议为 `product_image`，`LOGO` 建议为 `logo`，`大标题` 建议为 `title`。
- 建议映射必须允许用户在属性面板中修改。

### 保真边界

首版保留：

- 图层顺序
- 图层层级
- 位置和尺寸
- 可见性
- 透明度
- 基础文字内容、字号、颜色、字体名
- 基础形状和图片素材

首版不强保留：

- 复杂混合模式
- 复杂蒙版
- 投影、内阴影、斜面浮雕
- 文字变形
- 完整 Photoshop 图层样式

## 存储设计

沿用当前应用本地数据思路：

- SQLite 存模板项目、图层 JSON、字段绑定、导出记录。
- 应用数据目录下保存模板素材。
- 每个模板有独立 asset 目录，存放 PSD 导出的透明 PNG、智能对象图片、合成预览。

建议目录结构：

```text
app_data/
  ecommerce_templates.sqlite3
  templates/
    <template_id>/
      template.json
      preview.png
      assets/
        <asset_id>.png
```

## 后端命令

首版需要新增 Tauri command：

- `import_psd_template(psdPath: string) -> TemplateProject`
  - 解析 PSD。
  - 导出资产。
  - 生成模板草稿。
- `list_ecommerce_templates() -> TemplateSummary[]`
  - 列出已保存模板。
- `load_ecommerce_template(id: string) -> TemplateProject`
  - 读取模板详情。
- `save_ecommerce_template(project: TemplateProject) -> TemplateProject`
  - 保存模板和图层 JSON。
- `import_batch_table(path: string) -> BatchDataPreview`
  - 读取 CSV/Excel。
  - 返回字段和数据预览。
- `create_batch_from_folder(folderPath: string, imageBindingKey: string) -> BatchDataPreview`
  - 读取图片文件夹。
  - 生成快速批量数据。
- `export_ecommerce_images(request: ExportRequest) -> ExportResult`
  - 根据模板和批量数据导出 PNG。

## 批量数据规则

### 表格模式

- 第一行是字段名。
- 字段名匹配模板 `bindingKey`。
- 每一行生成一张 PNG。
- 图片字段填写本地路径。
- 文字字段填写文案。
- 多余字段不报错，在预览中标记为未使用。
- 缺失字段给出提示，允许继续导出并使用模板默认值。

### 文件夹模式

- 选择商品图文件夹。
- 支持常见图片格式：PNG、JPG、JPEG、WebP。
- 每张图片生成一行数据。
- 默认绑定到 `product_image`。
- 输出名优先使用图片文件名。

## 导出规则

- 首版只导出 PNG。
- 输出目录由用户选择。
- 文件名优先使用 `name` 字段。
- 如果没有 `name`，使用 `title` 字段。
- 如果没有 `title`，使用序号，例如 `001.png`。
- 单行失败不影响其他行。
- 导出完成后展示：
  - 总数
  - 成功数
  - 失败数
  - 输出文件列表
  - 失败行号、字段名、失败原因

## 错误处理

### PSD 导入

- 文件不存在：提示路径无效。
- 非 PSD 文件：提示请选择 PSD 文件。
- 不支持颜色模式：提示当前只支持 RGB / 8-bit。
- PSD 整体解析失败：提示解析错误。
- 单个图层解析失败：记录失败原因，尽量栅格化为图片图层，不中断整体导入。

### 字段匹配

- 缺失字段：导入前提示，允许继续。
- 多余字段：不报错，标为未使用。
- 字段类型不匹配：图片字段传入非图片路径时，该行导出失败。

### 图片路径

- 图片不存在：该行导出失败，记录行号、字段名和路径。
- 图片无法读取：该行导出失败，记录具体错误。

### 字体

- 记录 PSD 字体名。
- 本机缺失时用默认中文字体替代。
- 在模板中提示字体已替换。

## 测试计划

- PSD 导入测试
  - 使用用户提供的 PSD 验证图层数量、文字内容、图片槽、形状、分组和字段推断。
- 模板保存/加载测试
  - 保存后重新打开，图层和字段一致。
- 批量数据测试
  - CSV/Excel 表格模式。
  - 图片文件夹快速模式。
- 导出测试
  - 单张导出。
  - 多行批量导出。
  - 缺图失败不中断。
- 前端交互测试
  - 选择图层。
  - 拖拽移动。
  - 缩放图层。
  - 属性修改后画布同步。
- 构建测试
  - `npm run build`
  - 必要时执行 Tauri/Rust 编译检查。

## 实施注意事项

- PSD 解析依赖需要在实现阶段确认 Rust 生态能力。如果 Rust 解析库不足以满足图层读取，可考虑：
  - Rust 后端集成可用 PSD crate；
  - 或把 PSD 解析作为独立命令/sidecar；
  - 或首版先实现 PSD 合成预览和图层元数据读取，再逐步增强。
- 批量 PNG 渲染应复用同一套模板数据，避免前端画布和后端导出结果不一致。
- 画布编辑应优先保证移动、缩放、文字修改、图片替换这些核心路径稳定。
- `.superpowers/` 是视觉伴侣临时目录，不应提交到仓库。
