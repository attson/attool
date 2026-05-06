<script setup lang="ts">
import { computed, ref } from 'vue';
import { convertFileSrc } from '@tauri-apps/api/core';
import type { TemplateAsset, TemplateLayer } from '../../types/ecommerceTemplate';
import { flattenLayers } from '../../utils/ecommerceTemplate';

const props = defineProps<{
  canvasWidth: number;
  canvasHeight: number;
  layers: TemplateLayer[];
  assets: TemplateAsset[];
  selectedLayerId: string | null;
}>();

const emit = defineEmits<{
  select: [layerId: string];
  update: [layer: TemplateLayer];
}>();

const flatLayers = computed(() => flattenLayers(props.layers).filter((layer) => layer.type !== 'group' && layer.visible));
const canvasStyle = computed(() => ({ aspectRatio: `${props.canvasWidth} / ${props.canvasHeight}` }));

const interaction = ref<null | {
  mode: 'move' | 'resize';
  layer: TemplateLayer;
  startX: number;
  startY: number;
  startLayerX: number;
  startLayerY: number;
  startWidth: number;
  startHeight: number;
}>(null);

function layerStyle(layer: TemplateLayer) {
  return {
    left: `${(layer.x / props.canvasWidth) * 100}%`,
    top: `${(layer.y / props.canvasHeight) * 100}%`,
    width: `${(layer.width / props.canvasWidth) * 100}%`,
    height: `${(layer.height / props.canvasHeight) * 100}%`,
    opacity: layer.opacity,
    transform: `rotate(${layer.rotation}deg)`
  };
}

function assetSrc(layer: TemplateLayer) {
  const asset = props.assets.find((item) => item.id === layer.image?.assetId);
  return asset ? convertFileSrc(asset.path) : '';
}

function startMove(event: PointerEvent, layer: TemplateLayer) {
  emit('select', layer.id);
  interaction.value = {
    mode: 'move',
    layer,
    startX: event.clientX,
    startY: event.clientY,
    startLayerX: layer.x,
    startLayerY: layer.y,
    startWidth: layer.width,
    startHeight: layer.height
  };
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}

function startResize(event: PointerEvent, layer: TemplateLayer) {
  event.stopPropagation();
  interaction.value = {
    mode: 'resize',
    layer,
    startX: event.clientX,
    startY: event.clientY,
    startLayerX: layer.x,
    startLayerY: layer.y,
    startWidth: layer.width,
    startHeight: layer.height
  };
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}

function movePointer(event: PointerEvent) {
  if (!interaction.value) return;
  const canvas = (event.currentTarget as HTMLElement).closest('.template-canvas');
  if (!canvas) return;
  const rect = canvas.getBoundingClientRect();
  const dx = ((event.clientX - interaction.value.startX) / rect.width) * props.canvasWidth;
  const dy = ((event.clientY - interaction.value.startY) / rect.height) * props.canvasHeight;
  const source = interaction.value.layer;
  if (interaction.value.mode === 'move') {
    emit('update', {
      ...source,
      x: Math.max(0, Math.min(props.canvasWidth - source.width, interaction.value.startLayerX + dx)),
      y: Math.max(0, Math.min(props.canvasHeight - source.height, interaction.value.startLayerY + dy))
    });
  } else {
    emit('update', {
      ...source,
      width: Math.max(8, Math.min(props.canvasWidth - source.x, interaction.value.startWidth + dx)),
      height: Math.max(8, Math.min(props.canvasHeight - source.y, interaction.value.startHeight + dy))
    });
  }
}

function stopPointer(event: PointerEvent) {
  interaction.value = null;
  try {
    (event.target as HTMLElement).releasePointerCapture(event.pointerId);
  } catch {
    // The browser can release capture before pointerup during fast drags.
  }
}
</script>

<template>
  <div class="template-canvas-wrap">
    <div class="template-canvas" :style="canvasStyle">
      <button
        v-for="layer in flatLayers"
        :key="layer.id"
        type="button"
        :class="['template-canvas-layer', layer.type, { selected: layer.id === selectedLayerId }]"
        :style="layerStyle(layer)"
        @click.stop="emit('select', layer.id)"
        @pointerdown="startMove($event, layer)"
        @pointermove="movePointer"
        @pointerup="stopPointer"
        @pointercancel="stopPointer"
      >
        <span v-if="layer.type === 'text'" class="template-text-layer" :style="{ color: layer.text?.color, fontSize: `${layer.text?.fontSize ?? 24}px`, fontFamily: layer.text?.fontFamily }">
          {{ layer.text?.text }}
        </span>
        <img v-else-if="layer.type === 'image' && assetSrc(layer)" :src="assetSrc(layer)" alt="模板图片图层" draggable="false" />
        <span v-else-if="layer.type === 'shape'" class="template-shape-layer" :style="{ background: layer.shape?.fill, borderColor: layer.shape?.stroke, borderWidth: `${layer.shape?.strokeWidth ?? 0}px`, borderRadius: `${layer.shape?.radius ?? 0}px` }" />
        <span v-if="layer.id === selectedLayerId" class="template-resize-handle" @pointerdown="startResize($event, layer)" />
      </button>
    </div>
  </div>
</template>
