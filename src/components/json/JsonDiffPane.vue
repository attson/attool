<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import CodeEditor from './CodeEditor.vue';
import { diffJson, diffJsonHtml } from '../../utils/jsondiff';

const left = ref('');
const right = ref('');
const html = ref('');
const leftError = ref<string | null>(null);
const rightError = ref<string | null>(null);
const equal = ref(true);

let timer: number | null = null;
function schedule() {
  if (timer) window.clearTimeout(timer);
  timer = window.setTimeout(recompute, 200);
}

function recompute() {
  if (!left.value.trim() && !right.value.trim()) {
    html.value = '';
    leftError.value = null;
    rightError.value = null;
    equal.value = true;
    return;
  }
  const result = diffJson(left.value || '{}', right.value || '{}');
  leftError.value = result.leftError ?? null;
  rightError.value = result.rightError ?? null;
  if (result.leftError || result.rightError) {
    html.value = '';
    equal.value = false;
    return;
  }
  if (result.delta === null) {
    html.value = '';
    equal.value = true;
    return;
  }
  equal.value = false;
  html.value = diffJsonHtml(left.value, right.value);
}

watch([left, right], schedule);

const status = computed(() => {
  if (leftError.value) return `左侧 JSON 解析失败：${leftError.value}`;
  if (rightError.value) return `右侧 JSON 解析失败：${rightError.value}`;
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
    <div class="status" v-else>{{ status || '粘贴左右两份 JSON 进行对比' }}</div>
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
</style>
