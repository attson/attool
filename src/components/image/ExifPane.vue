<script setup lang="ts">
import { ref } from 'vue';
import { NAlert, NButton, NInput, NInputGroup } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import Panel from '../ui/Panel.vue';

interface ExifField {
  tag: string;
  value: string;
}

const inputPath = ref('');
const outputDir = ref('');
const fields = ref<ExifField[]>([]);
const notice = ref('');
const success = ref('');
const loading = ref(false);
const stripping = ref(false);

async function pickFile() {
  notice.value = '';
  success.value = '';
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'Images', extensions: ['jpg', 'jpeg'] }]
    });
    if (typeof selected === 'string') {
      inputPath.value = selected;
      await readExif();
    }
  } catch (err) {
    notice.value = String(err);
  }
}

async function pickOutputDir() {
  try {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === 'string') outputDir.value = selected;
  } catch (err) {
    notice.value = String(err);
  }
}

async function readExif() {
  if (!inputPath.value.trim()) {
    notice.value = '请先选择图片';
    return;
  }
  loading.value = true;
  notice.value = '';
  try {
    const result = await invoke<ExifField[]>('read_image_exif', { path: inputPath.value.trim() });
    fields.value = result;
    if (result.length === 0) {
      notice.value = '未找到 EXIF 信息（或图片不含 EXIF）';
    }
  } catch (err) {
    fields.value = [];
    notice.value = String(err);
  } finally {
    loading.value = false;
  }
}

async function stripExif() {
  notice.value = '';
  success.value = '';
  if (!inputPath.value.trim()) {
    notice.value = '请先选择图片';
    return;
  }
  if (!outputDir.value.trim()) {
    notice.value = '请选择输出目录';
    return;
  }
  stripping.value = true;
  try {
    const response = await invoke<{ outputPath: string }>('strip_image_exif', {
      request: {
        inputPath: inputPath.value.trim(),
        outputDir: outputDir.value.trim()
      }
    });
    success.value = `已生成无 EXIF 副本：${response.outputPath}`;
  } catch (err) {
    notice.value = String(err);
  } finally {
    stripping.value = false;
  }
}

function basename(path: string): string {
  const idx = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
  return idx >= 0 ? path.slice(idx + 1) : path;
}
</script>

<template>
  <div class="exif-pane">
    <Panel title="输入">
      <div class="form-row">
        <n-input-group>
          <n-input
            v-model:value="inputPath"
            placeholder="选择 JPEG 图片以查看 EXIF"
            @keyup.enter="readExif"
          />
          <n-button secondary @click="pickFile">选择图片</n-button>
          <n-button secondary :loading="loading" @click="readExif">读取</n-button>
        </n-input-group>
      </div>
    </Panel>

    <Panel title="EXIF 字段">
      <template #right>
        <span class="mono">{{ fields.length }} 项</span>
      </template>
      <div v-if="fields.length === 0" class="empty">
        {{ inputPath ? '尚未读取 EXIF 或此图不含 EXIF' : '还没有选择图片' }}
      </div>
      <table v-else class="exif-table">
        <thead>
          <tr>
            <th>字段</th>
            <th>值</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="field in fields" :key="field.tag">
            <td class="tag">{{ field.tag }}</td>
            <td class="value">{{ field.value }}</td>
          </tr>
        </tbody>
      </table>
    </Panel>

    <Panel title="清除 EXIF">
      <div class="form">
        <p class="note">
          目前仅支持 JPEG 清除。PNG / WebP 可先到"格式转换" tab 转成 JPEG 再回来清。
          清除会生成新副本，原图不动。
        </p>
        <label class="field">
          <span class="lbl">输出目录</span>
          <n-input-group>
            <n-input v-model:value="outputDir" placeholder="/Users/you/Pictures/no_exif" />
            <n-button secondary @click="pickOutputDir">选择</n-button>
          </n-input-group>
        </label>

        <n-alert v-if="notice" type="error" :bordered="false">{{ notice }}</n-alert>
        <n-alert v-if="success" type="success" :bordered="false">{{ success }}</n-alert>

        <n-button
          type="primary"
          block
          :loading="stripping"
          :disabled="!inputPath.trim() || !outputDir.trim()"
          @click="stripExif"
        >
          {{ stripping ? '清除中...' : '生成无 EXIF 副本' }}
        </n-button>
      </div>
    </Panel>
  </div>
</template>

<style scoped>
.exif-pane {
  display: grid;
  gap: 16px;
  padding: 16px;
  height: 100%;
  overflow: auto;
}
.form-row { display: grid; gap: 8px; }
.form { display: grid; gap: 12px; }
.field { display: grid; gap: 6px; }
.lbl { font-size: var(--fs-xxs); color: var(--text-muted); }
.note {
  margin: 0;
  color: var(--text-muted);
  font-size: var(--fs-xs);
  line-height: 1.6;
}
.empty {
  padding: 40px 16px;
  text-align: center;
  color: var(--text-muted);
  border: 1px dashed var(--line-strong);
  border-radius: var(--radius-md);
  font-size: var(--fs-sm);
}
.exif-table {
  width: 100%;
  border-collapse: collapse;
  font-size: var(--fs-sm);
}
.exif-table thead th {
  text-align: left;
  color: var(--text-muted);
  font-size: var(--fs-xxs);
  font-weight: 500;
  padding: 6px 10px;
  border-bottom: 1px solid var(--line);
}
.exif-table tbody td {
  padding: 6px 10px;
  border-bottom: 1px solid var(--line-weak, var(--line));
  vertical-align: top;
}
.tag { color: var(--text-muted); width: 40%; }
.value {
  font-family: var(--font-mono, monospace);
  font-size: var(--fs-xs);
  word-break: break-all;
}
.mono { font-family: var(--font-mono, monospace); font-size: var(--fs-xs); }
</style>
