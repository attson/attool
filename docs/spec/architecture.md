# 架构

## 总览

```
┌──────────────────────────────────────────────────────────┐
│  Vue 3 SFC（src/）                                       │
│  ├─ App.vue          主路由壳 + Aria2 工具页              │
│  ├─ shell/*          AppShell + Sidebar + Topbar + ...   │
│  ├─ ui/*             Panel / TaskRow / StatPill / Kbd    │
│  ├─ ecommerce/*      模板编辑器 6 个 SFC                  │
│  ├─ composables/*    useSidebarState / useLastTool /     │
│  │                   useTheme（注入 storage / root 测试）  │
│  ├─ theme/index.ts   Naive UI overrides（dark + light）   │
│  ├─ styles/*.css     tokens + reset + template-editor    │
│  ├─ types/*          Tool / DownloadTask / Template...   │
│  └─ utils/*          ecommerceTemplate（图层增删改）      │
│                                                          │
│           ↕ invoke() / listen()                          │
│                                                          │
│  Rust（src-tauri/src/）                                  │
│  ├─ lib.rs           run() + 下载相关 8 个命令           │
│  └─ ecommerce/       commands / models / psd_bridge /   │
│                      render / storage（rusqlite）         │
│                                                          │
│           ↕ Command::new                                 │
│                                                          │
│  本机外部进程                                             │
│  ├─ aria2c           HTTP/FTP/BT 下载                    │
│  ├─ python3 + psd-tools   PSD 解析                        │
│  └─ open / explorer / xdg-open    打开文件夹              │
└──────────────────────────────────────────────────────────┘
```

## 前后端通信

- **请求-响应：** `invoke<T>('command_name', { arg })` （`@tauri-apps/api/core`）
- **流式事件：** Rust `app_handle.emit("event-name", payload)` → 前端 `listen<T>('event-name', cb)`
  - 当前唯一事件：`download-progress`，由 aria2 输出解析后推送
- **错误：** Rust 命令返回 `Result<T, String>`，前端 `try { await invoke(...) } catch (e)` 捕获

## 状态分层

| 层 | 存储 | 例 |
|---|---|---|
| UI session | Vue ref | 当前选中工具、表单值、loading flags |
| 浏览器持久 | `localStorage` | sidebar 折叠状态、上次打开工具、主题、属性面板折叠 |
| 应用持久 | rusqlite（Tauri data dir） | 模板项目、模板素材库、Logo 预设（遗留）、下载任务历史 |
| 临时 | Tauri runtime State | 进行中的下载子进程句柄（`DownloadTasks`） |

## 软件更新

- Rust 端：`tauri-plugin-updater` + `tauri-plugin-process`，在 `lib.rs` 的 `run()` 里注册
- 配置：`tauri.conf.json plugins.updater.endpoints` + `pubkey`
- 前端：`src/composables/useUpdater.ts`（状态机，注入式 client）+ `useUpdaterPrefs.ts`（autoCheck / skippedVersion 持久化）
- UI：`UpdateBanner.vue`（topbar 下方，4 状态）+ `SettingsModal.vue`（sidebar 齿轮触发）
- CI 签名：`TAURI_SIGNING_PRIVATE_KEY`（Environment `prod` 的 secret）+ 空字符串密码注入到 `tauri build` 的环境变量；Tauri 自身完成签名生成 `.sig` 文件
- `latest.json` 由 `.github/scripts/build-latest-json.mjs` 在 release job 里生成（聚合 5 个 matrix 上传的 artifact），不依赖 `tauri-action` 的内置功能
- **Linux 不在 updater 覆盖范围**：Tauri 不签 `.deb`，所以 `latest.json` 的 `platforms` 字段只有 `darwin-aarch64` / `darwin-x86_64` / `windows-x86_64`；脚本检测到 `.sig` 缺失会自动跳过该平台

## 关键模块

### `App.vue`

顶层装配点：

- 定义 `tools[]` 数组（id + name + description + status + icon）
- 起 composable：`useSidebarState` / `useLastTool` / `useTheme`
- Naive UI ConfigProvider 切换主题
- 渲染 `<AppShell>` + `<template v-else-if="selectedTool.id === '<id>'">` 分支
- 嵌入 Aria2 工具页（典型 form + list 形态，作为参考实现）
- 注册全局快捷键 `⌘\` / `⌘K`

### `src/composables/`

模式：注入式副作用依赖，便于 Vitest 单测无需 jsdom。

```ts
export function useSidebarState(storage: KVStorage = localStorage) { ... }
export function useTheme(
  storage: KVStorage = localStorage,
  root: ThemeRoot = document.documentElement
) { ... }
```

测试时传入 fake storage / fake root，断言行为与持久化结果。

### `src/components/ecommerce/TemplateTool.vue`

模板编辑器主组件。组合 5 个子组件：

- `TemplateResourcePanel` —— 左侧资源面板，5 个 tab（文字 / 图片 / 素材 / 图层 / 模板）
- `LayerTree` —— 图层树（资源 panel 的"图层"tab 内）
- `TemplateCanvas` —— 中央画布（带选中框 / 手柄 / 拖拽）
- `LayerProperties` —— 右侧属性面板（位置 / 外观 / 字段 / 字体 / 对齐 / 高级）
- `BatchTaskPanel` —— 底部批量任务面板（按图层注入变体 → 1:1 笛卡尔展开）

状态全部在 `TemplateTool.vue` 顶层 `project: ref<TemplateProject>`，下传不下推 —— 子组件只 emit 修改意图，由 `TemplateTool` 调用 `src/utils/ecommerceTemplate.ts` 中的纯函数返回新 project。

### `src-tauri/src/ecommerce/`

- `mod.rs` —— pub use；register_handlers
- `models.rs` —— TemplateProject / TemplateLayer / TemplateAsset 结构
- `commands.rs` —— `import_psd_template` / `save_ecommerce_template` / `list_ecommerce_templates` / `load_ecommerce_template` / `rename_ecommerce_template` / `delete_ecommerce_template` / `list_template_assets` / `delete_template_asset` / 批量导出系列
- `psd_bridge.rs` —— shell out 到 `python3 src-tauri/python/psd_template_bridge.py`
- `render.rs` —— 把 `TemplateProject` 渲染成 PNG（用于批量导出）
- `storage.rs` —— rusqlite 表创建 / CRUD

### Tauri 命令注册

`src-tauri/src/lib.rs` 的 `run()` 在 `invoke_handler![]` 列出全部命令。新增命令必须**同时**：

1. 写 `#[tauri::command]` 函数
2. 在 `invoke_handler![]` 列表追加

否则前端 `invoke('xxx')` 会运行时报错。

## 测试策略

- **逻辑模块** —— Vitest，colocated `*.test.ts`：composable / theme / utils
- **Vue SFC** —— 不写单元测试（不引 jsdom）；改动后人工目视回归
- **后端 Rust** —— 模板模块下有 `tests/` 目录，跑业务测试
- **集成** —— 当前无 e2e；依赖 `npm run tauri:dev` + 手动操作验收

## 构建与发布

### 本地

- `npm run build` —— TS 类型检查 + Vite 生产构建（输出 `dist/`）
- `npm run tauri:build` —— 上一步 + Rust 编译 + 打包

`tauri.conf.json` 的 `bundle.targets` 限制为 `["dmg", "app", "deb", "nsis"]` —— macOS 出 dmg + app.tar.gz、Linux 出 deb、Windows 出 NSIS exe。其它平台对应字段静默忽略。

### CI（`.github/workflows/build.yml`）

两阶段 job 结构：

1. **build matrix（5 平台）**

   每个 runner 跑：
   - `dtolnay/rust-toolchain@stable` 装 target
   - `swatinem/rust-cache@v2` 缓存 Cargo
   - `actions/setup-node@v4` + `npm ci`
   - `npm run tauri -- build --target <triple>` 编译 + 打包 + 签名（签名走 env：`TAURI_SIGNING_PRIVATE_KEY` 来自 environment `prod` 的 secret，密码空字符串）
   - `bash .github/scripts/stage-bundles.sh <target> <label> <stage-dir>` 把 bundle 输出 copy 到 `runner.temp/stage` 并改名（aarch64 → arm64、x86_64/x64 → amd64、NSIS 去掉 `-setup`）
   - `actions/upload-artifact@v4` 把 stage 全部上传，名 `bundle-<label>`

2. **release job（单 ubuntu-latest）** 仅在 tag push 时跑

   - `actions/download-artifact@v4 merge-multiple: true` 把 5 个 bundle artifact 合并到一个目录
   - `node .github/scripts/build-latest-json.mjs <dir> <tag> <repo>` 扫描目录、按后缀（`_arm64.app.tar.gz` / `_amd64.app.tar.gz` / `_amd64.exe` / `_amd64.deb` / `_arm64.deb`）找 bundle，读取相邻 `.sig` 文件，输出 `latest.json` 到同目录
   - `gh release create --draft --title ...` 一次性挂载所有文件创建草稿 release

### 触发条件

| 事件 | 行为 |
|---|---|
| `workflow_dispatch` | 仅跑 build matrix，artifact 14 天有效；不进入 release job |
| push tag `v*` | build matrix → release job，创建草稿 release，待人工 publish |

### 代码签名

macOS app bundle 使用 `signingIdentity: "-"` 做 ad-hoc 签名，确保 `.app` 内资源被封存，避免下载后因无 sealed resources 被判定为损坏。未配 macOS Developer ID notarization / Windows Authenticode；用户首次打开仍可能触发 Gatekeeper / SmartScreen 警告，按 OS 说明放行即可。这与 Tauri updater 签名（`TAURI_SIGNING_PRIVATE_KEY`，独立机制）不是同一回事 —— updater 签名是有的。

## 性能 / 包大小现状

- 前端 bundle：~600 kB minified（180 kB gzip）—— Naive UI 占大头
- Rust 二进制：未优化体积；`bundle/macos/AT Tool.app` 约 30 MB（含 webview2 / 系统 webkit 链接）
- 已知优化空间：vite 代码分割（命令面板等惰加载）、删除未引用 Naive UI 组件、调整 Rust LTO

## 扩展指南

参考 `AGENTS.md` 的"加新工具的最小路径"章节。
