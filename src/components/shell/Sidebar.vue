<script setup lang="ts">
import { computed } from 'vue';
import BrandMark from './BrandMark.vue';
import ToolIcon from './ToolIcon.vue';
import Kbd from '../ui/Kbd.vue';
import type { Tool } from '../../types/tool';

const props = defineProps<{
  tools: Tool[];
  activeId: string | null;
  collapsed: boolean;
}>();

const emit = defineEmits<{
  select: [id: string];
  toggle: [];
  brand: [];
  search: [];
}>();

const ready = computed(() => props.tools.filter((t) => t.status === 'ready'));
const soon = computed(() => props.tools.filter((t) => t.status === 'soon'));
</script>

<template>
  <aside class="sidebar" :class="{ collapsed }">
    <button class="brand-row" type="button" @click="emit('brand')">
      <BrandMark />
      <span class="brand-name">AT Tool</span>
    </button>

    <button class="search" type="button" @click="emit('search')">
      <span class="ico"></span>
      <span class="label">搜索工具</span>
      <Kbd>⌘K</Kbd>
    </button>

    <div class="group">已就绪</div>
    <button
      v-for="tool in ready"
      :key="tool.id"
      type="button"
      class="item"
      :class="{ active: tool.id === activeId }"
      :title="tool.name"
      @click="emit('select', tool.id)"
    >
      <ToolIcon :name="tool.icon" />
      <span class="label">{{ tool.name }}</span>
    </button>

    <div class="group">规划中</div>
    <button
      v-for="tool in soon"
      :key="tool.id"
      type="button"
      class="item dim"
      disabled
      :title="tool.name"
    >
      <ToolIcon :name="tool.icon" />
      <span class="label">{{ tool.name }}</span>
      <span class="pill">Soon</span>
    </button>

    <div class="foot">
      <span class="ver">v0.1.0</span>
      <button class="toggle" type="button" :title="collapsed ? '展开' : '折叠'" @click="emit('toggle')">
        {{ collapsed ? '›' : '‹' }}
      </button>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  width: 220px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  background: var(--bg-elevated);
  border-right: 1px solid var(--line);
  transition: width var(--motion-mid);
  overflow: hidden;
}

.sidebar.collapsed { width: 56px; }

.brand-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 14px;
  border: 0;
  background: transparent;
  border-bottom: 1px solid var(--line);
  cursor: pointer;
  text-align: left;
  color: var(--text);
}
.brand-name {
  font-size: var(--fs-md);
  font-weight: 600;
  letter-spacing: -0.012em;
  white-space: nowrap;
  overflow: hidden;
}
.sidebar.collapsed .brand-name { display: none; }
.sidebar.collapsed .brand-row { justify-content: center; padding: 14px 0; }

.search {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 10px 10px 6px;
  padding: 6px 9px;
  border: 1px solid var(--line);
  border-radius: var(--radius);
  background: var(--bg-elev-2);
  color: var(--text-muted);
  font-size: var(--fs-xs);
  cursor: pointer;
}
.search .ico {
  width: 12px; height: 12px;
  border: 1.5px solid currentColor;
  border-radius: 50%;
  position: relative;
}
.search .ico::after {
  content: "";
  position: absolute;
  width: 4px; height: 1.5px;
  background: currentColor;
  bottom: -2px; right: -3px;
  transform: rotate(45deg);
}
.search .label { flex: 1; text-align: left; }
.sidebar.collapsed .search { display: none; }

.group {
  font-size: var(--fs-xxs);
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.08em;
  padding: 12px 14px 4px;
  font-weight: 600;
}
.sidebar.collapsed .group {
  height: 12px;
  color: transparent;
  padding: 12px 0 0;
}

.item {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 1px 8px;
  padding: 6px 8px;
  border: 0;
  border-radius: var(--radius);
  background: transparent;
  color: var(--text);
  font-size: var(--fs-sm);
  cursor: pointer;
  text-align: left;
}
.item:hover:not(:disabled) { background: var(--bg-elev-2); }
.item:disabled { cursor: not-allowed; }
.item.active {
  background: var(--accent-soft);
  color: var(--accent);
}
.item :deep(.tool-icon) { color: var(--text-muted); }
.item.active :deep(.tool-icon) { color: var(--accent); }
.item.dim { color: var(--text-muted); }
.item .label {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex: 1;
}
.item .pill {
  margin-left: auto;
  background: var(--line-strong);
  color: var(--text-muted);
  font-size: 10px;
  padding: 1px 6px;
  border-radius: var(--radius-pill);
  flex-shrink: 0;
}
.sidebar.collapsed .item { justify-content: center; padding: 7px; margin: 1px 4px; }
.sidebar.collapsed .item .label,
.sidebar.collapsed .item .pill { display: none; }

.foot {
  margin-top: auto;
  padding: 10px;
  border-top: 1px solid var(--line);
  display: flex;
  align-items: center;
  gap: 10px;
  color: var(--text-muted);
  font-size: var(--fs-xxs);
}
.sidebar.collapsed .foot .ver { display: none; }
.sidebar.collapsed .foot { justify-content: center; }

.toggle {
  margin-left: auto;
  width: 22px; height: 22px;
  display: grid;
  place-items: center;
  border: 0;
  border-radius: var(--radius-sm);
  background: var(--bg-elev-2);
  color: var(--text-muted);
  cursor: pointer;
  font-size: 13px;
  line-height: 1;
}
.toggle:hover { color: var(--text); }
</style>
