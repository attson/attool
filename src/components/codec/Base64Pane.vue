<script setup lang="ts">
import { ref } from 'vue';
import { NAlert, NButton, NInput, NSwitch } from 'naive-ui';
import { base64Decode, base64Encode } from '../../utils/codec';
import Panel from '../ui/Panel.vue';
import PaneActions from './PaneActions.vue';

const plain = ref('');
const encoded = ref('');
const urlSafe = ref(false);
const error = ref('');

function doEncode() {
  error.value = '';
  try {
    encoded.value = base64Encode(plain.value, urlSafe.value);
  } catch (err) {
    error.value = String(err);
  }
}

function doDecode() {
  error.value = '';
  try {
    plain.value = base64Decode(encoded.value, urlSafe.value);
  } catch (err) {
    error.value = String(err);
  }
}
</script>

<template>
  <div class="pane">
    <div class="opts">
      <label class="opt">
        <span>URL Safe（`-`/`_`，去 padding）</span>
        <n-switch v-model:value="urlSafe" size="small" />
      </label>
    </div>

    <div class="grid">
      <Panel title="明文">
        <n-input
          v-model:value="plain"
          type="textarea"
          placeholder="要 Base64 编码的文本"
          :autosize="{ minRows: 10, maxRows: 20 }"
        />
      </Panel>

      <div class="middle">
        <n-button block type="primary" @click="doEncode">编码 →</n-button>
        <n-button block secondary @click="doDecode">← 解码</n-button>
      </div>

      <Panel title="Base64">
        <template #right>
          <PaneActions :value="encoded" />
        </template>
        <n-input
          v-model:value="encoded"
          type="textarea"
          placeholder="Base64 结果，或粘贴一段 Base64 后按 ← 解码"
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
