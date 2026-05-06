<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { NAlert, NButton, NCard, NGrid, NGridItem, NPageHeader, NSpace, NTag } from 'naive-ui';
import type { TemplateLayer, TemplateProject, TemplateSummary } from '../../types/ecommerceTemplate';
import { flattenLayers } from '../../utils/ecommerceTemplate';
import LayerProperties from './LayerProperties.vue';
import LayerTree from './LayerTree.vue';
import TemplateCanvas from './TemplateCanvas.vue';
import { createEmptyTemplateProject } from './templateDefaults';

const templates = ref<TemplateSummary[]>([]);
const project = ref<TemplateProject>(createEmptyTemplateProject());
const selectedLayerId = ref<string | null>(null);
const notice = ref('');
const importing = ref(false);
const saving = ref(false);

const selectedLayer = computed(() => flattenLayers(project.value.layers).find((layer) => layer.id === selectedLayerId.value) ?? null);

onMounted(loadTemplateList);

async function loadTemplateList() {
  templates.value = await invoke<TemplateSummary[]>('list_ecommerce_templates');
}

async function importPsd() {
  notice.value = '';
  importing.value = true;
  try {
    const selected = await open({ multiple: false, filters: [{ name: 'PSD', extensions: ['psd'] }] });
    if (typeof selected === 'string') {
      project.value = await invoke<TemplateProject>('import_psd_template', { psdPath: selected });
      selectedLayerId.value = project.value.layers[0]?.id ?? null;
      await loadTemplateList();
    }
  } catch (error) {
    notice.value = String(error);
  } finally {
    importing.value = false;
  }
}

async function saveTemplate() {
  notice.value = '';
  saving.value = true;
  try {
    project.value = await invoke<TemplateProject>('save_ecommerce_template', { project: project.value });
    await loadTemplateList();
  } catch (error) {
    notice.value = String(error);
  } finally {
    saving.value = false;
  }
}

function selectLayer(layerId: string) {
  selectedLayerId.value = layerId;
}

function updateLayer(updated: TemplateLayer) {
  const replace = (layers: TemplateLayer[]): TemplateLayer[] =>
    layers.map((layer) => {
      if (layer.id === updated.id) return updated;
      if (layer.children) return { ...layer, children: replace(layer.children) };
      return layer;
    });
  project.value = { ...project.value, layers: replace(project.value.layers), updatedAt: new Date().toLocaleString() };
}
</script>

<template>
  <n-space vertical :size="16">
    <n-page-header subtitle="导入 PSD 生成模板草稿，绑定字段后批量导出 PNG 主图。">
      <template #title>电商主图模板</template>
      <template #extra>
        <n-space>
          <n-tag round>{{ project.canvasWidth }}x{{ project.canvasHeight }}</n-tag>
          <n-button secondary :loading="saving" @click="saveTemplate">保存模板</n-button>
          <n-button type="primary" :loading="importing" @click="importPsd">导入 PSD</n-button>
        </n-space>
      </template>
    </n-page-header>

    <n-alert v-if="notice" type="error" :bordered="false">{{ notice }}</n-alert>

    <n-grid responsive="screen" cols="1 l:5" :x-gap="12" :y-gap="12">
      <n-grid-item span="1">
        <n-card title="图层" size="small" :bordered="false" class="panel-card template-editor-panel">
          <LayerTree :layers="project.layers" :selected-layer-id="selectedLayerId" @select="selectLayer" @update="updateLayer" />
        </n-card>
      </n-grid-item>
      <n-grid-item span="1 l:3">
        <n-card title="画布" size="small" :bordered="false" class="panel-card template-canvas-card">
          <TemplateCanvas :canvas-width="project.canvasWidth" :canvas-height="project.canvasHeight" :layers="project.layers" :assets="project.assets" :selected-layer-id="selectedLayerId" @select="selectLayer" @update="updateLayer" />
        </n-card>
      </n-grid-item>
      <n-grid-item span="1">
        <n-card title="属性" size="small" :bordered="false" class="panel-card template-editor-panel">
          <LayerProperties :layer="selectedLayer" @update="updateLayer" />
        </n-card>
      </n-grid-item>
    </n-grid>
  </n-space>
</template>
