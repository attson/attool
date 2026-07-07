<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { NButton, NInput, NSelect, useMessage } from 'naive-ui';
import ClipboardItemCard from './ClipboardItemCard.vue';
import { useClipboardHistory } from '../../composables/useClipboardHistory';
import type { ClipboardHistoryItem } from '../../types/clipboard';

const history = useClipboardHistory();
const message = useMessage();
const currentWindow = getCurrentWindow();

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

onMounted(() => {
  history.refresh();
  window.addEventListener('keydown', handleKeydown);
});

onUnmounted(() => window.removeEventListener('keydown', handleKeydown));
</script>

<template>
  <main class="clipboard-window">
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
    <div class="clipboard-grid">
      <ClipboardItemCard
        v-for="item in history.filteredItems.value"
        :key="item.id"
        :item="item"
        @restore="restore"
        @delete="history.deleteItem"
        @pin="history.setPinned"
      />
    </div>
  </main>
</template>

<style scoped>
.clipboard-window {
  min-height: 100vh;
  padding: 18px;
  background: var(--bg-base);
  color: var(--text);
}

.clipboard-window__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 14px;
}

.clipboard-window__title {
  margin: 0;
  color: var(--text);
  font-size: var(--fs-lg);
  line-height: 1.2;
}

.clipboard-window__hint {
  margin: 6px 0 0;
}

.clipboard-kind-select { width: 140px; }
</style>
