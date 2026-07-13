import { onBeforeUnmount, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

// ---- 后端契约（与 Rust `updater::models` 对齐）----

export type UpdaterPhase =
  | { kind: 'idle' }
  | { kind: 'checking' }
  | { kind: 'up-to-date' }
  | { kind: 'available'; info: ReleaseInfo }
  | { kind: 'downloading'; pct: number; downloaded: number; total: number }
  | { kind: 'verifying' }
  | { kind: 'ready'; version: string }
  | { kind: 'applying' }
  | { kind: 'error'; message: string };

export interface ReleaseInfo {
  version: string;
  notes: string;
  publishedAt: string;
  assetName: string;
  assetUrl: string;
  assetSize: number;
}

export interface UpdateStateSnapshot {
  phase: UpdaterPhase;
  current: string;
  autoCheck: boolean;
  lastCheckAt: number | null;
  error: string | null;
}

// ---- 兼容旧 UI 消费的表面（尽量少改 SettingsModal / UpdateBanner）----

export type Status =
  | 'idle' | 'checking' | 'up-to-date'
  | 'available' | 'downloading' | 'verifying' | 'ready' | 'applying' | 'error';

export type Trigger = 'auto' | 'manual';

export interface UpdaterState {
  status: Status;
  trigger: Trigger;
  available?: { version: string; notes?: string };
  downloadPercent?: number;
  error?: string;
}

export interface UpdaterClient {
  getState(): Promise<UpdateStateSnapshot>;
  check(): Promise<UpdateStateSnapshot>;
  download(): Promise<UpdateStateSnapshot>;
  apply(): Promise<void>;
  cancel(): Promise<void>;
  onState(cb: (snap: UpdateStateSnapshot) => void): Promise<UnlistenFn>;
}

function defaultClient(): UpdaterClient {
  return {
    getState() { return invoke('updater_get_state'); },
    check() { return invoke('updater_check'); },
    download() { return invoke('updater_download'); },
    apply() { return invoke('updater_apply'); },
    cancel() { return invoke('updater_cancel'); },
    onState(cb) { return listen<UpdateStateSnapshot>('updater://state', (ev) => cb(ev.payload)); }
  };
}

const UP_TO_DATE_REVERT_MS = 3000;

function snapshotToUiState(snap: UpdateStateSnapshot, trigger: Trigger): UpdaterState {
  const base: UpdaterState = { status: 'idle', trigger };
  const p = snap.phase;
  switch (p.kind) {
    case 'idle': return { ...base, status: 'idle' };
    case 'checking': return { ...base, status: 'checking' };
    case 'up-to-date': return { ...base, status: 'up-to-date' };
    case 'available': return {
      ...base,
      status: 'available',
      available: { version: p.info.version, notes: p.info.notes }
    };
    case 'downloading': return {
      ...base,
      status: 'downloading',
      available: currentAvailable(snap),
      downloadPercent: p.pct
    };
    case 'verifying': return {
      ...base,
      status: 'verifying',
      available: currentAvailable(snap)
    };
    case 'ready': return {
      ...base,
      status: 'ready',
      available: { version: p.version },
      downloadPercent: 100
    };
    case 'applying': return {
      ...base,
      status: 'applying',
      available: currentAvailable(snap)
    };
    case 'error': return { ...base, status: 'error', error: p.message };
  }
}

// 后端 Available 在下载时会切换到 Downloading，前端保留 available.version 用作展示；
// 简单起见没在后端 snapshot 里同时暴露，所以这里如果 phase 不是 available 就返回 undefined。
function currentAvailable(snap: UpdateStateSnapshot): UpdaterState['available'] {
  if (snap.phase.kind === 'available') {
    return { version: snap.phase.info.version, notes: snap.phase.info.notes };
  }
  if (snap.phase.kind === 'ready') {
    return { version: snap.phase.version };
  }
  return undefined;
}

export function useUpdater(client: UpdaterClient = defaultClient()) {
  const state = ref<UpdaterState>({ status: 'idle', trigger: 'manual' });
  let unlisten: UnlistenFn | null = null;
  let currentTrigger: Trigger = 'manual';
  let upToDateTimer: ReturnType<typeof setTimeout> | null = null;

  async function bind() {
    if (unlisten) return;
    try {
      unlisten = await client.onState((snap) => {
        state.value = snapshotToUiState(snap, currentTrigger);
        maybeArmUpToDateRevert();
      });
      const initial = await client.getState();
      state.value = snapshotToUiState(initial, currentTrigger);
    } catch (e) {
      state.value = { status: 'error', trigger: currentTrigger, error: errorMessage(e) };
    }
  }

  function maybeArmUpToDateRevert() {
    if (state.value.status !== 'up-to-date') return;
    if (upToDateTimer) clearTimeout(upToDateTimer);
    upToDateTimer = setTimeout(() => {
      if (state.value.status === 'up-to-date') {
        state.value = { status: 'idle', trigger: state.value.trigger };
      }
    }, UP_TO_DATE_REVERT_MS);
  }

  async function check(trigger: Trigger = 'manual') {
    currentTrigger = trigger;
    await bind();
    try {
      const snap = await client.check();
      state.value = snapshotToUiState(snap, trigger);
      maybeArmUpToDateRevert();
    } catch (e) {
      state.value = { status: 'error', trigger, error: errorMessage(e) };
    }
  }

  async function install() {
    const trigger = state.value.trigger;
    const available = state.value.available;
    state.value = { status: 'downloading', trigger, available, downloadPercent: 0 };
    void bind();
    try {
      const snap = await client.download();
      state.value = snapshotToUiState(snap, trigger);
    } catch (e) {
      state.value = { status: 'error', trigger, available, error: errorMessage(e) };
    }
  }

  async function relaunch() {
    // "relaunch" 在新架构下等价于 apply（覆盖 + 拉起新版）
    try {
      await client.apply();
    } catch (e) {
      state.value = { ...state.value, status: 'error', error: errorMessage(e) };
    }
  }

  async function cancel() {
    try { await client.cancel(); } catch {}
  }

  function dismiss() {
    state.value = { status: 'idle', trigger: 'manual' };
  }

  onBeforeUnmount(() => {
    if (unlisten) { unlisten(); unlisten = null; }
    if (upToDateTimer) { clearTimeout(upToDateTimer); upToDateTimer = null; }
  });

  return { state, check, install, relaunch, cancel, dismiss };
}

function errorMessage(e: unknown): string {
  if (e instanceof Error) return e.message;
  return String(e);
}
