<script setup lang="ts">
import { computed, ref } from 'vue';
import { NButton, NCheckbox, NInput } from 'naive-ui';
import { cleanText, computeStats } from '../../utils/textTools';
import Panel from '../ui/Panel.vue';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';

const input = ref('');
const dedup = ref(true);
const dropEmpty = ref(true);
const trimEachLine = ref(true);
const collapseSpaces = ref(false);

const output = computed(() =>
  cleanText(input.value, {
    dedup: dedup.value,
    dropEmpty: dropEmpty.value,
    trimEachLine: trimEachLine.value,
    collapseSpaces: collapseSpaces.value,
    keepOrder: true
  })
);

const stats = computed(() => computeStats(output.value));

const copyState = ref<'idle' | 'ok' | 'fail'>('idle');
async function copy() {
  try {
    await writeText(output.value);
    copyState.value = 'ok';
  } catch {
    copyState.value = 'fail';
  }
  setTimeout(() => (copyState.value = 'idle'), 1200);
}

const copyLabel = computed(() =>
  copyState.value === 'ok' ? '已复制' : copyState.value === 'fail' ? '失败' : '复制结果'
);
</script>

<template>
  <div class="pane">
    <div class="opts">
      <n-checkbox v-model:checked="dedup">去重</n-checkbox>
      <n-checkbox v-model:checked="dropEmpty">去空行</n-checkbox>
      <n-checkbox v-model:checked="trimEachLine">每行 trim</n-checkbox>
      <n-checkbox v-model:checked="collapseSpaces">折叠连续空格</n-checkbox>
    </div>

    <div class="grid">
      <Panel title="输入">
        <n-input
          v-model:value="input"
          type="textarea"
          placeholder="粘贴多行文本"
          :autosize="{ minRows: 14, maxRows: 24 }"
        />
      </Panel>

      <Panel title="结果">
        <template #right>
          <n-button size="tiny" secondary :disabled="!output" @click="copy">{{ copyLabel }}</n-button>
        </template>
        <n-input
          :value="output"
          type="textarea"
          readonly
          :autosize="{ minRows: 14, maxRows: 24 }"
        />
      </Panel>
    </div>

    <div class="stats mono">
      <span>行 {{ stats.lines }}</span>
      <span>字符 {{ stats.chars }}</span>
      <span>去空格 {{ stats.charsNoSpace }}</span>
      <span>词 {{ stats.words }}</span>
      <span>字节 {{ stats.bytes }}</span>
      <span>中文 {{ stats.chinese }}</span>
    </div>
  </div>
</template>

<style scoped>
.pane { display: grid; gap: 12px; padding: 16px; height: 100%; overflow: auto; }
.opts { display: flex; gap: 16px; flex-wrap: wrap; }
.grid { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
@media (max-width: 900px) { .grid { grid-template-columns: 1fr; } }
.stats {
  display: flex;
  gap: 16px;
  flex-wrap: wrap;
  padding: 8px 12px;
  background: var(--bg-elev);
  border-radius: var(--radius-sm);
  font-size: var(--fs-xs);
  color: var(--text-muted);
}
.mono { font-family: var(--font-mono, ui-monospace, monospace); font-variant-numeric: tabular-nums; }
</style>
