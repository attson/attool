<script setup lang="ts">
import { ref } from 'vue';
import { NEllipsis, NEmpty, NTag } from 'naive-ui';
import type { TemplateLayer } from '../../types/ecommerceTemplate';

type LayerDropPlacement = 'before' | 'after';

const props = defineProps<{
  layers: TemplateLayer[];
  selectedLayerId: string | null;
}>();

const emit = defineEmits<{
  select: [layerId: string];
  reorder: [draggedLayerId: string, targetLayerId: string, placement: LayerDropPlacement];
}>();

const draggingLayerId = ref<string | null>(null);
const dragOverLayerId = ref<string | null>(null);
const dropPlacement = ref<LayerDropPlacement>('before');

function renderType(layer: TemplateLayer) {
  const labels = { text: '文字', image: '图片', shape: '形状', group: '组合' };
  return labels[layer.type];
}

function startDrag(event: DragEvent, layerId: string) {
  draggingLayerId.value = layerId;
  event.dataTransfer?.setData('text/plain', layerId);
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = 'move';
  }
}

function dragOver(event: DragEvent, layerId: string) {
  if (draggingLayerId.value === layerId) return;
  event.preventDefault();
  dragOverLayerId.value = layerId;
  const target = event.currentTarget as HTMLElement;
  const bounds = target.getBoundingClientRect();
  dropPlacement.value = event.clientY > bounds.top + bounds.height / 2 ? 'after' : 'before';
  if (event.dataTransfer) {
    event.dataTransfer.dropEffect = 'move';
  }
}

function dropLayer(event: DragEvent, targetLayerId: string) {
  event.preventDefault();
  const draggedLayerId = event.dataTransfer?.getData('text/plain') || draggingLayerId.value;
  draggingLayerId.value = null;
  dragOverLayerId.value = null;
  const placement = dropPlacement.value;
  dropPlacement.value = 'before';
  if (draggedLayerId && draggedLayerId !== targetLayerId) {
    emit('reorder', draggedLayerId, targetLayerId, placement);
  }
}

function endDrag() {
  draggingLayerId.value = null;
  dragOverLayerId.value = null;
  dropPlacement.value = 'before';
}
</script>

<template>
  <div class="template-layer-tree">
    <n-empty v-if="props.layers.length === 0" description="还没有图层" />
    <template v-for="layer in props.layers" :key="layer.id">
      <button
        type="button"
        draggable="true"
        :class="['template-layer-item', { active: layer.id === props.selectedLayerId, dragging: layer.id === draggingLayerId, 'drag-over-before': layer.id === dragOverLayerId && dropPlacement === 'before', 'drag-over-after': layer.id === dragOverLayerId && dropPlacement === 'after' }]"
        @dragstart.stop="startDrag($event, layer.id)"
        @dragover.stop="dragOver($event, layer.id)"
        @dragleave.stop="dragOverLayerId = null"
        @drop.stop="dropLayer($event, layer.id)"
        @dragend.stop="endDrag"
        @click="emit('select', layer.id)"
      >
        <n-ellipsis :tooltip="false">{{ layer.name }}</n-ellipsis>
        <n-tag size="small" round>{{ renderType(layer) }}</n-tag>
      </button>
      <button
        v-for="child in layer.children ?? []"
        :key="child.id"
        type="button"
        draggable="true"
        :class="['template-layer-item', 'child', { active: child.id === props.selectedLayerId, dragging: child.id === draggingLayerId, 'drag-over-before': child.id === dragOverLayerId && dropPlacement === 'before', 'drag-over-after': child.id === dragOverLayerId && dropPlacement === 'after' }]"
        @dragstart.stop="startDrag($event, child.id)"
        @dragover.stop="dragOver($event, child.id)"
        @dragleave.stop="dragOverLayerId = null"
        @drop.stop="dropLayer($event, child.id)"
        @dragend.stop="endDrag"
        @click="emit('select', child.id)"
      >
        <n-ellipsis :tooltip="false">{{ child.name }}</n-ellipsis>
        <n-tag size="small" round>{{ renderType(child) }}</n-tag>
      </button>
    </template>
  </div>
</template>
