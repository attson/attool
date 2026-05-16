<script setup lang="ts">
import { computed, ref } from 'vue';
import type { JsonValue } from '../../types/json';

const props = defineProps<{
  value: JsonValue;
  name?: string;
  path?: string;
  depth?: number;
}>();

const emit = defineEmits<{
  copyPath: [path: string];
}>();

const open = ref((props.depth ?? 0) < 1);
const currentPath = computed(() => props.path ?? '$');

const kind = computed<'object' | 'array' | 'primitive'>(() => {
  if (Array.isArray(props.value)) return 'array';
  if (props.value && typeof props.value === 'object') return 'object';
  return 'primitive';
});

const entries = computed(() => {
  if (kind.value === 'array') {
    return (props.value as JsonValue[]).map((v, i) => ({ key: String(i), child: v, childPath: `${currentPath.value}[${i}]` }));
  }
  if (kind.value === 'object') {
    const obj = props.value as { [k: string]: JsonValue };
    return Object.keys(obj).map((key) => ({ key, child: obj[key], childPath: `${currentPath.value}.${key}` }));
  }
  return [];
});

const sizeLabel = computed(() =>
  kind.value === 'array' ? `[${(props.value as JsonValue[]).length}]` :
  kind.value === 'object' ? `{${Object.keys(props.value as { [k: string]: JsonValue }).length}}` :
  '',
);

const primitiveDisplay = computed(() => {
  const v = props.value;
  if (v === null) return 'null';
  if (typeof v === 'string') return JSON.stringify(v);
  return String(v);
});

const primitiveClass = computed(() => {
  const v = props.value;
  if (v === null) return 'null';
  if (typeof v === 'string') return 'string';
  if (typeof v === 'number') return 'number';
  if (typeof v === 'boolean') return 'boolean';
  return '';
});
</script>

<template>
  <div class="tree-node">
    <div class="tree-row" v-if="kind === 'primitive'">
      <span v-if="name !== undefined" class="key">{{ name }}:</span>
      <span :class="['value', primitiveClass]">{{ primitiveDisplay }}</span>
      <button class="path-btn" type="button" :title="`复制 ${currentPath}`" @click="emit('copyPath', currentPath)">⧉</button>
    </div>
    <template v-else>
      <button type="button" class="tree-toggle" @click="open = !open">
        <span class="caret">{{ open ? '▾' : '▸' }}</span>
        <span v-if="name !== undefined" class="key">{{ name }}:</span>
        <span class="meta">{{ sizeLabel }}</span>
      </button>
      <div v-if="open" class="children">
        <JsonTreeView
          v-for="entry in entries"
          :key="entry.key"
          :value="entry.child"
          :name="entry.key"
          :path="entry.childPath"
          :depth="(depth ?? 0) + 1"
          @copy-path="(p) => emit('copyPath', p)"
        />
      </div>
    </template>
  </div>
</template>

<style scoped>
.tree-node { font-family: var(--font-mono); font-size: var(--fs-xs); line-height: 1.6; }
.tree-row { display: flex; gap: 6px; align-items: center; padding-left: 18px; }
.tree-toggle {
  background: none;
  border: 0;
  padding: 0 0 0 0;
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
.children { padding-left: 14px; border-left: 1px dashed var(--line); margin-left: 6px; }
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
