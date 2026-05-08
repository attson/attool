<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { NAlert, NButton, NCard, NPageHeader, NSpace, NTag } from 'naive-ui';
import type { ShapeKind, TemplateAsset, TemplateLayer, TemplateProject, TemplateSummary } from '../../types/ecommerceTemplate';
import {
  collectBindingKeys,
  createImageLayer,
  createShapeLayer,
  createTextLayer,
  duplicateLayer,
  flattenLayers,
  insertLayer,
  moveLayer,
  reorderLayer,
  removeSelectedLayer,
  updateLayerById
} from '../../utils/ecommerceTemplate';
import LayerProperties from './LayerProperties.vue';
import TemplateResourcePanel, { type ResourceTab } from './TemplateResourcePanel.vue';
import TemplateCanvas from './TemplateCanvas.vue';
import BatchPanel from './BatchPanel.vue';
import { createEmptyTemplateProject } from './templateDefaults';

const templates = ref<TemplateSummary[]>([]);
const project = ref<TemplateProject>(createEmptyTemplateProject());
const selectedLayerId = ref<string | null>(null);
const notice = ref('');
const importing = ref(false);
const saving = ref(false);
const activeResourceTab = ref<ResourceTab>('text');
const pasteTarget = ref<'canvas' | 'library'>('canvas');

watch(activeResourceTab, (tab) => {
  pasteTarget.value = tab === 'image' ? 'library' : 'canvas';
});

function focusPasteTargetCanvas() {
  pasteTarget.value = 'canvas';
}

function focusPasteTargetLibrary() {
  if (activeResourceTab.value === 'image') pasteTarget.value = 'library';
}

function removeAsset(asset: TemplateAsset) {
  project.value = touch({
    ...project.value,
    assets: project.value.assets.filter((item) => item.id !== asset.id)
  });
}

const selectedLayer = computed(() => flattenLayers(project.value.layers).find((layer) => layer.id === selectedLayerId.value) ?? null);
const requiredFields = computed(() => collectBindingKeys(project.value.layers));

onMounted(loadTemplateList);
onMounted(() => window.addEventListener('paste', handlePaste));
onUnmounted(() => window.removeEventListener('paste', handlePaste));

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

function touch(next: TemplateProject): TemplateProject {
  return { ...next, updatedAt: new Date().toLocaleString() };
}

function updateLayer(updated: TemplateLayer) {
  project.value = touch({ ...project.value, layers: updateLayerById(project.value.layers, updated.id, () => updated) });
}

function reorderLayers(draggedLayerId: string, targetLayerId: string, placement: 'before' | 'after') {
  const layers = reorderLayer(project.value.layers, draggedLayerId, targetLayerId, placement);
  if (layers === project.value.layers) return;
  project.value = touch({ ...project.value, layers });
  selectedLayerId.value = draggedLayerId;
}

function addTextLayer(preset: 'title' | 'subtitle' | 'body' | 'price') {
  const layer = createTextLayer({ canvasWidth: project.value.canvasWidth, canvasHeight: project.value.canvasHeight, preset });
  project.value = insertLayer(project.value, layer);
  selectedLayerId.value = layer.id;
}

function addShapeLayer(shape: ShapeKind) {
  const layer = createShapeLayer({ canvasWidth: project.value.canvasWidth, canvasHeight: project.value.canvasHeight, shape });
  project.value = insertLayer(project.value, layer);
  selectedLayerId.value = layer.id;
}

async function addImageLayer() {
  notice.value = '';
  try {
    const selected = await open({ multiple: false, filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp'] }] });
    if (typeof selected !== 'string') return;
    const asset = await invoke<TemplateAsset>('import_template_asset_from_path', {
      projectId: project.value.id,
      sourcePath: selected
    });
    const layer = createImageLayer({ canvasWidth: project.value.canvasWidth, canvasHeight: project.value.canvasHeight, asset });
    project.value = touch({ ...insertLayer(project.value, layer), assets: [...project.value.assets, asset] });
    selectedLayerId.value = layer.id;
  } catch (error) {
    notice.value = String(error);
  }
}

function addAssetImageLayer(asset: TemplateAsset) {
  const layer = createImageLayer({ canvasWidth: project.value.canvasWidth, canvasHeight: project.value.canvasHeight, asset });
  layer.name = asset.name;
  project.value = insertLayer(project.value, layer);
  selectedLayerId.value = layer.id;
}

async function handlePaste(event: ClipboardEvent) {
  if (isTypingTarget(event.target)) return;

  const file = findClipboardImage(event);
  if (!file) return;

  event.preventDefault();
  notice.value = '';
  try {
    const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
    const asset = await invoke<TemplateAsset>('save_pasted_template_asset', {
      projectId: project.value.id,
      name: file.name || `粘贴图片-${new Date().toLocaleTimeString()}.png`,
      mimeType: file.type || 'image/png',
      bytes
    });
    if (pasteTarget.value === 'canvas') {
      const layer = createImageLayer({ canvasWidth: project.value.canvasWidth, canvasHeight: project.value.canvasHeight, asset });
      project.value = touch({ ...insertLayer(project.value, layer), assets: [...project.value.assets, asset] });
      selectedLayerId.value = layer.id;
    } else {
      project.value = touch({ ...project.value, assets: [...project.value.assets, asset] });
    }
  } catch (error) {
    notice.value = String(error);
  }
}

function findClipboardImage(event: ClipboardEvent): File | null {
  const items = Array.from(event.clipboardData?.items ?? []);
  for (const item of items) {
    if (item.type.startsWith('image/')) {
      return item.getAsFile();
    }
  }
  return null;
}

function isTypingTarget(target: EventTarget | null) {
  const element = target instanceof HTMLElement ? target : null;
  if (!element) return false;
  return Boolean(element.closest('input, textarea, [contenteditable="true"]'));
}

function handleLayerAction(action: 'duplicate' | 'delete' | 'front' | 'back' | 'lock' | 'toggle-visible', layer: TemplateLayer) {
  if (action === 'delete') {
    const result = removeSelectedLayer(project.value, layer.id);
    project.value = result.project;
    selectedLayerId.value = result.selectedLayerId;
    return;
  }
  if (action === 'duplicate') {
    project.value = touch({ ...project.value, layers: duplicateLayer(project.value.layers, layer.id) });
    return;
  }
  if (action === 'front' || action === 'back') {
    project.value = touch({ ...project.value, layers: moveLayer(project.value.layers, layer.id, action) });
    return;
  }
  if (action === 'lock') {
    updateLayer({ ...layer, locked: !layer.locked });
    return;
  }
  if (action === 'toggle-visible') {
    updateLayer({ ...layer, visible: !layer.visible });
  }
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

    <div class="template-workbench">
      <TemplateResourcePanel
        v-model:active-tab="activeResourceTab"
        :layers="project.layers"
        :assets="project.assets"
        :selected-layer-id="selectedLayerId"
        :is-paste-target="pasteTarget === 'library' && activeResourceTab === 'image'"
        @add-text="addTextLayer"
        @add-shape="addShapeLayer"
        @add-image="addImageLayer"
        @add-asset-image="addAssetImageLayer"
        @remove-asset="removeAsset"
        @panel-mousedown="focusPasteTargetLibrary"
        @select="selectLayer"
        @reorder="reorderLayers"
      />

      <n-card
        title="画布"
        size="small"
        :bordered="false"
        :class="['panel-card', 'template-canvas-card', { 'is-paste-target': pasteTarget === 'canvas' }]"
        @mousedown="focusPasteTargetCanvas"
      >
        <TemplateCanvas :canvas-width="project.canvasWidth" :canvas-height="project.canvasHeight" :layers="project.layers" :assets="project.assets" :selected-layer-id="selectedLayerId" @select="selectLayer" @update="updateLayer" @action="handleLayerAction" />
      </n-card>

      <n-card title="属性" size="small" :bordered="false" class="panel-card template-editor-panel">
        <template #header-extra>
          <n-button
            text
            class="template-header-action"
            :class="{ 'is-active': selectedLayer?.visible ?? true }"
            :disabled="!selectedLayer"
            :title="selectedLayer?.visible === false ? '显示图层' : '隐藏图层'"
            @click="selectedLayer && updateLayer({ ...selectedLayer, visible: !selectedLayer.visible })"
          >
            {{ selectedLayer?.visible === false ? '🚫' : '👁' }}
          </n-button>
          <n-button
            text
            class="template-header-action"
            :class="{ 'is-active': Boolean(selectedLayer?.locked) }"
            :disabled="!selectedLayer"
            :title="selectedLayer?.locked ? '解锁图层' : '锁定图层'"
            @click="selectedLayer && updateLayer({ ...selectedLayer, locked: !selectedLayer.locked })"
          >
            {{ selectedLayer?.locked ? '🔒' : '🔓' }}
          </n-button>
        </template>
        <LayerProperties :layer="selectedLayer" @update="updateLayer" />
      </n-card>
    </div>

    <n-card title="批量生成" size="small" :bordered="false" class="panel-card">
      <BatchPanel :template-id="project.id" :required-fields="requiredFields" />
    </n-card>
  </n-space>
</template>
