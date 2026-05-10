import { ref } from 'vue';

export type ProgressEvent =
  | { event: 'Started'; data: { contentLength?: number } }
  | { event: 'Progress'; data: { chunkLength: number } }
  | { event: 'Finished' };

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
        downloadAndInstall: (cb) => update.downloadAndInstall(cb)
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
