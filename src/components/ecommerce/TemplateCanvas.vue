<script setup lang="ts">
import { computed } from 'vue';
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
}>();

const flatLayers = computed(() => flattenLayers(props.layers).filter((layer) => layer.type !== 'group' && layer.visible));
const canvasStyle = computed(() => ({ aspectRatio: `${props.canvasWidth} / ${props.canvasHeight}` }));

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
      >
        <span v-if="layer.type === 'text'" class="template-text-layer" :style="{ color: layer.text?.color, fontSize: `${layer.text?.fontSize ?? 24}px`, fontFamily: layer.text?.fontFamily }">
          {{ layer.text?.text }}
        </span>
        <img v-else-if="layer.type === 'image' && assetSrc(layer)" :src="assetSrc(layer)" alt="模板图片图层" draggable="false" />
        <span v-else-if="layer.type === 'shape'" class="template-shape-layer" :style="{ background: layer.shape?.fill, borderColor: layer.shape?.stroke, borderWidth: `${layer.shape?.strokeWidth ?? 0}px`, borderRadius: `${layer.shape?.radius ?? 0}px` }" />
      </button>
    </div>
  </div>
</template>
