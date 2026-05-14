<script setup lang="ts">
import { onMounted } from 'vue';
import { NButton, NInput, NPopconfirm, NSelect } from 'naive-ui';
import Panel from '../ui/Panel.vue';
import ClipboardItemCard from './ClipboardItemCard.vue';
import { useClipboardHistory } from '../../composables/useClipboardHistory';
import type { ClipboardHistoryItem } from '../../types/clipboard';

const history = useClipboardHistory();

async function restore(item: ClipboardHistoryItem) {
  await navigator.clipboard.writeText(item.kind === 'files' ? item.filePaths.join('\n') : item.contentText);
}

onMounted(() => history.refresh());
</script>

<template>
  <div class="tool-page">
    <Panel title="剪贴板历史">
      <p class="clipboard-muted clipboard-intro">
        AT Tool 运行时记录文本、图片和文件路径；按 Command/Ctrl + Shift + V 打开快捷面板。
      </p>
      <div class="clipboard-toolbar">
        <n-input v-model:value="history.query.value" placeholder="搜索内容、文件名或路径" clearable />
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
        <n-button secondary :loading="history.loading.value" @click="history.refresh">刷新</n-button>
        <n-popconfirm @positive-click="history.clearHistory">
          <template #trigger><n-button secondary type="error">清空未收藏</n-button></template>
          确认清空所有未收藏的剪贴板历史？
        </n-popconfirm>
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
    </Panel>
  </div>
</template>

<style scoped>
.clipboard-intro { margin: 0 0 14px; }
.clipboard-kind-select { width: 140px; }
</style>
