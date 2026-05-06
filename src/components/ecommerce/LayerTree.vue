<script setup lang="ts">
import { NEllipsis, NEmpty, NTag } from 'naive-ui';
import type { TemplateLayer } from '../../types/ecommerceTemplate';

const props = defineProps<{
  layers: TemplateLayer[];
  selectedLayerId: string | null;
}>();

const emit = defineEmits<{
  select: [layerId: string];
}>();

function renderType(layer: TemplateLayer) {
  const labels = { text: '文字', image: '图片', shape: '形状', group: '组合' };
  return labels[layer.type];
}
</script>

<template>
  <div class="template-layer-tree">
    <n-empty v-if="props.layers.length === 0" description="还没有图层" />
    <template v-for="layer in props.layers" :key="layer.id">
      <button
        type="button"
        :class="['template-layer-item', { active: layer.id === props.selectedLayerId }]"
        @click="emit('select', layer.id)"
      >
        <n-ellipsis :tooltip="false">{{ layer.name }}</n-ellipsis>
        <n-tag size="small" round>{{ renderType(layer) }}</n-tag>
      </button>
      <button
        v-for="child in layer.children ?? []"
        :key="child.id"
        type="button"
        :class="['template-layer-item', 'child', { active: child.id === props.selectedLayerId }]"
        @click="emit('select', child.id)"
      >
        <n-ellipsis :tooltip="false">{{ child.name }}</n-ellipsis>
        <n-tag size="small" round>{{ renderType(child) }}</n-tag>
      </button>
    </template>
  </div>
</template>
