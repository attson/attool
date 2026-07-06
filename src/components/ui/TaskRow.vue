<script setup lang="ts">
import { computed, ref } from 'vue';
import type { DownloadStatus, DownloadTask } from '../../types/download';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import {
  computeDurationSeconds,
  formatClock,
  formatDurationSeconds,
} from '../../utils/downloadFormat';

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

const terminal = computed(
  () =>
    props.task.status === 'completed' ||
    props.task.status === 'failed' ||
    props.task.status === 'cancelled'
);
const startedClock = computed(() => formatClock(props.task.startedAt));
const finishedClock = computed(() => formatClock(props.task.finishedAt));
const durationLabel = computed(() =>
  formatDurationSeconds(computeDurationSeconds(props.task.startedAt, props.task.finishedAt))
);
const showTimeMeta = computed(() => startedClock.value !== '' || terminal.value);
const showPathMeta = computed(
  () => props.task.status === 'completed' && !!props.task.localPath
);

const pathCopyState = ref<'idle' | 'ok' | 'fail'>('idle');
const pathCopyLabel = computed(() => {
  if (pathCopyState.value === 'ok') return '已复制';
  if (pathCopyState.value === 'fail') return '复制失败';
  return '复制路径';
});
async function copyPath() {
  if (!props.task.localPath) return;
  try {
    await writeText(props.task.localPath);
    pathCopyState.value = 'ok';
  } catch {
    pathCopyState.value = 'fail';
  }
  setTimeout(() => { pathCopyState.value = 'idle'; }, 1500);
}
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

    <div v-if="showTimeMeta" class="time-meta tnum">
      <template v-if="startedClock">开始 {{ startedClock }}</template>
      <template v-if="startedClock && finishedClock"> · </template>
      <template v-if="finishedClock">完成 {{ finishedClock }}</template>
      <template v-if="durationLabel"> · 用时 {{ durationLabel }}</template>
    </div>

    <div v-if="showPathMeta" class="path-meta">
      <span class="path" :title="task.localPath ?? ''">{{ task.localPath }}</span>
      <button class="btn ghost" type="button" @click="copyPath">{{ pathCopyLabel }}</button>
    </div>

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

.time-meta {
  color: var(--text-muted);
  font-size: var(--fs-xxs);
}

.path-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}
.path-meta .path {
  flex: 1;
  min-width: 0;
  color: var(--text);
  font-size: var(--fs-xs);
  font-family: var(--font-mono);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
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
