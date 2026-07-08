<script setup lang="ts">
import { ref } from 'vue';
import { NButton, NInputNumber, NSelect } from 'naive-ui';
import { lorem } from '../../utils/generators';
import Panel from '../ui/Panel.vue';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';

const kind = ref<'en' | 'cn'>('cn');
const unit = ref<'word' | 'sentence' | 'paragraph'>('paragraph');
const count = ref(3);
const output = ref('');

const kindOptions = [
  { label: '中文', value: 'cn' },
  { label: '英文', value: 'en' }
];
const unitOptions = [
  { label: '段', value: 'paragraph' },
  { label: '句', value: 'sentence' },
  { label: '词', value: 'word' }
];

function run() {
  output.value = lorem(kind.value, unit.value, Math.max(1, Math.min(50, count.value)));
}

async function copy() { try { await writeText(output.value); } catch {} }
</script>

<template>
  <div class="pane">
    <Panel title="参数">
      <div class="row">
        <label class="field">
          <span class="lbl">语言</span>
          <n-select v-model:value="kind" :options="kindOptions" size="small" style="width: 110px" />
        </label>
        <label class="field">
          <span class="lbl">单位</span>
          <n-select v-model:value="unit" :options="unitOptions" size="small" style="width: 110px" />
        </label>
        <label class="field">
          <span class="lbl">数量</span>
          <n-input-number v-model:value="count" :min="1" :max="50" size="small" style="width: 100px" />
        </label>
        <n-button type="primary" @click="run">生成</n-button>
      </div>
    </Panel>

    <Panel v-if="output" title="结果">
      <template #right>
        <n-button size="tiny" secondary @click="copy">复制</n-button>
      </template>
      <div class="output">{{ output }}</div>
    </Panel>
  </div>
</template>

<style scoped>
.pane { display: grid; gap: 12px; padding: 16px; height: 100%; overflow: auto; }
.row { display: flex; align-items: end; gap: 20px; }
.field { display: grid; gap: 6px; }
.lbl { font-size: var(--fs-xxs); color: var(--text-muted); }
.output {
  padding: 12px;
  background: var(--bg-elev);
  border-radius: var(--radius-sm);
  font-size: var(--fs-sm);
  line-height: 1.7;
  white-space: pre-wrap;
}
</style>
