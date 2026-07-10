<script setup lang="ts">
import { computed, ref } from 'vue';
import { NButton, NInput } from 'naive-ui';
import type { HttpHistoryItem } from './types';

const props = defineProps<{
  items: HttpHistoryItem[];
  collapsed: boolean;
}>();

const emit = defineEmits<{
  (e: 'load', item: HttpHistoryItem, mode: 'active' | 'new'): void;
  (e: 'delete', id: string): void;
  (e: 'clear'): void;
  (e: 'toggle-collapse'): void;
}>();

const query = ref('');
const menuFor = ref<string | null>(null);

const filtered = computed(() => {
  const q = query.value.trim().toLowerCase();
  if (!q) return props.items;
  return props.items.filter(
    (h) =>
      h.method.toLowerCase().includes(q) ||
      h.url.toLowerCase().includes(q) ||
      (h.respSummary ?? '').toLowerCase().includes(q)
  );
});

function statusClass(s: number | null): string {
  if (s === null) return 'st-err';
  if (s < 300) return 'st-2';
  if (s < 400) return 'st-3';
  if (s < 500) return 'st-4';
  return 'st-5';
}

function formatTime(ts: number): string {
  const diff = Date.now() - ts;
  if (diff < 60_000) return '刚刚';
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)} 分前`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)} 小时前`;
  return `${Math.floor(diff / 86_400_000)} 天前`;
}

function formatMs(ms: number | null): string {
  if (ms === null) return '---';
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

function statusText(s: number | null): string {
  return s === null ? '---' : String(s);
}

function onClick(item: HttpHistoryItem, ev: MouseEvent) {
  if (ev.button === 1) {
    emit('load', item, 'new');
    return;
  }
  emit('load', item, 'active');
}

function onContext(item: HttpHistoryItem, ev: MouseEvent) {
  ev.preventDefault();
  menuFor.value = item.id;
}

function pathOnly(url: string): string {
  try {
    const u = new URL(url);
    return u.pathname + u.search;
  } catch {
    return url;
  }
}

function shortSummary(text: string | null): string {
  if (!text) return '';
  return text.length > 200 ? text.slice(0, 200) + '…' : text;
}
</script>

<template>
  <aside class="http-sidebar" :class="{ collapsed }">
    <div class="head">
      <n-button size="tiny" quaternary @click="emit('toggle-collapse')">
        <span class="mono">{{ collapsed ? '▶' : '◀' }}</span>
      </n-button>
      <template v-if="!collapsed">
        <n-input
          v-model:value="query"
          size="tiny"
          placeholder="搜索历史……"
          clearable
        />
      </template>
    </div>

    <div v-if="!collapsed" class="list">
      <div v-if="filtered.length === 0" class="empty">暂无历史</div>
      <div
        v-for="item in filtered"
        :key="item.id"
        class="row"
        :title="shortSummary(item.respSummary)"
        @mousedown.left="onClick(item, $event)"
        @mousedown.middle="onClick(item, $event)"
        @dblclick="emit('load', item, 'new')"
        @contextmenu="onContext(item, $event)"
      >
        <div class="row-line">
          <span class="method mono">{{ item.method }}</span>
          <span :class="['status', 'mono', statusClass(item.status)]">
            {{ statusText(item.status) }}
          </span>
          <span class="ms mono">{{ formatMs(item.elapsedMs) }}</span>
          <span class="time">· {{ formatTime(item.createdAt) }}</span>
        </div>
        <div class="row-url mono">{{ pathOnly(item.url) }}</div>
        <div v-if="menuFor === item.id" class="menu" @mouseleave="menuFor = null">
          <button @click="emit('load', item, 'new'); menuFor = null">在新 tab 打开</button>
          <button @click="emit('load', item, 'active'); menuFor = null">回填当前 tab</button>
          <button @click="emit('delete', item.id); menuFor = null">删除</button>
          <button @click="emit('clear'); menuFor = null">清空全部历史</button>
        </div>
      </div>
    </div>
  </aside>
</template>

<style scoped>
.http-sidebar {
  width: 240px;
  border-right: 1px solid var(--line);
  display: flex;
  flex-direction: column;
  background: var(--bg-base);
  transition: width 0.15s ease;
}
.http-sidebar.collapsed { width: 32px; }
.head { display: flex; gap: 6px; align-items: center; padding: 8px; border-bottom: 1px solid var(--line); }
.head .n-input { flex: 1; }
.list { flex: 1; overflow-y: auto; padding: 4px 0; }
.empty { padding: 16px; text-align: center; color: var(--text-muted); font-size: var(--fs-xs); }
.row {
  padding: 8px 10px;
  border-bottom: 1px solid var(--line-weak, var(--line));
  cursor: pointer;
  position: relative;
}
.row:hover { background: var(--bg-elev); }
.row-line {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: var(--fs-xxs);
}
.method { color: var(--text); font-weight: 600; width: 42px; }
.status { width: 30px; }
.ms { color: var(--text-muted); width: 48px; }
.time { color: var(--text-muted); flex: 1; text-align: right; }
.row-url {
  font-size: var(--fs-xs);
  color: var(--text-muted);
  word-break: break-all;
  margin-top: 2px;
}
.st-2 { color: #10b981; font-weight: 600; }
.st-3 { color: #3b82f6; font-weight: 600; }
.st-4 { color: #f59e0b; font-weight: 600; }
.st-5, .st-err { color: #ef4444; font-weight: 600; }
.mono { font-family: var(--font-mono, ui-monospace, monospace); font-variant-numeric: tabular-nums; }
.menu {
  position: absolute;
  right: 6px;
  top: 8px;
  background: var(--bg-elev);
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  padding: 4px;
  display: grid;
  gap: 2px;
  z-index: 4;
  min-width: 140px;
}
.menu button {
  background: none;
  border: none;
  color: var(--text);
  padding: 4px 8px;
  text-align: left;
  font-size: var(--fs-xs);
  cursor: pointer;
  border-radius: 4px;
}
.menu button:hover { background: var(--bg-base); }
</style>
