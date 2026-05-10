# AT Tool

个人桌面工具集合，基于 Tauri 2 + Vue 3 + Naive UI。Mono Dark / Light 双主题，集成 in-app 自动更新。

## 当前内置工具

- **Aria2 多线程下载** —— shell out 到本机 `aria2c`，支持多连接 / 分片 / 批量 URL / 实时进度
- **电商主图模板** —— PSD 导入（Python `psd-tools` 桥接）+ 字段占位 `{{key}}` + 笛卡尔展开批量导出 PNG

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

```bash
npm install
npm run tauri:dev   # 开发：起 vite + 拉起桌面窗口
```

仅前端（浏览器调试）：

```bash
npm run dev         # http://127.0.0.1:1420
```

测试 + 构建：

```bash
npm test            # vitest run
npm run build       # tsc + vite build
npm run tauri:build # 全量打包桌面应用
```

更多上下文：

- `AGENTS.md` —— 给 AI 看的工程地图（技术栈、约定、目录速览、加新工具最小路径、发布流程）
- `docs/spec/` —— 当前态规范（overview / ui-design-system / architecture）
- `docs/superpowers/` —— 每个任务的设计文档 + 实施计划
