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

### 电商 PSD 模板导入

PSD 导入首版通过本机 Python 解析 PSD 图层结构，需要安装 psd-tools：

```bash
python3 -m pip install --user psd-tools
```

PSD 图层名可用 `{{field_name}}` 标记批量替换字段，例如 `{{product_image}} 商品图`、`{{title}} 大标题`、`{{bottom_title}} 底部文案`。

## 电商主图模板工具

首版支持从分层 PSD 导入主图模板草稿，编辑图层字段，并用表格或图片文件夹批量导出 PNG。

推荐 PSD 图层命名：

- `{{product_image}} 商品图`
- `{{logo}} LOGO`
- `{{title}} 大标题`
- `{{subtitle}} 副标题`
- `{{selling_point_1}} 卖点 1`
- `{{bottom_title}} 底部文案`

批量表格要求：

- 第一行是字段名。
- 图片字段填写本地图片路径。
- 每一行导出一张 PNG。
- 缺失字段使用模板默认值。
