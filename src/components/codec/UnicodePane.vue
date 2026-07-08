<script setup lang="ts">
import { ref } from 'vue';
import { NAlert, NButton, NInput } from 'naive-ui';
import { unicodeEscape, unicodeUnescape } from '../../utils/codec';
import Panel from '../ui/Panel.vue';
import PaneActions from './PaneActions.vue';

const plain = ref('');
const escaped = ref('');
const error = ref('');

function doEncode() {
  error.value = '';
  try {
    escaped.value = unicodeEscape(plain.value);
  } catch (err) {
    error.value = String(err);
  }
}

function doDecode() {
  error.value = '';
  try {
    plain.value = unicodeUnescape(escaped.value);
  } catch (err) {
    error.value = String(err);
  }
}
</script>

<template>
  <div class="pane">
    <div class="grid">
      <Panel title="原文">
        <n-input
          v-model:value="plain"
          type="textarea"
          placeholder="含中文/emoji 的文本"
          :autosize="{ minRows: 10, maxRows: 20 }"
        />
      </Panel>

      <div class="middle">
        <n-button block type="primary" @click="doEncode">转义 →</n-button>
        <n-button block secondary @click="doDecode">← 还原</n-button>
      </div>

      <Panel title="\uXXXX">
        <template #right>
          <PaneActions :value="escaped" />
        </template>
        <n-input
          v-model:value="escaped"
          type="textarea"
          placeholder="含 \uXXXX 的字符串"
          :autosize="{ minRows: 10, maxRows: 20 }"
        />
      </Panel>
    </div>

    <n-alert v-if="error" type="error" :bordered="false">{{ error }}</n-alert>
  </div>
</template>

<style scoped>
.pane { display: grid; gap: 12px; padding: 16px; height: 100%; overflow: auto; }
.grid { display: grid; grid-template-columns: 1fr 120px 1fr; gap: 12px; }
@media (max-width: 900px) { .grid { grid-template-columns: 1fr; } }
.middle { display: grid; gap: 8px; align-content: center; }
</style>
