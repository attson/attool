# 抖音链接提取工具 · 设计文档

## 目标

新增一个内置工具「抖音链接提取」，把从抖音 App 直接分享出来的一整段带表情、中文、口令的文案，清洗成一个或多个可点击的 `v.douyin.com` 短链。**只做文本清洗，不做无水印直链解析、不做网络请求。**

## 范围

- 仅识别 `v.douyin.com` 短链（不识别 `www.douyin.com` / `www.iesdouyin.com` 等长链）
- 一次输入可能含多条短链，全部提取、去重、按出现顺序展示
- 每条链接提供「复制」与「浏览器打开」两个操作
- 顶部提供「全部复制」（以 `\n` 拼接）
- 完全离线，不落库，不写"提取历史"

## 非目标

- 无水印视频直链解析
- 抖音网页版 URL、火山 / 抖音极速版、快手 / 西瓜 / B 站等其它站点
- 剪贴板监听（项目已有独立的「剪贴板工具」承担该职责）
- 从剪贴板一键读入的按钮（YAGNI，用户可直接 Cmd+V）

## 架构

按 `AGENTS.md` 定义的「加新工具的最小路径」实施，纯前端，**不改 Rust 侧**。

| 位置 | 变更 | 说明 |
|---|---|---|
| `src/types/tool.ts` | `ToolIconId` 联合类型新增 `'video'` | 侧栏图标枚举 |
| `src/components/shell/ToolIcon.vue` | 新增 `<template v-else-if="name === 'video'">` 内联 SVG（三角播放符号） | 遵守"不引 lucide/vicons"约定 |
| `src/App.vue` | `tools[]` 在 `json` 之后、`text` 之前插入抖音条目；模板加 `<template v-else-if="selectedTool.id === 'douyin'">`；异步 import `DouyinTool` | 与 `JsonTool` 同样走 `defineAsyncComponent` |
| `src/utils/douyinLink.ts`（新） | 导出纯函数 `extractDouyinLinks(text: string): string[]` | 无副作用，可直接测 |
| `src/utils/douyinLink.test.ts`（新） | Vitest 覆盖至少 8 个用例 | TDD 先行 |
| `src/components/douyin/DouyinTool.vue`（新） | 工具主组件，`<script setup>` + Naive UI | 依赖 `douyinLink.ts` 与 `@tauri-apps/plugin-clipboard-manager` |

**新增依赖**：无。`@tauri-apps/plugin-clipboard-manager` 已在 `package.json`。

## 核心算法

```ts
// src/utils/douyinLink.ts
const DOUYIN_SHORT_URL_RE = /https?:\/\/v\.douyin\.com\/[A-Za-z0-9_-]+\/?/gi;

export function extractDouyinLinks(text: string): string[] {
  if (!text) return [];
  const matches = text.match(DOUYIN_SHORT_URL_RE) ?? [];
  const seen = new Set<string>();
  const out: string[] = [];
  for (const raw of matches) {
    const normalized = normalize(raw);
    if (!seen.has(normalized)) {
      seen.add(normalized);
      out.push(normalized);
    }
  }
  return out;
}

function normalize(url: string): string {
  const withHttps = url.replace(/^http:\/\//i, 'https://');
  const lowercaseHost = withHttps.replace(/^https:\/\/v\.douyin\.com/i, 'https://v.douyin.com');
  // 尾斜杠归一：无论 0 个或多个尾斜杠，一律以单个 / 结尾
  return lowercaseHost.replace(/\/*$/, '/');
}
```

**要点**：

- 全局（`g`）+ 大小写不敏感（`i`）匹配
- 尾字符集 `[A-Za-z0-9_-]+`（`_-` 是防御性冗余）
- 规范化把 `http` 升 `https`、域名部分小写、尾斜杠归一为一个 `/`；**路径部分保留原大小写**（抖音短链 id 大小写敏感）
- 用 `Set<string>` + 数组的组合去重并保留首次出现顺序

## UI 结构

上下两个 `Panel` 单列布局，与 Aria2 / JSON 工具视觉语言一致：

```
┌ 抖音链接提取 ─────────────────────────────┐
│ 从抖音 App 分享文案中提取 v.douyin.com 短链│
├───────────────────────────────────────────┤
│ Panel: 分享文案                            │
│   [n-input textarea, autosize 8~14 行]     │
│       [清空]         [提取链接]            │
├───────────────────────────────────────────┤
│ Panel: 提取结果   共 N 条 · [全部复制]     │
│   1. https://v.douyin.com/xxxx/  [复制][打开] │
│   2. https://v.douyin.com/yyyy/  [复制][打开] │
│   ...                                      │
│   (未粘贴：引导语；已粘贴无匹配：空态提示) │
└───────────────────────────────────────────┘
```

**交互细节**：

- 输入：`n-input type="textarea"`，`:autosize="{ minRows: 8, maxRows: 14 }"`
- **触发方式**：**输入即时防抖 300ms 自动提取** + 「提取链接」按钮兜底同步触发
- 「清空」按钮：把 textarea 内容和结果一起清掉
- 每条结果：`序号 · 链接文本 · [复制] [打开]`（两个 secondary 小按钮）
- 「复制」按钮反馈：走 `writeText(url)`，按钮临时文案变「已复制」，1500ms 恢复
- 「打开」按钮：调用 `window.open(url, '_blank')`。Tauri 2 webview 默认把外部 URL 交给系统浏览器打开
- 「全部复制」：`results.join('\n')` 一次性写入剪贴板，反馈与单条复制一致
- **状态区分**：
  - textarea 为空：结果面板显示引导语「粘贴分享文案后自动提取」
  - textarea 非空但无匹配：显示空态「未检测到 v.douyin.com 短链」
  - 有结果：正常列出

## 错误处理

| 场景 | 行为 |
|---|---|
| 复制失败（`writeText` 抛异常） | 按钮短暂变「复制失败」，1500ms 恢复；不弹全局提示 |
| 打开失败（`window.open` 返回 `null`） | 无声降级为写剪贴板，按钮变「已复制链接」；不弹全局提示 |
| 输入超大 | 正则是 O(n)，不做上限；300ms 防抖足够 |
| 快速连续粘贴 | 防抖自然合并 |

## 测试策略

### 自动化（Vitest，纯逻辑）

`src/utils/douyinLink.test.ts` 至少覆盖：

1. 空串 → `[]`
2. 纯空白 → `[]`
3. 无匹配文本 → `[]`
4. 经典分享文案（含 emoji + 中文 + 口令 + 单条短链）→ 1 条链接
5. 一段含两条不同短链的文案 → 2 条链接，顺序按出现顺序
6. 同一短链出现两次（一次带尾斜杠，一次不带）→ 去重后 1 条
7. `HTTPS://V.DOUYIN.COM/AbC123` 大写形式 → 归一为 `https://v.douyin.com/AbC123/`（域名小写，路径大小写保留）
8. `http://` 前缀 → 升为 `https://`
9. 短链紧贴中文（无空格分隔）→ 能提取

跑法：`npm test`

### 手动目视验收

1. 侧栏出现「抖音链接提取」，图标为播放三角
2. 粘贴一条真实抖音分享文案（含 emoji + 中文 + 短链）→ 300ms 内自动出结果
3. 单条「复制」→ 剪贴板正确；按钮反馈「已复制」1.5s
4. 单条「打开」→ 系统浏览器打开对应 v.douyin.com 短链
5. 「全部复制」→ 剪贴板为多条链接以 `\n` 拼接
6. 「清空」→ 结果消失回引导语
7. dark / light 主题外观正常
8. 输入非抖音链接文本 → 空态提示「未检测到 v.douyin.com 短链」

## 依赖变更

无。

## 后续可能

- 若确认此类工具会持续增加同类需求，未来可扩为「短链清洗器」（B 站 / 小红书 / 快手），把 `extractDouyinLinks` 泛化为按 domain 白名单驱动的 `extractShortLinks(text, hosts)`。当前保持最小实现。
