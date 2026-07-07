<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { NButton, NInput, NInputNumber, NPopconfirm, NSelect, NSwitch, useMessage } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import Panel from '../ui/Panel.vue';
import ClipboardItemCard from './ClipboardItemCard.vue';
import { useClipboardHistory } from '../../composables/useClipboardHistory';
import { DEFAULT_CLIPBOARD_SHORTCUT, formatShortcutLabel } from '../../utils/clipboardHistory';
import type { ClipboardHistoryItem, ClipboardHistorySettings } from '../../types/clipboard';

const history = useClipboardHistory();
const message = useMessage();
const shortcutLabel = formatShortcutLabel(DEFAULT_CLIPBOARD_SHORTCUT);
const settings = ref<ClipboardHistorySettings>({
  captureEnabled: true,
  retentionLimit: 500,
  shortcut: DEFAULT_CLIPBOARD_SHORTCUT,
});

async function restore(item: ClipboardHistoryItem) {
  try {
    await history.restoreItem(item.id);
    message.success('已复制到剪贴板');
  } catch (error) {
    message.error(`复制失败：${error}`);
  }
}

async function loadSettings() {
  settings.value = await invoke<ClipboardHistorySettings>('get_clipboard_settings');
}

async function saveSettings() {
  settings.value = await invoke<ClipboardHistorySettings>('save_clipboard_settings', { settings: settings.value });
}

onMounted(() => {
  history.refresh();
  loadSettings();
});
</script>

<template>
  <div class="tool-page">
    <Panel title="剪贴板历史">
      <p class="clipboard-muted clipboard-intro">
        AT Tool 运行时记录文本、图片和文件路径；按 {{ shortcutLabel }} 打开快捷面板。
      </p>
      <div class="clipboard-toolbar">
        <n-switch v-model:value="settings.captureEnabled" @update:value="saveSettings" />
        <span class="clipboard-muted">{{ settings.captureEnabled ? '正在记录剪贴板' : '已暂停记录剪贴板' }}</span>
        <n-input-number
          v-model:value="settings.retentionLimit"
          class="clipboard-limit-input"
          :min="50"
          :max="5000"
          @blur="saveSettings"
        />
        <span class="clipboard-muted">最多保留条数</span>
      </div>
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
.clipboard-limit-input { width: 140px; }
</style>
