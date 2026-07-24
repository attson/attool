<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue';
import { NButton } from 'naive-ui';
import CodeEditor from './CodeEditor.vue';
import JsonTreeView from './JsonTreeView.vue';
import { useFileDrop } from '../../composables/useFileDrop';
import { useJsonWorker } from '../../composables/useJsonWorker';
import type { JsonValue } from '../../types/json';

const worker = useJsonWorker();

const text = ref('');
const error = ref<string | null>(null);
const elapsedMs = ref(0);
const parsed = ref<JsonValue | null>(null);
const treeRef = ref<{ expandAll: () => void; collapseAll: () => void } | null>(null);

const charCount = computed(() => text.value.length);

let parseTimer: number | null = null;
function scheduleReparse() {
  if (parseTimer) window.clearTimeout(parseTimer);
  parseTimer = window.setTimeout(() => { void reparse(); }, 300);
}

async function reparse() {
  if (!text.value.trim()) {
    parsed.value = null;
    error.value = null;
    elapsedMs.value = 0;
    return;
  }
  const res = await worker.parse(text.value, 'format:parse');
  if (res === null) return; // superseded
  elapsedMs.value = res.elapsedMs;
  if (res.error) {
    parsed.value = null;
    const line = res.error.line ? `第 ${res.error.line} 行：` : '';
    error.value = `${line}${res.error.message ?? '解析失败'}`;
  } else {
    parsed.value = res.value;
    error.value = null;
  }
}

function setText(next: string) {
  text.value = next;
  scheduleReparse();
}

async function doSerialize(mode: 'format' | 'minify' | 'sort') {
  try {
    let value = parsed.value;
    if (value === null) {
      const p = await worker.parse(text.value, 'format:parse');
      if (p === null) return;
      if (p.error) { error.value = p.error.message ?? '解析失败'; return; }
      value = p.value;
    }
    if (value === null) return;
    const out = await worker.serialize(value, mode, 2, 'format:serialize');
    if (out === null) return;
    if (!out.ok) { error.value = out.error; return; }
    setText(out.text);
  } catch (e) {
    error.value = (e as Error).message;
  }
}

function doFormat()   { void doSerialize('format'); }
function doMinify()   { void doSerialize('minify'); }
function doSortKeys() { void doSerialize('sort'); }

function expandAll()   { treeRef.value?.expandAll(); }
function collapseAll() { treeRef.value?.collapseAll(); }

async function doCopy() { await navigator.clipboard.writeText(text.value); }

const drop = useFileDrop(
  (content) => setText(content),
  { accept: ['json', 'yaml', 'yml', 'toml', 'csv', 'txt'], onError: (m) => { error.value = m; } },
);

function copyTreePath(path: string) {
  navigator.clipboard.writeText(path).catch(() => undefined);
}

onBeforeUnmount(() => { if (parseTimer) window.clearTimeout(parseTimer); });
</script>

<template>
  <div class="format-pane" @drop="drop.onDrop" @dragover="drop.onDragOver">
    <div class="toolbar">
      <n-button size="small" @click="doFormat">格式化</n-button>
      <n-button size="small" @click="doMinify">最小化</n-button>
      <n-button size="small" @click="doSortKeys">键排序</n-button>
      <n-button size="small" @click="doCopy">复制</n-button>
      <span class="spacer" />
      <n-button size="small" @click="expandAll">全部展开</n-button>
      <n-button size="small" @click="collapseAll">全部折叠</n-button>
      <n-button size="small" @click="drop.openFile">📂 打开文件</n-button>
    </div>
    <div class="split">
      <CodeEditor :model-value="text" language="json" @update:model-value="setText" height="100%" />
      <div class="tree-pane">
        <JsonTreeView v-if="parsed !== null" ref="treeRef" :value="parsed" @copy-path="copyTreePath" />
        <div v-else class="empty">{{ text.trim() ? '等待有效 JSON' : '左侧粘贴 / 拖入 JSON 文本' }}</div>
      </div>
    </div>
    <div class="status">
      <span class="err" v-if="error">{{ error }}</span>
      <span class="meta">字符 {{ charCount }} · 解析 {{ elapsedMs }} ms</span>
    </div>
  </div>
</template>

<style scoped>
.format-pane { display: grid; grid-template-rows: auto 1fr auto; gap: 8px; height: 100%; }
.toolbar { display: flex; gap: 6px; align-items: center; }
.toolbar .spacer { flex: 1; }
.split { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; min-height: 0; }
.tree-pane {
  border: 1px solid var(--line);
  border-radius: var(--radius);
  background: var(--bg-elev-2);
  overflow: auto;
  padding: 8px 12px;
  font-size: var(--fs-xs);
}
.empty { color: var(--text-muted); padding: 40px 0; text-align: center; }
.status { display: flex; justify-content: space-between; font-size: var(--fs-xxs); }
.status .err { color: var(--error); }
.status .meta { color: var(--text-muted); }
</style>
