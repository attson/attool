<script setup lang="ts">
import { computed, ref } from 'vue';
import { NAlert, NButton, NCheckbox, NInput, NSelect } from 'naive-ui';
import { BUILTIN_PATTERNS, extractMatches } from '../../utils/textTools';
import Panel from '../ui/Panel.vue';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';

const input = ref('');
const modeKey = ref('URL');
const customPattern = ref('');
const customFlags = ref('gi');
const dedup = ref(true);
const error = ref('');

const modeOptions = [
  ...Object.keys(BUILTIN_PATTERNS).map((k) => ({ label: k, value: k })),
  { label: '自定义正则', value: '__custom' }
];

const matches = computed<string[]>(() => {
  error.value = '';
  if (!input.value) return [];
  try {
    let pattern: RegExp;
    if (modeKey.value === '__custom') {
      if (!customPattern.value) return [];
      pattern = new RegExp(customPattern.value, customFlags.value.includes('g') ? customFlags.value : customFlags.value + 'g');
    } else {
      pattern = BUILTIN_PATTERNS[modeKey.value];
    }
    return extractMatches(input.value, pattern, dedup.value);
  } catch (err) {
    error.value = String(err);
    return [];
  }
});

async function copyAll() {
  try {
    await writeText(matches.value.join('\n'));
  } catch {
    // noop
  }
}
</script>

<template>
  <div class="pane">
    <div class="opts">
      <label class="field">
        <span class="lbl">抽取</span>
        <n-select v-model:value="modeKey" :options="modeOptions" style="width: 180px" />
      </label>
      <template v-if="modeKey === '__custom'">
        <label class="field grow">
          <span class="lbl">Regex</span>
          <n-input v-model:value="customPattern" placeholder="\d+" />
        </label>
        <label class="field">
          <span class="lbl">Flags</span>
          <n-input v-model:value="customFlags" placeholder="gi" style="width: 100px" />
        </label>
      </template>
      <n-checkbox v-model:checked="dedup">去重</n-checkbox>
    </div>

    <div class="grid">
      <Panel title="输入">
        <n-input v-model:value="input" type="textarea" placeholder="粘贴文本，右侧实时列出所有匹配" :autosize="{ minRows: 14, maxRows: 24 }" />
      </Panel>

      <Panel :title="`结果（${matches.length}）`">
        <template #right>
          <n-button size="tiny" secondary :disabled="matches.length === 0" @click="copyAll">复制全部</n-button>
        </template>
        <n-alert v-if="error" type="error" :bordered="false">{{ error }}</n-alert>
        <ul v-else-if="matches.length > 0" class="matches">
          <li v-for="(m, i) in matches" :key="i" class="mono">{{ m }}</li>
        </ul>
        <p v-else class="muted">暂无匹配</p>
      </Panel>
    </div>
  </div>
</template>

<style scoped>
.pane { display: grid; gap: 12px; padding: 16px; height: 100%; overflow: auto; }
.opts { display: flex; align-items: end; gap: 12px; flex-wrap: wrap; }
.field { display: grid; gap: 6px; }
.field.grow { flex: 1; min-width: 200px; }
.lbl { font-size: var(--fs-xxs); color: var(--text-muted); }
.grid { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
@media (max-width: 900px) { .grid { grid-template-columns: 1fr; } }
.matches {
  list-style: none;
  padding: 0;
  margin: 0;
  display: grid;
  gap: 3px;
  max-height: 460px;
  overflow: auto;
}
.matches li {
  padding: 5px 10px;
  background: var(--bg-elev);
  border-radius: var(--radius-sm);
  font-size: var(--fs-xs);
  word-break: break-all;
}
.mono { font-family: var(--font-mono, ui-monospace, monospace); }
.muted { color: var(--text-muted); font-size: var(--fs-sm); margin: 0; }
</style>
