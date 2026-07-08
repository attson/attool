<script setup lang="ts">
import { computed, ref } from 'vue';
import { NButton, NInput, NSwitch } from 'naive-ui';
import { diffSummary, lineDiff, type DiffLine } from '../../utils/textTools';
import Panel from '../ui/Panel.vue';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';

const original = ref('');
const revised = ref('');
const collapse = ref(true);
const contextLines = 3;

const diff = computed<DiffLine[]>(() => lineDiff(original.value, revised.value));
const summary = computed(() => diffSummary(diff.value));

interface Row {
  type: 'equal' | 'add' | 'remove' | 'gap';
  text: string;
  lineNumA?: number;
  lineNumB?: number;
  gapSize?: number;
}

/** Fold long runs of equal lines into a "N lines unchanged" gap, keeping `contextLines` on each side. */
const rows = computed<Row[]>(() => {
  const src = diff.value;
  if (!collapse.value) return src.map((l) => ({ ...l }) as Row);

  const out: Row[] = [];
  let i = 0;
  while (i < src.length) {
    if (src[i].type !== 'equal') {
      out.push({ ...src[i] } as Row);
      i++;
      continue;
    }
    // Find end of this equal run
    let j = i;
    while (j < src.length && src[j].type === 'equal') j++;
    const runLen = j - i;
    const isStart = i === 0;
    const isEnd = j === src.length;
    const keepHead = isStart ? contextLines : contextLines;
    const keepTail = isEnd ? contextLines : contextLines;
    const threshold = keepHead + keepTail + 1;
    if (runLen <= threshold) {
      for (let k = i; k < j; k++) out.push({ ...src[k] } as Row);
    } else {
      if (!isStart) {
        for (let k = i; k < i + keepHead; k++) out.push({ ...src[k] } as Row);
      }
      out.push({ type: 'gap', text: '', gapSize: runLen - (isStart ? 0 : keepHead) - (isEnd ? 0 : keepTail) });
      if (!isEnd) {
        for (let k = j - keepTail; k < j; k++) out.push({ ...src[k] } as Row);
      }
    }
    i = j;
  }
  return out;
});

function unifiedText() {
  return diff.value
    .map((l) => (l.type === 'add' ? `+${l.text}` : l.type === 'remove' ? `-${l.text}` : ` ${l.text}`))
    .join('\n');
}

const copyState = ref<'idle' | 'ok' | 'fail'>('idle');
async function copyUnified() {
  try {
    await writeText(unifiedText());
    copyState.value = 'ok';
  } catch {
    copyState.value = 'fail';
  }
  setTimeout(() => (copyState.value = 'idle'), 1200);
}
const copyLabel = computed(() =>
  copyState.value === 'ok' ? '已复制' : copyState.value === 'fail' ? '失败' : '复制 unified diff'
);

function swap() {
  const tmp = original.value;
  original.value = revised.value;
  revised.value = tmp;
}

function marker(type: Row['type']): string {
  if (type === 'add') return '+';
  if (type === 'remove') return '-';
  if (type === 'gap') return '⋯';
  return ' ';
}
</script>

<template>
  <div class="pane">
    <div class="grid-inputs">
      <Panel title="原文（左）">
        <n-input v-model:value="original" type="textarea" placeholder="旧内容" :autosize="{ minRows: 8, maxRows: 14 }" />
      </Panel>
      <Panel title="新文（右）">
        <n-input v-model:value="revised" type="textarea" placeholder="新内容" :autosize="{ minRows: 8, maxRows: 14 }" />
      </Panel>
    </div>

    <div class="toolbar">
      <div class="summary mono">
        <span v-if="summary.identical" class="tag tag-equal">完全相同</span>
        <template v-else>
          <span class="tag tag-add">+{{ summary.added }}</span>
          <span class="tag tag-remove">-{{ summary.removed }}</span>
          <span class="muted">unchanged {{ summary.equal }}</span>
        </template>
      </div>
      <div class="ops">
        <label class="opt">
          <span>折叠未变行（保留上下文 {{ contextLines }} 行）</span>
          <n-switch v-model:value="collapse" size="small" />
        </label>
        <n-button size="small" secondary @click="swap">交换</n-button>
        <n-button size="small" secondary :disabled="summary.identical" @click="copyUnified">
          {{ copyLabel }}
        </n-button>
      </div>
    </div>

    <Panel title="Diff">
      <div class="diff-view">
        <div
          v-for="(row, idx) in rows"
          :key="idx"
          class="diff-row"
          :class="'row-' + row.type"
        >
          <span class="gutter num">{{ row.lineNumA ?? '' }}</span>
          <span class="gutter num">{{ row.lineNumB ?? '' }}</span>
          <span class="gutter marker">{{ marker(row.type) }}</span>
          <span v-if="row.type === 'gap'" class="text muted">
            {{ row.gapSize }} lines unchanged
          </span>
          <span v-else class="text mono">{{ row.text || ' ' }}</span>
        </div>
      </div>
    </Panel>
  </div>
</template>

<style scoped>
.pane { display: grid; gap: 12px; padding: 16px; height: 100%; overflow: auto; }
.grid-inputs { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
@media (max-width: 900px) { .grid-inputs { grid-template-columns: 1fr; } }
.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}
.summary { display: flex; align-items: center; gap: 8px; font-size: var(--fs-xs); }
.tag {
  display: inline-block;
  padding: 1px 8px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--line-strong);
  font-family: var(--font-mono, monospace);
  font-size: var(--fs-xxs);
}
.tag-add { color: #10b981; border-color: #10b981; }
.tag-remove { color: #ef4444; border-color: #ef4444; }
.tag-equal { color: var(--text-muted); }
.muted { color: var(--text-muted); font-size: var(--fs-xxs); }
.mono { font-family: var(--font-mono, ui-monospace, monospace); font-variant-numeric: tabular-nums; }
.ops { display: flex; align-items: center; gap: 12px; }
.opt { display: flex; align-items: center; gap: 6px; font-size: var(--fs-xxs); color: var(--text-muted); }

.diff-view {
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  overflow: auto;
  max-height: 60vh;
  background: var(--bg-base);
}
.diff-row {
  display: grid;
  grid-template-columns: 48px 48px 22px 1fr;
  align-items: baseline;
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: var(--fs-xs);
  line-height: 1.55;
}
.diff-row.row-add { background: rgba(16, 185, 129, 0.08); }
.diff-row.row-remove { background: rgba(239, 68, 68, 0.08); }
.diff-row.row-gap {
  background: var(--bg-elev);
  color: var(--text-muted);
  border-top: 1px dashed var(--line);
  border-bottom: 1px dashed var(--line);
}
.gutter {
  padding: 0 8px;
  color: var(--text-muted);
  text-align: right;
  font-variant-numeric: tabular-nums;
  user-select: none;
}
.gutter.marker { text-align: center; padding: 0 4px; }
.row-add .gutter.marker { color: #10b981; }
.row-remove .gutter.marker { color: #ef4444; }
.text {
  padding: 0 8px;
  white-space: pre-wrap;
  word-break: break-word;
}
.row-add .text { color: var(--text); }
.row-remove .text { color: var(--text); }
</style>
