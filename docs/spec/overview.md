# 概述

## 是什么

attool 是一个个人使用的桌面工具集合，运行在 Tauri 2 上。前端 Vue 3 + Naive UI，后端 Rust。所有数据本地存储（rusqlite），不联网（除工具自身职责，如下载）。

## 当前内置的工具

### 1. Aria2 多线程下载（`id: aria2`）

- shell out 到本机 `aria2c`
- 支持单服务器多连接、文件分片、批量 URL 提交、实时进度回传
- 任务状态：`queued | running | completed | failed | cancelled`
- 进度通过 Tauri event `download-progress` 流式推送给前端
- 前端组件：`src/components/ui/TaskRow.vue`
- Tauri 命令：`get_default_download_dir` / `list_download_tasks` / `start_download` / `cancel_download` / `open_download_folder`

### 2. 电商主图模板（`id: template`）

- PSD 导入（依赖本机 `python3 + psd-tools`，桥接脚本在 `src-tauri/python/psd_template_bridge.py`）
- 图层支持：图片 / 文本 / 形状 / 组；可拖拽、改尺寸、重新排序
- 字段占位：图层名 `{{field_name}}` 标记后，可在批量任务中按行替换
- 批量任务：可对单个图层注入多组变体（图片或文本），按 1:1 笛卡尔展开生成所有结果
- 模板存储：保存在本地 SQLite；图片资源单独入库（`template_assets` 表）
- 前端入口：`src/components/ecommerce/TemplateTool.vue`，下挂 5 个子组件
- 后端模块：`src-tauri/src/ecommerce/`

## 不在范围内

- 联网同步 / 多设备 / 账号体系
- 浅色/深色之外的多主题 / 国际化
- 命令面板（`⌘K` 占位 alert）
- 移动端 / Web 端

## 平台支持

通过 GitHub Actions 矩阵构建（`.github/workflows/build.yml`）：

| 平台 | Runner | Target |
|---|---|---|
| macOS Apple Silicon | macos-14 | aarch64-apple-darwin |
| macOS Intel | macos-13 | x86_64-apple-darwin |
| Linux x64 | ubuntu-24.04 | x86_64-unknown-linux-gnu |
| Linux ARM64 | ubuntu-24.04-arm | aarch64-unknown-linux-gnu |
| Windows x64 | windows-latest | x86_64-pc-windows-msvc |

打 `v*` tag 自动建 GitHub Release 草稿；`workflow_dispatch` 手动跑则只上传 14 天有效的 artifact。无代码签名（用户首次打开会触发 Gatekeeper / SmartScreen）。

## 运行时依赖

打包后用户机器上需要：

- `aria2c`（下载工具用）—— `brew install aria2` / `apt install aria2`
- `python3` + `psd-tools`（PSD 导入用）—— `python3 -m pip install --user psd-tools`
- 网络访问 GitHub Releases endpoint（`releases/latest/download/latest.json`）—— 仅在自动 / 手动检查更新时使用，可在 Settings 中关闭"启动时自动检查更新"

未来可考虑把 aria2c 静态编译进 sidecar、或把 PSD 解析改成纯 Rust（`psd` crate）。
