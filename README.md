# AT Tool

一个基于 Tauri 的个人桌面工具集合。当前首个工具是 aria2 多线程下载工作台，后续可以继续在左侧导航中扩展更多个人常用工具。

## 功能

- 使用本机 `aria2c` 启动多连接 / 分片下载
- 支持保存目录、输出文件名、连接数、分片数、最小分片大小配置
- 通过 Tauri 事件实时回传下载进度、速度、ETA 和任务状态
- 支持取消正在运行的下载任务

## 本机依赖

Tauri 需要 Rust 工具链，下载功能需要本机安装 aria2。

```bash
# macOS 示例
brew install rustup aria2
rustup-init
```

也可以参考 Tauri 官方文档安装对应系统依赖。

## 开发

```bash
npm install
npm run tauri:dev
```

仅调试前端页面：

```bash
npm run dev
```

## 构建

```bash
npm run tauri:build
```
