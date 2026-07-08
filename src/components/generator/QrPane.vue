<script setup lang="ts">
import { ref, watch } from 'vue';
import { NAlert, NButton, NInput, NInputNumber, NSelect } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import { save as saveDialog } from '@tauri-apps/plugin-dialog';
import Panel from '../ui/Panel.vue';

const text = ref('https://example.com');
const ecLevel = ref('M');
const modulePixels = ref(8);
const quietZone = ref(4);
const pngBase64 = ref('');
const error = ref('');
const generating = ref(false);

const ecOptions = [
  { label: 'L（容错 ~7%）', value: 'L' },
  { label: 'M（容错 ~15%）', value: 'M' },
  { label: 'Q（容错 ~25%）', value: 'Q' },
  { label: 'H（容错 ~30%）', value: 'H' }
];

let debounce: ReturnType<typeof setTimeout> | null = null;
watch([text, ecLevel, modulePixels, quietZone], () => {
  if (debounce) clearTimeout(debounce);
  debounce = setTimeout(run, 250);
}, { immediate: true });

async function run() {
  error.value = '';
  if (!text.value.trim()) {
    pngBase64.value = '';
    return;
  }
  generating.value = true;
  try {
    pngBase64.value = await invoke<string>('generate_qr_png', {
      text: text.value,
      ecLevel: ecLevel.value,
      modulePixels: modulePixels.value,
      quietZone: quietZone.value
    });
  } catch (err) {
    pngBase64.value = '';
    error.value = String(err);
  } finally {
    generating.value = false;
  }
}

async function saveAs() {
  if (!pngBase64.value) return;
  const target = await saveDialog({
    defaultPath: 'qrcode.png',
    filters: [{ name: 'PNG', extensions: ['png'] }]
  }).catch(() => null);
  if (!target) return;
  await invoke('write_binary_file', { path: target, base64: pngBase64.value }).catch((err) => {
    console.warn('[qr] save failed', err);
  });
}
</script>

<template>
  <div class="pane">
    <Panel title="内容">
      <div class="form">
        <n-input v-model:value="text" type="textarea" placeholder="URL / 文本" :autosize="{ minRows: 3, maxRows: 8 }" />
        <div class="opt-row">
          <label class="field">
            <span class="lbl">纠错级别</span>
            <n-select v-model:value="ecLevel" :options="ecOptions" size="small" style="width: 160px" />
          </label>
          <label class="field">
            <span class="lbl">模块像素</span>
            <n-input-number v-model:value="modulePixels" :min="1" :max="40" size="small" style="width: 100px" />
          </label>
          <label class="field">
            <span class="lbl">留白</span>
            <n-input-number v-model:value="quietZone" :min="0" :max="20" size="small" style="width: 100px" />
          </label>
        </div>
      </div>
    </Panel>

    <n-alert v-if="error" type="error" :bordered="false">{{ error }}</n-alert>

    <Panel v-if="pngBase64" title="预览">
      <template #right>
        <n-button size="tiny" secondary @click="saveAs">保存 PNG</n-button>
      </template>
      <div class="preview">
        <img :src="'data:image/png;base64,' + pngBase64" alt="qr" />
      </div>
    </Panel>
  </div>
</template>

<style scoped>
.pane { display: grid; gap: 12px; padding: 16px; height: 100%; overflow: auto; }
.form { display: grid; gap: 12px; }
.opt-row { display: flex; gap: 20px; flex-wrap: wrap; }
.field { display: grid; gap: 6px; }
.lbl { font-size: var(--fs-xxs); color: var(--text-muted); }
.preview {
  display: grid;
  place-items: center;
  padding: 16px;
  background: var(--bg-elev);
  border-radius: var(--radius-md);
}
.preview img {
  image-rendering: pixelated;
  max-width: 100%;
  max-height: 400px;
  background: #fff;
  padding: 8px;
  border-radius: 4px;
}
</style>
