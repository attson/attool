<script setup lang="ts">
import { computed, ref } from 'vue';
import { NButton, NInput, NSelect } from 'naive-ui';
import { sortLines, type SortMode } from '../../utils/textTools';
import Panel from '../ui/Panel.vue';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';

const input = ref('');
const mode = ref<SortMode>('asc');

const modeOptions = [
  { label: '升序（字典序）', value: 'asc' },
  { label: '降序（字典序）', value: 'desc' },
  { label: '自然序（含数字）', value: 'natural' },
  { label: '按长度升序', value: 'length-asc' },
  { label: '按长度降序', value: 'length-desc' },
  { label: '倒序（原顺序反转）', value: 'reverse' },
  { label: '随机', value: 'shuffle' }
];

const output = computed(() => sortLines(input.value, mode.value));

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
      <label class="field">
        <span class="lbl">排序方式</span>
        <n-select v-model:value="mode" :options="modeOptions" style="width: 240px" />
      </label>
    </div>

    <div class="grid">
      <Panel title="输入">
        <n-input v-model:value="input" type="textarea" placeholder="每行一个条目" :autosize="{ minRows: 14, maxRows: 24 }" />
      </Panel>
      <Panel title="结果">
        <template #right>
          <n-button size="tiny" secondary :disabled="!output" @click="copy">{{ copyLabel }}</n-button>
        </template>
        <n-input :value="output" type="textarea" readonly :autosize="{ minRows: 14, maxRows: 24 }" />
      </Panel>
    </div>
  </div>
</template>

<style scoped>
.pane { display: grid; gap: 12px; padding: 16px; height: 100%; overflow: auto; }
.opts { display: flex; gap: 12px; }
.field { display: grid; gap: 6px; }
.lbl { font-size: var(--fs-xxs); color: var(--text-muted); }
.grid { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
@media (max-width: 900px) { .grid { grid-template-columns: 1fr; } }
</style>
