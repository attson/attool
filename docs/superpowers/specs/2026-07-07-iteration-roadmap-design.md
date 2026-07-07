# 迭代路线：图片工具 + 视频链接抽取扩展

## 定位

attool 是 me-first 的个人桌面工具箱，不做品牌 / 落地页 / onboarding，不追下载量。所有取舍以"你自己是否每天用到"为准。本 spec 规划下一轮迭代方向，产出两个新工具（串行做），不深化现有工具，不做工程基建。

## 当前态

已上线 5 个工具（`ready`）：aria2 下载 / 主图模板 / 剪贴板 / JSON / 抖音链接。sidebar 里还有 3 个占位（`soon`）：文本 / 网络 / 编码。本轮不动占位，也不动已有 5 个工具的深度打磨。

## 本轮迭代主轴

**扩充新工具**（不是打磨、不是基建、不是上层体验）。两个大项，串行：

1. **M1 · 图片工具**（先）—— 一个 tool，内部 tab 集成 5 项能力：压缩/转码、格式转换、EXIF 查看/清除、截图标注、OCR
2. **M2 · 视频链接抽取扩展**（后）—— 把现有 `douyin` tool 改造成通用 `video-link`，覆盖小红书 / B 站 / YouTube

## 明确不在本轮范围

- 3 个占位工具（文本 / 网络 / 编码）继续保持 `soon`
- ⌘K 命令面板、全局快捷键、工具间接力 —— overview 已声明"不在范围内"
- 现有 5 个工具的深度打磨
- AI / 翻译 / 总结类能力
- DevToys 类小工具盒（JWT / cURL 反向 / regex / 颜色 / QR / 时间戳）
- 工程基建（App.vue 路由壳重构、工具 SDK 抽象、日志上报）

## M1 · 图片工具

### 形态

- sidebar 新增 `image` 项（`ToolIconId` 用 `image` 或 `picture`）
- 单一 tool，左侧 tab 栏切能力（复用 `TemplateTool` 的多子组件布局手法）
- 5 个 tab：`compress` / `convert` / `exif` / `annotate` / `ocr`

### 通用底盘（5 tab 共享）

- 顶部：拖拽区 + "从剪贴板粘贴"按钮 + "选择文件"按钮（复用 aria2 / template 的输入范式）
- 中部：预览 + 参数区
- 底部：输出目录 + "开始"按钮 + "打开输出目录"

抽出共享组件放到 `src/components/image/shared/`（`ImageInput.vue`、`ImageOutput.vue`）。

### 5 个 tab 详情

| tab | 功能 | 主要参数 |
|---|---|---|
| compress | 减小图片体积，保持格式 | 目标质量 / 目标大小 / 是否保持元数据 |
| convert | 格式互转（PNG / JPEG / WebP / HEIC / GIF） | 目标格式 / 是否保持元数据 |
| exif | 查看和清除 EXIF | 显示所有字段表，一键清位置 / 机型 / 时间 / 全清 |
| annotate | 截图标注 | 工具栏（箭头 / 矩形 / 马赛克 / 文字），复制/导出 |
| ocr | 图片抽文字 | 显示识别文本，一键复制，段落合并选项 |

### 技术选型

**图像处理引擎**：**混合**。压缩 / PNG↔WebP 走 Rust `image` crate；HEIC / 高级压缩走 shell out（macOS 用系统内置 `sips`，Linux 用 `heif-convert` 或 `libheif` CLI，Windows 兼容性另议）。避免打包体积爆炸，避免引入重量级依赖。

**OCR**：**平台自适应**。
- macOS：Rust 调 Apple Vision framework（objc/swift bridge，中文效果极佳，无用户侧依赖）
- Linux / Windows：shell out 到 `tesseract` + 中文语言包（沿用 `aria2c` / `psd-tools` 的"提示用户自装"模式）
- 两套后端对齐同一 Rust 接口 `run_ocr(image_path: &Path) -> Result<String>`

**标注 canvas**：**手写 canvas**。工具限定在 箭头 / 矩形 / 马赛克 / 文字四项，手写完全够，符合 AGENTS.md "不引新依赖"底线（不引 Fabric.js / Konva.js）。

### 阶段划分

| 阶段 | 内容 | 可发版节点 |
|---|---|---|
| M1.1 | 通用底盘 + `image` tool 路由壳 | ✗ 空 UI，不发 |
| M1.2 | compress tab（含 Rust `image` crate 接线） | ✓ 0.4.0 —— 用户已能压图 |
| M1.3 | convert + exif tab（复用 M1.2 引擎） | ✓ 0.4.1 |
| M1.4 | annotate tab（手写 canvas） | ✓ 0.4.2 |
| M1.5 | ocr tab（Vision + tesseract 双后端） | ✓ 0.4.3 |

### 风险

- OCR macOS Vision framework 需要 Rust ↔ Swift/Obj-C bridge，是本轮最陌生的技术。M1.5 之前留出 buffer，若卡壳可先只做 tesseract 路径（macOS 也走 tesseract），Vision 作为后续优化。
- HEIC 处理在 Windows 上的可用命令行工具较少，可能需要额外说明"Windows 用户先转 JPG"。
- 图片处理跨平台构建增量：需要在 CI 里验证 macOS / Linux / Windows 三平台的 image crate 编译通过（应该没问题，但值得跑一次）。

### 范围外

- 批量重命名（属于文件工具类，本轮不做）
- 视频抽帧、GIF 录制、图生视频
- 云端 OCR / AI 视觉分析

## M2 · 视频链接抽取扩展

### 形态

方案 A：**改造 `douyin` tool 为通用 `video-link`**。
- sidebar id 从 `douyin` 改为 `video-link`（`useLastTool` 存储保留 `douyin` 兼容映射）
- 用户流程不变：贴文案 → 系统识别平台（通过 URL 模式）→ 解析出直链 → 下载
- 每平台一个 resolver 模块（Rust），前端只感知统一的"平台 badge + 元数据（标题 / 封面 / 时长 / 直链）"

### 支持深度

| 深度 | 抖音 | 小红书 | B 站 | YouTube |
|---|---|---|---|---|
| 短链解析 | ✓ 已有 | 新增 | 新增 | 新增（`youtu.be` → `youtube.com`） |
| 提取直链 | ✓ 已有 | 新增 | 新增 | 新增 |
| 下载视频 | ✓ 已有 | 新增 | 新增 | 新增 |
| 下载封面 | 增强 | 新增 | 新增 | 新增 |
| 下载字幕 | — | — | 可选 | ✓ 覆盖 |

### 阶段划分

| 阶段 | 内容 | 可发版节点 |
|---|---|---|
| M2.1 | 重构 `douyin` → `video-link` 通用 shell，抖音 resolver 保持不动，前端抽象 platform badge | ✓ 0.5.0 —— 抖音功能与原来一致，为后续加平台留接口 |
| M2.2 | 小红书 resolver（图文帖 + 视频帖两种输出类型） | ✓ 0.5.1 |
| M2.3 | B 站 resolver（视频 + 封面，字幕可选） | ✓ 0.5.2 |
| M2.4 | YouTube resolver（视频 + 封面 + 字幕，代理设置字段） | ✓ 0.5.3 |

### 技术开放问题（在 M2 深度 spec 时定）

- B 站 / YouTube 抓取：复用 `yt-dlp`（Python，能力最全但依赖 python，已有 psd-tools 类似模式）vs 纯 Rust 手写 resolver（可控但脆，反爬变化时要跟）
- 小红书图文帖：需要 headless browser 抓 HTML（成本高，可能引入 chromium sidecar）vs API 反解（脆但轻）
- YouTube 代理：UI 里给一个"代理"字段（http/socks5），保存在应用配置中

### 风险

- YouTube 反爬频繁，可能"这个月能用下个月失效"，接受维护成本
- 小红书对来源风控严，脆弱性高于抖音
- 若最终选 `yt-dlp` 路线，用户机器需要 `python3` + `yt-dlp`，README 依赖清单要加一行

### 范围外

- 账号登录、评论 / 弹幕抓取
- 直播抽流、直播录制
- 批量频道订阅、定时抓取

## 版本节奏

沿用现有发布流程（详见 AGENTS.md "发布流程"）：三处版本号同步（`package.json` / `src-tauri/tauri.conf.json` / `src-tauri/Cargo.toml`）bump，annotated tag 触发 CI matrix，`ship-release` skill 自动化。每个阶段（M1.2 起，M2.1 起）跑一次完整发版流程。

## 后续 spec

本文档只做 **iteration 路线级** 决策。真正开写代码前，每个 milestone 会有自己的深度 spec：

- `2026-07-XX-image-tool-design.md` —— M1 深度设计（UI 细节、Rust 模块划分、tesseract / Vision 接口、canvas 交互）
- `2026-07-XX-video-link-refactor-design.md` —— M2.1 重构方案（douyin → video-link 迁移路径）
- 每个新平台 resolver 单独一份 spec（M2.2 / M2.3 / M2.4）

每份深度 spec 通过 `superpowers:brainstorming` → `superpowers:writing-plans` 流程产出后再动代码。

## 实际交付状态（2026-07-07 更新）

本 spec 写完后当天直接进入实现，未再拆独立 spec。已交付：

- ✅ M1.1 图片 tool shell（5 tab 骨架）
- ✅ M1.2 压缩 tab（JPEG 质量、PNG/WebP re-encode）
- ✅ M1.3 格式转换 + EXIF（JPEG↔PNG↔WebP，EXIF 只支持 JPEG 清除）
- ✅ M1.4 截图标注（rectangle / arrow / text，导出 PNG / 复制 data URL）
- ✅ M1.5 OCR（**tesseract 单引擎**，macOS Vision framework Rust bridge 未实现，延后）
- ✅ 0.4.0 版本 bump，M1 全量 push
- ✅ M2.1 douyin → video-link 重构（sidebar id 迁移、`platformDetect` 抽象、legacy id 兼容、UI 显示 platform badge、非抖音链接标 "解析尚未实现"）

- ✅ M2.2 小红书 resolver：xhslink.com 302 跟随 + `window.__INITIAL_STATE__` JSON 解析（视频帖抓 mp4，图文帖抓图片列表）
- ✅ M2.3 B 站 resolver：b23.tv 跟随 + `/x/web-interface/view` 拉元数据 + `/x/player/playurl` 无登录 720p DASH（video/audio 分开需 ffmpeg 合流）
- ✅ M2.4 YouTube resolver：shell out `yt-dlp -j` 拉 mp4 直链 + 字幕 + 代理字段（用户需自装 yt-dlp）
- ✅ 0.5.0 版本 bump

**运行时须知**：M2 三个新平台 resolver 都基于对平台当前公开接口的公开知识实现，抓取代码本质是逆向工程，若平台未来改版可能失效。首次实测时若命中"数据结构不匹配 / API 返回 code!=0"错误，通常意味着需要调整 JSON path 或 API 参数。yt-dlp 走的路径最稳，因为社区维护。小红书对 UA/登录墙敏感，B 站高清需 SESSDATA cookie（本轮未做，因为登录态存储/UI 是独立工作量）。
