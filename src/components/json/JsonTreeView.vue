<script setup lang="ts">
import { computed, ref, shallowRef, watch } from 'vue';
import { NVirtualList } from 'naive-ui';
import type { JsonValue } from '../../types/json';
import { flatten, allExpandableKeys, type FlatNode } from '../../utils/jsonTreeFlatten';

const props = defineProps<{
  value: JsonValue;
}>();

const emit = defineEmits<{
  copyPath: [path: string];
}>();

const openKeys = ref<Set<string>>(new Set(['$']));

watch(
  () => props.value,
  () => { openKeys.value = new Set(['$']); },
);

const nodes = shallowRef<FlatNode[]>([]);
function recompute() {
  nodes.value = flatten(props.value, openKeys.value);
}
watch(() => [props.value, openKeys.value], recompute, { immediate: true });

function toggle(node: FlatNode) {
  if (node.kind === 'primitive') return;
  const next = new Set(openKeys.value);
  if (next.has(node.key)) next.delete(node.key); else next.add(node.key);
  openKeys.value = next;
}

function isOpen(node: FlatNode) {
  return openKeys.value.has(node.key);
}

function copyPath(path: string) {
  emit('copyPath', path);
}

function expandAll() {
  openKeys.value = allExpandableKeys(props.value);
}

function collapseAll() {
  openKeys.value = new Set(['$']);
}

defineExpose({ expandAll, collapseAll });

const sizeLabel = (n: FlatNode) => n.kind === 'array' ? `[${n.size}]`
  : n.kind === 'object' ? `{${n.size}}` : '';
</script>

<template>
  <n-virtual-list
    class="tree-vlist"
    :items="nodes"
    :item-size="24"
    :item-resizable="false"
    key-field="key"
  >
    <template #default="{ item }">
      <div class="tree-row" :style="{ paddingLeft: `${item.depth * 14 + 4}px` }">
        <template v-if="item.kind === 'primitive'">
          <span v-if="item.label" class="key">{{ item.label }}:</span>
          <span :class="['value', item.primitiveClass]">{{ item.primitiveText }}</span>
          <button class="path-btn" type="button" :title="`复制 ${item.path}`" @click="copyPath(item.path)">⧉</button>
        </template>
        <template v-else>
          <button type="button" class="tree-toggle" @click="toggle(item)">
            <span class="caret">{{ isOpen(item) ? '▾' : '▸' }}</span>
            <span v-if="item.label" class="key">{{ item.label }}:</span>
            <span class="meta">{{ sizeLabel(item) }}</span>
          </button>
          <button class="path-btn" type="button" :title="`复制 ${item.path}`" @click="copyPath(item.path)">⧉</button>
        </template>
      </div>
    </template>
  </n-virtual-list>
</template>

<style scoped>
.tree-vlist {
  height: 100%;
  font-family: var(--font-mono);
  font-size: var(--fs-xs);
  line-height: 1.6;
}
.tree-row {
  display: flex;
  gap: 6px;
  align-items: center;
  height: 24px;
  white-space: nowrap;
}
.tree-toggle {
  background: none;
  border: 0;
  padding: 0;
  cursor: pointer;
  color: var(--text);
  font: inherit;
  display: inline-flex;
  align-items: center;
  gap: 4px;
}
.caret { width: 14px; display: inline-block; color: var(--text-muted); }
.key { color: var(--text-muted); }
.meta { color: var(--text-muted); }
.value.string { color: #16a34a; }
.value.number { color: #2563eb; }
.value.boolean { color: #d97706; }
.value.null { color: var(--text-muted); }
.path-btn {
  background: none;
  border: 0;
  color: var(--text-muted);
  cursor: pointer;
  font-size: 11px;
  padding: 0 4px;
}
.path-btn:hover { color: var(--text); }
</style>
