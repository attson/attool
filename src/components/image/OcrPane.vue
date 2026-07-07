<script setup lang="ts">
import { ref } from 'vue';
import { NAlert, NButton, NInput, NInputGroup, NSelect } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import Panel from '../ui/Panel.vue';

const inputPath = ref('');
const langs = ref('eng+chi_sim');
const text = ref('');
const notice = ref('');
const loading = ref(false);

const langOptions = [
  { label: '英文 + 简体中文', value: 'eng+chi_sim' },
  { label: '英文 + 繁体中文', value: 'eng+chi_tra' },
  { label: '仅简体中文', value: 'chi_sim' },
  { label: '仅英文', value: 'eng' },
  { label: '英文 + 日文', value: 'eng+jpn' }
];

async function pickFile() {
  notice.value = '';
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'Images', extensions: ['jpg', 'jpeg', 'png', 'webp', 'tiff', 'bmp'] }]
    });
    if (typeof selected === 'string') {
      inputPath.value = selected;
    }
  } catch (err) {
    notice.value = String(err);
  }
}

async function runOcr() {
  notice.value = '';
  text.value = '';
  if (!inputPath.value.trim()) {
    notice.value = '请先选择图片';
    return;
  }
  loading.value = true;
  try {
    const response = await invoke<{ text: string }>('ocr_image', {
      request: {
        inputPath: inputPath.value.trim(),
        langs: langs.value
      }
    });
    text.value = response.text;
    if (!text.value.trim()) {
      notice.value = '未识别到任何文字';
    }
  } catch (err) {
    notice.value = String(err);
  } finally {
    loading.value = false;
  }
}

async function copyText() {
  if (!text.value.trim()) return;
  try {
    await writeText(text.value);
    notice.value = '已复制到剪贴板';
  } catch (err) {
    notice.value = String(err);
  }
}

function mergeParagraphs() {
  text.value = text.value
    .split(/\n{2,}/)
    .map((p) => p.replace(/\n/g, ' ').replace(/\s+/g, ' ').trim())
    .filter(Boolean)
    .join('\n\n');
}
</script>

<template>
  <div class="ocr-pane">
    <Panel title="输入">
      <div class="form">
        <label class="field">
          <span class="lbl">图片</span>
          <n-input-group>
            <n-input v-model:value="inputPath" placeholder="选择 JPG / PNG 图片" />
            <n-button secondary @click="pickFile">选择</n-button>
          </n-input-group>
        </label>

        <label class="field">
          <span class="lbl">识别语言</span>
          <n-select v-model:value="langs" :options="langOptions" />
        </label>

        <p class="note">
          依赖本机 tesseract：macOS <code>brew install tesseract tesseract-lang</code> /
          Ubuntu <code>apt install tesseract-ocr tesseract-ocr-chi-sim</code> /
          Windows 装 UB-Mannheim tesseract 并加入 PATH。
        </p>

        <n-alert v-if="notice" type="info" :bordered="false">{{ notice }}</n-alert>

        <n-button
          type="primary"
          block
          :loading="loading"
          :disabled="!inputPath.trim()"
          @click="runOcr"
        >
          {{ loading ? '识别中...' : '开始识别' }}
        </n-button>
      </div>
    </Panel>

    <Panel title="识别结果">
      <template #right>
        <div class="actions">
          <n-button size="small" secondary :disabled="!text.trim()" @click="mergeParagraphs">
            合并段落
          </n-button>
          <n-button size="small" secondary :disabled="!text.trim()" @click="copyText">
            复制
          </n-button>
        </div>
      </template>
      <n-input
        v-model:value="text"
        type="textarea"
        :autosize="{ minRows: 12, maxRows: 30 }"
        placeholder="识别后的文字会显示在这里，可以直接编辑再复制"
      />
    </Panel>
  </div>
</template>

<style scoped>
.ocr-pane {
  display: grid;
  gap: 16px;
  padding: 16px;
  height: 100%;
  overflow: auto;
}
.form { display: grid; gap: 12px; }
.field { display: grid; gap: 6px; }
.lbl { font-size: var(--fs-xxs); color: var(--text-muted); }
.note {
  margin: 0;
  color: var(--text-muted);
  font-size: var(--fs-xs);
  line-height: 1.6;
}
.note code {
  padding: 1px 6px;
  background: var(--bg-elev);
  border-radius: 3px;
  font-family: var(--font-mono, monospace);
}
.actions {
  display: flex;
  gap: 8px;
}
</style>
