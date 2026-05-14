<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { NButton, NInput, NSelect } from 'naive-ui';
import ClipboardItemCard from './ClipboardItemCard.vue';
import { useClipboardHistory } from '../../composables/useClipboardHistory';
import type { ClipboardHistoryItem } from '../../types/clipboard';

const history = useClipboardHistory();
const currentWindow = getCurrentWindow();

async function restore(item: ClipboardHistoryItem) {
  await navigator.clipboard.writeText(item.kind === 'files' ? item.filePaths.join('\n') : item.contentText);
  await currentWindow.hide();
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') currentWindow.hide();
}

onMounted(() => {
  history.refresh();
  window.addEventListener('keydown', handleKeydown);
});

onUnmounted(() => window.removeEventListener('keydown', handleKeydown));
</script>

<template>
  <main class="clipboard-window">
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

.clipboard-kind-select { width: 140px; }
</style>
