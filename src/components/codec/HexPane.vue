<script setup lang="ts">
import { ref } from 'vue';
import { NAlert, NButton, NInput, NSelect } from 'naive-ui';
import { hexDecode, hexEncode } from '../../utils/codec';
import Panel from '../ui/Panel.vue';
import PaneActions from './PaneActions.vue';

const plain = ref('');
const hex = ref('');
const separator = ref('');
const error = ref('');

const sepOptions = [
  { label: '无（连续）', value: '' },
  { label: '空格', value: ' ' },
  { label: '冒号', value: ':' },
  { label: '短横线', value: '-' }
];

function doEncode() {
  error.value = '';
  try {
    hex.value = hexEncode(plain.value, separator.value);
  } catch (err) {
    error.value = String(err);
  }
}

function doDecode() {
  error.value = '';
  try {
    plain.value = hexDecode(hex.value);
  } catch (err) {
    error.value = String(err);
  }
}
</script>

<template>
  <div class="pane">
    <div class="opts">
      <label class="opt">
        <span>编码分隔符</span>
        <n-select v-model:value="separator" :options="sepOptions" size="small" style="width: 140px" />
      </label>
    </div>

    <div class="grid">
      <Panel title="明文">
        <n-input
          v-model:value="plain"
          type="textarea"
          placeholder="要转 Hex 的文本"
          :autosize="{ minRows: 10, maxRows: 20 }"
        />
      </Panel>

      <div class="middle">
        <n-button block type="primary" @click="doEncode">编码 →</n-button>
        <n-button block secondary @click="doDecode">← 解码</n-button>
      </div>

      <Panel title="Hex">
        <template #right>
          <PaneActions :value="hex" />
        </template>
        <n-input
          v-model:value="hex"
          type="textarea"
          placeholder="Hex 字符串（支持 0x 前缀、空格/冒号/短横分隔）"
          :autosize="{ minRows: 10, maxRows: 20 }"
        />
      </Panel>
    </div>

    <n-alert v-if="error" type="error" :bordered="false">{{ error }}</n-alert>
  </div>
</template>

<style scoped>
.pane { display: grid; gap: 12px; padding: 16px; height: 100%; overflow: auto; }
.opts { display: flex; align-items: center; gap: 12px; font-size: var(--fs-xs); color: var(--text-muted); }
.opt { display: flex; align-items: center; gap: 6px; }
.grid { display: grid; grid-template-columns: 1fr 120px 1fr; gap: 12px; }
@media (max-width: 900px) { .grid { grid-template-columns: 1fr; } }
.middle { display: grid; gap: 8px; align-content: center; }
</style>
