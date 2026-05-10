<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { NAlert, NButton, NCard, NFlex, NInput, NModal, NPageHeader, NSpace, NTag } from 'naive-ui';
import type { ShapeKind, TemplateAsset, TemplateLayer, TemplateProject, TemplateSummary } from '../../types/ecommerceTemplate';
import {
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
import BatchTaskPanel from './BatchTaskPanel.vue';
import { createEmptyTemplateProject } from './templateDefaults';

const templates = ref<TemplateSummary[]>([]);
const project = ref<TemplateProject>(createEmptyTemplateProject());
const selectedLayerId = ref<string | null>(null);
const notice = ref('');
const importing = ref(false);
const saving = ref(false);
const activeResourceTab = ref<ResourceTab>('text');
const pasteTarget = ref<'canvas' | 'library'>('canvas');

const PROPS_COLLAPSED_KEY = 'attool.template.propsCollapsed';
const propsCollapsed = ref(localStorage.getItem(PROPS_COLLAPSED_KEY) !== '0');
function togglePropsCollapsed() {
  propsCollapsed.value = !propsCollapsed.value;
  localStorage.setItem(PROPS_COLLAPSED_KEY, propsCollapsed.value ? '1' : '0');
}

type NameDialogAction = { type: 'save' } | { type: 'rename'; id: string };
const nameDialogVisible = ref(false);
const nameDialogTitle = ref('');
const nameDialogValue = ref('');
const nameDialogBusy = ref(false);
const nameDialogAction = ref<NameDialogAction>({ type: 'save' });
const batchPanelRef = ref<InstanceType<typeof BatchTaskPanel> | null>(null);

watch(activeResourceTab, (tab) => {
  pasteTarget.value = tab === 'image' ? 'library' : 'canvas';
});

function focusPasteTargetCanvas() {
  pasteTarget.value = 'canvas';
}

function focusPasteTargetLibrary() {
  if (activeResourceTab.value === 'image') pasteTarget.value = 'library';
}

async function removeAsset(asset: TemplateAsset) {
  notice.value = '';
  try {
    await invoke('delete_template_asset', { assetId: asset.id });
    project.value = touch({
      ...project.value,
      assets: project.value.assets.filter((item) => item.id !== asset.id)
    });
  } catch (error) {
    notice.value = String(error);
  }
}

async function loadAssetLibrary() {
  try {
    const assets = await invoke<TemplateAsset[]>('list_template_assets');
    project.value = { ...project.value, assets };
  } catch (error) {
    notice.value = String(error);
  }
}

const selectedLayer = computed(() => flattenLayers(project.value.layers).find((layer) => layer.id === selectedLayerId.value) ?? null);

onMounted(loadTemplateList);
onMounted(loadAssetLibrary);
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

async function loadTemplate(id: string) {
  if (id === project.value.id) return;
  notice.value = '';
  try {
    const loaded = await invoke<TemplateProject>('load_ecommerce_template', { id });
    project.value = loaded;
    selectedLayerId.value = loaded.layers[0]?.id ?? null;
    await loadAssetLibrary();
  } catch (error) {
    notice.value = String(error);
  }
}

function requestSaveTemplate() {
  nameDialogAction.value = { type: 'save' };
  nameDialogTitle.value = '保存模板';
  nameDialogValue.value = project.value.name;
  nameDialogVisible.value = true;
}

function requestRenameTemplate(template: TemplateSummary) {
  nameDialogAction.value = { type: 'rename', id: template.id };
  nameDialogTitle.value = '重命名模板';
  nameDialogValue.value = template.name;
  nameDialogVisible.value = true;
}

async function confirmNameDialog() {
  const name = nameDialogValue.value.trim();
  if (!name) {
    notice.value = '模板名称不能为空';
    return;
  }
  nameDialogBusy.value = true;
  notice.value = '';
  try {
    if (nameDialogAction.value.type === 'save') {
      project.value = await invoke<TemplateProject>('save_ecommerce_template', {
        project: { ...project.value, name }
      });
      saving.value = false;
    } else {
      const id = nameDialogAction.value.id;
      const renamed = await invoke<TemplateProject>('rename_ecommerce_template', { id, name });
      if (id === project.value.id) {
        project.value = { ...project.value, name: renamed.name, updatedAt: renamed.updatedAt };
      }
    }
    await loadTemplateList();
    nameDialogVisible.value = false;
  } catch (error) {
    notice.value = String(error);
  } finally {
    nameDialogBusy.value = false;
  }
}

function openBatchTask(layer: TemplateLayer) {
  batchPanelRef.value?.addTaskForLayer(layer);
}

async function ensureProjectSaved() {
  project.value = await invoke<TemplateProject>('save_ecommerce_template', { project: project.value });
  await loadTemplateList();
}

async function deleteTemplate(template: TemplateSummary) {
  if (!window.confirm(`确认删除模板「${template.name}」？此操作无法撤销。`)) return;
  notice.value = '';
  try {
    await invoke('delete_ecommerce_template', { id: template.id });
    if (template.id === project.value.id) {
      project.value = createEmptyTemplateProject();
      project.value.assets = [];
      selectedLayerId.value = null;
      await loadAssetLibrary();
    }
    await loadTemplateList();
  } catch (error) {
    notice.value = String(error);
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
    const asset = await invoke<TemplateAsset>('import_template_asset_from_path', { sourcePath: selected });
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
    const asset = await invoke<TemplateAsset>('save_template_asset', {
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

function handleLayerAction(action: 'duplicate' | 'delete' | 'front' | 'back' | 'lock' | 'toggle-visible' | 'batch-replace', layer: TemplateLayer) {
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
    return;
  }
  if (action === 'batch-replace') {
    openBatchTask(layer);
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
          <n-button secondary @click="requestSaveTemplate">保存模板</n-button>
          <n-button type="primary" :loading="importing" @click="importPsd">导入 PSD</n-button>
        </n-space>
      </template>
    </n-page-header>

    <n-alert v-if="notice" type="error" :bordered="false">{{ notice }}</n-alert>

    <div class="template-workbench" :class="{ 'props-collapsed': propsCollapsed }">
      <TemplateResourcePanel
        v-model:active-tab="activeResourceTab"
        :layers="project.layers"
        :assets="project.assets"
        :templates="templates"
        :selected-layer-id="selectedLayerId"
        :current-template-id="project.id"
        :is-paste-target="pasteTarget === 'library' && activeResourceTab === 'image'"
        @add-text="addTextLayer"
        @add-shape="addShapeLayer"
        @add-image="addImageLayer"
        @add-asset-image="addAssetImageLayer"
        @remove-asset="removeAsset"
        @select-template="loadTemplate"
        @rename-template="requestRenameTemplate"
        @delete-template="deleteTemplate"
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
        <template #header-extra>
          <button
            type="button"
            class="template-props-toggle"
            :title="propsCollapsed ? '显示属性面板' : '隐藏属性面板'"
            @click="togglePropsCollapsed"
          >{{ propsCollapsed ? '‹' : '›' }}</button>
        </template>
        <TemplateCanvas :canvas-width="project.canvasWidth" :canvas-height="project.canvasHeight" :layers="project.layers" :assets="project.assets" :selected-layer-id="selectedLayerId" @select="selectLayer" @update="updateLayer" @action="handleLayerAction" />
      </n-card>

      <n-card v-if="!propsCollapsed" title="属性" size="small" :bordered="false" class="panel-card template-editor-panel">
        <template #header-extra>
          <n-button
            text
            class="template-header-action"
            :class="{ 'is-active': selectedLayer?.visible ?? true }"
            :disabled="!selectedLayer"
            :title="selectedLayer?.visible === false ? '显示图层' : '隐藏图层'"
            @click="selectedLayer && updateLayer({ ...selectedLayer, visible: !selectedLayer.visible })"
          >
            <svg v-if="selectedLayer?.visible === false" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94" />
              <path d="M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19" />
              <path d="M14.12 14.12a3 3 0 1 1-4.24-4.24" />
              <line x1="1" y1="1" x2="23" y2="23" />
            </svg>
            <svg v-else viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
              <circle cx="12" cy="12" r="3" />
            </svg>
          </n-button>
          <n-button
            text
            class="template-header-action"
            :class="{ 'is-active': Boolean(selectedLayer?.locked) }"
            :disabled="!selectedLayer"
            :title="selectedLayer?.locked ? '解锁图层' : '锁定图层'"
            @click="selectedLayer && updateLayer({ ...selectedLayer, locked: !selectedLayer.locked })"
          >
            <svg v-if="selectedLayer?.locked" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <rect x="4" y="11" width="16" height="10" rx="2" />
              <path d="M8 11V7a4 4 0 0 1 8 0v4" />
            </svg>
            <svg v-else viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <rect x="4" y="11" width="16" height="10" rx="2" />
              <path d="M8 11V7a4 4 0 0 1 7.5-2" />
            </svg>
          </n-button>
        </template>
        <LayerProperties :layer="selectedLayer" @update="updateLayer" />
      </n-card>
    </div>

    <n-card title="批量替换" size="small" :bordered="false" class="panel-card">
      <BatchTaskPanel ref="batchPanelRef" :template-id="project.id" :save-template="ensureProjectSaved" />
    </n-card>
  </n-space>

  <n-modal v-model:show="nameDialogVisible" preset="card" :title="nameDialogTitle" class="template-name-modal">
    <n-input
      v-model:value="nameDialogValue"
      placeholder="输入模板名称"
      autofocus
      @keyup.enter="confirmNameDialog"
    />
    <n-flex justify="end" :size="8" style="margin-top: 14px">
      <n-button @click="nameDialogVisible = false">取消</n-button>
      <n-button type="primary" :loading="nameDialogBusy" @click="confirmNameDialog">确认</n-button>
    </n-flex>
  </n-modal>
</template>
