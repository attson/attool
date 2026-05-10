# AGENTS.md

个人桌面工具箱（Tauri 2 + Vue 3 + Naive UI）。当前内置两个工具：Aria2 多线程下载、电商主图模板编辑器（PSD 导入 + 字段批量替换）。

## 技术栈

| 层 | 选型 |
|---|---|
| 桌面壳 | Tauri 2（Rust 2021 edition） |
| 前端 | Vue 3 `<script setup>` + Naive UI 2.44 |
| 构建 | Vite 8 |
| 测试 | Vitest 4（无 jsdom，纯逻辑） |
| 本地存储 | rusqlite（bundled SQLite） |
| 外部命令 | `aria2c`（下载）、`python3 + psd-tools`（PSD 解析） |

## 常用命令

```bash
npm run tauri:dev   # 开发：起 vite + 拉起桌面窗口
npm run dev         # 仅前端调试（http://127.0.0.1:1420）
npm run build       # tsc + vite build
npm run tauri:build # 全量打包桌面应用
npm test            # vitest run
```

## 工作约定

- **TDD 走逻辑**：composable / theme module / utils 这类纯函数模块先写测试再实现；Vue SFC 与样式改动手动目视验收
- **测试**：colocated `*.test.ts`，`vitest run`。**不引入 jsdom** —— composable 把副作用依赖（storage、document.documentElement）作为参数注入，便于无 DOM 测试
- **commit**：conventional 风格（`feat:` / `fix:` / `chore:` / `docs:` / `refactor:` / `ci:`），中文 / 英文 body 都行；每个 task 完成立即 commit，禁止跨任务积累未提交改动
- **不引入新依赖**，除非有明确必要（图标用内联 SVG、不引 lucide / vicons）
- **不写 emoji**（代码、commit message、文档都不写），用户明确要求时除外
- **少注释**：只在 WHY 非显而易见时写。不要写"做了什么"（代码自解释）、不要引用当前 task / PR 编号
- **风险动作要确认**：destructive git（reset --hard / push --force / branch -D）、影响远端的操作（push / 创建 PR / 合 PR / 打 tag）需要用户明确指示

## 目录速览

```
src/
├── App.vue                      # 顶层：theme switch + AppShell + 工具路由
├── main.ts                      # 仅引样式 + mount
├── styles/
│   ├── tokens.css               # 设计 token（dark + [data-theme=light] 覆写）
│   ├── reset.css                # 全局 reset
│   └── template-editor.css      # 模板编辑器全局样式（跨子组件共享）
├── theme/index.ts               # Naive UI darkOverrides + lightOverrides
├── composables/                 # useSidebarState / useLastTool / useTheme（均带测试）
├── components/
│   ├── shell/                   # AppShell / Sidebar / Topbar / Dashboard / BrandMark / ToolIcon
│   ├── ui/                      # Panel / TaskRow / StatPill / Kbd
│   └── ecommerce/               # 模板编辑器：TemplateTool + 5 个子组件
├── types/                       # tool.ts / download.ts / ecommerceTemplate.ts
└── utils/                       # ecommerceTemplate.ts（图层增删改 / 渲染样式）

src-tauri/
├── src/lib.rs                   # 下载相关 Tauri 命令 + run()
├── src/ecommerce/               # 模板模块（commands / models / psd_bridge / render / storage）
└── tauri.conf.json              # 打包配置

.github/workflows/build.yml      # CI 矩阵（mac arm/x64 + linux arm/x64 + win x64）
docs/spec/                       # 当前态规范（overview / ui-design-system / architecture）
docs/superpowers/                # superpowers 流程产物（每任务 1 份 spec + plan）
```

## UI 设计语言（一句话版）

**Mono Dark 默认 + Light 可切**，单一 emerald 主色，所有颜色 / 圆角 / 字号走 `var(--*)` token，圆角不超过 12px。详见 `docs/spec/ui-design-system.md`。

## 加新工具的最小路径

1. `src/types/tool.ts` 加 `ToolIconId` 枚举值
2. `src/components/shell/ToolIcon.vue` 加对应 `<template v-else-if="name === '...'">` SVG
3. `src/App.vue` 的 `tools[]` 数组加一项（`status: 'ready' | 'soon'`）
4. `src/App.vue` 模板里 `<template v-else-if="selectedTool.id === '<id>'">` 加渲染分支
5. 后端 Tauri 命令（如需）写到 `src-tauri/src/lib.rs` 或新 module，并在 `run()` 的 `invoke_handler` 注册

无需碰 Sidebar / Dashboard —— 它们从 `tools[]` 自动派生。

## 已知遗留

- `src-tauri/src/lib.rs` 还保留 `batch_add_logo` / `list_logo_presets` / `save_logo_preset` 命令，前端"电商图片处理"工具已下线（被主图模板替代）。要清理就把这些命令和相关 struct 一起删掉，并从 `invoke_handler` 列表移除
- 字体没打包，依赖系统字体栈（`-apple-system / SF Pro Text / PingFang SC / Inter / Segoe UI`）

## 不要做

- 不要在组件里写散装 hex 色、`rgba(...)`、不在 token 列表里的 `border-radius`
- 不要在 `<style scoped>` 里复制粘贴另一个组件的样式（应该走 token + 共享 class，或者放到 `src/styles/*.css`）
- 不要把 `--bg-base` / `--text` 等 dark 假设硬写进选择器（应该让 `[data-theme=light]` 自动接管）
- 不要在 `src/App.vue` 里塞业务逻辑 —— 当前已经是路由壳 + Aria2 工具，不要再扩。新增工具走独立组件
- 不要把"电商图片处理"功能加回来（已弃，主图模板覆盖该场景）
