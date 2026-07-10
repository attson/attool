# 概述

## 是什么

attool 是一个个人使用的桌面工具集合，运行在 Tauri 2 上。前端 Vue 3 + Naive UI，后端 Rust。所有数据本地存储（rusqlite），不联网（除工具自身职责，如下载）。

## 当前内置的工具

共 12 个工具，全部 `status: 'ready'`。前端在 `src/App.vue` 的 `tools[]` 数组注册，多数为惰加载（`defineAsyncComponent`）。下面详列两个"重后端"工具作为代表，其余见汇总表。

### Aria2 下载（`id: aria2`）

- shell out 到本机 `aria2c`，支持单服务器多连接、文件分片、批量 URL 提交、实时进度回传
- 任务状态：`queued | running | completed | failed | cancelled`
- 进度通过 Tauri event `download-progress` 流式推送给前端；任务历史存 SQLite
- Tauri 命令：`start_download` / `cancel_download` / `list_download_tasks` / `open_download_folder` 等
- 该工具直接内嵌在 `App.vue`，是"表单 + 列表"形态的参考实现

### 主图模板（`id: template`）

- PSD 导入（依赖本机 `python3 + psd-tools`，桥接脚本在 `src-tauri/src/ecommerce/psd_bridge/`）
- 图层支持图片 / 文本 / 形状 / 组，可拖拽、改尺寸、重排序
- 字段占位：图层名标 `{{field_name}}` 后，批量任务中按行替换
- 批量任务：对单个图层注入多组变体，按笛卡尔展开生成所有结果，渲染成 PNG
- 前端 `src/components/ecommerce/TemplateTool.vue`（下挂 5 子组件），后端 `src-tauri/src/ecommerce/`

### 其余工具

| 工具（id） | 能力 | 后端 |
|---|---|---|
| 剪贴板（`clipboard`） | 历史记录、图片/文本预览、Paste 风格恢复、快捷键自定义、独立浮窗 | `clipboard/`（SQLite + 系统剪贴板监听） |
| JSON（`json`） | 格式化、JSONPath 查询、对比、YAML/CSV/XML 转换 | 纯前端（Monaco） |
| 视频链接抽取（`douyin`） | 抖音 / B站 / 小红书 / YouTube 链接与文案解析 | `douyin/bilibili/xhs/youtube.rs` |
| 图片（`image`） | 压缩、格式转换、EXIF、跨平台截图、标注、OCR | `imaging/`（含 xcap 截图） |
| 文本（`text`） | 整理、排序、大小写、拆合、正则抽取、对比 | 纯前端 |
| 网络（`network`） | URL 分解、Ping、端口、DNS | `network/` |
| 编码（`codec`） | Base64/URL/Unicode/Hex/Hash/JWT | 纯前端 |
| 生成器（`generator`） | 密码、UUID·ULID、二维码、Lorem、假数据、骰子 | `qrcode.rs`（二维码），其余前端 |
| 时间（`time`） | 时间戳、时区、Cron、Duration | 纯前端 |
| HTTP 请求（`http`） | GET/POST/PUT/DELETE/PATCH + Headers/Query/Body | `http.rs` |

## 不在范围内

- 联网同步 / 多设备 / 账号体系
- 浅色/深色之外的多主题 / 国际化
- 命令面板（`⌘K` 占位 alert）
- 移动端 / Web 端

## 平台支持

通过 GitHub Actions 矩阵构建（`.github/workflows/build.yml`）：

| 平台 | Runner | Target | Bundle 产物 |
|---|---|---|---|
| macOS Apple Silicon | `macos-14` | `aarch64-apple-darwin` | `_arm64.dmg`、`_arm64.app.tar.gz`（updater） |
| macOS Intel | `macos-14`（cross-compile） | `x86_64-apple-darwin` | `_amd64.dmg`、`_amd64.app.tar.gz`（updater） |
| Linux x64 | `ubuntu-24.04` | `x86_64-unknown-linux-gnu` | `_amd64.deb` |
| Linux ARM64 | `ubuntu-24.04-arm` | `aarch64-unknown-linux-gnu` | `_arm64.deb` |
| Windows x64 | `windows-latest` | `x86_64-pc-windows-msvc` | `_amd64.exe`（NSIS） |

> macOS Intel runner（`macos-13`）在 GitHub 公共池容量极差（曾经排队数小时不动），所以 `macos-x64` 改在 `macos-14` 上 cross-compile。不依赖紧缺 runner，5-10 分钟全 matrix 跑完。

打 `v*` tag 自动建 GitHub Release 草稿；`workflow_dispatch` 手动跑则只上传 14 天有效的 artifact。macOS app bundle 仅做 ad-hoc 签名来封存资源，未做 Developer ID 公证；Windows 未做 Authenticode。用户首次打开仍可能触发 Gatekeeper / SmartScreen，但 Tauri updater 自己的签名是有的（独立机制，由 `TAURI_SIGNING_PRIVATE_KEY` 注入）。

## 运行时依赖

打包后用户机器上需要：

- `aria2c`（下载工具用）—— `brew install aria2` / `apt install aria2`
- `python3` + `psd-tools`（PSD 导入用）—— `python3 -m pip install --user psd-tools`
- 网络访问 GitHub Releases endpoint（`releases/latest/download/latest.json`）—— 仅在自动 / 手动检查更新时使用，可在 Settings 中关闭"启动时自动检查更新"

未来可考虑把 aria2c 静态编译进 sidecar、或把 PSD 解析改成纯 Rust（`psd` crate）。
