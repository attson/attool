<script setup lang="ts">
import { convertFileSrc } from '@tauri-apps/api/core';
import { NButton, NEmpty } from 'naive-ui';
import type { ShapeKind, TemplateAsset, TemplateLayer } from '../../types/ecommerceTemplate';
import LayerTree from './LayerTree.vue';

export type ResourceTab = 'text' | 'image' | 'shape' | 'layers';

const props = defineProps<{
  activeTab: ResourceTab;
  layers: TemplateLayer[];
  assets: TemplateAsset[];
  selectedLayerId: string | null;
}>();

const emit = defineEmits<{
  'update:activeTab': [tab: ResourceTab];
  'add-text': [preset: 'title' | 'subtitle' | 'body' | 'price'];
  'add-shape': [shape: ShapeKind];
  'add-image': [];
  'add-asset-image': [asset: TemplateAsset];
  select: [layerId: string];
  reorder: [draggedLayerId: string, targetLayerId: string, placement: 'before' | 'after'];
}>();

const tabs: { key: ResourceTab; label: string }[] = [
  { key: 'text', label: '文字' },
  { key: 'image', label: '图片' },
  { key: 'shape', label: '素材' },
  { key: 'layers', label: '图层' }
];

const textPresets = [
  { key: 'title', title: '标题', sample: '双击编辑标题' },
  { key: 'subtitle', title: '副标题', sample: '输入副标题' },
  { key: 'body', title: '正文', sample: '输入正文' },
  { key: 'price', title: '价格', sample: '¥99' }
] as const;

const shapePresets: { key: ShapeKind; title: string }[] = [
  { key: 'rect', title: '矩形' },
  { key: 'roundRect', title: '圆角矩形' },
  { key: 'ellipse', title: '椭圆/圆形' },
  { key: 'line', title: '线条' }
];

function assetPreviewSrc(asset: TemplateAsset) {
  return convertFileSrc(asset.path);
}
</script>

<template>
  <div class="template-workbench-rail">
    <button v-for="tab in tabs" :key="tab.key" type="button" :class="['template-rail-button', { active: activeTab === tab.key }]" @click="emit('update:activeTab', tab.key)">
      <span>{{ tab.label }}</span>
    </button>
  </div>

  <aside class="template-resource-panel">
    <template v-if="props.activeTab === 'text'">
      <div class="template-resource-heading">
        <h3>添加文字</h3>
        <p>选择一个文字样式插入画布</p>
      </div>
      <div class="template-preset-grid">
        <button v-for="preset in textPresets" :key="preset.key" type="button" class="template-text-preset" @click="emit('add-text', preset.key)">
          <strong>{{ preset.title }}</strong>
          <span>{{ preset.sample }}</span>
        </button>
      </div>
    </template>

    <template v-else-if="props.activeTab === 'image'">
      <div class="template-resource-heading">
        <h3>添加图片</h3>
      </div>
      <n-button type="primary" block @click="emit('add-image')">选择本地图片</n-button>
      <div v-if="props.assets.length" class="template-image-asset-grid">
        <button v-for="asset in props.assets" :key="asset.id" type="button" class="template-image-asset" @click="emit('add-asset-image', asset)">
          <img :src="assetPreviewSrc(asset)" :alt="asset.name" />
          <span>{{ asset.name }}</span>
        </button>
      </div>
    </template>

    <template v-else-if="props.activeTab === 'shape'">
      <div class="template-resource-heading">
        <h3>素材 / 形状</h3>
        <p>插入基础形状搭建模板</p>
      </div>
      <div class="template-preset-grid">
        <button v-for="shape in shapePresets" :key="shape.key" type="button" class="template-shape-preset" @click="emit('add-shape', shape.key)">
          <span :class="['shape-preview', shape.key]" />
          <strong>{{ shape.title }}</strong>
        </button>
      </div>
    </template>

    <template v-else>
      <div class="template-resource-heading">
        <h3>图层</h3>
        <p>选择和管理当前模板图层</p>
      </div>
      <LayerTree :layers="props.layers" :selected-layer-id="props.selectedLayerId" @select="emit('select', $event)" @reorder="(draggedLayerId, targetLayerId, placement) => emit('reorder', draggedLayerId, targetLayerId, placement)" />
      <n-empty v-if="!props.layers.length" description="暂无图层" />
    </template>
  </aside>
</template>
