# AGENTS.md

个人桌面工具箱（Tauri 2 + Vue 3 + Naive UI）。当前内置 12 个工具：Aria2 下载、主图模板、剪贴板、JSON、视频链接抽取、图片（含跨平台截图）、文本、网络、编码、生成器、时间、HTTP 请求（Apifox-lite：多 tab / 历史 / 环境变量 / cURL / multipart）。

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
| 更新 | 自研 updater（Ed25519 签名 SHA256SUMS + 平台专用 apply 脚本，覆盖 mac/win/**Linux**） |
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
│                                #   / useClipboardHistory / useFileDrop / useAria2Handoff
│                                #   / useHttpStore（HTTP 工具全局单例，带测试）
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
│   └── http/                    # HTTP 请求：三栏（Sidebar + TabBar + RequestEditor + ResponseView）
│                                #   + EnvModal + curl.ts / variables.ts / httpApi.ts / types.ts（带测试）
├── types/                       # tool.ts / download.ts / ecommerceTemplate.ts / clipboard.ts
└── utils/                       # ecommerceTemplate.ts / clipboardHistory.ts 等

src-tauri/
├── build.rs                     # 注入 ATTOOL_UPDATE_VERIFY_PUBLIC_KEY 到 OUT_DIR/verify_public_key.pem
├── src/lib.rs                   # 主入口 run() + Aria2 下载全部逻辑 + command 注册
├── src/clipboard/               # 剪贴板：commands / storage(SQLite) / watcher / models
├── src/imaging/                 # 图片：commands / compress / convert / exif / ocr
│                                #   / capture（截图）/ windows（xcap 窗口枚举）
├── src/ecommerce/               # 主图模板：commands / models / render / storage / psd_bridge
├── src/network/                 # 网络诊断：commands / ping / port / dns
├── src/http/                    # HTTP 请求：mod / models / send（含 multipart 与 cancel）
│                                #   / cancel / storage（tabs+history+envs+env_vars 4 张表）/ commands
├── src/updater/                 # 自研 updater：mod / check / verify / download / keys / state
│                                #   / commands / apply/{macos,windows,linux}
│                                #   + scripts/{install-linux.sh, update-windows.bat}（embed）
├── src/{qrcode,douyin,bilibili,xhs,youtube}.rs  # 单文件模块
└── tauri.conf.json              # 打包配置（5 个窗口 / 插件 / 无 updater plugin，走自研）

.github/
├── workflows/build.yml          # CI 矩阵（mac arm/x64 + linux arm/x64 + win x64）+ release job
└── scripts/
    ├── stage-bundles.sh         # 每个 matrix 跑完后打包 updater archive（tar/7z）+ 改名
    └── sign-checksums.mjs       # release job 用 Ed25519 私钥签 SHA256SUMS

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

## 软件更新（v0.8.5 起自研）

### 客户端

- 前端 `src/composables/useUpdater.ts`：状态机 `idle → checking → {up-to-date | available | error} → downloading → verifying → ready → applying`；订阅 `updater://state` 事件
- 后端 `src-tauri/src/updater/`：
  - `check.rs` 查 GitHub `releases/latest` API，semver 比较，按 GOOS/GOARCH 挑 asset（`AT.Tool_<v>_<arch>.{app.tar.gz | exe.zip | tar.gz}`）
  - `verify.rs` 校验 `SHA256SUMS.sig`（Ed25519 detached）+ 匹配单文件 sha256
  - `download.rs` reqwest 流式下载，边下边算 sha，150ms 节流进度事件；支持 cancel + 断点复用（stage 目录同名文件校验过就跳过）
  - `apply/macos.rs` tar 解 `.app.tar.gz` + rename swap `.app.old` + `open` 拉起
  - `apply/windows.rs` PowerShell Expand-Archive + embed `update-windows.bat`（timeout + copy /y + start）
  - `apply/linux.rs` embed `install-linux.sh`（等父进程死 + tar 解 + mv 覆盖 / 失败走 `pkexec` 提权 + `setsid` 分离）
- UI：sidebar 底部齿轮打开 `SettingsModal`；topbar 下方 `UpdateBanner`；发现新版点"现在安装"触发 download + apply

### 密钥（Ed25519）

- **公钥**：GitHub Environment `prod` secret `ATTOOL_UPDATE_VERIFY_PUBLIC_KEY`（PKCS8 SPKI PEM 整块）
- **私钥**：Environment `prod` secret `ATTOOL_UPDATE_SIGNING_PRIVATE_KEY`（PKCS8 PEM 整块），**必须离线备份**
- **注入方式**：`build.rs` 从环境变量读，写到 `$OUT_DIR/verify_public_key.pem`；`updater/keys.rs` 用 `include_str!()` 嵌入。**空串等价于禁用 updater**，dev build 自动走这条
- **CI 签名**：release job 里 `node .github/scripts/sign-checksums.mjs SHA256SUMS`，用 stdlib `crypto.sign('ed25519')`
- 生成一对新密钥（本机跑，不要在 Claude 会话里）：
  ```bash
  node -e "
    const c = require('crypto');
    const { publicKey, privateKey } = c.generateKeyPairSync('ed25519');
    console.log(publicKey.export({ type: 'spki', format: 'pem' }).trim());
    console.log(privateKey.export({ type: 'pkcs8', format: 'pem' }).trim());
  "
  ```

### Release 产物（v0.8.5+ 每次发版）

```
SHA256SUMS                      # <sha>  <filename> 每行一条
SHA256SUMS.sig                  # base64 单行 Ed25519 detached sig

AT.Tool_<v>_arm64.dmg           # 用户下载
AT.Tool_<v>_amd64.dmg
AT.Tool_<v>_amd64.exe           # Windows NSIS installer
AT.Tool_<v>_amd64.deb           # Linux
AT.Tool_<v>_arm64.deb

AT.Tool_<v>_arm64.app.tar.gz    # updater 归档（macOS .app 打包）
AT.Tool_<v>_amd64.app.tar.gz
AT.Tool_<v>_amd64.exe.zip       # updater 归档（Windows exe 打包）
AT.Tool_<v>_amd64.tar.gz        # updater 归档（Linux 裸二进制打包）
AT.Tool_<v>_arm64.tar.gz
```

**Linux 覆盖**：v0.8.5 起 Linux `.deb` 装的用户也走 in-app 升级；`install-linux.sh` 覆盖 `/usr/bin/attool` 通过 `pkexec` 提权，无 pkexec 环境会友好报错并要求手动 `sudo mv`。

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

   - **build job（matrix × 5）**：每个 runner 跑 `pnpm tauri build --target <triple>`，跑完 `stage-bundles.sh` 把 dmg/deb/exe 复制到 `runner.temp/stage` 并**同时打包 updater archive**（macOS 用 `tar` 打 `.app`；Windows 用 `7z` 打 exe；Linux 用 `tar` 打裸二进制），上传成 artifact
   - **release job（单 ubuntu，`environment: prod`）**：下载所有 artifact 到一个目录 → `find` 生成 `SHA256SUMS` → `sign-checksums.mjs` 签成 `.sig` → `gh release create` 直接发布正式 release（不再走草稿）

5. 全部跑完（约 6-10 分钟）后 GitHub Releases 会直接公开正式 release。文件名应该正好是上面 §Release 产物列的 12 个

6. release 公开后约 5 分钟（GitHub CDN 缓存），装着旧版的客户端启动 5 秒就会看到 banner

如果 CI 失败：
- 取消 stuck run（`gh run cancel <id>`）
- 删除 release（`gh release delete vX.Y.Z --yes`）
- 删除 tag 本地+远端（`git tag -d vX.Y.Z && git push origin :refs/tags/vX.Y.Z`）
- 修代码、push 到 main、重新打同名 tag

## 已知遗留

- `src-tauri/src/lib.rs` 还保留 `batch_add_logo` / `list_logo_presets` / `save_logo_preset` 命令，前端"电商图片处理"工具已下线（被主图模板替代）。要清理就把这些命令和相关 struct 一起删掉，并从 `invoke_handler` 列表移除
- 字体没打包，依赖系统字体栈（`-apple-system / SF Pro Text / PingFang SC / Inter / Segoe UI`）
- v0.8.4 及以前的用户**无法自动升到 v0.8.5**（老 tauri-plugin-updater 查 `latest.json`，已下线）—— 必须手动下载 v0.8.5 安装包装一次

## 不要做

- 不要在组件里写散装 hex 色、`rgba(...)`、不在 token 列表里的 `border-radius`
- 不要在 `<style scoped>` 里复制粘贴另一个组件的样式（应该走 token + 共享 class，或者放到 `src/styles/*.css`）
- 不要把 `--bg-base` / `--text` 等 dark 假设硬写进选择器（应该让 `[data-theme=light]` 自动接管）
- 不要在 `src/App.vue` 里塞业务逻辑 —— 当前已经是路由壳 + Aria2 工具，不要再扩。新增工具走独立组件
- 不要把"电商图片处理"功能加回来（已弃，主图模板覆盖该场景）
- 不要重新引入 `tauri-plugin-updater` / `tauri-plugin-process` —— 已被自研 updater 替代
