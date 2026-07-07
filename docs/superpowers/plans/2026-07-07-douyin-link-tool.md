# 抖音链接提取工具 · Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 AT Tool 桌面工具箱内新增「抖音链接提取」工具，从抖音 App 分享文案中提取所有 `v.douyin.com` 短链、去重、并提供复制/浏览器打开操作。

**Architecture:** 纯前端实现，无 Rust 侧改动，无网络请求。核心是一个可测试的 `extractDouyinLinks(text)` 纯函数（正则匹配 + 规范化 + 去重），配上一个 Vue SFC 组件；按项目既定的"加新工具最小路径"（`ToolIconId` → `ToolIcon.vue` → `tools[]` → `App.vue` 模板分支）接入侧栏。

**Tech Stack:** Vue 3 `<script setup>` + Naive UI 2.44 + Vitest 4 + `@tauri-apps/plugin-clipboard-manager`（已装）。

## Global Constraints

- **不引入新依赖**（`AGENTS.md`）
- **TDD 走逻辑**：纯函数模块先测再实现；SFC 与样式改动手动目视验收（`AGENTS.md`）
- **不写 emoji**（代码、commit message、文档都不写）
- **不写 jsdom / DOM 相关测试**：composable / util 保持纯函数可测
- **commit 规范**：conventional 风格；每 task 完成立即 commit
- **图标**：使用内联 SVG（不引 lucide / vicons）
- **样式**：颜色/圆角/字号走 `var(--*)` token；圆角 ≤ 12px；不写散装 hex 色

---

### Task 1: 核心提取函数（TDD）

**Files:**
- Create: `src/utils/douyinLink.ts`
- Create: `src/utils/douyinLink.test.ts`

**Interfaces:**
- Consumes: 无
- Produces: `export function extractDouyinLinks(text: string): string[]` — 从任意文本里提取所有 `v.douyin.com` 短链（大小写不敏感），规范化为 `https://v.douyin.com/<id>/` 形式并按首次出现顺序去重。

- [ ] **Step 1.1: 写失败测试**

创建 `src/utils/douyinLink.test.ts`：

```ts
import { describe, expect, it } from 'vitest';
import { extractDouyinLinks } from './douyinLink';

describe('extractDouyinLinks', () => {
  it('returns empty array for empty string', () => {
    expect(extractDouyinLinks('')).toEqual([]);
  });

  it('returns empty array for whitespace-only input', () => {
    expect(extractDouyinLinks('   \n\t  ')).toEqual([]);
  });

  it('returns empty array when no douyin link is present', () => {
    expect(extractDouyinLinks('hello world, no link here')).toEqual([]);
  });

  it('extracts a single link from a real-world share caption', () => {
    const text = '9.99 复制打开抖音，看看【标题】xxxx https://v.douyin.com/iRnKtwr8/ 复制此链接';
    expect(extractDouyinLinks(text)).toEqual(['https://v.douyin.com/iRnKtwr8/']);
  });

  it('extracts multiple distinct links in the order they appear', () => {
    const text = 'first https://v.douyin.com/aaa111/ second https://v.douyin.com/bbb222/';
    expect(extractDouyinLinks(text)).toEqual([
      'https://v.douyin.com/aaa111/',
      'https://v.douyin.com/bbb222/'
    ]);
  });

  it('deduplicates the same link when it appears twice, once with and once without trailing slash', () => {
    const text = 'a https://v.douyin.com/abc123/ b https://v.douyin.com/abc123 c';
    expect(extractDouyinLinks(text)).toEqual(['https://v.douyin.com/abc123/']);
  });

  it('lowercases the host but preserves case in the path (ids are case-sensitive)', () => {
    const text = 'HTTPS://V.DOUYIN.COM/AbC123';
    expect(extractDouyinLinks(text)).toEqual(['https://v.douyin.com/AbC123/']);
  });

  it('upgrades http to https', () => {
    const text = 'http://v.douyin.com/xyz789';
    expect(extractDouyinLinks(text)).toEqual(['https://v.douyin.com/xyz789/']);
  });

  it('extracts a link that is immediately adjacent to CJK characters', () => {
    const text = '看看【标题】https://v.douyin.com/abc/复制此链接';
    expect(extractDouyinLinks(text)).toEqual(['https://v.douyin.com/abc/']);
  });
});
```

- [ ] **Step 1.2: 跑测试确认失败**

Run: `npm test -- douyinLink`
Expected: FAIL —— `Failed to resolve import "./douyinLink"` 或 `extractDouyinLinks is not a function`

- [ ] **Step 1.3: 写最小实现**

创建 `src/utils/douyinLink.ts`：

```ts
const DOUYIN_SHORT_URL_RE = /https?:\/\/v\.douyin\.com\/[A-Za-z0-9_-]+\/?/gi;

export function extractDouyinLinks(text: string): string[] {
  if (!text) return [];
  const matches = text.match(DOUYIN_SHORT_URL_RE);
  if (!matches) return [];

  const seen = new Set<string>();
  const result: string[] = [];
  for (const raw of matches) {
    const normalized = normalize(raw);
    if (!seen.has(normalized)) {
      seen.add(normalized);
      result.push(normalized);
    }
  }
  return result;
}

function normalize(url: string): string {
  const withHttps = url.replace(/^http:\/\//i, 'https://');
  const lowercaseHost = withHttps.replace(/^https:\/\/v\.douyin\.com/i, 'https://v.douyin.com');
  return lowercaseHost.replace(/\/*$/, '/');
}
```

- [ ] **Step 1.4: 跑测试确认全部通过**

Run: `npm test -- douyinLink`
Expected: PASS（9 个用例全绿）

- [ ] **Step 1.5: Commit**

```bash
git add src/utils/douyinLink.ts src/utils/douyinLink.test.ts
git commit -m "feat(douyin): extract v.douyin.com short links from arbitrary text"
```

---

### Task 2: Vue 工具组件 `DouyinTool.vue`

**Files:**
- Create: `src/components/douyin/DouyinTool.vue`

**Interfaces:**
- Consumes: `extractDouyinLinks(text: string): string[]` from Task 1
- Produces: `<DouyinTool />` — 自成一体的 SFC，无 props、无事件；由 `App.vue` 直接渲染

- [ ] **Step 2.1: 建目录并写组件**

创建 `src/components/douyin/DouyinTool.vue`：

```vue
<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { NButton, NInput } from 'naive-ui';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import Panel from '../ui/Panel.vue';
import { extractDouyinLinks } from '../../utils/douyinLink';

const raw = ref('');
const results = ref<string[]>([]);
const copyState = ref<Record<string, 'idle' | 'ok' | 'fail'>>({});
const allCopyState = ref<'idle' | 'ok' | 'fail'>('idle');

let debounceTimer: ReturnType<typeof setTimeout> | null = null;

watch(raw, (value) => {
  if (debounceTimer) clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => {
    results.value = extractDouyinLinks(value);
  }, 300);
});

function extractNow() {
  if (debounceTimer) clearTimeout(debounceTimer);
  results.value = extractDouyinLinks(raw.value);
}

function clearAll() {
  raw.value = '';
  results.value = [];
  copyState.value = {};
  allCopyState.value = 'idle';
}

async function copyOne(link: string) {
  try {
    await writeText(link);
    copyState.value = { ...copyState.value, [link]: 'ok' };
  } catch {
    copyState.value = { ...copyState.value, [link]: 'fail' };
  }
  setTimeout(() => {
    copyState.value = { ...copyState.value, [link]: 'idle' };
  }, 1500);
}

async function copyAll() {
  try {
    await writeText(results.value.join('\n'));
    allCopyState.value = 'ok';
  } catch {
    allCopyState.value = 'fail';
  }
  setTimeout(() => { allCopyState.value = 'idle'; }, 1500);
}

async function openLink(link: string) {
  const handle = window.open(link, '_blank');
  if (handle) return;
  // fallback: 打开失败则复制到剪贴板
  try {
    await writeText(link);
    copyState.value = { ...copyState.value, [link]: 'ok' };
    setTimeout(() => {
      copyState.value = { ...copyState.value, [link]: 'idle' };
    }, 1500);
  } catch {
    /* noop */
  }
}

function copyLabel(link: string): string {
  const state = copyState.value[link] ?? 'idle';
  if (state === 'ok') return '已复制';
  if (state === 'fail') return '复制失败';
  return '复制';
}

const allCopyLabel = computed(() => {
  if (allCopyState.value === 'ok') return '已全部复制';
  if (allCopyState.value === 'fail') return '复制失败';
  return '全部复制';
});

const hasInput = computed(() => raw.value.trim().length > 0);
</script>

<template>
  <div class="page">
    <header class="page-header">
      <h2>抖音链接提取</h2>
      <p>从抖音 App 分享文案中提取所有 v.douyin.com 短链。</p>
    </header>

    <Panel title="分享文案">
      <div class="form">
        <n-input
          v-model:value="raw"
          type="textarea"
          placeholder="粘贴抖音 App 分享出来的整段文案，例如：9.99 复制打开抖音，看看【标题】... https://v.douyin.com/xxxxx/ 复制此链接..."
          :autosize="{ minRows: 8, maxRows: 14 }"
        />
        <div class="actions">
          <n-button secondary @click="clearAll">清空</n-button>
          <n-button type="primary" @click="extractNow">提取链接</n-button>
        </div>
      </div>
    </Panel>

    <Panel :title="`提取结果${results.length ? ' · 共 ' + results.length + ' 条' : ''}`">
      <template v-if="results.length" #right>
        <n-button size="small" secondary @click="copyAll">{{ allCopyLabel }}</n-button>
      </template>
      <div v-if="results.length" class="list">
        <div v-for="(link, index) in results" :key="link" class="row">
          <span class="idx">{{ index + 1 }}</span>
          <span class="link">{{ link }}</span>
          <div class="row-actions">
            <n-button size="small" secondary @click="copyOne(link)">{{ copyLabel(link) }}</n-button>
            <n-button size="small" secondary @click="openLink(link)">打开</n-button>
          </div>
        </div>
      </div>
      <div v-else-if="hasInput" class="empty">未检测到 v.douyin.com 短链</div>
      <div v-else class="empty">粘贴分享文案后自动提取</div>
    </Panel>
  </div>
</template>

<style scoped>
.page { display: grid; gap: 16px; }
.page-header { display: grid; gap: 4px; }
.page-header h2 {
  margin: 0;
  font-size: var(--fs-xl);
  font-weight: 600;
  letter-spacing: -0.012em;
}
.page-header p {
  margin: 0;
  color: var(--text-muted);
  font-size: var(--fs-xs);
}

.form { display: grid; gap: 12px; }
.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

.list { display: grid; gap: 6px; }
.row {
  display: grid;
  grid-template-columns: 28px 1fr auto;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  background: var(--bg-base);
}
.idx {
  color: var(--text-muted);
  font-size: var(--fs-xs);
  text-align: right;
  font-variant-numeric: tabular-nums;
}
.link {
  color: var(--text);
  font-size: var(--fs-sm);
  font-family: var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace);
  word-break: break-all;
}
.row-actions { display: flex; gap: 6px; }

.empty {
  padding: 40px 20px;
  text-align: center;
  color: var(--text-muted);
  border: 1px dashed var(--line-strong);
  border-radius: var(--radius-md);
  font-size: var(--fs-sm);
}
</style>
```

- [ ] **Step 2.2: 跑构建确认编译过**

Run: `npm run build`
Expected: PASS（tsc 无报错，vite build 成功）

- [ ] **Step 2.3: Commit**

```bash
git add src/components/douyin/DouyinTool.vue
git commit -m "feat(douyin): DouyinTool SFC with debounce + copy + open"
```

---

### Task 3: 侧栏图标 + 工具注册 + App 挂载

**Files:**
- Modify: `src/types/tool.ts`
- Modify: `src/components/shell/ToolIcon.vue`
- Modify: `src/App.vue`

**Interfaces:**
- Consumes: `<DouyinTool />` from Task 2
- Produces: 侧栏可见的 `douyin` 工具项，点击后渲染 `<DouyinTool />`

- [ ] **Step 3.1: 扩展 `ToolIconId` 联合类型**

修改 `src/types/tool.ts`，在联合类型末尾加上 `'video'`：

```ts
export type ToolStatus = 'ready' | 'soon';

export type ToolIconId =
  | 'download'
  | 'layout'
  | 'clipboard'
  | 'type'
  | 'wifi'
  | 'hash'
  | 'code'
  | 'video';

export interface Tool {
  id: string;
  name: string;
  description: string;
  status: ToolStatus;
  icon: ToolIconId;
}
```

- [ ] **Step 3.2: 添加 video 图标 SVG**

修改 `src/components/shell/ToolIcon.vue`，在 `code` 分支的 `</template>` 之后（`</svg>` 之前）新增：

```vue
    <!-- video -->
    <template v-else-if="name === 'video'">
      <polygon points="6 4 20 12 6 20 6 4" />
    </template>
```

（三角形播放图标：`M6,4 → 20,12 → 6,20 → close`，撑满 24×24 viewBox 且与其它 stroke 图标视觉重量一致。）

- [ ] **Step 3.3: 在 `App.vue` 注册工具项 + 挂载组件**

修改 `src/App.vue`：

第一处 —— 在 `JsonTool` 的 `defineAsyncComponent` 之后新增一行：

```ts
const JsonTool = defineAsyncComponent(() => import('./components/json/JsonTool.vue'));
const DouyinTool = defineAsyncComponent(() => import('./components/douyin/DouyinTool.vue'));
```

第二处 —— 在 `tools[]` 数组中，把 `json` 之后、`text` 之前插入一行：

```ts
const tools: Tool[] = [
  { id: 'aria2',     name: 'Aria2 下载',     description: 'HTTP / HTTPS / FTP / BT 多连接下载', status: 'ready', icon: 'download' },
  { id: 'template',  name: '主图模板',       description: 'PSD 导入、字段替换、批量生成主图',   status: 'ready', icon: 'layout' },
  { id: 'clipboard', name: '剪贴板工具',     description: 'Paste 风格剪贴板历史与快捷恢复',     status: 'ready', icon: 'clipboard' },
  { id: 'json',      name: 'JSON 工具',       description: '格式化 / 查询 / 对比 / 转换',          status: 'ready', icon: 'code' },
  { id: 'douyin',    name: '抖音链接提取',   description: '从分享文案中提取 v.douyin.com 短链',   status: 'ready', icon: 'video' },
  { id: 'text',      name: '文本工具',       description: '去重、排序、分割、大小写转换',       status: 'soon',  icon: 'type' },
  { id: 'network',   name: '网络工具',       description: 'Ping、端口检查、URL 分析',           status: 'soon',  icon: 'wifi' },
  { id: 'codec',     name: '编码转换',       description: 'Base64、URL Encode、Hash 摘要',      status: 'soon',  icon: 'hash' }
];
```

第三处 —— 在 `<template v-else-if="selectedTool.id === 'json'">` 之后插入渲染分支：

```vue
        <template v-else-if="selectedTool.id === 'json'">
          <JsonTool />
        </template>

        <template v-else-if="selectedTool.id === 'douyin'">
          <DouyinTool />
        </template>
```

- [ ] **Step 3.4: 跑构建 + 测试**

Run: `npm run build && npm test`
Expected: 均 PASS

- [ ] **Step 3.5: Commit**

```bash
git add src/types/tool.ts src/components/shell/ToolIcon.vue src/App.vue
git commit -m "feat(douyin): register tool in sidebar and route to DouyinTool"
```

---

### Task 4: 手动目视验收

**Files:**
- None

**Interfaces:**
- Consumes: 前 3 个 task 完成后的完整功能
- Produces: 一份验收记录（口头 / 消息汇报）

- [ ] **Step 4.1: 启动 dev server 与桌面窗口**

Run: `npm run tauri:dev`
Expected: 桌面窗口打开，侧栏出现「抖音链接提取」项（三角播放图标）

- [ ] **Step 4.2: 手动核对验收清单**

依次做下面 8 项操作，每一步观察是否符合预期：

1. 点击侧栏「抖音链接提取」 → 进入工具页
2. 粘贴测试文案：`9.99 复制打开抖音，看看【测试】 https://v.douyin.com/iRnKtwr8/ 复制此链接，https://v.douyin.com/abc/ 一起打开` → 300ms 内自动出现 2 条结果
3. 点击某条结果的「复制」 → 按钮变「已复制」1.5s → 到系统别处 Cmd+V 得到该链接
4. 点击某条结果的「打开」 → 系统默认浏览器打开该 v.douyin.com 短链
5. 点击「全部复制」 → 按钮变「已全部复制」1.5s → 到系统别处 Cmd+V 得到两条链接以 `\n` 拼接
6. 点击「清空」 → textarea 与结果同时清空，结果面板回到「粘贴分享文案后自动提取」引导语
7. 粘贴任意无 v.douyin.com 短链的文本 → 结果面板显示「未检测到 v.douyin.com 短链」
8. 顶栏切换 dark / light 主题 → 外观正常，色彩、边框、按钮均遵循 token

- [ ] **Step 4.3: 提交验收记录**

若上面 8 项全部通过：无需 commit（无代码改动），直接汇报「验收通过」。
若发现问题：回到对应 Task 修复，走"改代码 → 跑测试 → commit"闭环。

---

## Self-Review

**1. Spec coverage:**
- 侧栏图标 + 工具注册 → Task 3
- `extractDouyinLinks` 核心算法（正则 + 归一化 + 去重） → Task 1
- 单 textarea + 防抖自动提取 + 按钮兜底 → Task 2 Step 2.1（`watch(raw)` + `extractNow`）
- 结果列表 + 每条复制 + 每条打开 → Task 2 Step 2.1（`copyOne` / `openLink`）
- 全部复制 → Task 2 Step 2.1（`copyAll`）
- 空态区分（未粘贴 / 已粘贴无匹配） → Task 2 Step 2.1（`hasInput` 三态模板）
- 复制/打开错误无声降级 → Task 2 Step 2.1（catch + fallback）
- 9 项 Vitest 用例 → Task 1 Step 1.1（含中文邻接、大写、去重）
- 8 项手动验收 → Task 4

**2. Placeholder scan:** 无 TBD / TODO；每步都有完整代码；无 "similar to Task N" 引用。

**3. Type consistency:**
- Task 1 输出 `extractDouyinLinks(text: string): string[]`
- Task 2 `import { extractDouyinLinks } from '../../utils/douyinLink'` + `results.value = extractDouyinLinks(...)` —— 匹配
- Task 3 `defineAsyncComponent(() => import('./components/douyin/DouyinTool.vue'))` —— 路径与 Task 2 一致
- `ToolIconId` 新增值 `'video'` —— Task 3 Step 3.1 定义、Step 3.2 在 SVG 里消费、Step 3.3 在 `tools[]` 里消费 —— 三处名字一致
