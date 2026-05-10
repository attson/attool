# UI 重做 · Mono Dark / Emerald · 设计稿

**日期：** 2026-05-10
**状态：** 设计已批准，待写实施计划
**范围：** 全量 —— 外壳 + Aria2 + 加 Logo + 模板编辑器 + 全部 Naive UI 组件

---

## 1. 目标与背景

当前 UI 是"米色羊皮纸 + 苔藓绿 + 橙金"的暖色复古配色，圆角偏大（卡片 24 px），带毛玻璃和径向渐变。用户认为"太土"，要求整体翻新。

新方向：**Mono Dark · Linear / Vercel 一类的深色极简**，配 **Emerald (#34D399)** 作为唯一主色，保持工具感与开发者审美。

---

## 2. 关键决策（已锁定）

| # | 决定 | 选择 | 关键原因 |
|---|---|---|---|
| 1 | 整体方向 | Mono Dark | 用户在三方向 mockup 中选 A |
| 2 | 主色 | Emerald `#34D399` | 用户在四主色 mockup 中选 2；与下载/任务流的"运行/成功"语义贴合 |
| 3 | 导航布局 | 可折叠左侧 sidebar | 用户在二选一中选 sidebar + 折叠 |
| 4 | 改造范围 | 全量（外壳 + 所有工具页 + 模板编辑器） | 用户选"全量（推荐）" |
| 5 | 明暗模式 | **仅深色** | 个人工具、降低 token 系统复杂度，后续如需可扩展 `light` |
| 6 | 组件库 | **保留 Naive UI** | 自带 darkTheme + themeOverrides，retheme 成本远低于自写组件 |
| 7 | 字体 | 系统栈 + 等宽数字 | 桌面应用最自然；数字/路径走 mono |
| 8 | 首页 | 极简 dashboard（品牌 + 最近使用 + 快速入口） | 保留品牌存在感，但不再是必经的"工具集合首页" |
| 9 | 画布透明区 | 棋盘格背景 | 模板编辑器画布有意义的视觉提示 |

---

## 3. 设计 Token

### 3.1 颜色

```
/* Surface */
--bg-base:        #0A0A0B   /* 应用底色、内容区 */
--bg-elevated:    #131316   /* 面板、卡片、sidebar */
--bg-elev-2:      #18181C   /* hover、次级面板、表单内嵌输入 */
--bg-overlay:     #1D1D22   /* 模态、下拉、popover */

/* Line */
--line:           #1F1F23   /* 默认 1px 分隔线 */
--line-strong:    #28282D   /* 输入框、复选框边框、可视分组 */

/* Text */
--text:           #EDEDED   /* 正文 */
--text-muted:     #8B8B8B   /* 辅助、元信息 */
--text-faint:     #6A6A6F   /* 占位、面包屑分隔符 */

/* Accent */
--accent:         #34D399   /* 主按钮、focus、活跃菜单、进度条 */
--accent-hover:   #4ADE80
--accent-pressed: #16A34A
--accent-fg:      #0A0A0B   /* accent 上的前景文字 */
--accent-soft:    rgba(52,211,153,.18)   /* badge 底色、focus 光晕、活跃 sidebar item */

/* Semantic */
--warning:        #F5B94B
--error:          #F87171
--info:           #60A5FA
--success:        var(--accent)   /* 复用 emerald，避免双绿 */
```

> **规则：** 任何新颜色必须能映射到上面 16 个 token 之一；不允许在组件里写散装 hex。

### 3.2 圆角

```
--radius-sm:    4px    /* 标签内嵌元素 */
--radius:       6px    /* 按钮、输入框、徽标按钮 */
--radius-md:    8px    /* 卡片、面板、任务行 */
--radius-lg:   12px    /* 模态、大型 popover */
--radius-pill: 999px   /* 状态徽标、tag */
```

> 现状大量使用 14 / 18 / 24 px，全部下调；Card 的 24 px 不允许出现。

### 3.3 字体

```
--font-sans: -apple-system, "SF Pro Text", "PingFang SC",
             "Inter", "Segoe UI", sans-serif;
--font-mono: "SF Mono", "JetBrains Mono", Menlo, Consolas, monospace;
```

字号阶梯（不再用 0.x rem）：

| token | px | 用途 |
|---|---|---|
| `--fs-xxs` | 11 | 分组 LABEL、kbd |
| `--fs-xs`  | 12 | 元信息、tag、表单 label |
| `--fs-sm`  | 12.5 | 表单输入文本 |
| `--fs-md`  | 13 | 正文、绝大多数 UI |
| `--fs-lg`  | 15 | 区块标题（如面板 header） |
| `--fs-xl`  | 17 | 页面标题 |
| `--fs-2xl` | 22 | 偶尔的展示型标题（首页问候语） |

- 标题 `letter-spacing: -0.012em`，正文不调
- 数字字段（速度、ETA、百分比、坐标、尺寸、文件大小）统一加 `font-feature-settings: "tnum"` 并使用 `--font-mono`
- 字段名占位（`{{ product_image }}`）使用 `--font-mono`

### 3.4 间距与密度

- 基本单位 4 px；常用 gap：6 / 8 / 10 / 12 / 16 / 22
- 全局 Naive UI `size="small"` 维持紧凑（已与现状一致）
- 内容区内边距：22 px（顶/侧）；面板内边距：14 px

### 3.5 阴影 / 发光

- 默认 **不用阴影**，层级靠背景明度差表达
- 仅 modal / popover 用：`0 8px 24px rgba(0,0,0,.45)`
- focus 用 `box-shadow: 0 0 0 3px var(--accent-soft)`，不要 `outline`

### 3.6 动效

- hover / 颜色过渡：`120ms ease`
- 布局过渡（sidebar 收合）：`180ms ease`
- 不要 transform 弹跳、不要旋转、不要颜色循环

---

## 4. 外壳骨架

```
┌──────────┬──────────────────────────────────────────────┐
│  brand   │  [面包屑]  工具 / Aria2 下载    [pill][pill] │
│  ⌘K      ├──────────────────────────────────────────────┤
│          │                                              │
│ 已就绪   │  页面标题 · 副标题                           │
│ • Aria2  │                                              │
│ • 模板   │  ┌── 面板 ────────────┐ ┌── 面板 ────────┐   │
│ • Logo   │  │                    │ │                │   │
│          │  └────────────────────┘ └────────────────┘   │
│ 规划中   │                                              │
│ ⋯ Soon   │                                              │
│          │                                              │
│ v0.1 [‹] │                                              │
└──────────┴──────────────────────────────────────────────┘
```

### 4.1 Sidebar

| 状态 | 宽度 | 触发 |
|---|---|---|
| 展开 | 220 px | 默认 |
| 折叠 | 56 px | 底部 `‹/›` 按钮；快捷键 `⌘\` |

- 折叠状态保存到 `localStorage.attool.sidebar.collapsed`
- 过渡 180 ms ease
- 折叠态：仅显示图标、品牌 mark、底部 `›`；hover 显示 tooltip
- 内部分组：**已就绪 / 规划中**；规划中条目右端带 `Soon` pill
- 活跃项：背景 `--accent-soft`，文字与图标变 `--accent`
- 顶部品牌行：26 px emerald 方块（mark "A"）+ "AT Tool" 文字
- 品牌行下方：搜索框样式的 `⌘K` 入口（先留位，命令面板后续单独做，本次只渲染入口外观，点击直接 alert "敬请期待"）

### 4.2 Topbar

- 高度约 40 px（一行）
- 左：面包屑 `工具 / <当前>`，分隔符用 `--text-faint`
- 右：当前工具自定义 pill 区（如 Aria2 的"进行中 N"、"已完成 N"），允许工具组件渲染自己的徽标
- 与 sidebar 同色（`--bg-elevated`）形成顶部一条横向元素带

### 4.3 Page chrome

- 内容区 padding `18px 22px`
- 页面标题：`fs-xl + sub`（`fs-xs --text-muted`），同行排
- 面板（取代现有 `<n-card>`）：`bg-elevated` + 1 px line + `radius-md`，`ph` 头部 10/14 padding，`pb` 主体 14 padding

### 4.4 首页（Dashboard）

新增极简 dashboard（替代当前的"工具卡片网格首页"）：

```
┌─ 欢迎回来 ─────────────────────────────────┐
│ 共 3 个工具就绪 · 4 个规划中               │
│                                            │
│ 最近使用                                   │
│ • Aria2 下载    上次 14:32                 │
│ • 主图模板      昨天                       │
│                                            │
│ 快速入口                                   │
│ [新下载] [新模板] [批量加 Logo]            │
└────────────────────────────────────────────┘
```

- 用户首次启动停在 dashboard
- 之后启动恢复"上次打开的工具"（`localStorage.attool.lastTool`）
- 点击品牌 mark 回 dashboard

---

## 5. Naive UI 主题策略

### 5.1 整体

- 用 `<n-config-provider :theme="darkTheme" :theme-overrides="overrides">` 包裹整个 app
- 弃用当前 App.vue 里那套 `themeOverrides`（暖色苔藓绿）
- 新建 `src/theme/index.ts` 导出 `darkOverrides` 与 `cssVariables`（CSS 变量挂在 `:root`，供自写组件使用）

### 5.2 关键组件覆写

| 组件 | 覆写要点 |
|---|---|
| `common` | `primaryColor=#34D399`、`primaryColorHover=#4ADE80`、`primaryColorPressed=#16A34A`、`primaryColorSuppl=#34D399`、`borderRadius=6px`、`borderRadiusSmall=4px`、`bodyColor=#0A0A0B`、`cardColor=#131316`、`textColor1=#EDEDED`、`textColor2=#8B8B8B`、`fontFamily=...` |
| `Card` | `borderRadius=8px`、`color=#131316`、`borderColor=#1F1F23`、padding 收紧 |
| `Button` | `borderRadiusMedium=6px`、`heightMedium=32`、ghost / secondary 都映射到 `--bg-elev-2 + --line-strong` |
| `Input` / `InputNumber` / `Select` | bg `#18181C`、border `#28282D`、focus border `--accent` + 3 px soft 光晕；数字输入加 mono 字体 |
| `Tag` | round 默认；success / warning / error 颜色对接语义 token |
| `Modal` / `Drawer` | `color=#1D1D22`、阴影按 3.5 节 |
| `Alert` | 各类型 bg 走 `color-mix(in srgb, <semantic> 15%, transparent)`，无边框 |
| `Tooltip` | bg `#1D1D22`、文字 `#EDEDED`、12 px |
| `Slider` | 轨道 `--line-strong`、激活 `--accent`、把手 `--accent` + 1.5 px `#0A0A0B` 描边 |
| `ColorPicker` | 不动外观（已经够通用），仅边框统一 `--line-strong` |

### 5.3 自写组件（不靠 Naive UI）

新建 `src/components/shell/`：

- `AppShell.vue`：sidebar + topbar + slot
- `Sidebar.vue`：折叠态、分组、活跃项
- `Topbar.vue`：面包屑 + 右侧 pill 槽
- `BrandMark.vue`：26 px emerald 方块

新建 `src/components/ui/`：

- `Panel.vue`（替代多数 `<n-card>` 的薄容器，自写更可控）
- `TaskRow.vue`（Aria2 任务行，从 `TaskCard.vue` 改）
- `StatPill.vue`（topbar 右侧 pill）
- `Kbd.vue`（`⌘K` 等键盘快捷键样式）

---

## 6. 工具页适配

### 6.1 Aria2

- 顶栏 pill：`进行中 {{ activeCount }}`、`已完成 {{ completedCount }}`
- 左侧"新建下载"面板 → `Panel`，表单字段统一新输入框样式
- 右侧"任务队列" → `Panel` + `TaskRow.vue` 列表
- `TaskRow` 内：标题（13 px）+ url（11 px mono muted）+ meta 一行（mono 速度 / ETA / 百分比）+ 3 px progress bar（emerald）
- 状态徽标：下载中 = `accent-soft`、等待中 = `warning-soft`、已完成 = `--line-strong + --text-muted`、失败 = `error-soft`

### 6.2 电商图片处理（加 Logo）

- 左侧 320 px 素材面板（保留现状结构）：图片列表条目按 `Panel + TaskRow` 风格重画，hover 与 active 走新 token
- 右侧预览框：背景换为深色 `#0F0F12`，图片居中显示，1 px line 边框，去掉之前的米色渐变
- 拖拽手柄（`logo-resize-handle`、`template-resize-handle`）颜色由 `--moss` 改为 `--accent`，描边由 `#fff` 改为 `#0A0A0B`
- 模态（保存方案）走新 modal 样式

### 6.3 模板编辑器（最复杂）

四列布局保留，重新染色：

```
[rail 56] [resource 220] [canvas 弹性] [props 240]
```

- **rail**：`bg-elevated` + 1 px line right；图标按钮 38 px 高、`radius` 7 px、active 走 `accent-soft + accent`
- **resource panel**：`bg-elevated`，图层 / 模板 / 图片资源全部走 `Panel` + 列表样式
- **canvas wrap**：背景为 12 px 棋盘格（深 `#18181C` × 深 `#0A0A0B`），表达透明区
- **canvas 自身**：保留模板内部的实际渲染色（用户内容），仅外框 1 px `--line-strong`，阴影 `0 12px 32px rgba(0,0,0,.5)`
- **selection / handle**：选中 outline `1.5 px var(--accent)`，外发光 `0 0 0 3px var(--accent-soft)`，四角 8 px emerald 方块 + 1.5 px `#0A0A0B` 描边
- **layer toolbar**（图层上方浮出的 mini 工具条）：`bg-overlay` + `radius-md` + 阴影
- **props panel**：分段（位置/外观/字段/字体/对齐/高级），分段标题用 `fs-xxs uppercase muted`；输入框统一 26 px 高度、紧凑布局保留
- 所有 `{{ field_name }}` 文本走 `--font-mono`

---

## 7. 实现顺序（建议）

按降低风险的顺序，每步都能独立运行：

1. **Token 与主题入口**：建 `src/theme/index.ts`、`src/styles/tokens.css`，把 `:root` CSS 变量与 Naive UI overrides 挂上；旧 `styles.css` 暂时保留
2. **AppShell + Sidebar + Topbar + Dashboard**：替换 `App.vue` 顶层结构；旧的 home grid 页面删除（被 dashboard 替代）；导航走 sidebar
3. **基础组件**：`Panel`、`TaskRow`、`StatPill`、`BrandMark`、`Kbd`
4. **Aria2 工具页适配**：用 Panel + TaskRow 重组，删除旧 task-card 样式
5. **加 Logo 工具页适配**：素材列表 + 预览框 + 拖拽手柄
6. **模板编辑器适配**：rail / resource / props 三段配色 + 棋盘格 canvas + 选中样式
7. **`styles.css` 清理**：删除暖色调残余、未引用样式、`.brand-mark` / `.tool-card` / `.template-*` 旧规则
8. **快捷键与命令面板入口**：`⌘\` 切 sidebar；`⌘K` 入口先 alert（占位）

> 命令面板本身**不在本次范围**，本次只渲染入口外观。

---

## 8. 不在本次范围（Non-goals）

- 浅色主题（仅深色）
- 命令面板（`⌘K`）的真实功能
- 国际化 / 多语言
- 任何工具的功能改动（不动 Tauri 命令、不改业务逻辑）
- 字体打包（不引入 Inter / Geist 静态字体；走系统栈）
- 图标系统统一（本次保留现有图标策略，仅在 sidebar / rail 用色块占位即可，后续可换 lucide / phosphor）

---

## 9. 验收标准

实施完成后，下面每条都为真：

- [ ] 启动后默认深色 + emerald accent，无任何暖色残余
- [ ] sidebar 可折叠（220 ↔ 56），状态在重启后保留
- [ ] 三个已就绪工具（Aria2 / 模板 / 加 Logo）在 sidebar 切换无回首页跳转
- [ ] 顶栏面包屑显示"工具 / <当前>"
- [ ] Aria2 任务行：状态徽标颜色按 6.1 区分；速度 / ETA / 百分比走等宽数字
- [ ] 模板编辑器画布：棋盘格透明背景；选中图层有 emerald 边框 + 4 角手柄
- [ ] 所有 `<n-card>` 圆角 ≤ 8 px；`<n-button>` 圆角 = 6 px
- [ ] 没有任何 `border-radius: 14px` / `18px` / `24px`
- [ ] `styles.css` 中不再有 `--moss`、`--ember`、`--gold`、`--paper` 等旧 token
- [ ] 在 1280 × 800 窗口下不出现横向滚动；折叠 sidebar 后可在 1100 px 窗口内正常工作

---

## 10. 风险与开放问题

- **Naive UI 个别组件可能不完全跟随 token**（如 `NPageHeader` 内部边距、`NEmpty` 插画色），实施中如发现需要补 CSS hack 或自写替换
- **Tauri WebView 不同平台字体回退**（Windows 没 SF Pro / PingFang）：字体栈已加 `Segoe UI` / `Inter`，需在 Windows 实机验证
- **棋盘格背景**对眼睛刺激度（特别是面对纯白模板时反差大）：若实测刺眼，备选方案是纯 `#0F0F12` 实色 + 单根十字标尺
- **dashboard 的"最近使用"从哪来**：本次仅记录最后一个工具 ID，不做完整历史；未来如需多条要建本地 store
