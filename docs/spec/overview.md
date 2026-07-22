# 概述

## 是什么

attool 是一个个人使用的桌面工具集合，运行在 Tauri 2 上。前端 Vue 3 + Naive UI，后端 Rust。所有数据本地存储（rusqlite），不联网（除工具自身职责如下载，以及自研 updater 检查 GitHub Releases）。

## 当前内置的工具

共 12 个工具，全部 `status: 'ready'`。前端在 `src/App.vue` 的 `tools[]` 数组注册，多数为惰加载（`defineAsyncComponent`）。下面详列 3 个"重后端"工具作为代表，其余见汇总表。

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

### HTTP 请求（`id: http`）

Apifox-lite。三栏布局：左侧历史 + 中间多 tab 工作区 + 顶部环境切换。**v0.8.9 起 tab 分 3 种类型**（`HttpTab.kind`）：

| Kind | 场景 | 编辑器 |
|---|---|---|
| `http` | 一次请求 → 响应 | `HttpRequestEditor` + `HttpResponseView` |
| `sse` | 长连接事件流（AI Stream 之类） | `SseRequestEditor` + `StreamMessageList` |
| `ws` | 长连接双向消息 | `WsRequestEditor` + `StreamMessageList` + 发送框 + 模板 |

`+ 新 tab` 按钮为 NDropdown，可选 HTTP / SSE / WebSocket 三种。`http_tabs` 表加了 `kind` 列区分（老库自动迁移，默认 `http`）。

**共用能力**（所有 kind 都有）：

- 多 tab（浏览器风格 + / ✕ / 中键关闭），每 tab 一份 spec；改动 300ms debounce upsert 到 SQLite，app 重启完整恢复
- 多环境变量：dev / staging / prod 自由建；URL / headers / query / body（HTTP） / 发送内容（WS）里 `{{var}}` 字面替换；active 环境优先于全局
- Auth：Bearer / Basic，发送 / 握手时 inject Authorization header（不出现在 headers 表格）
- 快捷键 `⌘Enter` / `⌘T` / `⌘W` / `⌘B` / `⌘E`

**HTTP kind 特有**：

- 请求集合 / 目录树：左侧栏可在「集合 / 历史」之间切换；集合、目录、请求节点落 `http.sqlite3`，点击请求回填当前 tab，双击 / 中键 / 右键可新 tab 打开
- OpenAPI JSON 导入：从 `openapi.json`（OAS 3.x）读取 `servers[0].url`，导入时可覆盖为真实 URL 或 `{{baseUrl}}`，按 path 首段生成目录并把 operation 保存为集合请求
- 发送历史侧栏：本地搜索、单击回填当前 tab、右键/双击/中键新 tab 打开、hover 显示响应摘要（前 4KB）
- Body：`json` / `form` / `text` / `multipart`（file 走 Tauri dialog，reqwest 组装 `Part::bytes`）
- 响应视图：Pretty / Raw / Preview（HTML `<iframe sandbox>` + 图片）+ Cookies（从 `Set-Cookie` 解析，不做 jar 持久）
- 取消进行中请求：前端 cancel token → 后端 `Mutex<HashMap<String, oneshot::Sender>>` registry
- cURL 双向：粘贴命令解析成 spec；复制为 cURL（模板保留 `{{}}` / 已展开两种）

**SSE / WS 特有**：

- 长连接 session 挂在后端 tokio task：连接 / 消息接收 / 消息发送（WS） / 取消都是 async，session_id = tab.id
- **切走 tab 不断线**：后端 task 独立于 UI 生命周期，前端 listener 在 store 里全局注册；切回 tab 通过 `list_stream_messages` 拉一次全量快照后靠 event 增量
- **消息不落库**：只驻内存 buffer，2000 条 / 5 MB 上限，溢出 pop 最旧并 emit 一条 `BufferTruncated { dropped }` meta 消息，前端顶部提示"已丢弃 N 条旧消息"
- **同步 URL 校验**：`open_stream` 在插入 session 前先校验 URL scheme（SSE 需 `http/https`，WS 需 `ws/wss`），不匹配直接同步 `Err`，不 spawn 任务
- **重连语义**：`open_stream` 时若相同 sessionId 已有连接，先 cancel 老 task 再 spawn 新 task；每个 session 一个 generation 计数器，`state.remove_if_generation` 保证老 task cleanup 不会误删新 handle
- **消息截断安全**：SSE `data` 与 WS text 上限 1 MB，按 UTF-8 char boundary 截断（中文 / emoji 不会 panic）；WS binary 只留前 4 KB base64 作为预览，不做二进制**发送**
- **不做自动重连**：Closed / Error 之后要重连由用户手动点【连接】；SSE spec 里预留 `lastEventId` 字段，手动重连时会带 `Last-Event-ID` header
- **消息视图**：`StreamMessageList` 按类型上色（Open 绿 / Closed 灰 / Error 红 / BufferTruncated 黄 / WsBinary 蓝 / SseEvent · WsText 默认），自动滚动到底部
- WS 发送模板：`WsSpec.templates: Array<{name, text}>`，可保存 / 加载 / 删除；`{{var}}` 在发送前展开

**前端** `src/components/http/`（含 HttpTool + Sidebar + TabBar + OpenApiImportModal + [Sse|Ws]Tool + [Sse|Ws]RequestEditor + StreamMessageList + EnvModal + curl.ts / variables.ts / openapiImport.ts / httpApi.ts / streamApi.ts / streamMessageTone.ts / types.ts）+ `src/composables/useHttpStore.ts`（http + stream + collections 都走这个单例）

**后端** `src-tauri/src/http/`（mod + models + send（含 multipart）+ cancel + storage（tabs/history/envs/env_vars + collections/folders/requests）+ commands）+ `src-tauri/src/http/stream/`（session + parser + buffer + sse + ws + commands）

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

## 不在范围内

- 联网同步 / 多设备 / 账号体系
- 浅色/深色之外的多主题 / 国际化
- 移动端 / Web 端
- HTTP 工具的代码生成、脚本、cookie jar
- OpenAPI YAML / Swagger 2.0 完整兼容、复杂 schema 示例生成
- SSE / WS 消息落库回放、自动重连策略、WS 二进制**发送**、WS 自定义 subprotocol 协商细节（`subprotocols` 只做透传）

## 平台支持

通过 GitHub Actions 矩阵构建（`.github/workflows/build.yml`）：

| 平台 | Runner | Target | 用户下载 | Updater 归档 |
|---|---|---|---|---|
| macOS Apple Silicon | `macos-14` | `aarch64-apple-darwin` | `_arm64.dmg` | `_arm64.app.tar.gz` |
| macOS Intel | `macos-14`（cross-compile） | `x86_64-apple-darwin` | `_amd64.dmg` | `_amd64.app.tar.gz` |
| Linux x64 | `ubuntu-24.04` | `x86_64-unknown-linux-gnu` | `_amd64.deb` | `_amd64.tar.gz` |
| Linux ARM64 | `ubuntu-24.04-arm` | `aarch64-unknown-linux-gnu` | `_arm64.deb` | `_arm64.tar.gz` |
| Windows x64 | `windows-latest` | `x86_64-pc-windows-msvc` | `_amd64.exe`（NSIS） | `_amd64.exe.zip` |

> macOS Intel runner（`macos-13`）在 GitHub 公共池容量极差（曾经排队数小时不动），所以 `macos-x64` 改在 `macos-14` 上 cross-compile。不依赖紧缺 runner，5-10 分钟全 matrix 跑完。

打 `v*` tag 自动建 GitHub Release（直接 publish，不再走草稿）；`workflow_dispatch` 手动跑则只上传 14 天有效的 artifact。macOS app bundle 仅做 ad-hoc 签名来封存资源，未做 Developer ID 公证；Windows 未做 Authenticode。用户首次打开仍可能触发 Gatekeeper / SmartScreen。**Updater 归档由自研 Ed25519 密钥统一签名 SHA256SUMS**（由 `ATTOOL_UPDATE_SIGNING_PRIVATE_KEY` 注入），跟 Gatekeeper / SmartScreen 无关。

## 运行时依赖

打包后用户机器上需要：

- `aria2c`（下载工具用）—— `brew install aria2` / `apt install aria2`
- `python3` + `psd-tools`（PSD 导入用）—— `python3 -m pip install --user psd-tools`
- `pkexec`（Linux in-app 升级提权，一般随桌面环境自带）—— `apt install policykit-1`
- 网络访问 GitHub API（`api.github.com/repos/attson/attool/releases/latest`）+ Releases 静态资源 —— 仅在自动 / 手动检查更新时使用，可在 Settings 中关闭"启动时自动检查更新"

未来可考虑把 aria2c 静态编译进 sidecar、或把 PSD 解析改成纯 Rust（`psd` crate）。
