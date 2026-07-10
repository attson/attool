# AT Tool

个人桌面工具集合，基于 Tauri 2 + Vue 3 + Naive UI。Mono Dark / Light 双主题，集成 in-app 自动更新。

## 当前内置工具

| 工具 | 说明 |
|---|---|
| **Aria2 下载** | shell out 到本机 `aria2c`，多连接 / 分片 / 批量 URL / 实时进度 |
| **主图模板** | PSD 导入（`psd-tools` 桥接）+ 字段占位 `{{key}}` + 笛卡尔展开批量导出 PNG |
| **剪贴板** | 剪贴板历史记录，图片 / 文本预览，Paste 风格快速恢复，快捷键可自定义 |
| **JSON** | 格式化、JSONPath 查询、对比、YAML/CSV/XML 转换（Monaco 编辑器） |
| **视频链接抽取** | 抖音 / B站 / 小红书 / YouTube 视频链接与文案解析 |
| **图片** | 压缩、格式转换、EXIF 读取/去除、跨平台截图、标注、OCR |
| **文本** | 整理、排序、大小写、行拆合、正则抽取、对比 |
| **网络** | URL 分解、Ping、端口检查、DNS 查询 |
| **编码** | Base64 / URL / Unicode / Hex / Hash / JWT 解码 |
| **生成器** | 密码、UUID·ULID、二维码、Lorem、假数据、骰子 |
| **时间** | Unix 时间戳、时区转换、Cron、时间差 |
| **HTTP 请求** | GET/POST/PUT/DELETE/PATCH + Headers/Query/Body（Postman Lite） |

截图支持全平台：macOS 用系统 `screencapture`，Linux / Windows 用 xcap。

## 下载安装

到 [Releases](https://github.com/attson/attool/releases) 抓对应平台的安装包：

| 文件 | 适用平台 |
|---|---|
| `AT Tool_<version>_arm64.dmg` | macOS Apple Silicon (M1/M2/M3/M4) |
| `AT Tool_<version>_amd64.dmg` | macOS Intel |
| `AT Tool_<version>_amd64.exe` | Windows 64-bit |
| `AT Tool_<version>_amd64.deb` | Linux x86_64（Debian / Ubuntu） |
| `AT Tool_<version>_arm64.deb` | Linux ARM64 |

未做代码签名，首次打开 macOS / Windows 会有 Gatekeeper / SmartScreen 警告，按系统说明放行即可。

应用启动后会自动检查更新（可在设置里关闭）；macOS / Windows 走 in-app 一键更新，Linux 仍需重新下载 `.deb` 安装。

## 运行时依赖

| 依赖 | 用途 | 安装 |
|---|---|---|
| `aria2c` | 下载工具调用 | `brew install aria2` / `apt install aria2` / `choco install aria2` |
| `python3` + `psd-tools` | PSD 模板解析 | `python3 -m pip install --user psd-tools` |

## 开发

本项目使用 [pnpm](https://pnpm.io/)（`packageManager` 字段锁定版本，建议用 `corepack enable` 自动匹配）。

```bash
pnpm install
pnpm tauri:dev      # 开发：起 vite + 拉起桌面窗口
```

仅前端（浏览器调试）：

```bash
pnpm dev            # http://127.0.0.1:1420
```

测试 + 构建：

```bash
pnpm test           # vitest run
pnpm build          # tsc + vite build
pnpm tauri:build    # 全量打包桌面应用
```

### Linux 截图依赖

浮层截图在 Linux 上通过 [xcap](https://github.com/nashaofu/xcap) 实现，编译时需要以下开发库：

```bash
sudo apt install libxcb1-dev libxrandr-dev libdbus-1-dev \
                 libpipewire-0.3-dev libgbm-dev clang
```

- **X11 会话**：截图与窗口枚举全功能可用。
- **Wayland 会话**：截图走 `xdg-desktop-portal`（每次会弹权限确认，且比 X11 慢），部分合成器/场景可能失败——失败时会提示改用 X11 会话；窗口枚举在 Wayland 下降级为不可用（区域选择仍正常）。

更多上下文：

- `AGENTS.md` —— 给 AI 看的工程地图（技术栈、约定、目录速览、加新工具最小路径、发布流程）
- `docs/spec/` —— 当前态规范（overview / ui-design-system / architecture）
- `docs/superpowers/` —— 每个任务的设计文档 + 实施计划
