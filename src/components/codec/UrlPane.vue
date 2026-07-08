<script setup lang="ts">
import { ref } from 'vue';
import { NAlert, NButton, NInput, NSwitch } from 'naive-ui';
import { urlDecode, urlEncode } from '../../utils/codec';
import Panel from '../ui/Panel.vue';
import PaneActions from './PaneActions.vue';

const plain = ref('');
const encoded = ref('');
const componentMode = ref(true);
const error = ref('');

function doEncode() {
  error.value = '';
  try {
    encoded.value = urlEncode(plain.value, componentMode.value);
  } catch (err) {
    error.value = String(err);
  }
}

function doDecode() {
  error.value = '';
  try {
    plain.value = urlDecode(encoded.value, componentMode.value);
  } catch (err) {
    error.value = String(err);
  }
}
</script>

<template>
  <div class="pane">
    <div class="opts">
      <label class="opt">
        <span>Component 模式（encodeURIComponent，转义所有保留字符）</span>
        <n-switch v-model:value="componentMode" size="small" />
      </label>
    </div>

    <div class="grid">
      <Panel title="原文">
        <n-input
          v-model:value="plain"
          type="textarea"
          placeholder="要 URL 编码的文本或 URL"
          :autosize="{ minRows: 10, maxRows: 20 }"
        />
      </Panel>

      <div class="middle">
        <n-button block type="primary" @click="doEncode">编码 →</n-button>
        <n-button block secondary @click="doDecode">← 解码</n-button>
      </div>

      <Panel title="Percent-encoded">
        <template #right>
          <PaneActions :value="encoded" />
        </template>
        <n-input
          v-model:value="encoded"
          type="textarea"
          placeholder="URL 编码结果，或粘贴 percent-encoded 后按 ← 解码"
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
