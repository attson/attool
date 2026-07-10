# AGENTS.md

个人桌面工具箱（Tauri 2 + Vue 3 + Naive UI）。当前内置 12 个工具：Aria2 下载、主图模板、剪贴板、JSON、视频链接抽取、图片（含跨平台截图）、文本、网络、编码、生成器、时间、HTTP 请求。

## 技术栈

| 层 | 选型 |
|---|---|
| 桌面壳 | Tauri 2（Rust 2021 edition） |
| 前端 | Vue 3.5 `<script setup>` + Naive UI 2.44 |
| 构建 | Vite 8 + TypeScript 6 |
| 包管理 | pnpm（`packageManager: pnpm@10.29.3` 锁版本，用 `corepack enable` 自动匹配） |
| 测试 | Vitest 4（无 jsdom，纯逻辑） |
| 本地存储 | rusqlite（bundled SQLite） |
| 编辑器 | Monaco Editor（JSON 工具） |
| 截图 | macOS core-graphics / Linux·Windows xcap 0.8 |
| 外部命令 | `aria2c`（下载）、`python3 + psd-tools`（PSD 解析） |

## 常用命令

```bash
pnpm tauri:dev      # 开发：起 vite + 拉起桌面窗口
pnpm dev            # 仅前端调试（http://127.0.0.1:1420）
pnpm build          # tsc + vite build
pnpm tauri:build    # 全量打包桌面应用
pnpm test           # vitest run
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
├── App.vue                      # 顶层：theme switch + AppShell + 工具路由 + Aria2 UI
├── main.ts                      # 仅引样式 + mount
├── styles/                      # tokens.css（设计 token）/ reset.css / template-editor.css
├── theme/index.ts               # Naive UI darkOverrides + lightOverrides
├── composables/                 # useTheme / useSidebarState / useLastTool / useUpdater
│                                #   / useClipboardHistory / useFileDrop / useAria2Handoff（带测试）
├── components/
│   ├── shell/                   # 外壳：AppShell / Sidebar / Topbar / Dashboard / ToolIcon
│   │                            #   / SettingsModal / UpdateBanner / ShortcutErrorNotifier
│   ├── ui/                      # 通用原子：Panel / TaskRow / StatPill / Kbd
│   ├── clipboard/               # 剪贴板：ClipboardTool + ClipboardHistoryWindow（独立窗口）+ ItemCard
│   ├── ecommerce/               # 主图模板：TemplateTool + 5 子组件（图层树/属性/画布/资源/批量）
│   ├── image/                   # 图片：ImageTool（tabs：压缩/转换/EXIF/标注/OCR/截图）
│   │                            #   + CaptureOverlay（截图浮层）+ CapturePinWindow
│   ├── json/                    # JSON：格式化/JSONPath 查询/对比/转换（Monaco）
│   ├── codec/                   # 编码：Base64/URL/Unicode/Hex/Hash/JWT
│   ├── generator/               # 生成器：密码/UUID·ULID/QR/Lorem/假数据/骰子
│   ├── text/                    # 文本：整理/排序/大小写/拆合/正则抽取/对比
│   ├── time/                    # 时间：时间戳/时区/Cron/Duration
│   ├── network/                 # 网络：URL 分解/Ping/端口/DNS
│   ├── douyin/                  # 视频链接抽取（抖音，后端另支持 B站/小红书/YouTube）
│   └── http/                    # HTTP 请求（Postman Lite）
├── types/                       # tool.ts / download.ts / ecommerceTemplate.ts / clipboard.ts
└── utils/                       # ecommerceTemplate.ts / clipboardHistory.ts 等

src-tauri/
├── src/lib.rs                   # 主入口 run() + Aria2 下载全部逻辑 + command 注册
├── src/clipboard/              # 剪贴板：commands / storage(SQLite) / watcher / models
├── src/imaging/                # 图片：commands / compress / convert / exif / ocr
│                               #   / capture（截图）/ windows（xcap 窗口枚举）
├── src/ecommerce/              # 主图模板：commands / models / render / storage / psd_bridge
├── src/network/               # 网络诊断：commands / ping / port / dns
├── src/{http,qrcode,douyin,bilibili,xhs,youtube}.rs  # 单文件模块
└── tauri.conf.json             # 打包配置（5 个窗口 / 插件 / updater endpoint）

.github/
├── workflows/build.yml          # CI 矩阵（mac arm/x64 + linux arm/x64 + win x64）+ release job
└── scripts/
    ├── stage-bundles.sh         # 每个 matrix 跑完后改名 + 暂存到 artifact
    └── build-latest-json.mjs    # release job 汇总所有 staging 后生成 latest.json

docs/spec/                       # 当前态规范（overview / ui-design-system / architecture）
docs/superpowers/                # superpowers 流程产物（每任务 1 份 spec + plan，已 gitignore）
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

## 软件更新

- 走 Tauri 官方 `tauri-plugin-updater`（v2）+ `tauri-plugin-process`，配置在 `src-tauri/tauri.conf.json` 的 `plugins.updater`，endpoint 指向 GitHub Releases 的 `latest.json`
- 启动 5s 后若 `attool.updater.autoCheck=1` 自动检查；发现新版在 topbar 下方显示 banner，用户点 "现在安装" 触发下载 + 安装
- 设置入口：sidebar 底栏齿轮按钮 → SettingsModal
- 状态机：`idle → checking → {up-to-date | available | error}`；`available → downloading → ready → relaunch`
- 签名密钥（**必须妥善备份**）：本地 `tauri signer generate` 出私钥 + 公钥；公钥写入 `tauri.conf.json`；私钥**整文件内容**存 GitHub Environment `prod` 的 secret `TAURI_SIGNING_PRIVATE_KEY`（密码因为生成时没设所以 workflow 里直接写空字符串 `""`，不用 secret 存）
- **更新覆盖范围**：`latest.json` 只声明 macOS arm64 / macOS amd64 / Windows amd64 三个平台。Linux `.deb` 不被 Tauri 签名（`createUpdaterArtifacts` 不支持 deb），所以 Linux 用户走"重新下载 deb 安装"，不走 in-app 一键升级

## 发布流程

1. 在 main 上把功能合并 + push
2. 三处版本号同步 bump：

   - `package.json` `"version"`
   - `src-tauri/tauri.conf.json` `"version"`
   - `src-tauri/Cargo.toml` `version = "..."`

   跑一次 `cargo check`（更新 Cargo.lock）和 `pnpm build`（确认编译过），把这 4 个文件一起 commit：`chore: bump version to X.Y.Z`

3. 打 annotated tag + 推：

   ```bash
   git tag -a vX.Y.Z -m "vX.Y.Z — <一句话亮点>"
   git push origin main
   git push origin vX.Y.Z
   ```

4. tag push 触发 `.github/workflows/build.yml`：

   - **build job（matrix × 5）**：每个 runner 跑 `pnpm tauri build --target <triple>`，跑完用 `stage-bundles.sh` 把产物 copy 到 `runner.temp/stage` 并按 `amd64`/`arm64` 改名，上传成 artifact
   - **release job（单 ubuntu）**：下载所有 artifact 到一个目录，跑 `build-latest-json.mjs` 生成 `latest.json`，`gh release create` 一次性创建正式 release + 把所有文件挂上去

5. 全部跑完（约 5-10 分钟）后 GitHub Releases 会直接公开正式 release。文件名应该正好这 5 个 + macOS 的 2 个 `.app.tar.gz`（updater 内部用）+ 各自的 `.sig` + 一个 `latest.json`

6. release 公开后约 5 分钟（GitHub CDN 缓存），装着旧版的 macOS / Windows 客户端启动 5 秒就会看到 banner

如果 CI 失败：
- 取消 stuck run（`gh run cancel <id>`）
- 删除 release（`gh release delete vX.Y.Z --yes`）
- 删除 tag 本地+远端（`git tag -d vX.Y.Z && git push origin :refs/tags/vX.Y.Z`）
- 修代码、push 到 main、重新打同名 tag

## 已知遗留

- `src-tauri/src/lib.rs` 还保留 `batch_add_logo` / `list_logo_presets` / `save_logo_preset` 命令，前端"电商图片处理"工具已下线（被主图模板替代）。要清理就把这些命令和相关 struct 一起删掉，并从 `invoke_handler` 列表移除
- 字体没打包，依赖系统字体栈（`-apple-system / SF Pro Text / PingFang SC / Inter / Segoe UI`）

## 不要做

- 不要在组件里写散装 hex 色、`rgba(...)`、不在 token 列表里的 `border-radius`
- 不要在 `<style scoped>` 里复制粘贴另一个组件的样式（应该走 token + 共享 class，或者放到 `src/styles/*.css`）
- 不要把 `--bg-base` / `--text` 等 dark 假设硬写进选择器（应该让 `[data-theme=light]` 自动接管）
- 不要在 `src/App.vue` 里塞业务逻辑 —— 当前已经是路由壳 + Aria2 工具，不要再扩。新增工具走独立组件
- 不要把"电商图片处理"功能加回来（已弃，主图模板覆盖该场景）
