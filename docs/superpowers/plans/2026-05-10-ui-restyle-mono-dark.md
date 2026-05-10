# UI 重做（Mono Dark + Emerald + 可折叠 Sidebar）实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 attool 的整体 UI 从"米色羊皮纸 + 苔藓绿"翻新为 Mono Dark + Emerald 主色 + 可折叠侧栏的 Linear/Vercel 风深色工具样貌。

**Architecture:** 三层 ——
1. Token 层（CSS 变量 + Naive UI ThemeOverrides）做单一颜色/圆角/字体真实来源
2. Shell 层（AppShell + Sidebar + Topbar + Dashboard）替换原来的"工具卡首页 → 进入"模式
3. 工具页用新自写组件（Panel / TaskRow / StatPill / Kbd）+ Naive UI（套了主题）混搭

业务逻辑/Tauri 调用/数据结构一概不动，仅外观与导航结构。

**Tech Stack:** Vue 3（`<script setup>`）· Naive UI 2.44（深色主题 + 覆写）· Vitest 4 · Tauri 2 · 无新依赖

**Spec:** `docs/superpowers/specs/2026-05-10-ui-restyle-design.md`

---

## 文件结构（增/改概览）

**新建：**

```
src/
├── styles/
│   ├── tokens.css            # CSS 变量（颜色、圆角、字体、间距）
│   └── reset.css             # 全局 reset + body 背景
├── theme/
│   └── index.ts              # 导出 darkOverrides 给 NConfigProvider
├── composables/
│   ├── useSidebarState.ts    # sidebar 折叠状态 + localStorage
│   ├── useSidebarState.test.ts
│   ├── useLastTool.ts        # 记忆上次打开的工具
│   └── useLastTool.test.ts
├── components/
│   ├── shell/
│   │   ├── AppShell.vue      # sidebar + topbar + 内容槽
│   │   ├── Sidebar.vue       # 折叠侧栏
│   │   ├── Topbar.vue        # 面包屑 + pill 槽
│   │   ├── BrandMark.vue     # emerald 26px 方块
│   │   └── Dashboard.vue     # 首屏极简 dashboard
│   └── ui/
│       ├── Panel.vue         # 卡片容器（替代多数 NCard）
│       ├── TaskRow.vue       # Aria2 任务行（替代 TaskCard.vue）
│       ├── StatPill.vue      # topbar 右侧 pill
│       └── Kbd.vue           # ⌘K 等键位标签
└── types/
    └── tool.ts               # Tool 接口（从 App.vue 抽出）
```

**修改：**

- `src/main.ts` — 引入 `tokens.css` + `reset.css`，去掉 `styles.css`
- `src/App.vue` — 顶层结构改用 AppShell + Dashboard + 工具路由；保留每个工具的现有模板片段
- `src/components/ecommerce/TemplateTool.vue` 与其下子组件 — 按 spec § 6.3 重新染色
- `src/styles.css` — 末步全部删除（其内容由各组件 scoped style 接管 + tokens 兜底）
- `src/components/TaskCard.vue` — 末步删除（已被 TaskRow.vue 取代）

**配置：**

- 不动 `package.json` / `vite.config.ts` / `vitest`（已就绪）
- 不引入新依赖（不要 jsdom；composable 测试用注入 storage 模式）

---

## 实施顺序与原则

任务按"能跑、能 commit"切分：每个 task 完成后 `npm run dev` 都应能起来（视觉上可能阶段性混搭）。**禁止在中途留下未提交的改动跨任务**。

测试策略：

- **逻辑文件**（composable / theme module）走 TDD（先红后绿，Vitest）
- **纯展示组件 / 样式改动**（Vue SFC、`*.css`）不强写单测，每个任务末尾给"目视验收清单"
- 总入口验证：`npm run build`（TS + Vite 都通）+ `npm run dev` 后人工对照 spec § 9 的 11 条验收

---

## Task 1：建立 Token 与 Theme 入口

**Files:**

- Create: `src/styles/tokens.css`
- Create: `src/styles/reset.css`
- Create: `src/theme/index.ts`
- Create: `src/theme/index.test.ts`

- [ ] **Step 1.1 — 写 token 模块的失败测试**

`src/theme/index.test.ts`：

```ts
import { describe, it, expect } from 'vitest';
import { darkOverrides, accentHex } from './index';

describe('theme', () => {
  it('exports emerald accent hex', () => {
    expect(accentHex).toBe('#34D399');
  });

  it('common.primaryColor uses emerald', () => {
    expect(darkOverrides.common?.primaryColor).toBe('#34D399');
    expect(darkOverrides.common?.primaryColorHover).toBe('#4ADE80');
    expect(darkOverrides.common?.primaryColorPressed).toBe('#16A34A');
  });

  it('common.borderRadius is 6px (no chunky corners)', () => {
    expect(darkOverrides.common?.borderRadius).toBe('6px');
    expect(darkOverrides.common?.borderRadiusSmall).toBe('4px');
  });

  it('Card uses 8px radius and elevated bg', () => {
    expect(darkOverrides.Card?.borderRadius).toBe('8px');
    expect(darkOverrides.Card?.color).toBe('#131316');
    expect(darkOverrides.Card?.borderColor).toBe('#1F1F23');
  });

  it('Button radii are tight', () => {
    expect(darkOverrides.Button?.borderRadiusMedium).toBe('6px');
    expect(darkOverrides.Button?.borderRadiusSmall).toBe('4px');
  });
});
```

- [ ] **Step 1.2 — 跑测试确认失败**

```
npm test -- src/theme
```

预期：`Cannot find module './index'`。

- [ ] **Step 1.3 — 写 theme 模块**

`src/theme/index.ts`：

```ts
import type { GlobalThemeOverrides } from 'naive-ui';

export const accentHex = '#34D399';
export const accentHoverHex = '#4ADE80';
export const accentPressedHex = '#16A34A';

const fontSans =
  '-apple-system, "SF Pro Text", "PingFang SC", "Inter", "Segoe UI", sans-serif';

export const darkOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: accentHex,
    primaryColorHover: accentHoverHex,
    primaryColorPressed: accentPressedHex,
    primaryColorSuppl: accentHex,
    borderRadius: '6px',
    borderRadiusSmall: '4px',
    bodyColor: '#0A0A0B',
    cardColor: '#131316',
    modalColor: '#1D1D22',
    popoverColor: '#1D1D22',
    textColor1: '#EDEDED',
    textColor2: '#EDEDED',
    textColor3: '#8B8B8B',
    placeholderColor: '#6A6A6F',
    dividerColor: '#1F1F23',
    borderColor: '#28282D',
    inputColor: '#18181C',
    inputColorDisabled: '#131316',
    tableColor: '#131316',
    tagColor: '#18181C',
    actionColor: '#18181C',
    fontFamily: fontSans
  },
  Card: {
    borderRadius: '8px',
    color: '#131316',
    colorEmbedded: '#131316',
    borderColor: '#1F1F23'
  },
  Button: {
    borderRadiusMedium: '6px',
    borderRadiusSmall: '4px',
    heightMedium: '32px',
    heightSmall: '28px'
  },
  Input: {
    borderRadius: '6px',
    color: '#18181C',
    colorFocus: '#18181C',
    border: '1px solid #28282D',
    borderHover: '1px solid #3a3a40',
    borderFocus: `1px solid ${accentHex}`,
    boxShadowFocus: `0 0 0 3px rgba(52,211,153,0.18)`
  },
  Tag: {
    borderRadius: '999px'
  },
  Modal: {
    color: '#1D1D22'
  },
  Slider: {
    fillColor: accentHex,
    fillColorHover: accentHoverHex,
    railColor: '#28282D',
    handleColor: accentHex
  }
};
```

- [ ] **Step 1.4 — 跑测试确认通过**

```
npm test -- src/theme
```

预期：5 个测试全部 PASS。

- [ ] **Step 1.5 — 写 tokens.css**

`src/styles/tokens.css`：

```css
:root {
  /* Surface */
  --bg-base: #0a0a0b;
  --bg-elevated: #131316;
  --bg-elev-2: #18181c;
  --bg-overlay: #1d1d22;

  /* Line */
  --line: #1f1f23;
  --line-strong: #28282d;

  /* Text */
  --text: #ededed;
  --text-muted: #8b8b8b;
  --text-faint: #6a6a6f;

  /* Accent */
  --accent: #34d399;
  --accent-hover: #4ade80;
  --accent-pressed: #16a34a;
  --accent-fg: #0a0a0b;
  --accent-soft: rgba(52, 211, 153, 0.18);

  /* Semantic */
  --warning: #f5b94b;
  --error: #f87171;
  --info: #60a5fa;
  --success: var(--accent);

  /* Radius */
  --radius-sm: 4px;
  --radius: 6px;
  --radius-md: 8px;
  --radius-lg: 12px;
  --radius-pill: 999px;

  /* Font */
  --font-sans: -apple-system, "SF Pro Text", "PingFang SC", "Inter", "Segoe UI", sans-serif;
  --font-mono: "SF Mono", "JetBrains Mono", Menlo, Consolas, monospace;

  /* Font size */
  --fs-xxs: 11px;
  --fs-xs: 12px;
  --fs-sm: 12.5px;
  --fs-md: 13px;
  --fs-lg: 15px;
  --fs-xl: 17px;
  --fs-2xl: 22px;

  /* Motion */
  --motion-fast: 120ms ease;
  --motion-mid: 180ms ease;

  /* Shadow */
  --shadow-pop: 0 8px 24px rgba(0, 0, 0, 0.45);
  --shadow-canvas: 0 12px 32px rgba(0, 0, 0, 0.5);
}
```

- [ ] **Step 1.6 — 写 reset.css**

`src/styles/reset.css`：

```css
*, *::before, *::after {
  box-sizing: border-box;
}

html, body, #root {
  margin: 0;
  padding: 0;
  min-height: 100vh;
}

body {
  background: var(--bg-base);
  color: var(--text);
  font-family: var(--font-sans);
  font-size: var(--fs-md);
  line-height: 1.5;
  -webkit-font-smoothing: antialiased;
  text-rendering: optimizeLegibility;
}

button, input, select, textarea {
  font: inherit;
  color: inherit;
}

.mono,
.tnum {
  font-family: var(--font-mono);
  font-feature-settings: "tnum";
}
```

- [ ] **Step 1.7 — Commit**

```
git add src/styles/tokens.css src/styles/reset.css src/theme/index.ts src/theme/index.test.ts
git commit -m "feat(theme): mono dark + emerald token system and naive overrides"
```

---

## Task 2：把 Dark Theme 接入 ConfigProvider

> 这一步会让现有 UI 看起来"半新半旧" —— token 已生效但旧 styles.css 里的暖色硬编码依然在。这是计划内的中间状态。

**Files:**

- Modify: `src/main.ts`
- Modify: `src/App.vue`（仅改 `<script setup>` 的 themeOverrides 与 `<n-config-provider>` 用法 + 引入 darkTheme）

- [ ] **Step 2.1 — 改 main.ts 的样式入口**

把 `src/main.ts` 整体替换为：

```ts
import { createApp } from 'vue';
import App from './App.vue';
import './styles/tokens.css';
import './styles/reset.css';
import './styles.css'; // 暂留，Task 14 删

createApp(App).mount('#root');
```

- [ ] **Step 2.2 — 改 App.vue：换主题对象**

在 `src/App.vue` 的 `<script setup>` 顶部 `import` 区，把：

```ts
import {
  NAlert,
  NButton,
  ...
  NText
} from 'naive-ui';
```

改成（追加 `darkTheme`）：

```ts
import {
  NAlert,
  NButton,
  NCard,
  NConfigProvider,
  NEllipsis,
  NEmpty,
  NFlex,
  NForm,
  NFormItem,
  NGrid,
  NGridItem,
  NInput,
  NInputGroup,
  NInputNumber,
  NMessageProvider,
  NModal,
  NPageHeader,
  NSelect,
  NSpace,
  NTag,
  NText,
  darkTheme
} from 'naive-ui';
import { darkOverrides } from './theme';
```

删除原来这一段（约 41-57 行）：

```ts
const themeOverrides = {
  common: {
    primaryColor: '#56715d',
    ...
  },
  ...
};
```

- [ ] **Step 2.3 — 改 App.vue：把 ConfigProvider 套上 dark theme**

`src/App.vue` 中找到 `<n-config-provider :theme-overrides="themeOverrides">`（约 574 行），改为：

```html
<n-config-provider :theme="darkTheme" :theme-overrides="darkOverrides">
```

- [ ] **Step 2.4 — 跑 dev 验证**

```
npm run dev
```

打开本地 1420 端口（或 vite 提示的端口），确认：
- 启动不报错
- 按钮颜色已经是 emerald；卡片背景偏深
- （旧 styles.css 还在生效，所以会有米色背景透出 — 这是预期的中间态）

- [ ] **Step 2.5 — 跑构建验证**

```
npm run build
```

预期：tsc + vite 都通过。

- [ ] **Step 2.6 — Commit**

```
git add src/main.ts src/App.vue
git commit -m "feat(theme): wire naive dark theme + token overrides into root"
```

---

## Task 3：useSidebarState Composable

**Files:**

- Create: `src/composables/useSidebarState.ts`
- Create: `src/composables/useSidebarState.test.ts`

- [ ] **Step 3.1 — 写失败测试**

`src/composables/useSidebarState.test.ts`：

```ts
import { describe, it, expect } from 'vitest';
import { useSidebarState, type KVStorage } from './useSidebarState';

function fakeStorage(initial: Record<string, string> = {}) {
  const data = new Map(Object.entries(initial));
  const storage: KVStorage = {
    getItem: (k) => (data.has(k) ? data.get(k)! : null),
    setItem: (k, v) => { data.set(k, v); }
  };
  return { storage, data };
}

describe('useSidebarState', () => {
  it('defaults to expanded when storage is empty', () => {
    const { storage } = fakeStorage();
    const { collapsed } = useSidebarState(storage);
    expect(collapsed.value).toBe(false);
  });

  it('restores collapsed state from storage', () => {
    const { storage } = fakeStorage({ 'attool.sidebar.collapsed': '1' });
    const { collapsed } = useSidebarState(storage);
    expect(collapsed.value).toBe(true);
  });

  it('toggle flips and persists value', () => {
    const { storage, data } = fakeStorage();
    const { collapsed, toggle } = useSidebarState(storage);
    toggle();
    expect(collapsed.value).toBe(true);
    expect(data.get('attool.sidebar.collapsed')).toBe('1');
    toggle();
    expect(collapsed.value).toBe(false);
    expect(data.get('attool.sidebar.collapsed')).toBe('0');
  });
});
```

- [ ] **Step 3.2 — 跑测试确认失败**

```
npm test -- src/composables/useSidebarState
```

预期：`Cannot find module './useSidebarState'`。

- [ ] **Step 3.3 — 写实现**

`src/composables/useSidebarState.ts`：

```ts
import { ref } from 'vue';

export interface KVStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

const STORAGE_KEY = 'attool.sidebar.collapsed';

export function useSidebarState(storage: KVStorage = localStorage) {
  const initial = storage.getItem(STORAGE_KEY) === '1';
  const collapsed = ref(initial);

  function toggle() {
    collapsed.value = !collapsed.value;
    storage.setItem(STORAGE_KEY, collapsed.value ? '1' : '0');
  }

  return { collapsed, toggle };
}
```

- [ ] **Step 3.4 — 跑测试通过**

```
npm test -- src/composables/useSidebarState
```

预期：3 个 PASS。

- [ ] **Step 3.5 — Commit**

```
git add src/composables/useSidebarState.ts src/composables/useSidebarState.test.ts
git commit -m "feat(composable): useSidebarState with storage-injected persistence"
```

---

## Task 4：useLastTool Composable

**Files:**

- Create: `src/composables/useLastTool.ts`
- Create: `src/composables/useLastTool.test.ts`

- [ ] **Step 4.1 — 写失败测试**

`src/composables/useLastTool.test.ts`：

```ts
import { describe, it, expect } from 'vitest';
import { useLastTool } from './useLastTool';
import type { KVStorage } from './useSidebarState';

function fakeStorage(initial: Record<string, string> = {}) {
  const data = new Map(Object.entries(initial));
  const storage: KVStorage = {
    getItem: (k) => (data.has(k) ? data.get(k)! : null),
    setItem: (k, v) => { data.set(k, v); }
  };
  return { storage, data };
}

describe('useLastTool', () => {
  it('returns null when no last tool stored', () => {
    const { storage } = fakeStorage();
    const { lastToolId } = useLastTool(storage);
    expect(lastToolId.value).toBeNull();
  });

  it('returns stored last tool id', () => {
    const { storage } = fakeStorage({ 'attool.lastTool': 'aria2' });
    const { lastToolId } = useLastTool(storage);
    expect(lastToolId.value).toBe('aria2');
  });

  it('remember persists tool id and updates state', () => {
    const { storage, data } = fakeStorage();
    const { lastToolId, remember } = useLastTool(storage);
    remember('template');
    expect(lastToolId.value).toBe('template');
    expect(data.get('attool.lastTool')).toBe('template');
  });
});
```

- [ ] **Step 4.2 — 跑测试确认失败**

```
npm test -- src/composables/useLastTool
```

预期：`Cannot find module './useLastTool'`。

- [ ] **Step 4.3 — 写实现**

`src/composables/useLastTool.ts`：

```ts
import { ref } from 'vue';
import type { KVStorage } from './useSidebarState';

const STORAGE_KEY = 'attool.lastTool';

export function useLastTool(storage: KVStorage = localStorage) {
  const lastToolId = ref<string | null>(storage.getItem(STORAGE_KEY));

  function remember(id: string) {
    lastToolId.value = id;
    storage.setItem(STORAGE_KEY, id);
  }

  return { lastToolId, remember };
}
```

- [ ] **Step 4.4 — 跑测试通过**

```
npm test -- src/composables/useLastTool
```

预期：3 个 PASS。

- [ ] **Step 4.5 — Commit**

```
git add src/composables/useLastTool.ts src/composables/useLastTool.test.ts
git commit -m "feat(composable): useLastTool to remember last opened tool"
```

---

## Task 5：抽出 Tool 类型

**Files:**

- Create: `src/types/tool.ts`

- [ ] **Step 5.1 — 写类型文件**

`src/types/tool.ts`：

```ts
export type ToolStatus = 'ready' | 'soon';

export interface Tool {
  id: string;
  name: string;
  description: string;
  status: ToolStatus;
}
```

- [ ] **Step 5.2 — Commit**

```
git add src/types/tool.ts
git commit -m "refactor(types): extract Tool interface"
```

---

## Task 6：BrandMark + Kbd + StatPill（小展示组件）

**Files:**

- Create: `src/components/shell/BrandMark.vue`
- Create: `src/components/ui/Kbd.vue`
- Create: `src/components/ui/StatPill.vue`

- [ ] **Step 6.1 — 写 BrandMark.vue**

`src/components/shell/BrandMark.vue`：

```vue
<script setup lang="ts">
defineProps<{ size?: number }>();
</script>

<template>
  <span class="brand-mark" :style="size ? { width: `${size}px`, height: `${size}px` } : undefined">A</span>
</template>

<style scoped>
.brand-mark {
  display: grid;
  place-items: center;
  width: 26px;
  height: 26px;
  border-radius: 7px;
  background: var(--accent);
  color: var(--accent-fg);
  font-weight: 700;
  font-size: 12px;
  letter-spacing: 0.02em;
  flex-shrink: 0;
}
</style>
```

- [ ] **Step 6.2 — 写 Kbd.vue**

`src/components/ui/Kbd.vue`：

```vue
<template>
  <kbd class="kbd"><slot /></kbd>
</template>

<style scoped>
.kbd {
  display: inline-flex;
  align-items: center;
  border: 1px solid var(--line-strong);
  border-radius: 3px;
  padding: 1px 5px;
  font-family: var(--font-mono);
  font-size: 10px;
  color: var(--text-faint);
  background: transparent;
  line-height: 1.3;
}
</style>
```

- [ ] **Step 6.3 — 写 StatPill.vue**

`src/components/ui/StatPill.vue`：

```vue
<script setup lang="ts">
defineProps<{ tone?: 'default' | 'accent' | 'warning' | 'error' }>();
</script>

<template>
  <span class="stat-pill" :data-tone="tone ?? 'default'">
    <slot />
  </span>
</template>

<style scoped>
.stat-pill {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 9px;
  border-radius: var(--radius-pill);
  background: var(--bg-elevated);
  border: 1px solid var(--line);
  color: var(--text-muted);
  font-size: var(--fs-xs);
  font-feature-settings: "tnum";
  white-space: nowrap;
}

.stat-pill[data-tone="accent"] {
  background: var(--accent-soft);
  color: var(--accent);
  border-color: transparent;
}

.stat-pill[data-tone="warning"] {
  background: color-mix(in srgb, var(--warning) 18%, transparent);
  color: var(--warning);
  border-color: transparent;
}

.stat-pill[data-tone="error"] {
  background: color-mix(in srgb, var(--error) 18%, transparent);
  color: var(--error);
  border-color: transparent;
}
</style>
```

- [ ] **Step 6.4 — Commit**

```
git add src/components/shell/BrandMark.vue src/components/ui/Kbd.vue src/components/ui/StatPill.vue
git commit -m "feat(shell): BrandMark, Kbd, StatPill primitives"
```

---

## Task 7：Sidebar 组件

**Files:**

- Create: `src/components/shell/Sidebar.vue`

- [ ] **Step 7.1 — 写 Sidebar.vue**

`src/components/shell/Sidebar.vue`：

```vue
<script setup lang="ts">
import { computed } from 'vue';
import BrandMark from './BrandMark.vue';
import Kbd from '../ui/Kbd.vue';
import type { Tool } from '../../types/tool';

const props = defineProps<{
  tools: Tool[];
  activeId: string | null;
  collapsed: boolean;
}>();

const emit = defineEmits<{
  select: [id: string];
  toggle: [];
  brand: [];
  search: [];
}>();

const ready = computed(() => props.tools.filter((t) => t.status === 'ready'));
const soon = computed(() => props.tools.filter((t) => t.status === 'soon'));
</script>

<template>
  <aside class="sidebar" :class="{ collapsed }">
    <button class="brand-row" type="button" @click="emit('brand')">
      <BrandMark />
      <span class="brand-name">AT Tool</span>
    </button>

    <button class="search" type="button" @click="emit('search')">
      <span class="ico"></span>
      <span class="label">搜索工具</span>
      <Kbd>⌘K</Kbd>
    </button>

    <div class="group">已就绪</div>
    <button
      v-for="tool in ready"
      :key="tool.id"
      type="button"
      class="item"
      :class="{ active: tool.id === activeId }"
      :title="tool.name"
      @click="emit('select', tool.id)"
    >
      <span class="ico"></span>
      <span class="label">{{ tool.name }}</span>
    </button>

    <div class="group">规划中</div>
    <button
      v-for="tool in soon"
      :key="tool.id"
      type="button"
      class="item dim"
      disabled
      :title="tool.name"
    >
      <span class="ico"></span>
      <span class="label">{{ tool.name }}</span>
      <span class="pill">Soon</span>
    </button>

    <div class="foot">
      <span class="ver">v0.1.0</span>
      <button class="toggle" type="button" :title="collapsed ? '展开' : '折叠'" @click="emit('toggle')">
        {{ collapsed ? '›' : '‹' }}
      </button>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  width: 220px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  background: var(--bg-elevated);
  border-right: 1px solid var(--line);
  transition: width var(--motion-mid);
  overflow: hidden;
}

.sidebar.collapsed { width: 56px; }

.brand-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 14px;
  border: 0;
  background: transparent;
  border-bottom: 1px solid var(--line);
  cursor: pointer;
  text-align: left;
  color: var(--text);
}
.brand-name {
  font-size: var(--fs-md);
  font-weight: 600;
  letter-spacing: -0.012em;
  white-space: nowrap;
  overflow: hidden;
}
.sidebar.collapsed .brand-name { display: none; }
.sidebar.collapsed .brand-row { justify-content: center; padding: 14px 0; }

.search {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 10px 10px 6px;
  padding: 6px 9px;
  border: 1px solid var(--line);
  border-radius: var(--radius);
  background: var(--bg-elev-2);
  color: var(--text-muted);
  font-size: var(--fs-xs);
  cursor: pointer;
}
.search .ico {
  width: 12px; height: 12px;
  border: 1.5px solid currentColor;
  border-radius: 50%;
  position: relative;
}
.search .ico::after {
  content: "";
  position: absolute;
  width: 4px; height: 1.5px;
  background: currentColor;
  bottom: -2px; right: -3px;
  transform: rotate(45deg);
}
.search .label { flex: 1; text-align: left; }
.sidebar.collapsed .search { display: none; }

.group {
  font-size: var(--fs-xxs);
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.08em;
  padding: 12px 14px 4px;
  font-weight: 600;
}
.sidebar.collapsed .group {
  height: 12px;
  color: transparent;
  padding: 12px 0 0;
}

.item {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 1px 8px;
  padding: 6px 8px;
  border: 0;
  border-radius: var(--radius);
  background: transparent;
  color: var(--text);
  font-size: var(--fs-sm);
  cursor: pointer;
  text-align: left;
}
.item:hover:not(:disabled) { background: var(--bg-elev-2); }
.item:disabled { cursor: not-allowed; }
.item.active {
  background: var(--accent-soft);
  color: var(--accent);
}
.item .ico {
  width: 14px; height: 14px;
  border-radius: 3px;
  background: var(--line-strong);
  flex-shrink: 0;
}
.item.active .ico { background: var(--accent); }
.item.dim { color: var(--text-muted); }
.item .label {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex: 1;
}
.item .pill {
  margin-left: auto;
  background: var(--line-strong);
  color: var(--text-muted);
  font-size: 10px;
  padding: 1px 6px;
  border-radius: var(--radius-pill);
  flex-shrink: 0;
}
.sidebar.collapsed .item { justify-content: center; padding: 7px; margin: 1px 4px; }
.sidebar.collapsed .item .label,
.sidebar.collapsed .item .pill { display: none; }

.foot {
  margin-top: auto;
  padding: 10px;
  border-top: 1px solid var(--line);
  display: flex;
  align-items: center;
  gap: 10px;
  color: var(--text-muted);
  font-size: var(--fs-xxs);
}
.sidebar.collapsed .foot .ver { display: none; }
.sidebar.collapsed .foot { justify-content: center; }

.toggle {
  margin-left: auto;
  width: 22px; height: 22px;
  display: grid;
  place-items: center;
  border: 0;
  border-radius: var(--radius-sm);
  background: var(--bg-elev-2);
  color: var(--text-muted);
  cursor: pointer;
  font-size: 13px;
  line-height: 1;
}
.toggle:hover { color: var(--text); }
</style>
```

- [ ] **Step 7.2 — Commit**

```
git add src/components/shell/Sidebar.vue
git commit -m "feat(shell): Sidebar with collapsible state and tool list"
```

---

## Task 8：Topbar 组件

**Files:**

- Create: `src/components/shell/Topbar.vue`

- [ ] **Step 8.1 — 写 Topbar.vue**

`src/components/shell/Topbar.vue`：

```vue
<script setup lang="ts">
defineProps<{
  crumb?: string;
}>();
</script>

<template>
  <header class="topbar">
    <div class="crumb">
      <span class="root">工具</span>
      <span v-if="crumb" class="sep">/</span>
      <span v-if="crumb" class="here">{{ crumb }}</span>
    </div>
    <div class="right">
      <slot name="right" />
    </div>
  </header>
</template>

<style scoped>
.topbar {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 22px;
  background: var(--bg-base);
  border-bottom: 1px solid var(--line);
  font-size: var(--fs-xs);
  color: var(--text-muted);
}

.crumb {
  display: flex;
  align-items: center;
  gap: 6px;
}
.crumb .sep { color: var(--text-faint); }
.crumb .here {
  color: var(--text);
  font-weight: 500;
}

.right {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 8px;
}
</style>
```

- [ ] **Step 8.2 — Commit**

```
git add src/components/shell/Topbar.vue
git commit -m "feat(shell): Topbar with breadcrumb + right pill slot"
```

---

## Task 9：Dashboard 组件

**Files:**

- Create: `src/components/shell/Dashboard.vue`

- [ ] **Step 9.1 — 写 Dashboard.vue**

`src/components/shell/Dashboard.vue`：

```vue
<script setup lang="ts">
import { computed } from 'vue';
import type { Tool } from '../../types/tool';

const props = defineProps<{
  tools: Tool[];
  lastToolId: string | null;
}>();

const emit = defineEmits<{
  open: [id: string];
}>();

const ready = computed(() => props.tools.filter((t) => t.status === 'ready'));
const soonCount = computed(() => props.tools.filter((t) => t.status === 'soon').length);

const lastTool = computed(() =>
  props.lastToolId ? ready.value.find((t) => t.id === props.lastToolId) ?? null : null
);
</script>

<template>
  <section class="dashboard">
    <div class="hero">
      <div class="title">欢迎回来</div>
      <div class="meta">{{ ready.length }} 个工具就绪 · {{ soonCount }} 个规划中</div>
    </div>

    <div v-if="lastTool" class="block">
      <div class="block-title">上次使用</div>
      <button class="last" type="button" @click="emit('open', lastTool.id)">
        <span class="dot"></span>
        <span class="text">
          <strong>{{ lastTool.name }}</strong>
          <span class="desc">{{ lastTool.description }}</span>
        </span>
        <span class="arrow">→</span>
      </button>
    </div>

    <div class="block">
      <div class="block-title">快速入口</div>
      <div class="grid">
        <button
          v-for="tool in ready"
          :key="tool.id"
          class="tile"
          type="button"
          @click="emit('open', tool.id)"
        >
          <span class="tile-name">{{ tool.name }}</span>
          <span class="tile-desc">{{ tool.description }}</span>
        </button>
      </div>
    </div>
  </section>
</template>

<style scoped>
.dashboard {
  max-width: 720px;
  margin: 36px auto 0;
  padding: 0 22px 36px;
  display: grid;
  gap: 28px;
}

.hero .title {
  font-size: var(--fs-2xl);
  font-weight: 600;
  letter-spacing: -0.012em;
  margin-bottom: 4px;
}
.hero .meta {
  color: var(--text-muted);
  font-size: var(--fs-xs);
}

.block-title {
  font-size: var(--fs-xxs);
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--text-muted);
  font-weight: 600;
  margin-bottom: 8px;
}

.last {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  background: var(--bg-elevated);
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  padding: 12px 14px;
  color: var(--text);
  cursor: pointer;
  text-align: left;
}
.last:hover { border-color: var(--line-strong); }
.last .dot {
  width: 8px; height: 8px;
  background: var(--accent);
  border-radius: 50%;
  flex-shrink: 0;
}
.last .text { flex: 1; display: flex; flex-direction: column; gap: 2px; }
.last .text strong { font-size: var(--fs-md); font-weight: 600; }
.last .text .desc { color: var(--text-muted); font-size: var(--fs-xs); }
.last .arrow { color: var(--text-muted); font-size: 14px; }

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 10px;
}

.tile {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 12px 14px;
  background: var(--bg-elevated);
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  cursor: pointer;
  color: var(--text);
  text-align: left;
}
.tile:hover { border-color: var(--line-strong); }
.tile-name { font-size: var(--fs-md); font-weight: 600; }
.tile-desc { color: var(--text-muted); font-size: var(--fs-xs); }
</style>
```

- [ ] **Step 9.2 — Commit**

```
git add src/components/shell/Dashboard.vue
git commit -m "feat(shell): Dashboard with last tool + quick links"
```

---

## Task 10：AppShell + 替换 App.vue 顶层结构

> 这一步把"首页 grid → 进入工具"的旧导航换成"sidebar 切换工具"。每个工具的具体模板片段（Aria2 表单、Logo 编辑器）暂时原样保留，下面的 task 再各自重做。

**Files:**

- Create: `src/components/shell/AppShell.vue`
- Modify: `src/App.vue`

- [ ] **Step 10.1 — 写 AppShell.vue**

`src/components/shell/AppShell.vue`：

```vue
<script setup lang="ts">
import Sidebar from './Sidebar.vue';
import Topbar from './Topbar.vue';
import type { Tool } from '../../types/tool';

defineProps<{
  tools: Tool[];
  activeId: string | null;
  collapsed: boolean;
  crumb?: string;
}>();

const emit = defineEmits<{
  select: [id: string];
  toggle: [];
  brand: [];
  search: [];
}>();
</script>

<template>
  <div class="app-shell">
    <Sidebar
      :tools="tools"
      :active-id="activeId"
      :collapsed="collapsed"
      @select="(id) => emit('select', id)"
      @toggle="emit('toggle')"
      @brand="emit('brand')"
      @search="emit('search')"
    />
    <main class="main">
      <Topbar :crumb="crumb">
        <template #right>
          <slot name="topbar-right" />
        </template>
      </Topbar>
      <div class="content">
        <slot />
      </div>
    </main>
  </div>
</template>

<style scoped>
.app-shell {
  display: flex;
  min-height: 100vh;
  background: var(--bg-base);
}

.main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.content {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 18px 22px;
}
</style>
```

- [ ] **Step 10.2 — 改 App.vue：移除旧的 home grid + 重构 selectedTool 状态**

打开 `src/App.vue`：

1. 在 `<script setup>` 顶部 `import` 区追加：

```ts
import AppShell from './components/shell/AppShell.vue';
import Dashboard from './components/shell/Dashboard.vue';
import StatPill from './components/ui/StatPill.vue';
import { useSidebarState } from './composables/useSidebarState';
import { useLastTool } from './composables/useLastTool';
import type { Tool } from './types/tool';
```

2. 把现有的 `tools` 数组（约 59-109 行的 `ToolEntry[]`）替换为 `Tool[]` 形态：

```ts
const tools: Tool[] = [
  { id: 'aria2',     name: 'Aria2 下载',     description: 'HTTP / HTTPS / FTP / BT 多连接下载', status: 'ready' },
  { id: 'template',  name: '主图模板',       description: 'PSD 导入、字段替换、批量生成主图',   status: 'ready' },
  { id: 'image',     name: '电商图片处理',   description: '批量加 Logo、商品图处理',             status: 'ready' },
  { id: 'clipboard', name: '剪贴板工具',     description: '剪贴板历史、清洗与批量转换',         status: 'soon' },
  { id: 'text',      name: '文本工具',       description: '去重、排序、分割、大小写转换',       status: 'soon' },
  { id: 'network',   name: '网络工具',       description: 'Ping、端口检查、URL 分析',           status: 'soon' },
  { id: 'codec',     name: '编码转换',       description: 'Base64、URL Encode、Hash 摘要',      status: 'soon' }
];
```

并删除老的 `type ToolEntry` 声明。

3. 在状态区追加：

```ts
const { collapsed: sidebarCollapsed, toggle: toggleSidebar } = useSidebarState();
const { lastToolId, remember: rememberLastTool } = useLastTool();
```

4. 把原来的 `selectedToolId = ref<string | null>(null);` 改为初始读取 lastTool（只在工具仍存在且为 ready 时使用）：

```ts
const initialId = (() => {
  const id = lastToolId.value;
  if (!id) return null;
  const t = tools.find((x) => x.id === id);
  return t && t.status === 'ready' ? id : null;
})();
const selectedToolId = ref<string | null>(initialId);
```

5. 替换原 `openTool` / `goHome`：

```ts
function selectTool(id: string) {
  const tool = tools.find((t) => t.id === id);
  if (!tool || tool.status !== 'ready') return;
  selectedToolId.value = id;
  rememberLastTool(id);
}

function goHome() {
  selectedToolId.value = null;
}

function openSearch() {
  alert('命令面板敬请期待');
}
```

6. 把整个 `<template>` 中 `<n-config-provider>` 内的 `<main class="app-shell">…</main>` 全部替换为：

```html
<n-config-provider :theme="darkTheme" :theme-overrides="darkOverrides">
  <n-message-provider>
    <AppShell
      :tools="tools"
      :active-id="selectedToolId"
      :collapsed="sidebarCollapsed"
      :crumb="selectedTool?.name"
      @select="selectTool"
      @toggle="toggleSidebar"
      @brand="goHome"
      @search="openSearch"
    >
      <template #topbar-right>
        <template v-if="selectedTool?.id === 'aria2'">
          <StatPill tone="accent">进行中 {{ activeCount }}</StatPill>
          <StatPill>已完成 {{ completedCount }}</StatPill>
        </template>
      </template>

      <Dashboard
        v-if="!selectedTool"
        :tools="tools"
        :last-tool-id="lastToolId"
        @open="selectTool"
      />

      <template v-else-if="selectedTool.id === 'aria2'">
        <!-- 保留原 Aria2 区块的 <n-page-header> ... <n-grid> ... 整段 -->
        <!-- 见下面 Step 10.3 -->
      </template>

      <template v-else-if="selectedTool.id === 'template'">
        <TemplateTool />
      </template>

      <template v-else-if="selectedTool.id === 'image'">
        <!-- 保留原 image 工具整段 -->
      </template>
    </AppShell>

    <n-modal v-model:show="showPresetModal" preset="card" title="保存当前方案" class="preset-modal">
      <!-- 保留原 modal 内容 -->
    </n-modal>
  </n-message-provider>
</n-config-provider>
```

- [ ] **Step 10.3 — 把 Aria2 与 image 工具的现有模板片段拷回新结构**

在 Step 10.2 留出的 `<!-- 保留原 ... -->` 占位处，把原 `<template v-if="selectedTool.id === 'aria2'">` 与 `<template v-else-if="selectedTool.id === 'image'">` 内**完整的子模板**（即原文件 628-720 行 / 727-865 行）整段粘进去；不动业务变量。

> 提示：拷贝时移除最外层 `<n-space vertical :size="18">` 与"返回首页"按钮（导航现在由 sidebar 接管）。

- [ ] **Step 10.4 — 删除 App.vue 里失效的旧片段**

确认删除：

- `<n-card v-if="!selectedTool" class="surface-card home-surface">` 整块（约 577-616 行）
- `<n-card v-else class="surface-card tool-surface">` 的最外层包装（保留内部模板，移到 AppShell 槽位）
- `<n-flex justify="space-between" align="center"> ... 返回首页 ...` 的整段（约 620-626 行）

- [ ] **Step 10.5 — 跑构建验证**

```
npm run build
```

预期：tsc 通过、vite 通过；如果出现"未使用的 import" 警告（如 `NPageHeader` 在 Aria2 区还在用），保留即可。

- [ ] **Step 10.6 — 跑 dev 验证**

```
npm run dev
```

人工验收（仍然是中间态，旧 styles.css 未删）：

- 启动后看到 sidebar + dashboard，不再是工具卡片首页
- 点 sidebar 中"Aria2 下载"，主区切到下载工作台；面包屑显示"工具 / Aria2 下载"
- 顶栏右侧显示"进行中 N · 已完成 N"两个 pill
- 点 sidebar 折叠按钮 ‹，宽度变 56；刷新页面后仍保持折叠状态
- 点品牌行（最上方"AT Tool"）回到 dashboard
- "Soon" 工具点不动；规划中分组带 Soon pill

- [ ] **Step 10.7 — Commit**

```
git add src/components/shell/AppShell.vue src/App.vue
git commit -m "feat(shell): replace home grid with sidebar nav + dashboard"
```

---

## Task 11：Panel 组件

**Files:**

- Create: `src/components/ui/Panel.vue`

- [ ] **Step 11.1 — 写 Panel.vue**

`src/components/ui/Panel.vue`：

```vue
<script setup lang="ts">
defineProps<{
  title?: string;
  flush?: boolean; // 去掉 body padding
}>();
</script>

<template>
  <section class="panel">
    <header v-if="title || $slots.title || $slots.right" class="ph">
      <span class="pt"><slot name="title">{{ title }}</slot></span>
      <span class="pr"><slot name="right" /></span>
    </header>
    <div class="pb" :class="{ flush }">
      <slot />
    </div>
  </section>
</template>

<style scoped>
.panel {
  background: var(--bg-elevated);
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.ph {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 10px 14px;
  border-bottom: 1px solid var(--line);
  font-size: var(--fs-xs);
  color: var(--text-muted);
}
.pt { color: var(--text); font-weight: 500; font-size: var(--fs-md); }

.pb { padding: 14px; }
.pb.flush { padding: 0; }
</style>
```

- [ ] **Step 11.2 — Commit**

```
git add src/components/ui/Panel.vue
git commit -m "feat(ui): Panel container as NCard replacement"
```

---

## Task 12：TaskRow（替换 TaskCard）

**Files:**

- Create: `src/components/ui/TaskRow.vue`

- [ ] **Step 12.1 — 写 TaskRow.vue**

`src/components/ui/TaskRow.vue`：

```vue
<script setup lang="ts">
import { computed } from 'vue';
import type { DownloadStatus, DownloadTask } from '../../types/download';

const STATUS_TEXT: Record<DownloadStatus, string> = {
  queued: '排队中',
  running: '下载中',
  completed: '已完成',
  failed: '失败',
  cancelled: '已取消'
};

const STATUS_TONE: Record<DownloadStatus, 'accent' | 'warning' | 'error' | 'done'> = {
  queued: 'warning',
  running: 'accent',
  completed: 'done',
  failed: 'error',
  cancelled: 'warning'
};

const props = defineProps<{ task: DownloadTask }>();
const emit = defineEmits<{
  cancel: [id: string];
  openFolder: [id: string];
}>();

const cancellable = computed(() => props.task.status === 'queued' || props.task.status === 'running');
const openable = computed(() => props.task.status === 'completed');
const pct = computed(() => Math.round(Math.min(Math.max(props.task.progress, 0), 100)));
</script>

<template>
  <article class="row">
    <header class="top">
      <div class="title">
        <span class="name">{{ task.fileName || task.url }}</span>
        <span v-if="task.fileName" class="url">{{ task.url }}</span>
      </div>
      <span class="badge" :data-tone="STATUS_TONE[task.status]">{{ STATUS_TEXT[task.status] }}</span>
    </header>

    <div class="meta tnum">
      <span>{{ task.speed ? task.speed : '—' }}</span>
      <span>{{ task.eta ? `ETA ${task.eta}` : 'ETA --' }}</span>
      <span class="pct">{{ pct }}%</span>
    </div>

    <div class="progress" :data-tone="STATUS_TONE[task.status]">
      <i :style="{ width: `${pct}%` }"></i>
    </div>

    <p v-if="task.message" class="message">{{ task.message }}</p>

    <footer v-if="cancellable || openable" class="actions">
      <button v-if="cancellable" class="btn ghost-warn" type="button" @click="emit('cancel', task.id)">
        取消任务
      </button>
      <button v-if="openable" class="btn ghost" type="button" @click="emit('openFolder', task.id)">
        打开文件夹
      </button>
    </footer>
  </article>
</template>

<style scoped>
.row {
  display: grid;
  gap: 6px;
  padding: 11px 12px;
  background: var(--bg-elevated);
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
}
.row + .row { margin-top: 6px; }

.top {
  display: flex;
  align-items: flex-start;
  gap: 10px;
}
.title { flex: 1; min-width: 0; display: grid; gap: 2px; }
.name {
  font-size: var(--fs-sm);
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.url {
  color: var(--text-muted);
  font-size: var(--fs-xxs);
  font-family: var(--font-mono);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.badge {
  flex-shrink: 0;
  padding: 2px 8px;
  border-radius: var(--radius-pill);
  font-size: 10.5px;
  font-weight: 500;
  background: var(--accent-soft);
  color: var(--accent);
}
.badge[data-tone="warning"] {
  background: color-mix(in srgb, var(--warning) 22%, transparent);
  color: var(--warning);
}
.badge[data-tone="error"] {
  background: color-mix(in srgb, var(--error) 22%, transparent);
  color: var(--error);
}
.badge[data-tone="done"] {
  background: var(--line-strong);
  color: var(--text-muted);
}

.meta {
  display: flex;
  gap: 14px;
  color: var(--text-muted);
  font-size: var(--fs-xxs);
}
.meta .pct { margin-left: auto; }

.progress {
  height: 3px;
  background: var(--line-strong);
  border-radius: var(--radius-pill);
  overflow: hidden;
}
.progress > i {
  display: block;
  height: 100%;
  background: var(--accent);
  transition: width var(--motion-fast);
}
.progress[data-tone="warning"] > i { background: var(--warning); }
.progress[data-tone="error"] > i { background: var(--error); }
.progress[data-tone="done"] > i { background: var(--text-muted); }

.message {
  margin: 0;
  font-size: var(--fs-xs);
  color: var(--text-muted);
  line-height: 1.5;
}

.actions { display: flex; gap: 8px; padding-top: 2px; }
.btn {
  display: inline-flex;
  align-items: center;
  padding: 4px 10px;
  border: 1px solid var(--line-strong);
  border-radius: var(--radius);
  background: var(--bg-elev-2);
  color: var(--text);
  font-size: var(--fs-xs);
  cursor: pointer;
}
.btn:hover { border-color: #3a3a40; }
.btn.ghost-warn {
  color: var(--warning);
  border-color: color-mix(in srgb, var(--warning) 35%, var(--line-strong));
}
</style>
```

- [ ] **Step 12.2 — Commit**

```
git add src/components/ui/TaskRow.vue
git commit -m "feat(ui): TaskRow replaces TaskCard with new visual language"
```

---

## Task 13：Aria2 工具页适配

**Files:**

- Modify: `src/App.vue`

- [ ] **Step 13.1 — 替换 Aria2 区块的 import / 模板**

在 App.vue 顶部 import 追加：

```ts
import Panel from './components/ui/Panel.vue';
import TaskRow from './components/ui/TaskRow.vue';
```

把 Aria2 区块（`<template v-if="selectedTool?.id === 'aria2'">` ... `</template>`）整段替换为：

```html
<template v-if="selectedTool?.id === 'aria2'">
  <div class="page">
    <header class="page-header">
      <h2>多线程下载工作台</h2>
      <p>本机 aria2c 引擎，支持断点续传、分片、多连接和实时进度回传。</p>
    </header>

    <div class="aria2-grid">
      <Panel title="新建下载">
        <template #right><span>支持批量</span></template>
        <form @submit.prevent="startDownload" class="form">
          <label class="field">
            <span class="lbl">资源链接（每行一个，或用逗号分隔）</span>
            <n-input
              v-model:value="url"
              type="textarea"
              placeholder="https://example.com/file-a.zip&#10;https://example.com/file-b.zip"
              :autosize="{ minRows: 5, maxRows: 10 }"
            />
          </label>

          <div class="row2">
            <label class="field">
              <span class="lbl">保存目录</span>
              <n-input-group>
                <n-input v-model:value="downloadDir" placeholder="/Users/you/Downloads" />
                <n-button secondary :loading="choosingDir" @click="chooseDownloadDir">选择文件夹</n-button>
              </n-input-group>
            </label>
            <label class="field">
              <span class="lbl">文件名（仅单个链接时生效）</span>
              <n-input v-model:value="fileName" placeholder="archive.zip" />
            </label>
          </div>

          <div class="row3">
            <label class="field">
              <span class="lbl">单服务器连接数</span>
              <n-input-number v-model:value="connections" :min="1" :max="16" style="width: 100%" />
            </label>
            <label class="field">
              <span class="lbl">分片数</span>
              <n-input-number v-model:value="split" :min="1" :max="64" style="width: 100%" />
            </label>
            <label class="field">
              <span class="lbl">最小分片大小</span>
              <n-select v-model:value="minSplitSize" :options="minSplitOptions" />
            </label>
          </div>

          <n-alert v-if="notice" type="error" :bordered="false" class="notice-alert">
            {{ notice }}
          </n-alert>

          <n-button type="primary" block attr-type="submit" :loading="submitting">
            {{ submitting ? '正在创建...' : '开始下载' }}
          </n-button>
        </form>
      </Panel>

      <Panel title="任务队列">
        <template #right><span class="mono">实时</span></template>
        <div v-if="tasks.length === 0" class="empty">还没有下载任务</div>
        <div v-else class="tasks">
          <TaskRow
            v-for="task in tasks"
            :key="task.id"
            :task="task"
            @cancel="cancelTask"
            @open-folder="openTaskFolder"
          />
        </div>
      </Panel>
    </div>
  </div>
</template>
```

- [ ] **Step 13.2 — 在 App.vue 末尾追加 scoped style（如果还没有 `<style>` 块就新建一个）**

```html
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

.aria2-grid {
  display: grid;
  grid-template-columns: 1.1fr 1fr;
  gap: 16px;
}
@media (max-width: 1100px) {
  .aria2-grid { grid-template-columns: 1fr; }
}

.form { display: grid; gap: 12px; }
.field { display: grid; gap: 5px; }
.field .lbl {
  font-size: var(--fs-xxs);
  color: var(--text-muted);
}
.row2 { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
.row3 { display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px; }

.notice-alert { margin-bottom: 4px; }

.tasks { display: grid; gap: 6px; }
.empty {
  padding: 60px 20px;
  text-align: center;
  color: var(--text-muted);
  border: 1px dashed var(--line-strong);
  border-radius: var(--radius-md);
  font-size: var(--fs-sm);
}
</style>
```

- [ ] **Step 13.3 — 删除 TaskCard.vue 与 App.vue 里残留的 `<NPageHeader>` 引用**

```
git rm src/components/TaskCard.vue
```

在 `src/App.vue` 的 import 中删除 `TaskCard` 与 `NPageHeader`（如果已无引用）。

- [ ] **Step 13.4 — 跑构建与 dev 验证**

```
npm run build
npm run dev
```

人工验收：

- Aria2 页样式更像设计稿 ③：左表单右队列，task row 紧凑，状态徽标走新颜色
- 速度 / ETA / 百分比是等宽数字
- 进度条 3px 高，emerald 填充
- 取消任务、打开文件夹按钮工作正常

- [ ] **Step 13.5 — Commit**

```
git add -A
git commit -m "feat(aria2): adopt Panel + TaskRow with new visual language"
```

---

## Task 14：电商图片处理（加 Logo）页适配

**Files:**

- Modify: `src/App.vue`（image 工具区块）

- [ ] **Step 14.1 — 替换 image 工具区块的模板**

把原 `<template v-else-if="selectedTool?.id === 'image'">` 整段重写为下面，主要变化：用 `Panel` 取代 `NCard`，去掉 `<NPageHeader>`，配色全部走 token，`logo-preview-frame` 背景改深色。

```html
<template v-else-if="selectedTool?.id === 'image'">
  <div class="page">
    <header class="page-header">
      <h2>电商图片处理</h2>
      <p>左侧选择商品图与 Logo，右侧拖拽 Logo 到任意位置，拖动右下角控制点调整大小。</p>
    </header>

    <div class="image-editor-layout">
      <Panel title="素材与参数">
        <n-space vertical :size="12">
          <n-button type="primary" block @click="chooseImages">添加图片</n-button>

          <div class="image-list">
            <button
              v-for="(path, index) in imagePaths"
              :key="path"
              :class="['image-list-item', { active: index === selectedPreviewIndex }]"
              type="button"
              @click="selectPreviewImage(index)"
            >
              <img class="image-list-thumb" :src="convertFileSrc(path)" alt="商品图缩略图" draggable="false" />
              <n-ellipsis :tooltip="false">{{ path }}</n-ellipsis>
              <span class="image-remove" @click.stop="removeImage(index)">删除</span>
            </button>
            <div v-if="imagePaths.length === 0" class="empty">还没有图片</div>
          </div>

          <n-form label-placement="top" size="small">
            <n-form-item label="已保存方案">
              <n-select
                v-model:value="selectedPresetId"
                :options="logoPresetOptions"
                clearable
                placeholder="选择方案快速应用"
                @update:value="applyLogoPreset"
              />
            </n-form-item>
          </n-form>

          <n-form label-placement="top" size="small">
            <n-form-item label="Logo 图片">
              <n-input-group>
                <n-input v-model:value="logoPath" readonly placeholder="选择 Logo 文件" />
                <n-button secondary @click="chooseLogo">选择 Logo</n-button>
              </n-input-group>
            </n-form-item>

            <n-form-item label="输出目录">
              <n-input-group>
                <n-input v-model:value="imageOutputDir" readonly placeholder="选择处理后图片保存位置" />
                <n-button secondary @click="chooseImageOutputDir">选择文件夹</n-button>
              </n-input-group>
            </n-form-item>

            <n-grid responsive="screen" cols="1 m:2" :x-gap="12">
              <n-grid-item>
                <n-form-item label="X 坐标（%）">
                  <n-input-number v-model:value="logoXPercent" :min="0" :max="100" style="width: 100%" @update:value="clampLogoPlacement" />
                </n-form-item>
              </n-grid-item>
              <n-grid-item>
                <n-form-item label="Y 坐标（%）">
                  <n-input-number v-model:value="logoYPercent" :min="0" :max="100" style="width: 100%" @update:value="clampLogoPlacement" />
                </n-form-item>
              </n-grid-item>
            </n-grid>

            <n-form-item label="Logo 宽度占比（%）">
              <n-input-number v-model:value="logoWidthPercent" :min="1" :max="100" style="width: 100%" @update:value="clampLogoPlacement" />
            </n-form-item>
          </n-form>

          <n-alert v-if="imageNotice" type="error" :bordered="false" class="notice-alert">
            {{ imageNotice }}
          </n-alert>

          <div class="image-action-row">
            <n-button class="image-apply-action" type="primary" :loading="imageProcessing" @click="addLogoBatch">
              {{ imageProcessing ? '正在处理...' : '应用到全部图片' }}
            </n-button>
            <n-button class="image-save-action" secondary @click="openPresetModal">
              保存当前方案
            </n-button>
          </div>
        </n-space>
      </Panel>

      <Panel title="预览与处理结果">
        <n-space vertical :size="12">
          <div
            v-if="previewImageSrc"
            ref="previewFrame"
            class="logo-preview-frame"
            @pointermove="moveLogo"
            @pointerup="stopLogoInteraction"
            @pointercancel="stopLogoInteraction"
          >
            <img ref="previewBaseImage" class="preview-base-image" :src="previewImageSrc" alt="商品图预览" draggable="false" />
            <div
              v-if="previewLogoSrc"
              class="preview-logo-layer"
              :style="logoStyle"
              @pointerdown="startLogoDrag"
            >
              <img :src="previewLogoSrc" alt="Logo 预览" draggable="false" />
              <span class="logo-resize-handle" @pointerdown="startLogoResize" />
            </div>
          </div>
          <div v-else class="empty">请先在左侧添加图片</div>

          <n-alert v-if="previewImageSrc && !previewLogoSrc" type="info" :bordered="false">
            请选择 Logo 图片后，可在预览图中拖拽位置和缩放大小。
          </n-alert>

          <n-alert v-if="imageResult" type="success" :bordered="false">
            共 {{ imageResult.total }} 张，成功 {{ imageResult.succeeded }} 张，失败 {{ imageResult.failed.length }} 张。
          </n-alert>

          <Panel v-if="imageResult?.outputs.length" title="输出文件" flush>
            <div class="result-list">
              <n-ellipsis v-for="output in imageResult.outputs" :key="output" :tooltip="false">
                {{ output }}
              </n-ellipsis>
            </div>
          </Panel>

          <Panel v-if="imageResult?.failed.length" title="失败记录" flush>
            <div class="result-list">
              <span v-for="item in imageResult.failed" :key="item.path" class="error-line">
                {{ item.path }}：{{ item.message }}
              </span>
            </div>
          </Panel>
        </n-space>
      </Panel>
    </div>
  </div>
</template>
```

- [ ] **Step 14.2 — 在 App.vue scoped style 末尾追加 image 工具样式（替换原 styles.css 中的暖色版本）**

```css
.image-editor-layout {
  display: grid;
  grid-template-columns: 320px minmax(0, 1fr);
  gap: 16px;
  align-items: start;
}
@media (max-width: 920px) {
  .image-editor-layout { grid-template-columns: 1fr; }
}

.image-list {
  display: grid;
  gap: 6px;
  max-height: 260px;
  overflow: auto;
  border: 1px dashed var(--line-strong);
  border-radius: var(--radius-md);
  background: var(--bg-elev-2);
  padding: 8px;
}

.image-list-item {
  display: grid;
  grid-template-columns: 36px minmax(0, 1fr) auto;
  gap: 8px;
  align-items: center;
  width: 100%;
  border: 1px solid transparent;
  border-radius: var(--radius);
  background: var(--bg-elevated);
  color: var(--text);
  padding: 7px 9px;
  text-align: left;
  cursor: pointer;
}
.image-list-item:hover { border-color: var(--line-strong); }
.image-list-item.active {
  border-color: color-mix(in srgb, var(--accent) 50%, var(--line-strong));
  background: var(--accent-soft);
  color: var(--accent);
}
.image-list-thumb {
  width: 36px; height: 36px;
  border-radius: var(--radius-sm);
  object-fit: cover;
  background: var(--bg-base);
}
.image-remove {
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, var(--error) 18%, transparent);
  color: var(--error);
  padding: 3px 8px;
  font-size: var(--fs-xxs);
  font-weight: 600;
}

.logo-preview-frame {
  position: relative;
  overflow: hidden;
  width: fit-content;
  max-width: 100%;
  margin: 0 auto;
  border: 1px solid var(--line-strong);
  border-radius: var(--radius-md);
  background: #0f0f12;
  touch-action: none;
  user-select: none;
}
.preview-base-image {
  display: block;
  max-width: 100%;
  max-height: 68vh;
  object-fit: contain;
  user-select: none;
}
.preview-logo-layer {
  position: absolute;
  cursor: move;
  touch-action: none;
}
.preview-logo-layer img {
  display: block;
  width: 100%;
  height: auto;
  user-select: none;
  pointer-events: none;
}
.logo-resize-handle {
  position: absolute;
  right: -6px;
  bottom: -6px;
  width: 12px;
  height: 12px;
  border: 1.5px solid #0a0a0b;
  border-radius: 2px;
  background: var(--accent);
  cursor: nwse-resize;
}

.image-action-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 128px;
  gap: 10px;
}
.image-action-row .n-button { width: 100%; }
@media (max-width: 420px) {
  .image-action-row { grid-template-columns: 1fr; }
}

.result-list {
  display: grid;
  gap: 4px;
  padding: 12px 14px;
  font-size: var(--fs-xs);
  color: var(--text-muted);
  font-family: var(--font-mono);
}
.error-line { color: var(--error); }
```

- [ ] **Step 14.3 — 跑构建与 dev 验证**

```
npm run build
npm run dev
```

人工验收（image 工具）：

- 左侧素材列表：暗背景 + 虚线框；激活项 emerald 高亮
- 右侧预览框背景为 `#0f0f12`，无米色渐变
- Logo 拖拽手柄是 12 × 12 emerald 方块（深色描边），不再是绿色圆点
- 删除徽标用红色，不是橙色
- 保存方案的模态框走 dark modal

- [ ] **Step 14.4 — Commit**

```
git add src/App.vue
git commit -m "feat(image): restyle logo editor in mono dark"
```

---

## Task 15：模板编辑器适配

> 这是表面积最大的一组改动，但**只改 CSS / 颜色 / 边框**，不动逻辑。重点：rail / resource panel / canvas 棋盘格 / props panel / selection 边框。

**Files:**

- Modify: `src/components/ecommerce/TemplateTool.vue`（仅 `<style>` 区，必要时小改 template 中的 class）
- Modify: `src/components/ecommerce/TemplateResourcePanel.vue`
- Modify: `src/components/ecommerce/LayerProperties.vue`
- Modify: `src/components/ecommerce/LayerTree.vue`
- Modify: `src/components/ecommerce/TemplateCanvas.vue`
- Modify: `src/components/ecommerce/BatchTaskPanel.vue`
- 暂不动：`src/styles.css` 里的 `.template-*` 系列（Task 16 集中清掉）

> 实施约定：先把这些组件用到的颜色 / 圆角 / 边框 **全部从 `styles.css` 中迁移到对应组件的 `<style scoped>`，并替换为 token**。源 hex 与新 token 的对照表如下。

**颜色映射表：**

| 旧值 | 新 token | 用途 |
|---|---|---|
| `rgba(255, 250, 238, 0.78)` / `0.68` | `var(--bg-elevated)` | 面板背景 |
| `rgba(255, 250, 238, 0.46)` / `0.38` | `var(--bg-elev-2)` | 列表底色 |
| `rgba(23, 33, 27, 0.08)` / `0.12` | `var(--line)` | 默认线 |
| `rgba(23, 33, 27, 0.16)` / `0.24` | `var(--line-strong)` | 强分隔/输入框线 |
| `rgba(86, 113, 93, 0.12)` / `0.14` | `var(--accent-soft)` | 激活底色 |
| `rgba(86, 113, 93, 0.34)` / `0.42` | `color-mix(in srgb, var(--accent) 35%, var(--line-strong))` | 激活边框 |
| `var(--moss)` `#56715d` | `var(--accent)` | 主交互色 |
| `var(--moss-dark)` `#263c2d` | `var(--accent)` | 深主色（合并） |
| `var(--ink)` `#17211b` | `var(--text)` | 正文 |
| `var(--muted)` `#667064` | `var(--text-muted)` | 次要文字 |
| `#fff7e9` | `var(--accent-fg)` | 激活态前景 |
| `rgba(214, 168, 79, 0.78~0.9)` | `var(--accent)` | 拖拽提示 |
| `rgba(23, 33, 27, 0.6~0.7)` | `var(--bg-overlay)` | 浮层小按钮底 |
| `0 18px 50px rgba(23, 33, 27, 0.14)` | `var(--shadow-canvas)` | 画布阴影 |

- [ ] **Step 15.1 — 把 `styles.css` 中 `.template-*` 系列样式拷贝到对应组件的 `<style scoped>`，并按上表替换颜色**

依次处理：

- `TemplateTool.vue`：吸入 `.template-workbench` / `.template-output-dir`
- `TemplateResourcePanel.vue`：吸入 `.template-workbench-rail`、`.template-resource-panel`、`.template-rail-button`、`.template-resource-heading`、`.template-image-asset*`、`.template-text-preset`、`.template-shape-preset`、`.shape-preview*`、`.template-summary*`
- `LayerTree.vue`：吸入 `.template-layer-tree`、`.template-layer-item*`
- `LayerProperties.vue`：吸入 `.template-editor-panel`、`.template-prop-*`、`.template-color-field`、`.template-switch-row`、`.template-layer-toolbar`
- `TemplateCanvas.vue`：吸入 `.template-canvas-wrap`、`.template-canvas`、`.template-canvas-layer`、`.template-text-*`、`.template-shape-layer`、`.template-resize-handle`
- `BatchTaskPanel.vue`：吸入 `.batch-*` 全系列

> 替换边规则：先粘贴原样式到 scoped style 里 → 再用编辑器的"全文替换"按颜色对照表跑一轮 → 把所有 `border-radius: 14px` / `18px` / `24px` 改为 `var(--radius-md)`（卡）或 `var(--radius)`（按钮）。

- [ ] **Step 15.2 — TemplateCanvas：换棋盘格背景与选中样式**

`TemplateCanvas.vue` 的 `.template-canvas-wrap` 改为：

```css
.template-canvas-wrap {
  display: grid;
  place-items: center;
  min-height: 68vh;
  overflow: auto;
  padding: 18px;
  background-color: var(--bg-base);
  background-image:
    linear-gradient(45deg, var(--bg-elev-2) 25%, transparent 25%),
    linear-gradient(-45deg, var(--bg-elev-2) 25%, transparent 25%),
    linear-gradient(45deg, transparent 75%, var(--bg-elev-2) 75%),
    linear-gradient(-45deg, transparent 75%, var(--bg-elev-2) 75%);
  background-size: 12px 12px;
  background-position: 0 0, 0 6px, 6px -6px, -6px 0px;
}

.template-canvas {
  position: relative;
  width: min(68vh, 100%);
  max-width: 100%;
  border: 1px solid var(--line-strong);
  background: #fff;
  box-shadow: var(--shadow-canvas);
}
```

`.template-canvas-layer.selected` 改为：

```css
.template-canvas-layer.selected {
  overflow: visible;
  border: 1.5px solid var(--accent);
  box-shadow: 0 0 0 3px var(--accent-soft);
}

.template-resize-handle {
  position: absolute;
  right: -5px;
  bottom: -5px;
  width: 8px;
  height: 8px;
  border: 1.5px solid var(--bg-base);
  border-radius: 2px;
  background: var(--accent);
  box-shadow: none;
  cursor: nwse-resize;
}
```

- [ ] **Step 15.3 — LayerProperties：浮动 toolbar 与 props 区**

把 `.template-layer-toolbar` 改为：

```css
.template-layer-toolbar {
  position: absolute;
  left: 0; top: -38px;
  z-index: 4;
  display: flex; gap: 4px;
  align-items: center;
  width: max-content;
  max-width: 360px;
  border-radius: var(--radius-md);
  background: var(--bg-overlay);
  border: 1px solid var(--line);
  box-shadow: var(--shadow-pop);
  padding: 6px;
}
.template-layer-toolbar.is-inside { top: 6px; }
.template-layer-toolbar button {
  border: 0;
  border-radius: var(--radius-sm);
  background: var(--bg-elev-2);
  color: var(--text);
  padding: 4px 7px;
  font-size: var(--fs-xs);
  cursor: pointer;
}
.template-layer-toolbar button:hover { background: #232328; }
```

把 props section 标题样式统一为：

```css
.template-prop-section h3 {
  margin: 0 0 6px;
  font-size: var(--fs-xxs);
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.08em;
  font-weight: 600;
}
```

- [ ] **Step 15.4 — TemplateResourcePanel：rail 按钮与资源面板**

`.template-rail-button` 改为：

```css
.template-rail-button {
  display: grid;
  place-items: center;
  min-height: 44px;
  border: 0;
  border-radius: var(--radius);
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
}
.template-rail-button:hover { background: var(--bg-elev-2); color: var(--text); }
.template-rail-button.active {
  background: var(--accent-soft);
  color: var(--accent);
}
```

把所有 `.template-resource-panel.is-paste-target`、`.template-canvas-card.is-paste-target` 的 `outline-color` 由 `var(--ink)` 改为 `var(--accent)`。

- [ ] **Step 15.5 — BatchTaskPanel：选中态、拖拽态颜色**

把 `.batch-output-card.selected`、`.batch-variant-image.drag-over-*`、`.batch-variant-text.drag-over-*` 的 `rgba(214, 168, 79, …)` 与 `rgba(86, 113, 93, …)` 全部按对照表改为 emerald。把 `.batch-task-remove`、`.batch-variant-remove`、`.batch-output-preview` 的浮层底色改为 `var(--bg-overlay)`，文字 `var(--text)`。

- [ ] **Step 15.6 — 跑构建与 dev 验证**

```
npm run build
npm run dev
```

人工验收（模板工具）：

- rail 按钮 hover / 激活态走 emerald
- 资源面板背景是 `#131316`，分组标题是灰色 LABEL
- 画布周边有棋盘格透明区
- 选中图层有 emerald 边框 + 4 角小手柄（深底描边）
- props 面板分段标题统一 LABEL 样式；输入框 dark
- 批量任务卡片选中是 emerald 边框 + 软底
- 浮动按钮（删除小 X 等）背景是深色 overlay

- [ ] **Step 15.7 — Commit**

```
git add -A
git commit -m "feat(template): restyle template editor (rail/canvas/props/batch) in mono dark"
```

---

## Task 16：清理旧 styles.css

**Files:**

- Modify: `src/styles.css`
- Modify: `src/main.ts`
- Possibly Modify: `src/App.vue`（删除残余 class 引用）

- [ ] **Step 16.1 — 找出仍在被引用的旧 class**

```
grep -RE "surface-card|home-surface|tool-surface|brand-mark|tool-card|tool-grid|home-brand|brand-name|task-card|panel-card|queue-card|preset-modal" src/
```

预期：只剩 App.vue 内 `<n-modal class="preset-modal">` 还引用 `.preset-modal`；其它 class 应该都已迁出或删除。

- [ ] **Step 16.2 — 删除 styles.css 内全部内容，仅保留 modal 宽度兜底**

`src/styles.css`：

```css
.preset-modal {
  width: min(420px, calc(100vw - 32px));
}
```

- [ ] **Step 16.3 — 跑构建与 dev 验证**

```
npm run build
npm run dev
```

人工验收：

- 视觉与上一步无差异（说明 styles.css 中删掉的样式都已被组件 scoped 接管）
- 整页无任何米色 / 苔藓绿 / 橙金残留
- `:root` 不再有 `--moss` / `--ember` / `--gold` / `--paper` 等旧变量

- [ ] **Step 16.4 — Commit**

```
git add src/styles.css
git commit -m "chore(style): purge legacy warm-palette styles.css"
```

---

## Task 17：键盘快捷键 `⌘\` + `⌘K` 占位

**Files:**

- Modify: `src/App.vue`

- [ ] **Step 17.1 — 在 App.vue `onMounted` 中注册全局快捷键**

在 `onMounted(() => { … })` 现有内容**末尾**追加：

```ts
function handleHotkey(event: KeyboardEvent) {
  const meta = event.metaKey || event.ctrlKey;
  if (!meta) return;

  if (event.key === '\\') {
    event.preventDefault();
    toggleSidebar();
  } else if (event.key === 'k' || event.key === 'K') {
    event.preventDefault();
    openSearch();
  }
}
window.addEventListener('keydown', handleHotkey);

const stopHotkey = () => window.removeEventListener('keydown', handleHotkey);
onUnmounted(stopHotkey);
```

> 已存在 `onUnmounted(...)` 调用（清理 download 监听）；把 `stopHotkey()` 加进同一个 onUnmounted 里也行 —— 选择对你最自然的方式即可，但**不要重复定义** `onUnmounted`。

- [ ] **Step 17.2 — dev 验证**

```
npm run dev
```

- 按 `⌘\`：sidebar 折叠 / 展开来回切换
- 按 `⌘K`：弹出 alert "命令面板敬请期待"

- [ ] **Step 17.3 — Commit**

```
git add src/App.vue
git commit -m "feat(shell): wire ⌘\\ for sidebar toggle and ⌘K placeholder"
```

---

## Task 18：最终验收

**Files:** 无

- [ ] **Step 18.1 — 跑全量测试**

```
npm test
```

预期：所有测试 PASS（含原 ecommerceTemplate 测试 + 新加的 theme / useSidebarState / useLastTool）。

- [ ] **Step 18.2 — 跑构建**

```
npm run build
```

预期：tsc 通过、vite build 通过、产物输出 `dist/`。

- [ ] **Step 18.3 — dev 模式对照 spec § 9 验收清单逐条勾**

`npm run dev` 后逐条核对 `docs/superpowers/specs/2026-05-10-ui-restyle-design.md` 的第 9 节：

- [ ] 启动后默认深色 + emerald accent，无任何暖色残余
- [ ] sidebar 可折叠（220 ↔ 56），状态在重启后保留
- [ ] 三个已就绪工具（Aria2 / 模板 / 加 Logo）在 sidebar 切换无回首页跳转
- [ ] 顶栏面包屑显示"工具 / <当前>"
- [ ] Aria2 任务行：状态徽标颜色按规则区分；速度 / ETA / 百分比走等宽数字
- [ ] 模板编辑器画布：棋盘格透明背景；选中图层有 emerald 边框 + 4 角手柄
- [ ] 所有 `<n-card>` 圆角 ≤ 8 px；`<n-button>` 圆角 = 6 px
- [ ] 没有任何 `border-radius: 14px / 18px / 24px`：`grep -RE "border-radius:\s*(14|18|24)px" src/` 应无结果
- [ ] `styles.css` 不再含 `--moss` / `--ember` / `--gold` / `--paper`
- [ ] 1280 × 800 窗口下不出横向滚动；折叠 sidebar 后 1100 px 窗口仍工作
- [ ] `⌘\` 折叠 sidebar、`⌘K` 占位 alert

- [ ] **Step 18.4 — Tauri 实机冒烟（可选但推荐）**

```
npm run tauri:dev
```

确认 Tauri 窗口里：字体回退正常（macOS 用 SF Pro / PingFang，看着舒服），所有交互（下载创建、文件夹选择、模板拖拽、加 Logo 拖拽）功能完好。

- [ ] **Step 18.5 — 最终 commit（如有遗留改动）**

```
git status
# 若有：
git add -A
git commit -m "chore(ui-restyle): finalize verification"
```

---

## Self-Review Notes

下面四类问题在本计划中均已避免：

1. **占位符**：所有步骤都有具体路径、命令、代码或精确的 grep 表达式
2. **类型一致性**：`Tool`、`KVStorage`、`DownloadTask` 在引用处和定义处签名相同；composable 返回值字段（`collapsed` / `toggle` / `lastToolId` / `remember`）在 App.vue 解构时一致
3. **Spec 覆盖**：spec § 9 的 11 条验收逐条对应 Task 1-17 的某一步；§ 7 的 8 个实施步骤映射到 Task 1-2、9-10、11、13、14、15、16、17
4. **风险（spec § 10）**：
   - Naive UI 个别组件可能不跟随 token —— Task 13/14 中保留 `<style scoped>` 兜底
   - Windows 字体回退 —— `--font-sans` 已含 `Inter` / `Segoe UI`
   - 棋盘格刺眼 —— Task 15.2 用 `var(--bg-elev-2)` 而非纯白格，对比度温和
   - dashboard "最近使用" —— Task 4/9 已用 useLastTool 实现单条记忆
