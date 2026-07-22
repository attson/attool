<script setup lang="ts">
import { computed, ref } from 'vue';
import { NButton, NInput } from 'naive-ui';
import type { HttpCollection, HttpCollectionFolder, HttpCollectionRequest, HttpHistoryItem } from './types';

const props = defineProps<{
  items: HttpHistoryItem[];
  collections: HttpCollection[];
  folders: HttpCollectionFolder[];
  requests: HttpCollectionRequest[];
  collapsed: boolean;
}>();

const emit = defineEmits<{
  (e: 'load', item: HttpHistoryItem, mode: 'active' | 'new'): void;
  (e: 'open-request', item: HttpCollectionRequest, mode: 'active' | 'new'): void;
  (e: 'delete-collection', id: string): void;
  (e: 'delete-request', id: string): void;
  (e: 'import-openapi'): void;
  (e: 'delete', id: string): void;
  (e: 'clear'): void;
  (e: 'toggle-collapse'): void;
}>();

const mode = ref<'collections' | 'history'>('collections');
const query = ref('');
const menuFor = ref<string | null>(null);
const collectionMenuFor = ref<string | null>(null);

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

const collectionMatches = computed(() => {
  const q = query.value.trim().toLowerCase();
  if (!q) return props.requests;
  return props.requests.filter((r) =>
    r.name.toLowerCase().includes(q) ||
    r.method.toLowerCase().includes(q) ||
    r.spec.url.toLowerCase().includes(q)
  );
});

function foldersFor(collectionId: string): HttpCollectionFolder[] {
  return props.folders.filter((f) => f.collectionId === collectionId && !f.parentId);
}

function requestsFor(collectionId: string, folderId: string | null): HttpCollectionRequest[] {
  const source = query.value.trim() ? collectionMatches.value : props.requests;
  return source.filter((r) => r.collectionId === collectionId && r.folderId === folderId);
}

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

function onRequestClick(item: HttpCollectionRequest, ev: MouseEvent) {
  if (ev.button === 1) {
    emit('open-request', item, 'new');
    return;
  }
  emit('open-request', item, 'active');
}

function onRequestContext(item: HttpCollectionRequest, ev: MouseEvent) {
  ev.preventDefault();
  collectionMenuFor.value = item.id;
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
          :placeholder="mode === 'collections' ? '搜索集合……' : '搜索历史……'"
          clearable
        />
      </template>
    </div>

    <template v-if="!collapsed">
      <div class="switch">
        <button :class="{ active: mode === 'collections' }" @click="mode = 'collections'">集合</button>
        <button :class="{ active: mode === 'history' }" @click="mode = 'history'">历史</button>
        <n-button size="tiny" secondary @click="emit('import-openapi')">导入</n-button>
      </div>

      <div v-if="mode === 'collections'" class="list">
        <div v-if="collections.length === 0" class="empty">暂无集合</div>
        <div v-for="collection in collections" :key="collection.id" class="collection">
          <div class="collection-head">
            <span>{{ collection.name }}</span>
            <button @click="emit('delete-collection', collection.id)">删除</button>
          </div>

          <div
            v-for="request in requestsFor(collection.id, null)"
            :key="request.id"
            class="request-row"
            @mousedown.left="onRequestClick(request, $event)"
            @mousedown.middle="onRequestClick(request, $event)"
            @dblclick="emit('open-request', request, 'new')"
            @contextmenu="onRequestContext(request, $event)"
          >
            <span class="method mono">{{ request.method }}</span>
            <span class="req-name">{{ request.name }}</span>
            <div v-if="collectionMenuFor === request.id" class="menu" @mouseleave="collectionMenuFor = null">
              <button @click="emit('open-request', request, 'new'); collectionMenuFor = null">在新 tab 打开</button>
              <button @click="emit('open-request', request, 'active'); collectionMenuFor = null">回填当前 tab</button>
              <button @click="emit('delete-request', request.id); collectionMenuFor = null">删除</button>
            </div>
          </div>

          <div v-for="folder in foldersFor(collection.id)" :key="folder.id" class="folder">
            <div class="folder-title">{{ folder.name }}</div>
            <div
              v-for="request in requestsFor(collection.id, folder.id)"
              :key="request.id"
              class="request-row"
              @mousedown.left="onRequestClick(request, $event)"
              @mousedown.middle="onRequestClick(request, $event)"
              @dblclick="emit('open-request', request, 'new')"
              @contextmenu="onRequestContext(request, $event)"
            >
              <span class="method mono">{{ request.method }}</span>
              <span class="req-name">{{ request.name }}</span>
              <div v-if="collectionMenuFor === request.id" class="menu" @mouseleave="collectionMenuFor = null">
                <button @click="emit('open-request', request, 'new'); collectionMenuFor = null">在新 tab 打开</button>
                <button @click="emit('open-request', request, 'active'); collectionMenuFor = null">回填当前 tab</button>
                <button @click="emit('delete-request', request.id); collectionMenuFor = null">删除</button>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div v-else class="list">
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
    </template>
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
.switch {
  display: grid;
  grid-template-columns: 1fr 1fr auto;
  gap: 4px;
  padding: 6px 8px;
  border-bottom: 1px solid var(--line);
}
.switch button {
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-muted);
  font-size: var(--fs-xs);
  cursor: pointer;
}
.switch button.active { background: var(--bg-elev-2); color: var(--text); }
.list { flex: 1; overflow-y: auto; padding: 4px 0; }
.empty { padding: 16px; text-align: center; color: var(--text-muted); font-size: var(--fs-xs); }
.collection { border-bottom: 1px solid var(--line); padding: 6px 0; }
.collection-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 4px 10px;
  color: var(--text);
  font-size: var(--fs-xs);
  font-weight: 600;
}
.collection-head button {
  border: 0;
  background: transparent;
  color: var(--text-faint);
  cursor: pointer;
  font-size: var(--fs-xxs);
}
.folder { padding-left: 8px; }
.folder-title {
  padding: 6px 10px 3px;
  color: var(--text-faint);
  font-size: var(--fs-xxs);
  font-weight: 600;
  text-transform: uppercase;
}
.request-row {
  display: grid;
  grid-template-columns: 42px 1fr;
  gap: 6px;
  align-items: center;
  padding: 6px 10px;
  cursor: pointer;
  position: relative;
}
.request-row:hover { background: var(--bg-elev); }
.req-name {
  color: var(--text-muted);
  font-size: var(--fs-xs);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
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
