<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import CodeEditor from './CodeEditor.vue';
import { useJsonWorker } from '../../composables/useJsonWorker';

const DIFF_HTML_MAX = 1_000_000;

const worker = useJsonWorker();

const left = ref('');
const right = ref('');
const html = ref('');
const leftError = ref<string | null>(null);
const rightError = ref<string | null>(null);
const equal = ref(true);
const tooBig = ref(false);
const forceHtml = ref(false);

let timer: number | null = null;
function schedule() {
  if (timer) window.clearTimeout(timer);
  timer = window.setTimeout(() => { void recompute(); }, 400);
}

async function recompute() {
  if (!left.value.trim() && !right.value.trim()) {
    html.value = '';
    leftError.value = null;
    rightError.value = null;
    equal.value = true;
    tooBig.value = false;
    return;
  }
  const withHtml = forceHtml.value || (left.value.length + right.value.length < DIFF_HTML_MAX);
  const res = await worker.diff(left.value || '{}', right.value || '{}', withHtml, 'diff:diff');
  if (res === null) return;
  leftError.value = res.leftError ?? null;
  rightError.value = res.rightError ?? null;
  if (res.leftError || res.rightError) {
    html.value = '';
    equal.value = false;
    tooBig.value = false;
    return;
  }
  if (res.equal) {
    html.value = '';
    equal.value = true;
    tooBig.value = false;
    return;
  }
  equal.value = false;
  if (res.html && res.html.length > DIFF_HTML_MAX && !forceHtml.value) {
    html.value = '';
    tooBig.value = true;
    return;
  }
  tooBig.value = false;
  html.value = res.html ?? '';
}

watch([left, right, forceHtml], schedule);
onBeforeUnmount(() => { if (timer) window.clearTimeout(timer); });

const status = computed(() => {
  if (leftError.value) return `左侧 JSON 解析失败：${leftError.value}`;
  if (rightError.value) return `右侧 JSON 解析失败：${rightError.value}`;
  if (tooBig.value) return '差异 HTML 过大，已隐藏；点右侧按钮强制显示';
  if (equal.value && (left.value.trim() || right.value.trim())) return '两侧内容等价';
  return '';
});
</script>

<template>
  <div class="diff-pane">
    <div class="split">
      <CodeEditor :model-value="left" language="json" @update:model-value="(v) => left = v" height="100%" />
      <CodeEditor :model-value="right" language="json" @update:model-value="(v) => right = v" height="100%" />
    </div>
    <div class="result" v-if="html" v-html="html" />
    <div class="status" v-else>
      {{ status || '粘贴左右两份 JSON 进行对比' }}
      <button v-if="tooBig" type="button" class="force-btn" @click="forceHtml = true">仍要显示</button>
    </div>
  </div>
</template>

<style scoped>
.diff-pane { display: grid; grid-template-rows: 1fr auto; gap: 8px; height: 100%; }
.split { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; min-height: 0; }
.result {
  border: 1px solid var(--line);
  border-radius: var(--radius);
  background: var(--bg-elev-2);
  padding: 10px 14px;
  font-family: var(--font-mono);
  font-size: var(--fs-xs);
  max-height: 240px;
  overflow: auto;
}
.status { color: var(--text-muted); font-size: var(--fs-xs); padding: 8px; }

/* jsondiffpatch HTML formatter styles (scoped overrides) */
.result :deep(.jsondiffpatch-delta) { font-family: inherit; }
.result :deep(.jsondiffpatch-added .jsondiffpatch-property-name),
.result :deep(.jsondiffpatch-added .jsondiffpatch-value) { background: color-mix(in srgb, #16a34a 18%, transparent); }
.result :deep(.jsondiffpatch-deleted .jsondiffpatch-property-name),
.result :deep(.jsondiffpatch-deleted .jsondiffpatch-value) { background: color-mix(in srgb, #dc2626 18%, transparent); text-decoration: line-through; }
.result :deep(.jsondiffpatch-modified .jsondiffpatch-left-value) { background: color-mix(in srgb, #dc2626 18%, transparent); text-decoration: line-through; }
.result :deep(.jsondiffpatch-modified .jsondiffpatch-right-value) { background: color-mix(in srgb, #16a34a 18%, transparent); }

.force-btn {
  margin-left: 12px;
  background: var(--bg-elev-2);
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  padding: 2px 8px;
  cursor: pointer;
  color: var(--text);
}
</style>
