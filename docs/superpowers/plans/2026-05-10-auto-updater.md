# 软件更新（auto updater）实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 接入 Tauri 官方 updater 插件，让 attool 启动时自动检查 GitHub Releases 上的新版本并通过 banner 引导一键安装。

**Architecture:**
1. Rust 后端注册 `tauri-plugin-updater` + `tauri-plugin-process` 两个插件，配置走 `tauri.conf.json` 的 `plugins.updater`（endpoints 指向 GitHub Releases 的 `latest.json`，pubkey 用本地生成的 ed25519 公钥）。
2. 前端两个 composable：`useUpdaterPrefs`（autoCheck / skippedVersion 持久化）+ `useUpdater`（状态机包 Tauri API，注入式 client 便于测试）。
3. UI 两个组件：`UpdateBanner`（嵌在 topbar 与 content 之间，状态驱动）+ `SettingsModal`（sidebar 齿轮触发，含版本信息 + 立即检查按钮 + 自动检查复选框）。CI 在 tag push 时把私钥从 GitHub Secrets 注入 `tauri-action`，自动生成 `latest.json`。

**Tech Stack:** Tauri 2 · `tauri-plugin-updater` v2 · `tauri-plugin-process` v2 · Vue 3 · Vitest 4 · `tauri-apps/tauri-action` v0

**Spec:** `docs/superpowers/specs/2026-05-10-auto-updater-design.md`

---

## 文件结构（增/改概览）

**新建：**

```
src/composables/
├── useUpdaterPrefs.ts        # autoCheck + skippedVersion + 持久化
├── useUpdaterPrefs.test.ts
├── useUpdater.ts             # 状态机 + 注入式 UpdaterClient
└── useUpdater.test.ts

src/components/shell/
├── UpdateBanner.vue          # available / downloading / ready / error 四态
├── SettingsModal.vue         # 软件更新设置块
└── SettingsIcon.vue          # 齿轮 SVG（与 ToolIcon 同套路）
```

**修改：**

- `src-tauri/Cargo.toml` — 加 `tauri-plugin-updater = "2"` + `tauri-plugin-process = "2"`
- `src-tauri/src/lib.rs` — `run()` 里注册两个新插件
- `src-tauri/tauri.conf.json` — 加 `plugins.updater.endpoints` + `pubkey`
- `src-tauri/capabilities/default.json` — 加 `updater:default` + `process:default`
- `package.json` — 加 `@tauri-apps/plugin-updater` + `@tauri-apps/plugin-process`
- `src/components/shell/Sidebar.vue` — 加齿轮按钮，emit `settings-toggle`
- `src/components/shell/AppShell.vue` — 透传 `settings-toggle` + 容纳 `<UpdateBanner>` 槽位
- `src/App.vue` — `useUpdater` + `useUpdaterPrefs` + 启动 5s 自动检查 + 渲染 banner + modal
- `.github/workflows/build.yml` — env + `includeUpdaterJson` + `updaterJsonPreferNsis`
- `AGENTS.md` — 加 updater 章节
- `docs/spec/architecture.md` — 加 plugin-updater 引用

**手动操作（一次性）：**

- 本地用 `tauri signer generate` 生成密钥对
- 把私钥文件 + 口令存到密码管理器
- 把私钥 + 口令贴到 GitHub Secrets

---

## 实施顺序与原则

逻辑模块（composable）走 TDD；UI 组件目视回归；CI / Rust 配置改动每步 `cargo build` + `npm run build` 验证。每 task 单独 commit；中途任何一步失败立即停止排查，不积累未提交改动。

---

## Task 1：生成 updater 签名密钥（手动）

**Files:** 无（仅本地操作 + 手动备份）

> 这是一次性的 operator 步骤，必须先做。否则 Task 3 的 tauri.conf.json 写不出 pubkey。

- [ ] **Step 1.1：生成密钥对**

```
./node_modules/.bin/tauri signer generate -w ~/.tauri-attool-signer.key
```

> CLI 会提示输入口令（password），输入一个高熵密码并**记住**。
> 输出会打印 base64 公钥字符串（一行），形如 `dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6...`。

- [ ] **Step 1.2：确认私钥文件已存在**

```
ls -l ~/.tauri-attool-signer.key
```

预期：文件存在，大小约 200 字节。

- [ ] **Step 1.3：备份私钥 + 口令到密码管理器**

把以下两项**手动**录入 1Password / Bitwarden / Apple Keychain 任一：

- Item 名建议：`attool tauri updater signing key`
- 私钥文件**整文件内容**（`cat ~/.tauri-attool-signer.key`）
- Step 1.1 设的口令

> 丢失任一 = 已发布 attool 的所有用户从此无法收到更新（公钥已编入 binary）。

- [ ] **Step 1.4：把公钥字符串记下来**

```
cat ~/.tauri-attool-signer.key.pub
```

把输出（一行 base64）复制下来，Task 3 要写进 tauri.conf.json。

- [ ] **Step 1.5：把私钥与口令加到 GitHub Secrets**

去 `https://github.com/attson/attool/settings/secrets/actions` → "New repository secret"，添加两条：

| Name | Value |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | `cat ~/.tauri-attool-signer.key` 的整文件内容 |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Step 1.1 设的口令 |

> 这两个 secret 在 Task 9 的 CI 改动里会被引用。可以先不加，等 Task 9 之前补上即可。

---

## Task 2：装 Rust + JS 插件依赖

**Files:**

- Modify: `src-tauri/Cargo.toml`
- Modify: `package.json`

- [ ] **Step 2.1：加 Rust 依赖**

在 `src-tauri/Cargo.toml` 的 `[dependencies]` 区追加（与 `tauri-plugin-dialog` 相邻）：

```toml
tauri-plugin-updater = "2"
tauri-plugin-process = "2"
```

- [ ] **Step 2.2：加 JS 依赖**

```
npm install @tauri-apps/plugin-updater @tauri-apps/plugin-process
```

- [ ] **Step 2.3：跑一次构建确认编译通过**

```
npm run build
```

预期：tsc + vite build 通过，无 import 错误。

> 此时还没注册插件，运行时不影响。

- [ ] **Step 2.4：Commit**

```
git add src-tauri/Cargo.toml package.json package-lock.json
git commit -m "chore(deps): add tauri-plugin-updater + tauri-plugin-process"
```

---

## Task 3：配置 updater 插件

**Files:**

- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/capabilities/default.json`

- [ ] **Step 3.1：写 plugins.updater 配置**

打开 `src-tauri/tauri.conf.json`。当前最外层有 `productName` / `version` / `identifier` / `build` / `app` / `bundle`。在 `bundle` 后追加 `plugins`（注意逗号）：

```json
  "bundle": {
    "active": true,
    "targets": "all"
  },
  "plugins": {
    "updater": {
      "endpoints": [
        "https://github.com/attson/attool/releases/latest/download/latest.json"
      ],
      "pubkey": "<把 Task 1.4 复制的公钥字符串粘贴在这里>"
    }
  }
}
```

> 把 `<把 Task 1.4 复制的公钥字符串粘贴在这里>` 替换为完整的 base64 公钥字符串（一行，无引号外多余空格）。

- [ ] **Step 3.2：在 lib.rs 里注册插件**

打开 `src-tauri/src/lib.rs`，把：

```rust
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
```

替换为：

```rust
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
```

- [ ] **Step 3.3：在 capabilities/default.json 里授权**

把：

```json
"permissions": ["core:default", "dialog:default"]
```

替换为：

```json
"permissions": [
  "core:default",
  "dialog:default",
  "updater:default",
  "process:default"
]
```

- [ ] **Step 3.4：跑构建验证**

```
npm run tauri build --debug 2>&1 | tail -30
```

或简单跑：

```
cd src-tauri && cargo check
```

预期：`cargo check` 通过；schema 校验 tauri.conf.json 不报错。

- [ ] **Step 3.5：Commit**

```
git add src-tauri/tauri.conf.json src-tauri/src/lib.rs src-tauri/capabilities/default.json
git commit -m "feat(updater): register tauri-plugin-updater + tauri-plugin-process"
```

---

## Task 4：useUpdaterPrefs composable

**Files:**

- Create: `src/composables/useUpdaterPrefs.ts`
- Create: `src/composables/useUpdaterPrefs.test.ts`

- [ ] **Step 4.1：写失败测试**

`src/composables/useUpdaterPrefs.test.ts`：

```ts
import { describe, it, expect } from 'vitest';
import { useUpdaterPrefs } from './useUpdaterPrefs';
import type { KVStorage } from './useSidebarState';

function fakeStorage(initial: Record<string, string> = {}) {
  const data = new Map(Object.entries(initial));
  const storage: KVStorage = {
    getItem: (k) => (data.has(k) ? data.get(k)! : null),
    setItem: (k, v) => { data.set(k, v); }
  };
  return { storage, data };
}

describe('useUpdaterPrefs', () => {
  it('autoCheck defaults to true', () => {
    const { storage } = fakeStorage();
    const { autoCheck } = useUpdaterPrefs(storage);
    expect(autoCheck.value).toBe(true);
  });

  it('autoCheck restores false from storage', () => {
    const { storage } = fakeStorage({ 'attool.updater.autoCheck': '0' });
    const { autoCheck } = useUpdaterPrefs(storage);
    expect(autoCheck.value).toBe(false);
  });

  it('setAutoCheck flips and persists', () => {
    const { storage, data } = fakeStorage();
    const { autoCheck, setAutoCheck } = useUpdaterPrefs(storage);
    setAutoCheck(false);
    expect(autoCheck.value).toBe(false);
    expect(data.get('attool.updater.autoCheck')).toBe('0');
    setAutoCheck(true);
    expect(data.get('attool.updater.autoCheck')).toBe('1');
  });

  it('skippedVersion is null by default', () => {
    const { storage } = fakeStorage();
    const { skippedVersion } = useUpdaterPrefs(storage);
    expect(skippedVersion.value).toBeNull();
  });

  it('skipVersion persists value and shouldSkip returns true for it only', () => {
    const { storage, data } = fakeStorage();
    const { skipVersion, shouldSkip } = useUpdaterPrefs(storage);
    skipVersion('0.2.0');
    expect(shouldSkip('0.2.0')).toBe(true);
    expect(shouldSkip('0.3.0')).toBe(false);
    expect(data.get('attool.updater.skipped')).toBe('0.2.0');
  });
});
```

- [ ] **Step 4.2：跑测试确认失败**

```
npm test -- src/composables/useUpdaterPrefs
```

预期：`Cannot find module './useUpdaterPrefs'`。

- [ ] **Step 4.3：写实现**

`src/composables/useUpdaterPrefs.ts`：

```ts
import { ref } from 'vue';
import type { KVStorage } from './useSidebarState';

const KEY_AUTO_CHECK = 'attool.updater.autoCheck';
const KEY_SKIPPED = 'attool.updater.skipped';

export function useUpdaterPrefs(storage: KVStorage = localStorage) {
  const autoCheck = ref(storage.getItem(KEY_AUTO_CHECK) !== '0');
  const skippedVersion = ref<string | null>(storage.getItem(KEY_SKIPPED));

  function setAutoCheck(v: boolean) {
    autoCheck.value = v;
    storage.setItem(KEY_AUTO_CHECK, v ? '1' : '0');
  }

  function skipVersion(v: string) {
    skippedVersion.value = v;
    storage.setItem(KEY_SKIPPED, v);
  }

  function shouldSkip(v: string) {
    return skippedVersion.value === v;
  }

  return { autoCheck, setAutoCheck, skippedVersion, skipVersion, shouldSkip };
}
```

- [ ] **Step 4.4：跑测试通过**

```
npm test -- src/composables/useUpdaterPrefs
```

预期：5 个 PASS。

- [ ] **Step 4.5：Commit**

```
git add src/composables/useUpdaterPrefs.ts src/composables/useUpdaterPrefs.test.ts
git commit -m "feat(composable): useUpdaterPrefs (autoCheck + skippedVersion)"
```

---

## Task 5：useUpdater composable（状态机）

**Files:**

- Create: `src/composables/useUpdater.ts`
- Create: `src/composables/useUpdater.test.ts`

- [ ] **Step 5.1：写失败测试**

`src/composables/useUpdater.test.ts`：

```ts
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { useUpdater, type UpdaterClient, type UpdateInfo } from './useUpdater';

function makeClient(opts: {
  update?: { version: string; notes?: string };
  failCheck?: string;
  failInstall?: string;
}): UpdaterClient {
  const dl = vi.fn(async (cb: (e: { event: string; data?: any }) => void) => {
    if (opts.failInstall) throw new Error(opts.failInstall);
    cb({ event: 'Started', data: { contentLength: 100 } });
    cb({ event: 'Progress', data: { chunkLength: 50 } });
    cb({ event: 'Progress', data: { chunkLength: 50 } });
    cb({ event: 'Finished' });
  });
  return {
    check: vi.fn(async (): Promise<UpdateInfo | null> => {
      if (opts.failCheck) throw new Error(opts.failCheck);
      return opts.update
        ? { version: opts.update.version, notes: opts.update.notes, downloadAndInstall: dl }
        : null;
    }),
    relaunch: vi.fn(async () => {})
  };
}

describe('useUpdater', () => {
  beforeEach(() => { vi.useFakeTimers(); });
  afterEach(() => { vi.useRealTimers(); });

  it('idle by default', () => {
    const { state } = useUpdater(makeClient({}));
    expect(state.value.status).toBe('idle');
  });

  it('check transitions idle → checking → up-to-date when no update', async () => {
    const { state, check } = useUpdater(makeClient({}));
    const p = check();
    expect(state.value.status).toBe('checking');
    await p;
    expect(state.value.status).toBe('up-to-date');
  });

  it('up-to-date auto-reverts to idle after 3s', async () => {
    const { state, check } = useUpdater(makeClient({}));
    await check();
    expect(state.value.status).toBe('up-to-date');
    vi.advanceTimersByTime(3000);
    expect(state.value.status).toBe('idle');
  });

  it('check transitions to available when update found', async () => {
    const { state, check } = useUpdater(makeClient({ update: { version: '0.2.0', notes: 'fixes' } }));
    await check();
    expect(state.value.status).toBe('available');
    expect(state.value.available).toEqual({ version: '0.2.0', notes: 'fixes' });
  });

  it('check error transitions to error with message', async () => {
    const { state, check } = useUpdater(makeClient({ failCheck: 'network down' }));
    await check();
    expect(state.value.status).toBe('error');
    expect(state.value.error).toBe('network down');
  });

  it('records trigger from check argument', async () => {
    const { state, check } = useUpdater(makeClient({ update: { version: '0.2.0' } }));
    await check('manual');
    expect(state.value.trigger).toBe('manual');
  });

  it('install transitions available → downloading → ready', async () => {
    const { state, check, install } = useUpdater(makeClient({ update: { version: '0.2.0' } }));
    await check();
    const p = install();
    expect(state.value.status).toBe('downloading');
    await p;
    expect(state.value.status).toBe('ready');
  });

  it('install ends with 100% download percent', async () => {
    const { state, check, install } = useUpdater(makeClient({ update: { version: '0.2.0' } }));
    await check();
    await install();
    expect(state.value.downloadPercent).toBe(100);
  });

  it('install error transitions downloading → error', async () => {
    const { state, check, install } = useUpdater(makeClient({
      update: { version: '0.2.0' },
      failInstall: 'download interrupted'
    }));
    await check();
    await install();
    expect(state.value.status).toBe('error');
    expect(state.value.error).toBe('download interrupted');
  });

  it('relaunch calls client.relaunch', async () => {
    const client = makeClient({ update: { version: '0.2.0' } });
    const { relaunch } = useUpdater(client);
    await relaunch();
    expect(client.relaunch).toHaveBeenCalled();
  });

  it('dismiss resets to idle from any state', async () => {
    const { state, check, dismiss } = useUpdater(makeClient({ update: { version: '0.2.0' } }));
    await check();
    expect(state.value.status).toBe('available');
    dismiss();
    expect(state.value.status).toBe('idle');
    expect(state.value.available).toBeUndefined();
  });

  it('install without prior check is no-op (stays idle)', async () => {
    const { state, install } = useUpdater(makeClient({}));
    await install();
    expect(state.value.status).toBe('idle');
  });
});
```

- [ ] **Step 5.2：跑测试确认失败**

```
npm test -- src/composables/useUpdater
```

预期：`Cannot find module './useUpdater'`。

- [ ] **Step 5.3：写实现**

`src/composables/useUpdater.ts`：

```ts
import { ref } from 'vue';

export interface ProgressEvent {
  event: 'Started' | 'Progress' | 'Finished';
  data?: { contentLength?: number; chunkLength?: number };
}

export interface UpdateInfo {
  version: string;
  notes?: string;
  downloadAndInstall(onProgress: (e: ProgressEvent) => void): Promise<void>;
}

export interface UpdaterClient {
  check(): Promise<UpdateInfo | null>;
  relaunch(): Promise<void>;
}

export type Status =
  | 'idle' | 'checking' | 'up-to-date'
  | 'available' | 'downloading' | 'ready' | 'error';

export type Trigger = 'auto' | 'manual';

export interface UpdaterState {
  status: Status;
  trigger: Trigger;
  available?: { version: string; notes?: string };
  downloadPercent?: number;
  error?: string;
}

const UP_TO_DATE_REVERT_MS = 3000;

function defaultClient(): UpdaterClient {
  return {
    async check() {
      const { check } = await import('@tauri-apps/plugin-updater');
      const update = await check();
      if (!update) return null;
      return {
        version: update.version,
        notes: update.body ?? undefined,
        downloadAndInstall: (cb) => update.downloadAndInstall(cb as any)
      };
    },
    async relaunch() {
      const { relaunch } = await import('@tauri-apps/plugin-process');
      await relaunch();
    }
  };
}

export function useUpdater(client: UpdaterClient = defaultClient()) {
  const state = ref<UpdaterState>({ status: 'idle', trigger: 'manual' });
  let currentInfo: UpdateInfo | null = null;

  async function check(trigger: Trigger = 'manual') {
    state.value = { status: 'checking', trigger };
    try {
      const info = await client.check();
      if (!info) {
        currentInfo = null;
        state.value = { status: 'up-to-date', trigger };
        setTimeout(() => {
          if (state.value.status === 'up-to-date') {
            state.value = { status: 'idle', trigger };
          }
        }, UP_TO_DATE_REVERT_MS);
      } else {
        currentInfo = info;
        state.value = {
          status: 'available',
          trigger,
          available: { version: info.version, notes: info.notes }
        };
      }
    } catch (e) {
      currentInfo = null;
      state.value = { status: 'error', trigger, error: errorMessage(e) };
    }
  }

  async function install() {
    if (!currentInfo) return;
    const trigger = state.value.trigger;
    const available = state.value.available;
    state.value = { status: 'downloading', trigger, available, downloadPercent: 0 };
    let downloaded = 0;
    let total = 0;
    try {
      await currentInfo.downloadAndInstall((e) => {
        if (e.event === 'Started') {
          total = e.data?.contentLength ?? 0;
        } else if (e.event === 'Progress') {
          downloaded += e.data?.chunkLength ?? 0;
          const pct = total > 0 ? Math.min(100, Math.round((downloaded / total) * 100)) : 0;
          state.value = { ...state.value, downloadPercent: pct };
        } else if (e.event === 'Finished') {
          state.value = { ...state.value, downloadPercent: 100 };
        }
      });
      state.value = { status: 'ready', trigger, available, downloadPercent: 100 };
    } catch (e) {
      state.value = { status: 'error', trigger, available, error: errorMessage(e) };
    }
  }

  async function relaunch() {
    await client.relaunch();
  }

  function dismiss() {
    currentInfo = null;
    state.value = { status: 'idle', trigger: 'manual' };
  }

  return { state, check, install, relaunch, dismiss };
}

function errorMessage(e: unknown): string {
  if (e instanceof Error) return e.message;
  return String(e);
}
```

- [ ] **Step 5.4：跑测试通过**

```
npm test -- src/composables/useUpdater
```

预期：12 个测试全部 PASS。

> 若 progress 测试不稳定（异步时序问题），把 `await Promise.resolve()` 改为 `await vi.advanceTimersByTimeAsync(0)` 或加 `await new Promise(r => setImmediate(r))`。

- [ ] **Step 5.5：Commit**

```
git add src/composables/useUpdater.ts src/composables/useUpdater.test.ts
git commit -m "feat(composable): useUpdater state machine wrapping tauri updater"
```

---

## Task 6：SettingsIcon（齿轮 SVG）

**Files:**

- Create: `src/components/shell/SettingsIcon.vue`

- [ ] **Step 6.1：写组件**

`src/components/shell/SettingsIcon.vue`：

```vue
<script setup lang="ts">
defineProps<{ size?: number }>();
</script>

<template>
  <svg
    class="settings-icon"
    :width="size ?? 14"
    :height="size ?? 14"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <circle cx="12" cy="12" r="3" />
    <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
  </svg>
</template>

<style scoped>
.settings-icon { display: block; flex-shrink: 0; }
</style>
```

- [ ] **Step 6.2：Commit**

```
git add src/components/shell/SettingsIcon.vue
git commit -m "feat(shell): SettingsIcon (gear svg)"
```

---

## Task 7：Sidebar 加齿轮按钮 + AppShell 透传

**Files:**

- Modify: `src/components/shell/Sidebar.vue`
- Modify: `src/components/shell/AppShell.vue`

- [ ] **Step 7.1：Sidebar.vue 加 import 与 emits**

打开 `src/components/shell/Sidebar.vue`，把：

```ts
import BrandMark from './BrandMark.vue';
import ToolIcon from './ToolIcon.vue';
import Kbd from '../ui/Kbd.vue';
import type { Tool } from '../../types/tool';
```

替换为：

```ts
import BrandMark from './BrandMark.vue';
import ToolIcon from './ToolIcon.vue';
import SettingsIcon from './SettingsIcon.vue';
import Kbd from '../ui/Kbd.vue';
import type { Tool } from '../../types/tool';
```

把：

```ts
const emit = defineEmits<{
  select: [id: string];
  toggle: [];
  brand: [];
  search: [];
  themeToggle: [];
}>();
```

替换为：

```ts
const emit = defineEmits<{
  select: [id: string];
  toggle: [];
  brand: [];
  search: [];
  themeToggle: [];
  settingsToggle: [];
}>();
```

- [ ] **Step 7.2：Sidebar.vue 在 footer 插入齿轮按钮**

把现在 `<div class="foot">` 内的：

```html
      <button
        class="theme-toggle"
        type="button"
        :title="theme === 'dark' ? '切换到亮色' : '切换到暗色'"
        @click="emit('themeToggle')"
      >
```

前面加一个齿轮按钮，得到：

```html
    <div class="foot">
      <span class="ver">v0.1.0</span>
      <button
        class="settings-toggle"
        type="button"
        title="设置"
        @click="emit('settingsToggle')"
      >
        <SettingsIcon />
      </button>
      <button
        class="theme-toggle"
        type="button"
        :title="theme === 'dark' ? '切换到亮色' : '切换到暗色'"
        @click="emit('themeToggle')"
      >
```

剩余内容（svg + collapse toggle）保持不变。

- [ ] **Step 7.3：Sidebar.vue 加齿轮按钮的样式**

在 `<style scoped>` 区找到：

```css
.toggle,
.theme-toggle {
```

替换为：

```css
.toggle,
.theme-toggle,
.settings-toggle {
```

并把：

```css
.toggle:hover,
.theme-toggle:hover { color: var(--text); }
.sidebar.collapsed .theme-toggle { display: none; }
```

替换为：

```css
.toggle:hover,
.theme-toggle:hover,
.settings-toggle:hover { color: var(--text); }
.sidebar.collapsed .theme-toggle,
.sidebar.collapsed .settings-toggle { display: none; }
```

- [ ] **Step 7.4：AppShell.vue 透传 settings-toggle**

打开 `src/components/shell/AppShell.vue`，把：

```ts
const emit = defineEmits<{
  select: [id: string];
  toggle: [];
  brand: [];
  search: [];
  themeToggle: [];
}>();
```

替换为：

```ts
const emit = defineEmits<{
  select: [id: string];
  toggle: [];
  brand: [];
  search: [];
  themeToggle: [];
  settingsToggle: [];
}>();
```

把模板里：

```html
    <Sidebar
      :tools="tools"
      :active-id="activeId"
      :collapsed="collapsed"
      :theme="theme"
      @select="(id) => emit('select', id)"
      @toggle="emit('toggle')"
      @brand="emit('brand')"
      @search="emit('search')"
      @theme-toggle="emit('themeToggle')"
    />
```

替换为：

```html
    <Sidebar
      :tools="tools"
      :active-id="activeId"
      :collapsed="collapsed"
      :theme="theme"
      @select="(id) => emit('select', id)"
      @toggle="emit('toggle')"
      @brand="emit('brand')"
      @search="emit('search')"
      @theme-toggle="emit('themeToggle')"
      @settings-toggle="emit('settingsToggle')"
    />
```

- [ ] **Step 7.5：跑构建验证**

```
npm run build
```

预期：tsc + vite 通过；齿轮按钮事件透传链通。

- [ ] **Step 7.6：Commit**

```
git add src/components/shell/Sidebar.vue src/components/shell/AppShell.vue
git commit -m "feat(shell): sidebar gear icon + AppShell pass-through for settings"
```

---

## Task 8：UpdateBanner 组件

**Files:**

- Create: `src/components/shell/UpdateBanner.vue`

- [ ] **Step 8.1：写组件**

`src/components/shell/UpdateBanner.vue`：

```vue
<script setup lang="ts">
import { computed } from 'vue';
import type { UpdaterState } from '../../composables/useUpdater';

const props = defineProps<{ state: UpdaterState }>();

const emit = defineEmits<{
  install: [];
  skip: [];
  relaunch: [];
  dismiss: [];
}>();

const visible = computed(() => {
  const s = props.state.status;
  if (s === 'available' || s === 'downloading' || s === 'ready') return true;
  if (s === 'error' && props.state.trigger === 'manual') return true;
  return false;
});

const versionText = computed(() => props.state.available?.version ?? '');
</script>

<template>
  <div v-if="visible" class="update-banner" :data-state="state.status">
    <span class="msg">
      <template v-if="state.status === 'available'">
        新版本 <strong>v{{ versionText }}</strong> 可用
      </template>
      <template v-else-if="state.status === 'downloading'">
        正在下载 v{{ versionText }}... {{ state.downloadPercent ?? 0 }}%
      </template>
      <template v-else-if="state.status === 'ready'">
        下载完成，重启以应用更新
      </template>
      <template v-else-if="state.status === 'error'">
        更新失败：{{ state.error }}
      </template>
    </span>

    <div class="actions">
      <template v-if="state.status === 'available'">
        <button class="btn primary" type="button" @click="emit('install')">现在安装</button>
        <button class="btn ghost" type="button" @click="emit('skip')">稍后</button>
      </template>
      <template v-else-if="state.status === 'ready'">
        <button class="btn primary" type="button" @click="emit('relaunch')">立即重启</button>
        <button class="btn ghost" type="button" @click="emit('dismiss')">稍后重启</button>
      </template>
      <template v-else-if="state.status === 'error'">
        <button class="btn ghost" type="button" @click="emit('dismiss')">关闭</button>
      </template>
    </div>

    <div v-if="state.status === 'downloading'" class="progress">
      <i :style="{ width: `${state.downloadPercent ?? 0}%` }"></i>
    </div>
  </div>
</template>

<style scoped>
.update-banner {
  position: relative;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 6px 22px;
  background: var(--accent-soft);
  color: var(--text);
  border-bottom: 1px solid var(--line);
  font-size: var(--fs-xs);
  min-height: 32px;
}
.update-banner[data-state="error"] {
  background: color-mix(in srgb, var(--error) 18%, transparent);
  color: var(--error);
}

.msg { flex: 1; min-width: 0; }
.msg strong { font-weight: 600; }

.actions { display: flex; gap: 6px; flex-shrink: 0; }

.btn {
  padding: 3px 10px;
  border-radius: var(--radius);
  border: 0;
  font-size: var(--fs-xxs);
  font-weight: 500;
  cursor: pointer;
  line-height: 1.4;
}
.btn.primary { background: var(--accent); color: var(--accent-fg); }
.btn.ghost {
  background: transparent;
  color: inherit;
  border: 1px solid var(--line-strong);
}
.btn.primary:hover { filter: brightness(1.05); }
.btn.ghost:hover { border-color: var(--text-muted); }

.progress {
  position: absolute;
  left: 0; right: 0; bottom: 0;
  height: 3px;
  background: transparent;
  overflow: hidden;
}
.progress > i {
  display: block;
  height: 100%;
  background: var(--accent);
  transition: width var(--motion-fast);
}
</style>
```

- [ ] **Step 8.2：Commit**

```
git add src/components/shell/UpdateBanner.vue
git commit -m "feat(shell): UpdateBanner with 4-state UI"
```

---

## Task 9：SettingsModal 组件

**Files:**

- Create: `src/components/shell/SettingsModal.vue`

- [ ] **Step 9.1：写组件**

`src/components/shell/SettingsModal.vue`：

```vue
<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { NModal, NButton } from 'naive-ui';
import { getVersion } from '@tauri-apps/api/app';
import type { UpdaterState } from '../../composables/useUpdater';

const props = defineProps<{
  show: boolean;
  state: UpdaterState;
  autoCheck: boolean;
}>();

const emit = defineEmits<{
  'update:show': [v: boolean];
  check: [];
  'update:autoCheck': [v: boolean];
}>();

const currentVersion = ref<string>('');

onMounted(async () => {
  try {
    currentVersion.value = await getVersion();
  } catch {
    currentVersion.value = 'unknown';
  }
});

const latestText = computed(() => {
  switch (props.state.status) {
    case 'idle': return '未检查';
    case 'checking': return '检查中...';
    case 'up-to-date': return '已是最新版本';
    case 'available': return `v${props.state.available!.version}`;
    case 'downloading': return `下载中 ${props.state.downloadPercent ?? 0}%`;
    case 'ready': return '已下载，待重启';
    case 'error': return '检查失败';
  }
});

const checking = computed(() =>
  props.state.status === 'checking' || props.state.status === 'downloading'
);

const showProxy = computed({
  get: () => props.show,
  set: (v) => emit('update:show', v)
});
</script>

<template>
  <n-modal v-model:show="showProxy" preset="card" title="设置" style="max-width: 480px">
    <section class="block">
      <h3>软件更新</h3>
      <div class="row">
        <span class="key">当前版本</span>
        <span class="val mono">v{{ currentVersion }}</span>
      </div>
      <div class="row">
        <span class="key">最新版本</span>
        <span class="val">{{ latestText }}</span>
      </div>
      <div class="actions">
        <n-button :loading="checking" @click="emit('check')">立即检查更新</n-button>
      </div>
      <label class="toggle-row">
        <input
          type="checkbox"
          :checked="autoCheck"
          @change="(e) => emit('update:autoCheck', (e.target as HTMLInputElement).checked)"
        />
        <span>启动时自动检查更新</span>
      </label>
    </section>
  </n-modal>
</template>

<style scoped>
.block { display: grid; gap: 10px; }
.block h3 {
  margin: 0 0 6px;
  font-size: var(--fs-xxs);
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.08em;
  font-weight: 600;
}
.row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: var(--fs-sm);
  padding: 4px 0;
  border-bottom: 1px solid var(--line);
}
.row .key { color: var(--text-muted); }
.row .val { color: var(--text); }
.row .val.mono { font-family: var(--font-mono); }

.actions { padding-top: 4px; }

.toggle-row {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: var(--fs-sm);
  cursor: pointer;
  padding-top: 6px;
}
.toggle-row input { cursor: pointer; }
</style>
```

- [ ] **Step 9.2：跑构建验证**

```
npm run build
```

预期：tsc + vite 通过。

- [ ] **Step 9.3：Commit**

```
git add src/components/shell/SettingsModal.vue
git commit -m "feat(shell): SettingsModal with version + auto-check toggle"
```

---

## Task 10：App.vue 集成

**Files:**

- Modify: `src/App.vue`
- Modify: `src/components/shell/AppShell.vue`

> AppShell 还需要承载 `<UpdateBanner>` 槽位（topbar 与 content 之间）。这一步先把 AppShell 的 banner 槽加上，再在 App.vue 里组合所有部件。

- [ ] **Step 10.1：AppShell 加 banner 槽**

打开 `src/components/shell/AppShell.vue`，找到模板：

```html
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
```

替换为：

```html
    <main class="main">
      <Topbar :crumb="crumb">
        <template #right>
          <slot name="topbar-right" />
        </template>
      </Topbar>
      <slot name="banner" />
      <div class="content">
        <slot />
      </div>
    </main>
```

- [ ] **Step 10.2：App.vue 加 import**

打开 `src/App.vue`，在已有的 import 区追加：

```ts
import UpdateBanner from './components/shell/UpdateBanner.vue';
import SettingsModal from './components/shell/SettingsModal.vue';
import { useUpdater } from './composables/useUpdater';
import { useUpdaterPrefs } from './composables/useUpdaterPrefs';
```

- [ ] **Step 10.3：App.vue 加 composable 状态**

在已有的 `const { theme, toggle: toggleTheme } = useTheme();` 行下面追加：

```ts
const { state: updaterState, check: updaterCheck, install: updaterInstall, relaunch: updaterRelaunch, dismiss: updaterDismiss } = useUpdater();
const { autoCheck: updaterAutoCheck, setAutoCheck: updaterSetAutoCheck, skipVersion: updaterSkipVersion, shouldSkip: updaterShouldSkip } = useUpdaterPrefs();
const settingsOpen = ref(false);
```

- [ ] **Step 10.4：App.vue 在 onMounted 末尾追加自动检查**

定位到现有 `onMounted(() => { ... });`（block 末尾在 `window.addEventListener('keydown', handleHotkey);` 之后）。在 `window.addEventListener('keydown', handleHotkey);` 这一行之后追加：

```ts
  if (updaterAutoCheck.value) {
    setTimeout(async () => {
      await updaterCheck('auto');
      if (updaterState.value.status === 'available' &&
          updaterShouldSkip(updaterState.value.available!.version)) {
        updaterDismiss();
      }
    }, 5000);
  }
```

- [ ] **Step 10.5：App.vue 加 banner / modal 处理函数**

在 `function openSearch() { ... }` 函数之后追加：

```ts
function handleInstall() {
  updaterInstall();
}
function handleSkip() {
  if (updaterState.value.available) {
    updaterSkipVersion(updaterState.value.available.version);
  }
  updaterDismiss();
}
function handleRelaunch() {
  updaterRelaunch();
}
function handleDismiss() {
  updaterDismiss();
}
function openSettings() {
  settingsOpen.value = true;
}
function handleSettingsCheck() {
  updaterCheck('manual');
}
```

- [ ] **Step 10.6：App.vue 模板加 settings-toggle 接线 + banner + modal**

把模板里：

```html
      <AppShell
        :tools="tools"
        :active-id="selectedToolId"
        :collapsed="sidebarCollapsed"
        :crumb="selectedTool?.name"
        :theme="theme"
        @select="selectTool"
        @toggle="toggleSidebar"
        @brand="goHome"
        @search="openSearch"
        @theme-toggle="toggleTheme"
      >
```

替换为：

```html
      <AppShell
        :tools="tools"
        :active-id="selectedToolId"
        :collapsed="sidebarCollapsed"
        :crumb="selectedTool?.name"
        :theme="theme"
        @select="selectTool"
        @toggle="toggleSidebar"
        @brand="goHome"
        @search="openSearch"
        @theme-toggle="toggleTheme"
        @settings-toggle="openSettings"
      >
        <template #banner>
          <UpdateBanner
            :state="updaterState"
            @install="handleInstall"
            @skip="handleSkip"
            @relaunch="handleRelaunch"
            @dismiss="handleDismiss"
          />
        </template>
```

> 注意：`<template #banner>` 是新加的具名插槽，与 `<template #topbar-right>` 平级，紧跟在它后面。

把模板末尾 `</n-message-provider>` 之前追加 modal：

```html
      <SettingsModal
        v-model:show="settingsOpen"
        :state="updaterState"
        :auto-check="updaterAutoCheck"
        @check="handleSettingsCheck"
        @update:auto-check="updaterSetAutoCheck"
      />
```

- [ ] **Step 10.7：跑测试 + 构建**

```
npm test
npm run build
```

预期：所有 vitest PASS（35 个）；build 通过无 TypeScript 错误。

- [ ] **Step 10.8：dev 模式快速人工验证**

```
npm run tauri:dev
```

人工核对：

- 启动后 sidebar 底栏多了齿轮按钮（在版本号 v0.1.0 与太阳/月亮之间）
- 点齿轮 → 设置 modal 弹出，显示当前版本 v0.1.0、最新版本"未检查"、"立即检查更新"按钮、"启动时自动检查更新" 复选框（默认勾选）
- 点"立即检查更新"按钮 → 文案变"检查中..."；如果本地没有 release（首次），结果应显示"已是最新版本"或"检查失败"
- sidebar 折叠后齿轮按钮也跟着隐藏（与太阳按钮一致）

> dev 模式 Tauri updater 是禁用的，因此 check 通常会以 "no update available" 或类似错误结束，这是预期行为。

- [ ] **Step 10.9：Commit**

```
git add src/App.vue src/components/shell/AppShell.vue
git commit -m "feat(updater): wire useUpdater + banner + settings into App.vue"
```

---

## Task 11：CI 改动 — 注入签名密钥 + 生成 latest.json

**Files:**

- Modify: `.github/workflows/build.yml`

- [ ] **Step 11.1：把 env 提到 job 级别**

打开 `.github/workflows/build.yml`，找到：

```yaml
  build:
    name: ${{ matrix.label }}
    strategy:
```

在 `runs-on: ${{ matrix.os }}` 之后、`steps:` 之前加 `env:` 块：

```yaml
    runs-on: ${{ matrix.os }}
    env:
      TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
      TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}

    steps:
```

- [ ] **Step 11.2：在两个 build step 上启用 updaterJson**

找到当前 "Build (release on tag)" step：

```yaml
      - name: Build (release on tag)
        if: startsWith(github.ref, 'refs/tags/v')
        uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          args: --target ${{ matrix.target }}
          tagName: ${{ github.ref_name }}
          releaseName: 'AT Tool ${{ github.ref_name }}'
          releaseDraft: true
          prerelease: false
```

替换为：

```yaml
      - name: Build (release on tag)
        if: startsWith(github.ref, 'refs/tags/v')
        uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          args: --target ${{ matrix.target }}
          tagName: ${{ github.ref_name }}
          releaseName: 'AT Tool ${{ github.ref_name }}'
          releaseDraft: true
          prerelease: false
          includeUpdaterJson: true
          updaterJsonPreferNsis: true
```

> 手动 dispatch step 不需要 `includeUpdaterJson`（不创建 release，没人拉 latest.json），但因为 job 级 env 已经注入，Rust 编译还是会用密钥签包，可下载 artifact 测试签名链路。

- [ ] **Step 11.3：Commit**

```
git add .github/workflows/build.yml
git commit -m "ci(updater): inject TAURI_SIGNING_PRIVATE_KEY + emit latest.json"
```

---

## Task 12：文档更新

**Files:**

- Modify: `AGENTS.md`
- Modify: `docs/spec/architecture.md`
- Modify: `docs/spec/overview.md`

- [ ] **Step 12.1：AGENTS.md 加 updater 章节**

在 `## 已知遗留` 之前插入：

```md
## 软件更新

- 走 Tauri 官方 `tauri-plugin-updater`（v2）+ `tauri-plugin-process`，配置在 `src-tauri/tauri.conf.json` 的 `plugins.updater`，endpoint 指向 GitHub Releases 的 `latest.json`
- 启动 5s 后若 `attool.updater.autoCheck=1` 自动检查；发现新版在 topbar 下方显示 banner，用户点 "现在安装" 触发下载 + 安装
- 设置入口：sidebar 底栏齿轮按钮 → SettingsModal
- 签名密钥（**必须妥善备份**）：本地 `tauri signer generate` 出私钥 + 公钥；公钥写入 `tauri.conf.json`；私钥 + 口令存 GitHub Secrets `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`，CI 在 tag push 时自动签名 + 上传 latest.json
- 状态机：`idle → checking → {up-to-date | available | error}`；`available → downloading → ready → relaunch`
```

- [ ] **Step 12.2：docs/spec/architecture.md 更新**

打开 `docs/spec/architecture.md`，在 "## 关键模块" 章节之前（或恰当位置）加一节：

```md
## 软件更新

- Rust 端：`tauri-plugin-updater` + `tauri-plugin-process`，在 `lib.rs` 的 `run()` 里注册
- 配置：`tauri.conf.json plugins.updater.endpoints` + `pubkey`
- 前端：`src/composables/useUpdater.ts`（状态机，注入式 client）+ `useUpdaterPrefs.ts`（autoCheck / skippedVersion 持久化）
- UI：`UpdateBanner.vue`（topbar 下方，4 状态）+ `SettingsModal.vue`（sidebar 齿轮触发）
- CI：`.github/workflows/build.yml` 在 tag push 时通过 `tauri-action` 的 `includeUpdaterJson: true` + secrets 注入私钥，自动生成签名后的 `latest.json` 上传到 GitHub Release
```

- [ ] **Step 12.3：docs/spec/overview.md 加运行时依赖**

在 `## 运行时依赖` 章节末尾追加：

```md
- 网络访问 GitHub Releases endpoint（`releases/latest/download/latest.json`）—— 仅在自动 / 手动检查更新时使用，可在 Settings 中关闭"启动时自动检查更新"
```

- [ ] **Step 12.4：Commit**

```
git add AGENTS.md docs/spec/architecture.md docs/spec/overview.md
git commit -m "docs: add updater section to AGENTS + spec"
```

---

## Task 13：最终验证

**Files:** 无

- [ ] **Step 13.1：跑全量测试**

```
npm test
```

预期：所有测试 PASS（原 30 + 新 5 + 12 = 47）。

- [ ] **Step 13.2：跑构建**

```
npm run build
cd src-tauri && cargo check && cd ..
```

预期：tsc 通过、vite 通过、cargo check 通过。

- [ ] **Step 13.3：dev 模式人工冒烟**

```
npm run tauri:dev
```

人工核对：

- 启动后 sidebar 底栏齿轮可点
- 设置 modal 弹出 + 当前版本号正确
- "立即检查更新" 在 dev 模式下立即返回（dev 模式 updater 禁用）—— 文案显示"已是最新"或"检查失败"
- sidebar 折叠后齿轮隐藏，与太阳/月亮一致
- 关闭"启动时自动检查更新" → 关闭重启 dev → 5s 后 banner 不出现
- 修改 `localStorage.attool.updater.autoCheck=1` 重启 dev → 5s 后会触发一次 check（dev 模式无效但 state 应短暂进 checking → up-to-date）

- [ ] **Step 13.4：（可选）打 v0.1.1 tag 走端到端**

> 这步需要 Task 1.5 的 GitHub Secrets 已经配好。如果还没配就先跳过，本地验完功能后再补。

把 `package.json`、`src-tauri/Cargo.toml` 和 `src-tauri/tauri.conf.json` 的版本号都改成 `0.1.1`，commit、tag、push：

```bash
# 修改三处版本号到 0.1.1
git add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json
git commit -m "chore: bump version to 0.1.1"
git push
git tag -a v0.1.1 -m "v0.1.1 — first auto-update test"
git push origin v0.1.1
```

去 https://github.com/attson/attool/actions 看 workflow 跑完，确认 release 草稿包含 `latest.json` + 5 平台的 .sig 签名文件。

打开 release 草稿 → publish。

回到本地装着 v0.1.0 的桌面应用 → 启动 → 5s 后应看到"新版本 v0.1.1 可用"banner → 点"现在安装" → 走完下载 + 安装 + 重启 → 验版本号 = v0.1.1。

如果有任何一步失败：
- "签名校验失败" → 检查 GitHub Secrets 里的私钥内容是否完整（包括 BEGIN/END 行 + 注释行）
- "无法连接 endpoint" → 把 release 从草稿改为 published；GitHub CDN 缓存 ~5 分钟
- Linux deb 安装弹密码框 → 正常，输入即可

---

## Self-Review Notes

下面四类问题在本计划中均已避免：

1. **占位符 / TODO** —— 仅 Task 3 的 `<把 Task 1.4 复制的公钥字符串粘贴在这里>` 是必要的人工填入位（无法预先知道值），其余步骤均含具体代码或精确命令。
2. **类型一致性** —— `UpdaterState` / `Status` / `Trigger` / `UpdateInfo` / `UpdaterClient` 在 useUpdater.ts、useUpdater.test.ts、UpdateBanner.vue、SettingsModal.vue、App.vue 的引用处签名一致；composable 返回字段（`state` / `check` / `install` / `relaunch` / `dismiss`）在 App.vue 解构使用一致。
3. **Spec 覆盖** —— spec § 9 的 10 条验收每条都对应至少一步 task：CI 改动（Task 11）、签名流程（Task 1）、auto-check 时序（Task 10.4）、关闭自动检查不发请求（Task 4 + Task 10.4 的 if）、banner 状态切换（Task 8）、稍后 + skipped 行为（Task 5 + Task 10.5）、设置文案（Task 9）、离线 manual error 显示（Task 8 visible 计算）、端到端流程（Task 13.4）、composable 测试（Task 4 / Task 5）。
4. **风险（spec § 10）** —— 第一个签名版本无法自升的注意事项已写进 Task 13.4；endpoint CDN 缓存的注意事项也写进了 Task 13.4 的 troubleshooting；密钥丢失风险在 Task 1.3 + AGENTS.md 文档中重复强调。
