<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue';
import { emitTo, listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow, LogicalPosition, LogicalSize, primaryMonitor } from '@tauri-apps/api/window';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { NButton, NInput, NSelect, useMessage } from 'naive-ui';
import ClipboardItemCard from './ClipboardItemCard.vue';
import { useClipboardHistory } from '../../composables/useClipboardHistory';
import type { ClipboardHistoryItem } from '../../types/clipboard';

const history = useClipboardHistory();
const message = useMessage();
const currentWindow = getCurrentWindow();
const listRef = ref<HTMLElement | null>(null);
let unlistenOpened: UnlistenFn | null = null;
const HISTORY_STRIP_HEIGHT = 260;
const PREVIEW_GAP = 12;
const PREVIEW_EDGE_MARGIN = 12;
const PREVIEW_MIN_WIDTH = 900;
const PREVIEW_MIN_HEIGHT = 420;

async function getPrimaryLogicalBounds() {
  const monitor = await primaryMonitor();
  if (!monitor) return null;
  const scale = monitor.scaleFactor;
  return {
    x: monitor.position.x / scale,
    y: monitor.position.y / scale,
    width: monitor.size.width / scale,
    height: monitor.size.height / scale,
  };
}

async function openPreviewWindow(item: ClipboardHistoryItem) {
  const bounds = await getPrimaryLogicalBounds();
  const previewWindow = await WebviewWindow.getByLabel('clipboard-preview');
  if (!previewWindow) return;
  if (!bounds) return;
  const maxWidth = Math.max(bounds.width - PREVIEW_EDGE_MARGIN * 2, PREVIEW_MIN_WIDTH);
  const maxHeight = Math.max(
    bounds.height - HISTORY_STRIP_HEIGHT - PREVIEW_GAP - PREVIEW_EDGE_MARGIN,
    PREVIEW_MIN_HEIGHT,
  );
  const width = Math.min(Math.max(bounds.width * 0.78, PREVIEW_MIN_WIDTH), maxWidth);
  const height = Math.min(Math.max(bounds.height * 0.7, PREVIEW_MIN_HEIGHT), maxHeight);
  const x = bounds.x + (bounds.width - width) / 2;
  const y = Math.max(
    bounds.y + PREVIEW_EDGE_MARGIN,
    bounds.y + bounds.height - HISTORY_STRIP_HEIGHT - PREVIEW_GAP - height,
  );
  await previewWindow.setSize(new LogicalSize(width, height));
  await previewWindow.setPosition(new LogicalPosition(x, y));
  await previewWindow.show();
  await previewWindow.setFocus();
  await emitTo('clipboard-preview', 'clipboard-preview-opened', item);
  setTimeout(() => {
    emitTo('clipboard-preview', 'clipboard-preview-opened', item).catch(() => undefined);
  }, 120);
}

async function restore(item: ClipboardHistoryItem) {
  try {
    await history.restoreItem(item.id);
    message.success('已复制到剪贴板');
    // 让 toast 有时间显示，再关闭快捷面板
    setTimeout(closeWindow, 400);
  } catch (error) {
    message.error(`复制失败：${error}`);
  }
}

async function closeWindow() {
  await currentWindow.hide();
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') closeWindow();
}

function handleWheel(event: WheelEvent) {
  const list = listRef.value;
  if (!list) return;
  const delta = Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : event.deltaY;
  if (delta === 0) return;
  event.preventDefault();
  list.scrollLeft += delta;
}

onMounted(() => {
  history.refresh();
  listen('clipboard-history-opened', () => {
    history.refresh();
  }).then((unlisten) => {
    unlistenOpened = unlisten;
  });
  window.addEventListener('keydown', handleKeydown);
});

onUnmounted(() => {
  if (unlistenOpened) {
    unlistenOpened();
    unlistenOpened = null;
  }
  window.removeEventListener('keydown', handleKeydown);
});
</script>

<template>
  <main class="clipboard-window" @wheel="handleWheel">
    <header class="clipboard-window__header">
      <div>
        <h1 class="clipboard-window__title">剪贴板历史</h1>
        <p class="clipboard-muted clipboard-window__hint">选择条目后会写入系统剪贴板</p>
      </div>
      <n-button secondary @click="closeWindow">关闭</n-button>
    </header>
    <div class="clipboard-toolbar">
      <n-input v-model:value="history.query.value" placeholder="搜索剪贴板历史" clearable @keyup.enter="history.refresh" />
      <n-select
        v-model:value="history.kind.value"
        class="clipboard-kind-select"
        :options="[
          { label: '全部', value: 'all' },
          { label: '文本', value: 'text' },
          { label: '图片', value: 'image' },
          { label: '文件', value: 'files' },
        ]"
      />
      <n-button secondary @click="history.refresh">刷新</n-button>
    </div>
    <p v-if="history.error.value" class="clipboard-muted">{{ history.error.value }}</p>
    <section class="clipboard-window__rail" aria-label="剪贴板历史列表">
      <div ref="listRef" class="clipboard-window__list">
        <ClipboardItemCard
          v-for="item in history.filteredItems.value"
          :key="item.id"
          :item="item"
          @restore="restore"
          @delete="history.deleteItem"
          @pin="history.setPinned"
          @preview="openPreviewWindow"
        />
      </div>
    </section>
  </main>
</template>

<style scoped>
.clipboard-window {
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  height: 100vh;
  min-height: 100vh;
  overflow: hidden;
  padding: 12px 14px;
  background: var(--bg-base);
  color: var(--text);
}

.clipboard-window__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 8px;
}

.clipboard-window__title {
  margin: 0;
  color: var(--text);
  font-size: var(--fs-lg);
  line-height: 1.2;
}

.clipboard-window__hint {
  display: none;
}

.clipboard-window__rail {
  display: flex;
  flex: 1;
  align-items: flex-end;
  min-height: 0;
  overflow: hidden;
}

.clipboard-window__list {
  display: flex;
  gap: 12px;
  width: 100%;
  overflow-x: auto;
  overflow-y: hidden;
  padding-bottom: 10px;
  scrollbar-gutter: stable;
}

.clipboard-window__list :deep(.clipboard-card) {
  display: flex;
  flex: 0 0 clamp(180px, 25vw, 240px);
  flex-direction: column;
  height: 132px;
}

.clipboard-window__list :deep(.clipboard-card__preview) {
  flex: 1;
  overflow: hidden;
}

.clipboard-window__list :deep(.clipboard-card__actions) {
  margin-top: 8px;
}

.clipboard-kind-select { width: 140px; }

.clipboard-window :deep(.clipboard-toolbar) {
  margin-bottom: 8px;
}
</style>
