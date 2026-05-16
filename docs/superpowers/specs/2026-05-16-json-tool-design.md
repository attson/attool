# JSON 工具设计 — 2026-05-16

## 目标

为 AT Tool 加一个常用 JSON 工作台。覆盖四类高频需求：

1. **格式化 / 查看**：粘贴 JSON → 美化、最小化、键排序、可折叠树视图。
2. **查询**：用 JSONPath 表达式从 JSON 中取子集 / 过滤。
3. **对比**：两份 JSON 的语义级 diff（不在意键顺序、缩进、空白）。
4. **格式转换**：JSON ↔ YAML / TOML / CSV 双向互转。

非目标（本期不做）：
- 编辑器内容跨会话持久化
- 多份「快照」侧边栏
- jq / JMESPath 等其他查询语法
- 文本级 diff（行/词级）

## 技术栈

| 关注点 | 选型 | 理由 |
|---|---|---|
| 编辑器 | **Monaco**（`monaco-editor`） | VSCode 内核体验，懒加载到工具页，主 bundle 不动 |
| 查询 | **jsonpath-plus** | 纯 JS，~6KB gzip，JSONPath 语法最广为人知 |
| Diff | **jsondiffpatch** | 同时给出 diff 数据 + HTML 渲染 |
| YAML | **js-yaml** | 事实标准 |
| TOML | **@iarna/toml** | 双向都支持 |
| CSV | 自写薄包装（无库） | 仅支持 array-of-flat-objects；范围窄到不值得引入 papaparse |

所有依赖纯前端，不动 Rust 端。

## 用户界面

整体布局：**顶部 Tabs**，四个子功能各自一个 tab。每个 tab 都有自己的双栏（输入 / 输出）布局。

### Tab 1 — 格式化

```
[ 格式化 ] [ 最小化 ] [ 键排序 ] [ 复制 ]              [ 📂 打开文件 ]
┌──────────────────────────┬──────────────────────────┐
│  Monaco (language: json) │  树视图（可折叠）         │
│  ←  当前 JSON 文本       │  ← 同源数据，实时同步     │
└──────────────────────────┴──────────────────────────┘
错误行号 · 字符数 · 解析耗时
```

- 「格式化」/「最小化」/「键排序」对左侧编辑器**原地修改**。
- 「复制」复制当前左侧文本。
- 树视图节点点击可折叠/展开；叶子节点点击复制其 JSONPath 路径。
- 文本无效时：左侧编辑器底部标红，标注错误行号；树视图显示「等待有效 JSON」。
- 「📂 打开文件」+ 拖拽文件 = 任一方式都能把内容塞进左侧编辑器（其余 tab 同理，下文不再重复）。

### Tab 2 — 查询

```
[ $.store.book[?(@.price < 10)].title    ] [ 执行 ] [ 复制结果 ]
┌──────────────────────────┬──────────────────────────┐
│  Monaco (源 JSON)         │  Monaco (结果，只读)      │
└──────────────────────────┴──────────────────────────┘
匹配数 N · 耗时 · （表达式错误时显示错误信息）
```

- 表达式输入框宽度自适应；按 ⌘/Ctrl+Enter 也触发执行。
- 结果总是数组（JSONPath 的语义），结果编辑器只读、自动美化。

### Tab 3 — 对比

```
┌──────────────────────────┬──────────────────────────┐
│  Monaco (JSON A)         │  Monaco (JSON B)         │
└──────────────────────────┴──────────────────────────┘
┌────────────────────────────────────────────────────┐
│  - items[0].price: 10                              │  ← 红
│  + items[0].price: 12                              │  ← 绿
│  + tags: ["new"]   (新增)                          │
└────────────────────────────────────────────────────┘
```

- 输入任意一边改动都重算 diff（debounce 200ms）。
- 渲染用 jsondiffpatch 内置的 HTML formatter，自定义 CSS 适配主题。
- 任一边无效 JSON 时，下方渲染区显示「左/右 JSON 解析失败：...」。

### Tab 4 — 转换

```
源: [ JSON ▾ ]              ⇄              目标: [ YAML ▾ ]
┌──────────────────────────┬──────────────────────────┐
│  Monaco (源)              │  Monaco (目标，只读)      │
└──────────────────────────┴──────────────────────────┘
CSV 限定：只允许 array-of-flat-objects；嵌套时报错。
```

- 源/目标各一个下拉：`json | yaml | toml | csv`。
- 切换格式时左侧编辑器的 `language` 也跟着变。Monaco 内置只识别 JSON 高亮；YAML/TOML/CSV 在 v1 退化为 `plaintext`（仍有行号、查找、折叠），后续可以接 `monaco-yaml` 之类的扩展。
- 实时转换（debounce 200ms）。
- 中间的 ⇄ 按钮交换源/目标。

## 文件结构

```
src/components/json/
  JsonTool.vue              # 入口：tabs + 公共布局
  CodeEditor.vue            # Monaco 包装；prop: language, modelValue, readonly, height
  JsonFormatPane.vue        # Tab 1
  JsonQueryPane.vue         # Tab 2
  JsonDiffPane.vue          # Tab 3
  JsonConvertPane.vue       # Tab 4
  JsonTreeView.vue          # 折叠树（在 Format pane 里用）

src/composables/
  useFileDrop.ts            # 拖拽 + 「打开文件」按钮，通用于后续工具
  useMonacoLoader.ts        # 懒加载 monaco-editor，单例

src/utils/
  jsonFormat.ts             # parse / stringify / minify / sortKeys
  jsonConvert.ts            # 双向格式互转：toJs(text, format) / fromJs(value, format)
  jsondiff.ts               # 包装 jsondiffpatch，加 HTML 渲染辅助

src/types/
  json.ts                   # 共享类型（ConvertFormat 等）
```

App.vue `tools` 数组新增：
```ts
{ id: 'json', name: 'JSON 工具', description: '格式化 / 查询 / 对比 / 转换', status: 'ready', icon: 'code' }
```

并加上对应的 `<template v-else-if="selectedTool.id === 'json'"><JsonTool /></template>` 分支。

## 数据流

每个 tab 是相对独立的组件，内部维护自己的 `ref<string>` 文本状态。Tab 之间**不**共享内容（你切到对比 tab 时不会自动把格式化 tab 的内容塞进左侧）。这是有意的：不同 tab 的语义不一样，自动共享反而困惑。

公共能力：
- `useFileDrop`：处理拖拽 / 「打开文件」按钮。返回 `{ onDrop, openFile }`，文本回调里塞进当前编辑器。
- `useMonacoLoader`：懒加载 monaco（动态 import），返回 Promise<typeof monaco>。`CodeEditor.vue` 在 mounted 时调用。

## 错误处理

| 场景 | 处理 |
|---|---|
| Format tab 输入无效 JSON | 底部红条 + 错误行号；树视图 placeholder「等待有效 JSON」 |
| Query tab 表达式不合法 | 底部红条标注错误；结果区清空 |
| Query tab 源 JSON 无效 | 底部红条；结果区清空 |
| Diff tab 任一边无效 | 渲染区显示「左/右 JSON 解析失败：&lt;message&gt;」 |
| Convert tab CSV 含嵌套 | 底部红条「CSV 不支持嵌套结构」 |
| Convert tab 解析失败 | 底部红条；目标区清空 |
| 拖拽非文本 / 大于 10MB 的文件 | toast 提示拒绝 |

## 测试

- **单元测试**（vitest）：
  - `jsonFormat.ts`：format / minify / sortKeys 的边界（空对象、嵌套、`null`、数字键序、循环引用应 throw）
  - `jsonConvert.ts`：6 个方向（JSON↔YAML/TOML/CSV）的往返；CSV 嵌套报错
  - `jsondiff.ts`：增/删/改 三类变更的 HTML 输出非空
- **组件测试**（vitest + @vue/test-utils）：
  - `JsonFormatPane`：输入无效 JSON 时显示错误条；点「键排序」后编辑器内容按字典序排列
  - `JsonQueryPane`：合法表达式 → 结果；非法表达式 → 错误条
- **手动验证**：四个 tab 在 light/dark 主题下的视觉、拖拽文件、打开文件按钮、复制按钮

## 范围 / YAGNI 说明

- 不做编辑器内容跨会话持久化（用户明确否决）
- 不做「快照」列表
- 不做 jq / JMESPath
- 不做文本级 diff
- 不做 Schema 校验
- 不做 Monaco 主题精细化（用 vs-light / vs-dark 即可）

## 风险

- **Monaco 体积**：~3MB。通过工具页懒加载 + Vite 动态 import 控制；首次进入「JSON 工具」时才下载。其他工具不受影响。
- **CSV 双向**：从 JSON 到 CSV 的范围窄是个 UX 风险。在 Convert tab 顶部明确写出「CSV 仅支持 array-of-flat-objects」。
- **大文件**：Monaco 在多 MB 文本时会变慢。10MB 上限作为防御性兜底，超出走 toast 拒绝。
