<script setup lang="ts">
import { computed, ref } from 'vue';
import { NButton, NInput } from 'naive-ui';
import { changeCase, type CaseMode } from '../../utils/textTools';
import Panel from '../ui/Panel.vue';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';

interface CaseSpec {
  mode: CaseMode;
  label: string;
  example: string;
}

const modes: CaseSpec[] = [
  { mode: 'upper', label: 'UPPER', example: 'HELLO WORLD' },
  { mode: 'lower', label: 'lower', example: 'hello world' },
  { mode: 'title', label: 'Title', example: 'Hello World' },
  { mode: 'sentence', label: 'Sentence.', example: 'Hello. World.' },
  { mode: 'camel', label: 'camelCase', example: 'helloWorld' },
  { mode: 'pascal', label: 'PascalCase', example: 'HelloWorld' },
  { mode: 'snake', label: 'snake_case', example: 'hello_world' },
  { mode: 'kebab', label: 'kebab-case', example: 'hello-world' },
  { mode: 'constant', label: 'CONSTANT_CASE', example: 'HELLO_WORLD' },
  { mode: 'swap', label: 'SwAP', example: 'hELLO wORLD' }
];

const input = ref('');
const output = ref('');
const activeMode = ref<CaseMode>('upper');

function pick(mode: CaseMode) {
  activeMode.value = mode;
  output.value = changeCase(input.value, mode);
}

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
    <div class="modes">
      <button
        v-for="m in modes"
        :key="m.mode"
        class="mode-chip"
        :class="{ active: activeMode === m.mode }"
        @click="pick(m.mode)"
      >
        <span class="chip-label mono">{{ m.label }}</span>
        <span class="chip-example muted">{{ m.example }}</span>
      </button>
    </div>

    <div class="grid">
      <Panel title="输入">
        <n-input v-model:value="input" type="textarea" placeholder="输入任意文本" :autosize="{ minRows: 12, maxRows: 22 }" />
      </Panel>
      <Panel title="结果">
        <template #right>
          <n-button size="tiny" secondary :disabled="!output" @click="copy">{{ copyLabel }}</n-button>
        </template>
        <n-input :value="output" type="textarea" readonly :autosize="{ minRows: 12, maxRows: 22 }" />
      </Panel>
    </div>
  </div>
</template>

<style scoped>
.pane { display: grid; gap: 12px; padding: 16px; height: 100%; overflow: auto; }
.modes {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
  gap: 8px;
}
.mode-chip {
  display: grid;
  gap: 2px;
  padding: 10px 12px;
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  background: var(--bg-base);
  cursor: pointer;
  text-align: left;
  transition: border-color 0.15s;
}
.mode-chip:hover { border-color: var(--line-strong); }
.mode-chip.active { border-color: var(--accent, #10b981); }
.chip-label { font-family: var(--font-mono, monospace); font-size: var(--fs-xs); color: var(--text); }
.chip-example { font-size: var(--fs-xxs); color: var(--text-muted); }
.grid { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
@media (max-width: 900px) { .grid { grid-template-columns: 1fr; } }
.muted { color: var(--text-muted); }
.mono { font-family: var(--font-mono, ui-monospace, monospace); }
</style>
