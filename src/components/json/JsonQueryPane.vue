<script setup lang="ts">
import { ref } from 'vue';
import { NButton, NInput } from 'naive-ui';
import CodeEditor from './CodeEditor.vue';
import { useFileDrop } from '../../composables/useFileDrop';
import { useJsonWorker } from '../../composables/useJsonWorker';
import type { JsonValue } from '../../types/json';

const worker = useJsonWorker();

const source = ref('');
const expression = ref('$');
const result = ref('');
const matchCount = ref(0);
const elapsedMs = ref(0);
const error = ref<string | null>(null);

let cachedSource = '';
let cachedParsed: JsonValue | null = null;

async function execute() {
  error.value = null;
  if (!source.value.trim()) {
    result.value = '';
    matchCount.value = 0;
    return;
  }
  try {
    if (source.value !== cachedSource || cachedParsed === null) {
      const p = await worker.parse(source.value, 'query:parse');
      if (p === null) return;
      if (p.error) {
        error.value = `源 JSON 解析失败：${p.error.message ?? ''}`;
        return;
      }
      cachedSource = source.value;
      cachedParsed = p.value;
    }
    const out = await worker.jsonpath(cachedParsed, expression.value || '$', 'query:jsonpath');
    if (out === null) return;
    if (!out.ok) { error.value = `表达式错误：${out.error}`; result.value = ''; return; }
    elapsedMs.value = out.elapsedMs;
    matchCount.value = Array.isArray(out.matches) ? out.matches.length : 0;
    result.value = out.text;
  } catch (e) {
    error.value = `表达式错误：${(e as Error).message}`;
    result.value = '';
  }
}

async function copy() {
  if (result.value) await navigator.clipboard.writeText(result.value);
}

function onExprKeydown(event: KeyboardEvent) {
  if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
    event.preventDefault();
    void execute();
  }
}

const drop = useFileDrop(
  (content) => { source.value = content; cachedSource = ''; cachedParsed = null; },
  { accept: ['json', 'txt'], onError: (m) => { error.value = m; } },
);
</script>

<template>
  <div class="query-pane" @drop="drop.onDrop" @dragover="drop.onDragOver">
    <div class="expr-row">
      <n-input
        v-model:value="expression"
        placeholder="$.store.book[?(@.price < 10)].title"
        class="mono"
        @keydown="onExprKeydown"
      />
      <n-button type="primary" size="small" @click="execute">执行 (⌘↵)</n-button>
      <n-button size="small" @click="copy">复制结果</n-button>
      <n-button size="small" @click="drop.openFile">📂</n-button>
    </div>
    <div class="split">
      <CodeEditor :model-value="source" language="json" @update:model-value="(v) => source = v" height="100%" />
      <CodeEditor :model-value="result" language="json" :readonly="true" height="100%" />
    </div>
    <div class="status">
      <span class="err" v-if="error">{{ error }}</span>
      <span class="meta">{{ matchCount }} 条匹配 · {{ elapsedMs }} ms</span>
    </div>
  </div>
</template>

<style scoped>
.query-pane { display: grid; grid-template-rows: auto 1fr auto; gap: 8px; height: 100%; }
.expr-row { display: flex; gap: 6px; align-items: center; }
.expr-row .mono :deep(input) { font-family: var(--font-mono); }
.split { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; min-height: 0; }
.status { display: flex; justify-content: space-between; font-size: var(--fs-xxs); }
.status .err { color: var(--error); }
.status .meta { color: var(--text-muted); }
</style>
