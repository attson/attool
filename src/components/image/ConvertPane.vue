<script setup lang="ts">
import { computed, ref } from 'vue';
import { NAlert, NButton, NInput, NInputGroup, NSelect, NSlider } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import Panel from '../ui/Panel.vue';

interface ConvertItemResult {
  inputPath: string;
  outputPath: string;
  originalSize: number;
  convertedSize: number;
}

interface ConvertFailure {
  inputPath: string;
  message: string;
}

interface ConvertResponse {
  succeeded: ConvertItemResult[];
  failed: ConvertFailure[];
}

const inputPaths = ref<string[]>([]);
const outputDir = ref('');
const targetFormat = ref<'jpg' | 'png' | 'webp'>('jpg');
const quality = ref(90);
const submitting = ref(false);
const notice = ref('');
const succeeded = ref<ConvertItemResult[]>([]);
const failed = ref<ConvertFailure[]>([]);

const formatOptions = [
  { label: 'JPEG (.jpg)', value: 'jpg' },
  { label: 'PNG (.png)', value: 'png' },
  { label: 'WebP (.webp)', value: 'webp' }
];

const showQuality = computed(() => targetFormat.value === 'jpg' || targetFormat.value === 'webp');

async function pickFiles() {
  notice.value = '';
  try {
    const selected = await open({
      multiple: true,
      filters: [{ name: 'Images', extensions: ['jpg', 'jpeg', 'png', 'webp', 'gif', 'bmp'] }]
    });
    if (Array.isArray(selected)) inputPaths.value = selected;
    else if (typeof selected === 'string') inputPaths.value = [selected];
  } catch (err) {
    notice.value = String(err);
  }
}

async function pickOutputDir() {
  notice.value = '';
  try {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === 'string') outputDir.value = selected;
  } catch (err) {
    notice.value = String(err);
  }
}

function clearInputs() {
  inputPaths.value = [];
  succeeded.value = [];
  failed.value = [];
}

function removeInput(index: number) {
  inputPaths.value.splice(index, 1);
}

async function startConvert() {
  notice.value = '';
  succeeded.value = [];
  failed.value = [];

  if (inputPaths.value.length === 0) {
    notice.value = '请选择要转换的图片';
    return;
  }
  if (!outputDir.value.trim()) {
    notice.value = '请选择输出目录';
    return;
  }

  submitting.value = true;
  try {
    const response = await invoke<ConvertResponse>('convert_images', {
      request: {
        inputPaths: inputPaths.value,
        outputDir: outputDir.value.trim(),
        targetFormat: targetFormat.value,
        quality: quality.value
      }
    });
    succeeded.value = response.succeeded;
    failed.value = response.failed;
  } catch (err) {
    notice.value = String(err);
  } finally {
    submitting.value = false;
  }
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

function basename(path: string): string {
  const idx = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
  return idx >= 0 ? path.slice(idx + 1) : path;
}
</script>

<template>
  <div class="convert-pane">
    <div class="grid">
      <Panel title="输入">
        <template #right>
          <span class="mono">{{ inputPaths.length }} 张</span>
        </template>
        <div class="input-actions">
          <n-button secondary @click="pickFiles">选择图片</n-button>
          <n-button secondary v-if="inputPaths.length > 0" @click="clearInputs">清空</n-button>
        </div>
        <div v-if="inputPaths.length === 0" class="empty">还没有选择图片</div>
        <ul v-else class="input-list">
          <li v-for="(path, idx) in inputPaths" :key="path" class="input-item">
            <span class="filename">{{ basename(path) }}</span>
            <button class="remove-btn" @click="removeInput(idx)">×</button>
          </li>
        </ul>
      </Panel>

      <Panel title="参数">
        <div class="form">
          <label class="field">
            <span class="lbl">目标格式</span>
            <n-select v-model:value="targetFormat" :options="formatOptions" />
          </label>

          <label v-if="showQuality" class="field">
            <span class="lbl">质量（1-100）</span>
            <div class="quality-row">
              <n-slider v-model:value="quality" :min="1" :max="100" style="flex: 1" />
              <span class="quality-value mono">{{ quality }}</span>
            </div>
          </label>

          <label class="field">
            <span class="lbl">输出目录</span>
            <n-input-group>
              <n-input v-model:value="outputDir" placeholder="/Users/you/Pictures/converted" />
              <n-button secondary @click="pickOutputDir">选择</n-button>
            </n-input-group>
          </label>

          <n-alert v-if="notice" type="error" :bordered="false">{{ notice }}</n-alert>

          <n-button
            type="primary"
            block
            :loading="submitting"
            :disabled="inputPaths.length === 0 || !outputDir.trim()"
            @click="startConvert"
          >
            {{ submitting ? '转换中...' : '开始转换' }}
          </n-button>
        </div>
      </Panel>
    </div>

    <Panel v-if="succeeded.length > 0 || failed.length > 0" title="结果">
      <ul v-if="succeeded.length > 0" class="result-list">
        <li v-for="item in succeeded" :key="item.outputPath" class="result-item">
          <span class="filename">{{ basename(item.inputPath) }}</span>
          <span class="sizes mono">
            {{ formatSize(item.originalSize) }} → {{ formatSize(item.convertedSize) }}
          </span>
        </li>
      </ul>
      <ul v-if="failed.length > 0" class="result-list failed">
        <li v-for="item in failed" :key="item.inputPath" class="result-item">
          <span class="filename">{{ basename(item.inputPath) }}</span>
          <span class="err">{{ item.message }}</span>
        </li>
      </ul>
    </Panel>
  </div>
</template>

<style scoped>
.convert-pane {
  display: grid;
  gap: 16px;
  padding: 16px;
  height: 100%;
  overflow: auto;
}
.grid {
  display: grid;
  grid-template-columns: 1.1fr 1fr;
  gap: 16px;
}
@media (max-width: 1100px) {
  .grid { grid-template-columns: 1fr; }
}
.input-actions {
  display: flex;
  gap: 8px;
  margin-bottom: 8px;
}
.input-list {
  list-style: none;
  padding: 0;
  margin: 0;
  display: grid;
  gap: 4px;
  max-height: 260px;
  overflow: auto;
}
.input-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 10px;
  background: var(--bg-elev);
  border-radius: var(--radius-sm);
  font-size: var(--fs-sm);
}
.filename {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 320px;
}
.remove-btn {
  background: none;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  font-size: 16px;
  line-height: 1;
  padding: 0 4px;
}
.remove-btn:hover { color: var(--text); }
.form { display: grid; gap: 12px; }
.field { display: grid; gap: 6px; }
.lbl {
  font-size: var(--fs-xxs);
  color: var(--text-muted);
}
.quality-row {
  display: flex;
  align-items: center;
  gap: 12px;
}
.quality-value {
  min-width: 32px;
  text-align: right;
}
.empty {
  padding: 40px 16px;
  text-align: center;
  color: var(--text-muted);
  border: 1px dashed var(--line-strong);
  border-radius: var(--radius-md);
  font-size: var(--fs-sm);
}
.result-list {
  list-style: none;
  padding: 0;
  margin: 0;
  display: grid;
  gap: 4px;
}
.result-list.failed { margin-top: 8px; }
.result-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 10px;
  background: var(--bg-elev);
  border-radius: var(--radius-sm);
  font-size: var(--fs-sm);
}
.sizes { color: var(--text-muted); font-size: var(--fs-xs); }
.err { color: var(--danger, #f87171); font-size: var(--fs-xs); }
.mono { font-family: var(--font-mono, monospace); font-size: var(--fs-xs); }
</style>
