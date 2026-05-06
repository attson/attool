<script setup lang="ts">
import { computed, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { NAlert, NButton, NDataTable, NEmpty, NSpace, NTag } from 'naive-ui';
import type { BatchDataPreview, BatchRow, ExportResult } from '../../types/ecommerceTemplate';

const props = defineProps<{
  templateId: string;
  requiredFields: string[];
}>();

const rows = ref<BatchRow[]>([]);
const preview = ref<BatchDataPreview | null>(null);
const outputDir = ref('');
const notice = ref('');
const importing = ref(false);
const exporting = ref(false);
const result = ref<ExportResult | null>(null);

const columns = computed(() =>
  (preview.value?.fields ?? []).slice(0, 8).map((field) => ({ title: field, key: field, render: (row: BatchRow) => row.values[field] ?? '' }))
);

async function importTable() {
  notice.value = '';
  importing.value = true;
  try {
    const selected = await open({ multiple: false, filters: [{ name: 'Table', extensions: ['csv', 'xlsx', 'xls'] }] });
    if (typeof selected === 'string') {
      preview.value = await invoke<BatchDataPreview>('import_batch_table', { path: selected, requiredFields: props.requiredFields });
      rows.value = preview.value.rows;
    }
  } catch (error) {
    notice.value = String(error);
  } finally {
    importing.value = false;
  }
}

async function importFolder() {
  notice.value = '';
  importing.value = true;
  try {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === 'string') {
      preview.value = await invoke<BatchDataPreview>('create_batch_from_folder', { folderPath: selected, imageBindingKey: 'product_image' });
      rows.value = preview.value.rows;
    }
  } catch (error) {
    notice.value = String(error);
  } finally {
    importing.value = false;
  }
}

async function chooseOutputDir() {
  const selected = await open({ directory: true, multiple: false, defaultPath: outputDir.value || undefined });
  if (typeof selected === 'string') outputDir.value = selected;
}

async function exportRows() {
  notice.value = '';
  result.value = null;
  if (!outputDir.value || rows.value.length === 0) {
    notice.value = '请选择输出目录并导入批量数据。';
    return;
  }
  exporting.value = true;
  try {
    result.value = await invoke<ExportResult>('export_ecommerce_images', {
      request: { templateId: props.templateId, outputDir: outputDir.value, rows: rows.value }
    });
  } catch (error) {
    notice.value = String(error);
  } finally {
    exporting.value = false;
  }
}
</script>

<template>
  <n-space vertical :size="12">
    <n-space>
      <n-button secondary :loading="importing" @click="importTable">导入表格</n-button>
      <n-button secondary :loading="importing" @click="importFolder">图片文件夹模式</n-button>
      <n-button secondary @click="chooseOutputDir">选择输出目录</n-button>
      <n-button type="primary" :loading="exporting" @click="exportRows">导出 PNG</n-button>
    </n-space>

    <n-alert v-if="notice" type="error" :bordered="false">{{ notice }}</n-alert>
    <n-alert v-if="preview?.missingFields.length" type="warning" :bordered="false">
      缺失字段：{{ preview.missingFields.join(', ') }}。缺失字段会使用模板默认值。
    </n-alert>
    <n-alert v-if="preview?.unusedFields.length" type="info" :bordered="false">
      未使用字段：{{ preview.unusedFields.join(', ') }}。
    </n-alert>

    <n-empty v-if="rows.length === 0" description="还没有批量数据" />
    <n-data-table v-else size="small" :columns="columns" :data="rows" :pagination="{ pageSize: 6 }" />

    <n-alert v-if="result" type="success" :bordered="false">
      共 {{ result.total }} 张，成功 {{ result.succeeded }} 张，失败 {{ result.failed.length }} 张。
    </n-alert>
    <n-space v-if="outputDir" align="center">
      <n-tag round>输出目录</n-tag><span class="template-output-dir">{{ outputDir }}</span>
    </n-space>
  </n-space>
</template>
