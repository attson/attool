<script setup lang="ts">
import { computed, ref } from 'vue';
import { NButton } from 'naive-ui';
import CodeEditor from './CodeEditor.vue';
import JsonTreeView from './JsonTreeView.vue';
import { format, minify, parseJson, sortKeys } from '../../utils/jsonFormat';
import { useFileDrop } from '../../composables/useFileDrop';
import type { JsonValue } from '../../types/json';

const text = ref('');
const error = ref<string | null>(null);
const elapsedMs = ref(0);
const parsed = ref<JsonValue | null>(null);

const treeRef = ref<{ expandAll: () => void; collapseAll: () => void } | null>(null);

function expandAll() { treeRef.value?.expandAll(); }
function collapseAll() { treeRef.value?.collapseAll(); }

const charCount = computed(() => text.value.length);

function reparse() {
  const start = performance.now();
  const result = parseJson(text.value);
  elapsedMs.value = Math.round(performance.now() - start);
  if (result.ok) {
    parsed.value = result.value ?? null;
    error.value = null;
  } else {
    parsed.value = null;
    if (text.value.trim().length === 0) {
      error.value = null;
    } else {
      const line = result.error?.line ? `第 ${result.error.line} 行：` : '';
      error.value = `${line}${result.error?.message ?? '解析失败'}`;
    }
  }
}

function setText(next: string) {
  text.value = next;
  reparse();
}

function doFormat() {
  try {
    setText(format(text.value));
  } catch (e) {
    error.value = (e as Error).message;
  }
}

function doMinify() {
  try {
    setText(minify(text.value));
  } catch (e) {
    error.value = (e as Error).message;
  }
}

function doSortKeys() {
  try {
    setText(sortKeys(text.value));
  } catch (e) {
    error.value = (e as Error).message;
  }
}

async function doCopy() {
  await navigator.clipboard.writeText(text.value);
}

const drop = useFileDrop(
  (content) => setText(content),
  { accept: ['json', 'yaml', 'yml', 'toml', 'csv', 'txt'], onError: (m) => { error.value = m; } },
);

function copyTreePath(path: string) {
  navigator.clipboard.writeText(path).catch(() => undefined);
}
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
