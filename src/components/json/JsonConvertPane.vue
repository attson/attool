<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { NButton, NSelect } from 'naive-ui';
import CodeEditor from './CodeEditor.vue';
import { useFileDrop } from '../../composables/useFileDrop';
import { useJsonWorker } from '../../composables/useJsonWorker';
import type { ConvertFormat } from '../../types/json';

const FORMAT_OPTIONS = [
  { label: 'JSON', value: 'json' },
  { label: 'YAML', value: 'yaml' },
  { label: 'TOML', value: 'toml' },
  { label: 'CSV',  value: 'csv'  },
];

const worker = useJsonWorker();

const source = ref('');
const target = ref('');
const fromFmt = ref<ConvertFormat>('json');
const toFmt = ref<ConvertFormat>('yaml');
const error = ref<string | null>(null);

let timer: number | null = null;
function schedule() {
  if (timer) window.clearTimeout(timer);
  timer = window.setTimeout(() => { void recompute(); }, 200);
}

async function recompute() {
  if (!source.value.trim()) {
    target.value = '';
    error.value = null;
    return;
  }
  const res = await worker.convert(source.value, fromFmt.value, toFmt.value, 'convert:convert');
  if (res === null) return;
  if (res.ok) {
    target.value = res.text;
    error.value = null;
  } else {
    target.value = '';
    error.value = res.error;
  }
}

watch([source, fromFmt, toFmt], schedule);
onBeforeUnmount(() => { if (timer) window.clearTimeout(timer); });

function swap() {
  const oldSource = source.value;
  const oldTarget = target.value;
  const oldFrom = fromFmt.value;
  fromFmt.value = toFmt.value;
  toFmt.value = oldFrom;
  source.value = oldTarget;
  target.value = oldSource;
}

const sourceLang = computed(() => fromFmt.value === 'json' ? 'json' : 'plaintext');
const targetLang = computed(() => toFmt.value === 'json' ? 'json' : 'plaintext');

const drop = useFileDrop(
  (content, filename) => {
    source.value = content;
    if (filename.endsWith('.yaml') || filename.endsWith('.yml')) fromFmt.value = 'yaml';
    else if (filename.endsWith('.toml')) fromFmt.value = 'toml';
    else if (filename.endsWith('.csv')) fromFmt.value = 'csv';
    else if (filename.endsWith('.json')) fromFmt.value = 'json';
  },
  { accept: ['json', 'yaml', 'yml', 'toml', 'csv', 'txt'], onError: (m) => { error.value = m; } },
);
</script>

<template>
  <div class="convert-pane" @drop="drop.onDrop" @dragover="drop.onDragOver">
    <div class="head">
      <div class="side">
        <span class="lbl">源</span>
        <n-select :value="fromFmt" :options="FORMAT_OPTIONS" size="small" @update:value="(v) => fromFmt = v" />
        <n-button size="small" @click="drop.openFile">📂</n-button>
      </div>
      <button class="swap" type="button" @click="swap" title="交换源/目标">⇄</button>
      <div class="side end">
        <span class="lbl">目标</span>
        <n-select :value="toFmt" :options="FORMAT_OPTIONS" size="small" @update:value="(v) => toFmt = v" />
      </div>
    </div>
    <div class="split">
      <CodeEditor :model-value="source" :language="sourceLang" @update:model-value="(v) => source = v" height="100%" />
      <CodeEditor :model-value="target" :language="targetLang" :readonly="true" height="100%" />
    </div>
    <div class="status">
      <span class="err" v-if="error">{{ error }}</span>
      <span class="meta">CSV 仅支持 array-of-flat-objects</span>
    </div>
  </div>
</template>

<style scoped>
.convert-pane { display: grid; grid-template-rows: auto 1fr auto; gap: 8px; height: 100%; }
.head { display: grid; grid-template-columns: 1fr 40px 1fr; align-items: center; gap: 8px; }
.side { display: flex; align-items: center; gap: 6px; }
.side.end { justify-content: flex-end; }
.side .lbl { color: var(--text-muted); font-size: var(--fs-xs); }
.swap {
  background: var(--bg-elev-2);
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  height: 28px;
  cursor: pointer;
  color: var(--text);
}
.split { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; min-height: 0; }
.status { display: flex; justify-content: space-between; font-size: var(--fs-xxs); }
.status .err { color: var(--error); }
.status .meta { color: var(--text-muted); }
</style>
