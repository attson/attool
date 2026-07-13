# 架构

## 总览

```
┌──────────────────────────────────────────────────────────┐
│  Vue 3 SFC（src/）                                       │
│  ├─ App.vue          主路由壳 + theme + Aria2 工具页       │
│  ├─ shell/*          AppShell / Sidebar / Topbar /        │
│  │                   Dashboard / SettingsModal /           │
│  │                   UpdateBanner / ...                    │
│  ├─ ui/*             Panel / TaskRow / StatPill / Kbd     │
│  ├─ <12 工具>/*      clipboard / image / json / codec /   │
│  │                   generator / text / time / network /  │
│  │                   http / douyin / ecommerce（懒加载）   │
│  ├─ composables/*    useTheme / useSidebarState /         │
│  │                   useUpdater / useClipboardHistory /    │
│  │                   useHttpStore（HTTP 单例）...          │
│  ├─ theme/index.ts   Naive UI overrides（dark + light）    │
│  ├─ styles/*.css     tokens + reset + template-editor     │
│  ├─ types/*          Tool / DownloadTask / Clipboard...   │
│  └─ utils/*          ecommerceTemplate / clipboardHistory │
│                                                          │
│           ↕ invoke() / listen()                          │
│                                                          │
│  Rust（src-tauri/src/）                                  │
│  ├─ build.rs         注入 updater 公钥 → OUT_DIR/*.pem     │
│  ├─ lib.rs           run() + Aria2 下载 + command 注册    │
│  ├─ clipboard/       历史存储 + 系统剪贴板监听（SQLite）   │
│  ├─ imaging/         压缩/转换/EXIF/OCR/截图（xcap）       │
│  ├─ ecommerce/       模板 render / storage / psd_bridge   │
│  ├─ network/         ping / port / dns                    │
│  ├─ http/            HTTP：send + storage(4 表) + cancel   │
│  ├─ updater/         自研更新：check / verify / download / │
│  │                   apply/{mac,win,linux} + embed scripts │
│  └─ *.rs             qrcode / douyin·bili·xhs·yt          │
│                                                          │
│           ↕ Command::new / xcap / reqwest                │
│                                                          │
│  本机外部进程 / 系统能力                                   │
│  ├─ aria2c           HTTP/FTP/BT 下载                    │
│  ├─ python3 + psd-tools   PSD 解析                        │
│  ├─ xcap / core-graphics  屏幕截图                        │
│  ├─ pkexec           Linux 升级提权（updater）             │
│  └─ open / explorer / xdg-open    打开文件夹              │
└──────────────────────────────────────────────────────────┘
```

## 前后端通信

- **请求-响应：** `invoke<T>('command_name', { arg })` （`@tauri-apps/api/core`）
- **流式事件：** Rust `app_handle.emit("event-name", payload)` → 前端 `listen<T>('event-name', cb)`
  - `download-progress`（aria2 输出解析后推送）、`clipboard-history-updated`（剪贴板监听）、`capture-overlay-init` / `capture-failed`（截图浮层）、`updater://state`（自研 updater 状态机推送）等
- **错误：** Rust 命令返回 `Result<T, String>`，前端 `try { await invoke(...) } catch (e)` 捕获

## 状态分层

| 层 | 存储 | 例 |
|---|---|---|
| UI session | Vue ref | 当前选中工具、表单值、loading flags |
| 浏览器持久 | `localStorage` | sidebar 折叠状态、上次打开工具、主题、属性面板折叠 |
| 应用持久 | rusqlite（Tauri data dir） | 下载任务历史、剪贴板历史、模板项目 / 素材库、**HTTP 工具的 tabs / history / envs / env_vars** |
| updater 缓存 | 文件（`app_cache_dir/updater/`） | 已下载但未 apply 的 archive stage |
| 临时 | Tauri runtime State | 进行中的下载子进程句柄（`DownloadTasks`）、HTTP cancel token 表、Updater 状态机 `UpdaterState` |

HTTP 工具的持久层分 4 张表存在独立 `http.sqlite3`：
- `http_tabs` —— 每 tab 一行，spec 序列化 JSON 存 `spec_json`；`is_active` 单例约束
- `http_history` —— 每次发送插一条，500 条上限 + 30 天 TTL，响应体只存前 4KB 到 `resp_summary`
- `http_envs` —— 环境集合，`is_active` 单例
- `http_env_vars` —— 变量条目，`env_id=''` 表示全局作用域

## 软件更新（v0.8.5 起自研）

**放弃 `tauri-plugin-updater`**（只支持 macOS `.app` / Windows `.msi` / Linux `.AppImage`，不覆盖 attool 的 `.deb` 分发）。替换为一套三平台统一的自研 updater。

### 端到端流程

```
1. useUpdater.check() → invoke('updater_check')
2. Rust check.rs: GET api.github.com/.../releases/latest
                  → semver 比较 → 挑 assetName (按 GOOS/GOARCH)
                  → phase = Available { info }
3. useUpdater.install() → invoke('updater_download')
4. Rust: GET SHA256SUMS + SHA256SUMS.sig
         → verify.rs: Ed25519.verify(pub, sums, sig)
         → lookup expected_sha256 = SHA256SUMS[assetName]
         → download.rs: 流式 GET assetUrl，边下边算 sha
         → phase = Ready
5. useUpdater.relaunch() → invoke('updater_apply')
6. Rust apply/*: 平台专用替换 + 重启 (见下)
```

### 平台专用 apply

| 平台 | 逻辑 |
|---|---|
| macOS | tar 解 `.app.tar.gz` → rename current `.app` 为 `.app.old` → rename 新 `.app` 到位（失败回滚）→ `open <bundle>` 拉起 → app.exit(0) |
| Windows | PowerShell `Expand-Archive` `.exe.zip` → embed `update-windows.bat`（timeout 2s → `copy /y` → `start`）→ `cmd /C start bat` → app.exit(0) |
| Linux | 写 embed `install-linux.sh` 到 `/tmp/attool-update-<pid>.sh` → chmod 755 → 传 `<pid> <staged.tar.gz> <current_exe_path>` → app.exit(0)；脚本内：等父进程死 → tar 解 → mv 覆盖 / 失败走 `pkexec` → `setsid` 拉起 → 清理 tmp |

### 密钥体系

- **Ed25519**（`ed25519-dalek` 校验、Node stdlib `crypto.sign` 签名）
- 私钥 → GitHub Environment `prod` secret `ATTOOL_UPDATE_SIGNING_PRIVATE_KEY`（PKCS8 PEM 整块），**必须离线备份**
- 公钥 → Environment secret `ATTOOL_UPDATE_VERIFY_PUBLIC_KEY`（SPKI PEM 整块）
- 公钥注入：`build.rs` 从 env 读，写到 `$OUT_DIR/verify_public_key.pem`，`updater/keys.rs` 用 `include_str!()` 嵌入
  - **不用 `cargo:rustc-env=`** 因为它是单行指令，多行 PEM 会被截断成第一行
- 空公钥 = updater 全禁用（dev build 场景）

### CI 侧

- `.github/workflows/build.yml` 每 matrix 除 installer 外**打 updater 归档**：macOS `tar -czf .app.tar.gz`、Windows `7z a .exe.zip`、Linux `tar -czf .tar.gz`（裸二进制 `attool`）
- release job（`environment: prod`）：`find` 生成 `SHA256SUMS` → `node .github/scripts/sign-checksums.mjs` → `gh release create` 一次性挂全部

### 前端

- `src/composables/useUpdater.ts`：兼容旧 UI 接口（`state / check / install / relaunch / dismiss`）；内部 `invoke() + listen('updater://state')`
- UI：`SettingsModal.vue`（sidebar 齿轮打开）+ `UpdateBanner.vue`（topbar 下）

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

**HTTP 工具的 `useHttpStore` 是单例 composable**（模块级 `reactive`），全局共享 tabs / history / envs / active 环境；`_resetHttpStoreForTest(api)` 提供注入点便于单测。

### `src/components/ecommerce/TemplateTool.vue`

模板编辑器主组件。组合 5 个子组件：

- `TemplateResourcePanel` —— 左侧资源面板，5 个 tab（文字 / 图片 / 素材 / 图层 / 模板）
- `LayerTree` —— 图层树（资源 panel 的"图层"tab 内）
- `TemplateCanvas` —— 中央画布（带选中框 / 手柄 / 拖拽）
- `LayerProperties` —— 右侧属性面板（位置 / 外观 / 字段 / 字体 / 对齐 / 高级）
- `BatchTaskPanel` —— 底部批量任务面板（按图层注入变体 → 1:1 笛卡尔展开）

状态全部在 `TemplateTool.vue` 顶层 `project: ref<TemplateProject>`，下传不下推 —— 子组件只 emit 修改意图，由 `TemplateTool` 调用 `src/utils/ecommerceTemplate.ts` 中的纯函数返回新 project。

### `src/components/http/HttpTool.vue`

HTTP 工具主容器（三栏）。组合 5 个子组件 + 3 个纯函数模块：

- `HttpSidebar` —— 左侧历史列表 + 搜索 + 右键菜单
- `HttpTabBar` —— 中上多 tab 条（+/✕/中键关闭）
- `HttpRequestEditor` —— 中下上半：地址栏（含变量高亮 `<VarText>`）+ 子 tabs（Params / Auth / Headers / Body / Settings）
- `HttpResponseView` —— 中下下半：Body（Pretty/Raw/Preview）+ Headers + Cookies
- `HttpEnvModal` —— 环境 + 变量 CRUD 弹窗
- `curl.ts` / `variables.ts` / `httpApi.ts` / `types.ts` —— 纯函数与类型，vitest 覆盖 28+ 用例

状态全部走 `useHttpStore` 单例；子组件通过 props 拿 spec，通过 emit 触发修改；`useHttpStore` 用 300ms debounce 把 tabs 写回 SQLite（关闭 / 发送 / 拖拽即时 flush）。

### 后端模块概览

`lib.rs` 承载主入口 `run()`、Aria2 下载全部逻辑与 command 注册；其余能力按模块拆分：

| 模块 | 职责 | 关键 command |
|---|---|---|
| `lib.rs` | Aria2 下载（shell out `aria2c` + 进度事件）、command 注册 | `start_download` / `cancel_download` / `list_download_tasks` / `generate_qr_png` / `open_external_url` |
| `clipboard/` | 剪贴板历史（SQLite 存储 + 系统剪贴板监听 + 快捷键面板） | `list_clipboard_items` / `restore_clipboard_item` / `clear_clipboard_history` / `get·save_clipboard_settings` |
| `imaging/` | 图片压缩 / 转换 / EXIF / OCR / 截图 | `compress_images` / `convert_images` / `read·strip_image_exif` / `ocr_image` / `capture_screen` / `open·commit·pin_capture_overlay` / `get·set_capture_shortcut` |
| `ecommerce/` | 主图模板：PSD 导入 / 图层渲染 / 批量替换 | `import_psd_template` / `save·load·list·delete_ecommerce_template` / `run_batch_replace_tasks` |
| `network/` | 网络诊断 | `ping_host` / `check_ports` / `resolve_dns` |
| `http/` | HTTP 工具后端：请求发送（含 multipart + cancel）+ tabs/history/envs/env_vars 4 张表 CRUD | `send_http` / `cancel_http` / `list·upsert·delete·set_active_http_{tab,env}` / `list·insert·delete·clear_http_history` / `list·upsert·delete_http_env_var` |
| `updater/` | 自研更新：check / verify / download / apply | `updater_get_state` / `updater_check` / `updater_download` / `updater_apply` / `updater_cancel` |
| `qrcode.rs` | 二维码生成 | `generate_qr_png` |
| `douyin.rs` / `bilibili.rs` / `xhs.rs` / `youtube.rs` | 视频链接解析（reqwest + 正则） | `extract_douyin_video` / `extract_bilibili_video` / `extract_youtube_video` / `resolve_douyin_url` |

**截图跨平台策略**（`imaging/capture.rs` + `windows.rs`）：macOS 走 `screencapture` + CoreGraphics 窗口枚举；Linux / Windows 走 `xcap`（`Monitor::capture_image` 截屏、`Window::all` 枚举）。浮层背景帧存为 BMP（无压缩，避免 PNG deflate 的秒级延迟）。Wayland 会话下 xcap 走 xdg-desktop-portal，可能弹权限 / 部分场景失败，窗口枚举降级为空列表（区域选择仍可用）。

### `src-tauri/src/ecommerce/`

- `mod.rs` —— pub use；register_handlers
- `models.rs` —— TemplateProject / TemplateLayer / TemplateAsset 结构
- `commands.rs` —— `import_psd_template` / `save_ecommerce_template` / `list_ecommerce_templates` / `load_ecommerce_template` / `rename_ecommerce_template` / `delete_ecommerce_template` / `list_template_assets` / `delete_template_asset` / 批量导出系列
- `psd_bridge.rs` —— shell out 到 `python3 src-tauri/python/psd_template_bridge.py`
- `render.rs` —— 把 `TemplateProject` 渲染成 PNG（用于批量导出）
- `storage.rs` —— rusqlite 表创建 / CRUD

### `src-tauri/src/http/`

- `mod.rs` —— 模块声明
- `models.rs` —— `HttpRequestSpec` / `HttpResponseInfo` / `KeyValue` / `MultipartField` / `HttpAuth` / 4 张表的 Row 结构
- `send.rs` —— reqwest 组装（含 auth inject、multipart 组装、cancel token 集成）
- `cancel.rs` —— `HttpCancelState`：`Mutex<HashMap<String, oneshot::Sender<()>>>`
- `storage.rs` —— 4 张表的 init + CRUD + history TTL 清理（启动时跑一次）
- `commands.rs` —— 全部 tauri 命令

### `src-tauri/src/updater/`

- `mod.rs` —— 模块声明
- `models.rs` —— `Phase` enum / `ReleaseInfo` / `Snapshot` / `GithubRelease` 结构
- `keys.rs` —— `include_str!(OUT_DIR/verify_public_key.pem)`；`is_enabled()` / `verify_public_key()`
- `check.rs` —— `fetch_latest_release()` + `newer_than_current()` + `expected_asset_name(os, arch)` + `build_release_info()`
- `verify.rs` —— `verify_checksums_signature()` + `lookup_expected_sha256()` + `compute_sha256_hex()`
- `download.rs` —— `fetch_text()` + `download_to_stage()`（流式 + 进度 emit + AtomicBool cancel）
- `state.rs` —— `UpdaterState`：Mutex<Snapshot> + stage_dir + cancel + cached_release + staged_asset
- `commands.rs` —— tauri 命令：`updater_{get_state, check, download, apply, cancel}`
- `apply/{mod, macos, windows, linux}.rs` —— 平台专用替换与重启
- `scripts/{install-linux.sh, update-windows.bat}` —— embed 脚本（`include_str!`）

### Tauri 命令注册

`src-tauri/src/lib.rs` 的 `run()` 在 `invoke_handler![]` 列出全部命令。新增命令必须**同时**：

1. 写 `#[tauri::command]` 函数
2. 在 `invoke_handler![]` 列表追加

否则前端 `invoke('xxx')` 会运行时报错。

## 测试策略

- **逻辑模块** —— Vitest，colocated `*.test.ts`：composable / theme / utils / HTTP 工具的 `variables.ts` `curl.ts` `httpApi.ts`
- **Vue SFC** —— 不写单元测试（不引 jsdom）；改动后人工目视回归
- **后端 Rust** —— 模板模块、updater 模块（`updater::verify` + `updater::check` 单测覆盖签名 / SHA256SUMS lookup / semver 比较）
- **集成** —— 当前无 e2e；依赖 `pnpm tauri:dev` + 手动操作验收

## 构建与发布

### 本地

- `pnpm build` —— TS 类型检查 + Vite 生产构建（输出 `dist/`）
- `pnpm tauri:build` —— 上一步 + Rust 编译 + 打包

`tauri.conf.json` 的 `bundle.targets` 限制为 `["dmg", "app", "deb", "nsis"]` —— macOS 出 dmg + app、Linux 出 deb、Windows 出 NSIS exe。**关闭 `createUpdaterArtifacts`**：updater archive 由 CI 自己打（不依赖 Tauri signer）。

### CI（`.github/workflows/build.yml`）

两阶段 job 结构：

1. **build matrix（5 平台）**

   每个 runner 跑：
   - `dtolnay/rust-toolchain@stable` 装 target
   - `swatinem/rust-cache@v2` 缓存 Cargo
   - `pnpm/action-setup@v4` + `actions/setup-node@v4` + `pnpm install --frozen-lockfile`
   - `pnpm tauri build --target <triple>` 编译 + 打包
     - env `ATTOOL_UPDATE_VERIFY_PUBLIC_KEY` 从 secret 注入到 `build.rs`（写到 `OUT_DIR/verify_public_key.pem`）
   - `bash .github/scripts/stage-bundles.sh <target> <label> <stage-dir>`：
     - 把 installer 复制到 stage 并规整名（`AT.Tool_<v>_<arch>.{dmg,deb,exe}`）
     - **同时打 updater archive**：macOS `tar -czf .app.tar.gz "AT Tool.app"`、Windows `7z a .exe.zip "AT Tool.exe"`、Linux `tar -czf .tar.gz attool`（Cargo package 名）
   - `actions/upload-artifact@v4` 把 stage 全部上传，名 `bundle-<label>`

2. **release job（单 ubuntu-latest，`environment: prod`）** 仅在 tag push 时跑

   - `actions/download-artifact@v4 merge-multiple: true` 把 5 个 bundle artifact 合并到一个目录
   - `find` 生成 `SHA256SUMS`（覆盖所有 dmg / deb / exe / tar.gz / zip）
   - `node .github/scripts/sign-checksums.mjs SHA256SUMS`（读 `ATTOOL_UPDATE_SIGNING_PRIVATE_KEY` env）产 `SHA256SUMS.sig`
   - `gh release create <tag> --title "AT Tool <tag>" --notes "" <all files>` 直接发布正式 release

### 触发条件

| 事件 | 行为 |
|---|---|
| `workflow_dispatch` | 仅跑 build matrix，artifact 14 天有效；不进入 release job |
| push tag `v*` | build matrix → release job，直接创建 published release |

### 代码签名

macOS app bundle 使用 `signingIdentity: "-"` 做 ad-hoc 签名，确保 `.app` 内资源被封存，避免下载后因无 sealed resources 被判定为损坏。未配 macOS Developer ID notarization / Windows Authenticode；用户首次打开仍可能触发 Gatekeeper / SmartScreen 警告，按 OS 说明放行即可。

**Updater 签名与 Gatekeeper 无关**，是独立机制：`ATTOOL_UPDATE_SIGNING_PRIVATE_KEY` 签 `SHA256SUMS`，客户端用编译进二进制的公钥（`build.rs` 写到 `OUT_DIR/verify_public_key.pem`）验签。

## 性能 / 包大小现状

- 前端 bundle：~630 kB minified（180 kB gzip）—— Naive UI + Monaco 占大头（Monaco 单独 chunk 惰加载）
- HTTP 工具 chunk：~63 kB minified（20 kB gzip），惰加载
- Rust 二进制：未优化体积；`bundle/macos/AT Tool.app` 约 30 MB（含 webview2 / 系统 webkit 链接）
- 已知优化空间：vite 代码分割（命令面板等惰加载）、删除未引用 Naive UI 组件、调整 Rust LTO

## 扩展指南

参考 `AGENTS.md` 的"加新工具的最小路径"章节。
