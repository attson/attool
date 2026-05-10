# 软件更新（auto updater）· 设计稿

**日期：** 2026-05-10
**状态：** 设计已批准，待写实施计划
**范围：** Tauri updater 插件接入 + 签名密钥流程 + CI 改造 + 前端 composable + UI（banner + settings modal）

---

## 1. 目标与背景

attool 当前已通过 GitHub Actions 在 5 平台出 release 包，但用户（首期 = 项目作者本人）需要手动到 Releases 页面下载安装新版。要求：

- 应用启动时自动检测新版本
- 发现新版后在 UI 显眼处提示，用户一键下载 + 安装
- 设置项里可关闭"启动时自动检查"，也可手动触发"立即检查"

## 2. 关键决策（已锁定）

| # | 决定 | 选择 | 关键原因 |
|---|---|---|---|
| 1 | 实现路径 | Tauri 官方 `tauri-plugin-updater` v2 | 应用内一键安装、签名校验内置 |
| 2 | 更新源 | GitHub Releases，单 endpoint 指向 `releases/latest/download/latest.json` | 已有 CI 出包，零新基础设施 |
| 3 | 签名密钥管理 | 用户本地生成、手动贴 GitHub Secrets | 私钥不入仓、用户掌控备份 |
| 4 | 自动更新档位 | "自动检查 + banner 提示，用户点击触发下载安装" | 不打扰当前手头活，最小惊吓 |
| 5 | 检查频率 | 启动后 5s 延迟单次；用户可随时在 Settings 手动触发 | 启动检查足够；定时轮询过度 |
| 6 | 设置 UI 入口 | sidebar 底栏的齿轮图标 → modal | 与现有 theme toggle 同一层级 |
| 7 | 多通道 | **不做**（仅 stable） | YAGNI |
| 8 | 后台静默自动安装 | **不做** | 用户已选"检查 + 提示"档位 |

## 3. 架构

```
┌─ 后端（Rust）──────────────────┐
│ tauri-plugin-updater v2        │
│ ↳ 暴露内置 commands             │
│ tauri.conf.json plugins.updater│
│ ↳ endpoints + pubkey           │
└────────────────────────────────┘
            ↕ invoke
┌─ 前端 ─────────────────────────┐
│ composables/                   │
│   useUpdaterPrefs.ts           │  # autoCheck / skippedVersion
│   useUpdater.ts                │  # 状态机 + check / install
│ components/shell/              │
│   UpdateBanner.vue             │  # topbar 下嵌；状态驱动渲染
│   SettingsModal.vue            │  # 齿轮触发；版本 + 开关 + 立即检查
│ App.vue                        │  # 启动 5s 后自动检查（若开启）
│                                │  # 拥有 useUpdater state；事件下传
└────────────────────────────────┘
            ↕
┌─ CI（扩展 build.yml）──────────┐
│ env: TAURI_SIGNING_PRIVATE_KEY │
│      TAURI_SIGNING_PRIVATE_KEY_│
│      PASSWORD                  │
│ tauri-action with              │
│   includeUpdaterJson: true     │
│   updaterJsonPreferNsis: true  │
│ → release 资产含 latest.json   │
└────────────────────────────────┘
```

## 4. 签名密钥流程

### 4.1 一次性密钥生成（用户本地）

```bash
./node_modules/.bin/tauri signer generate -w ~/.tauri-attool-signer.key
```

输出私钥文件 + 公钥字符串。私钥文件 + 用户设的口令必须备份到密码管理器；丢失 = 已发布版本的所有用户无法继续接收更新。

### 4.2 GitHub Secrets

仓库 Settings → Secrets and variables → Actions：

| Secret | 内容 |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | 私钥文件**整文件内容** |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 用户设的口令 |

### 4.3 `tauri.conf.json` 增加

```json
"plugins": {
  "updater": {
    "endpoints": [
      "https://github.com/attson/attool/releases/latest/download/latest.json"
    ],
    "pubkey": "<bs64 公钥>"
  }
}
```

### 4.4 `.github/workflows/build.yml` 改动

`tauri-action` step 加 env + 两个 with 参数：

```yaml
env:
  TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
with:
  includeUpdaterJson: true
  updaterJsonPreferNsis: true
```

env 提到 job 级别（两个 build step 共享），否则 manual dispatch 跑也会因为 `tauri build` 阶段缺密钥失败。

`tauri-action` 在矩阵全部跑完后会**合并 5 个平台的 latest.json** 上传到 release（每平台一个 entry，含下载 URL + 签名）。

## 5. 状态机与 composables

### 5.1 useUpdaterPrefs

```ts
export interface UpdaterPrefs {
  autoCheck: Ref<boolean>;
  setAutoCheck(v: boolean): void;
  skippedVersion: Ref<string | null>;
  skipVersion(v: string): void;
  shouldSkip(v: string): boolean;
}

export function useUpdaterPrefs(storage: KVStorage = localStorage): UpdaterPrefs;
```

存储键：

| Key | 值 | 默认 |
|---|---|---|
| `attool.updater.autoCheck` | `'1' \| '0'` | `'1'` |
| `attool.updater.skipped` | 版本字符串如 `'0.2.0'` | 无 |

`skipped` 仅记最近一次"稍后"的版本号；新版本号到来时盖写。

### 5.2 useUpdater

抽象 Tauri updater 为接口，便于注入 fake：

```ts
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

export interface UpdaterState {
  status: Status;
  trigger: 'auto' | 'manual';            // 用于 banner 显示策略
  available?: { version: string; notes?: string };
  downloadPercent?: number;
  error?: string;
}

export function useUpdater(client?: UpdaterClient): {
  state: Ref<UpdaterState>;
  check(trigger?: 'auto' | 'manual'): Promise<void>;
  install(): Promise<void>;
  relaunch(): Promise<void>;
  dismiss(): void;
};
```

状态转移：

```
       check()                           install()
idle ─────────► checking ─┬─► up-to-date          ─► (3s) ─► idle
                          │
                          ├─► available ──────────► downloading
                          │      │       (skip)             │
                          │      └─────► idle               │
                          │                            (progress)
                          └─► error                         ▼
                                                            ready ──► (relaunch())
```

`up-to-date` 3 秒后自动回 `idle`，避免长留。

默认 client 实现包：
- `@tauri-apps/plugin-updater` 的 `check()`
- 返回的 Update 对象的 `downloadAndInstall(cb)`
- `@tauri-apps/plugin-process` 的 `relaunch()`

### 5.3 启动调度（在 App.vue）

```ts
const { state, check, install, relaunch, dismiss } = useUpdater();
const { autoCheck, shouldSkip } = useUpdaterPrefs();

onMounted(() => {
  if (!autoCheck.value) return;
  setTimeout(async () => {
    await check('auto');
    if (state.value.status === 'available' &&
        shouldSkip(state.value.available!.version)) {
      dismiss();
    }
  }, 5000);
});
```

5 秒延迟避开启动期 UI 抖动。

## 6. UI

### 6.1 入口：sidebar 底栏齿轮

```
[v0.1.0]  [⚙]  [☀/🌙]  [‹]
```

折叠态（56px）三个按钮全部隐藏，与 theme toggle 现有行为一致。`Sidebar.vue` 接受 `themeToggle` 一样接受 `settingsToggle` 事件，App.vue 拥有 modal open 状态。

### 6.2 SettingsModal.vue

Naive `<n-modal preset="card" title="设置">`，宽 480px。当前只放"软件更新"块，预留扩展。

```
┌─ 设置 ──────────────────────────── ✕ ┐
│  软件更新                             │
│  当前版本    v0.1.0                   │
│  最新版本    v0.2.0  /  已是最新     │
│              检查中... / 检查失败     │
│                                       │
│  [ 立即检查更新 ]                     │
│                                       │
│  ☑ 启动时自动检查更新                 │
└───────────────────────────────────────┘
```

行为：

| 元素 | 来源 / 行为 |
|---|---|
| 当前版本 | `@tauri-apps/api/app` 的 `getVersion()` |
| 最新版本文案 | 由 `state.status` 派生（idle → "未检查"；checking → "检查中..."；up-to-date → "已是最新版本"；available → `v{version}`；error → "检查失败"） |
| "立即检查更新" 按钮 | `check('manual')`，loading = `state.status === 'checking'` |
| 自动检查复选框 | `v-model="autoCheck"`，调 `setAutoCheck` |

### 6.3 UpdateBanner.vue

挂在 `AppShell.vue` 的 topbar 与 content 之间，仅在 `state.status ∈ {available, downloading, ready, error}` 时渲染。`error` 状态额外要求 `state.trigger === 'manual'`（避免离线启动时打扰）。

约 32px 高，全宽，emerald-soft 底色。

```
┌─────────────────────────────────────────────────┐
│ 工具 / Aria2 下载              [pill][pill]     │  ← topbar
├─────────────────────────────────────────────────┤
│ ✦ 新版本 v0.2.0 可用    [现在安装] [稍后]   ✕  │  ← UpdateBanner
└─────────────────────────────────────────────────┘
```

四种状态：

| status | 文案 | 按钮 |
|---|---|---|
| `available` | `新版本 v{version} 可用` | `[现在安装]` `[稍后]` |
| `downloading` | `正在下载 v{version}... {percent}%`（底部 3px progress） | — |
| `ready` | `下载完成，重启以应用更新` | `[立即重启]` `[稍后重启]` |
| `error` | `更新失败：{error}` | `[关闭]` |

按钮事件 emit 给 App.vue：

| 按钮 | 处理 |
|---|---|
| 现在安装 | `install()` |
| 稍后 | `skipVersion(version)` + `dismiss()` |
| 立即重启 | `relaunch()` |
| 稍后重启 | `dismiss()`（已下载文件 Tauri 自留；下次 check 仍可触发 ready） |
| 关闭（error） | `dismiss()` |

### 6.4 配色

- 默认状态：底 `var(--accent-soft)`，文字 `var(--text)`，主按钮 `var(--accent)`
- error 状态：底 `color-mix(in srgb, var(--error) 18%, transparent)`，文字 `var(--error)`
- 进度条：3px 高，emerald 填充，跟 TaskRow 同款

## 7. 持久化与边界

### 7.1 持久化

| Key | 写入时机 |
|---|---|
| `attool.updater.autoCheck` | settings 复选框切换 |
| `attool.updater.skipped` | banner "稍后" |
| Tauri 内部下载缓存 | Tauri 自管理 |

### 7.2 错误分类

| 错误源 | status | 用户感知 |
|---|---|---|
| 网络不通 / endpoint 404 | `error` | trigger=auto: 沉默；trigger=manual: banner / modal 显示 |
| 签名校验失败 | `error` | banner "更新包签名校验失败" |
| 下载中网络断 | `error` | banner "下载中断，请稍后重试" |
| Linux deb 安装权限不足 | `error` | banner 显示原始错误 |
| relaunch 失败 | `error` | 提示用户手动重启 |

### 7.3 边界情形

- **没有任何 release**：endpoint 404，`check()` resolve 为 `null` → `up-to-date`（不当作错误）
- **当前版本 ≥ 最新**：`check()` resolve 为 `null` → `up-to-date`
- **launch 时已下载好但被杀进程**：Tauri 缓存的 staged 文件失效，下次 check 重新下载
- **跨大版本 downgrade**：Tauri 默认拒绝，无需额外处理
- **同 session 多次 check**：允许；状态机直接 reset 走新一轮
- **Tauri dev 模式**：updater 自动禁用，composable 不需特殊处理

## 8. 不在本次范围（Non-goals）

- 多通道（beta / stable / nightly）
- 增量补丁（delta updates）
- 应用内回滚 / 旧版本切换 UI
- 暂停 / 恢复下载（Tauri plugin 不支持）
- 后台定时轮询（仅启动检查 + 手动）
- 桌面 OS-level 通知（banner 已够显眼）
- "下次启动自动安装"档位（被决策 § 2.4 排除）
- macOS / Windows OS 级代码签名（与 updater 签名独立，本次不做）

## 9. 验收标准

实施完成后下面每条都为真：

- [ ] 用户本地能 `npm run tauri:build` 出包，且打包不要求 GitHub Secrets（缺 secrets 时本地构建仍走，但产物不带签名）
- [ ] CI 在打 tag `v*` 时自动签名 + 生成 latest.json + 上传到 release
- [ ] App 启动 5s 后自动 check（`autoCheck=true` 时）
- [ ] 关闭"自动检查"后下次启动不发 check 请求
- [ ] 发现新版后 banner 显示在 topbar 下方；点"现在安装"开始下载，进度更新到 banner 文案；下载完后变 ready 状态
- [ ] "稍后"按钮：banner 消失；同会话后续不再弹同版本；下次启动若 endpoint 仍是同版本，banner 不再弹（因 skipped）；endpoint 升新版本号 → banner 重新弹
- [ ] 设置 modal 文案随状态正确切换；"立即检查更新"按钮 loading 行为正确
- [ ] 离线启动时 status=error，trigger=auto → 不弹 banner；用户手动点检查时 → banner 显示错误
- [ ] 端到端：v0.1.0 客户端 + 远端 v0.1.1 release → banner 弹 → install → relaunch → 验版本号变 v0.1.1
- [ ] 所有 composable 测试 PASS（覆盖状态转移、prefs 持久化、auto-skip）

## 10. 风险与开放问题

- **第一个签名版本不能升级到自身**：`v0.1.0` 是首个签名版本，得等 `v0.1.1` release 后才能从已安装的 `v0.1.0` 实测升级。在那之前只能本地手动验证 `check()` 拿到的 latest.json 数据
- **Linux deb 包的 in-place 升级**：Tauri updater 在 Linux 默认更新 `.AppImage`；`.deb` 升级需要权限，可能弹系统密码框 —— 可接受，但首次实测后才能确认 UX
- **Windows 默认 NSIS**：CI 已加 `updaterJsonPreferNsis: true`；MSI 也支持但需要 user-level scope 安装
- **macOS DMG vs .app.tar.gz**：updater 用 `.app.tar.gz` 做替换；CI 默认两个都生成；不需手动配置
- **endpoint 缓存**：GitHub CDN 对 `releases/latest/download/` 的解析有 ~5 分钟缓存；新 release 后用户可能等几分钟才能看到 banner —— 可接受
- **降级备选**：若 Tauri updater 在某平台报错严重，可临时把 `autoCheck` 默认改 false 等修复
