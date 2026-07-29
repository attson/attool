<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { NButton, useMessage } from 'naive-ui';
import type { ClipboardHistoryItem } from '../../types/clipboard';

const currentWindow = getCurrentWindow();
const message = useMessage();
const item = ref<ClipboardHistoryItem | null>(null);
let unlistenPreview: UnlistenFn | null = null;

const title = computed(() => {
  if (!item.value) return '剪贴板预览';
  if (item.value.kind === 'image') return '剪贴板图片';
  if (item.value.kind === 'text') return '剪贴板文本';
  return '剪贴板文件';
});

const imageSrc = computed(() => {
  const current = item.value;
  if (!current || current.kind !== 'image') return null;
  return current.assetUrl ?? (current.assetPath ? convertFileSrc(current.assetPath) : null);
});

const createdAtText = computed(() => item.value?.createdAt.slice(0, 16).replace('T', ' ') ?? '');
const charCount = computed(() => item.value ? [...item.value.contentText].length : 0);

async function closeWindow() {
  await currentWindow.setAlwaysOnTop(false);
  await currentWindow.hide();
  const historyWindow = await WebviewWindow.getByLabel('clipboard-history');
  await historyWindow?.setAlwaysOnTop(true);
  await historyWindow?.setFocus();
}

async function copyItem() {
  if (!item.value) return;
  try {
    await invoke('restore_clipboard_item', { id: item.value.id });
    message.success('已复制到剪贴板');
  } catch (error) {
    message.error(`复制失败：${error}`);
  }
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') closeWindow();
}

onMounted(() => {
  listen<ClipboardHistoryItem>('clipboard-preview-opened', (event) => {
    item.value = event.payload;
  }).then((unlisten) => {
    unlistenPreview = unlisten;
  });
  window.addEventListener('keydown', handleKeydown);
});

onUnmounted(() => {
  if (unlistenPreview) {
    unlistenPreview();
    unlistenPreview = null;
  }
  window.removeEventListener('keydown', handleKeydown);
});
</script>

<template>
  <main class="clipboard-preview-window">
    <header class="clipboard-preview-window__header" data-tauri-drag-region>
      <div data-tauri-drag-region>
        <h1 class="clipboard-preview-window__title">{{ title }}</h1>
        <p v-if="item" class="clipboard-muted clipboard-preview-window__meta">
          {{ createdAtText }}<template v-if="item.kind === 'text'"> · {{ charCount }} 字</template>
        </p>
      </div>
      <n-button secondary @click="closeWindow">关闭</n-button>
    </header>

    <section class="clipboard-preview-window__body">
      <img
        v-if="imageSrc"
        :src="imageSrc"
        alt="剪贴板图片预览"
        class="clipboard-preview-window__image"
      />
      <pre v-else-if="item?.kind === 'text'" class="clipboard-preview-window__text">{{ item.contentText }}</pre>
      <div v-else-if="item?.kind === 'files'" class="clipboard-preview-window__files">
        <div v-for="path in item.filePaths" :key="path">{{ path }}</div>
      </div>
      <p v-else class="clipboard-muted">等待预览内容</p>
    </section>

    <footer class="clipboard-preview-window__footer">
      <span class="clipboard-muted">{{ item?.preview ?? '' }}</span>
      <n-button v-if="item" type="primary" @click="copyItem">复制到剪贴板</n-button>
    </footer>
  </main>
</template>

<style scoped>
.clipboard-preview-window {
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  height: 100vh;
  min-height: 100vh;
  overflow: hidden;
  padding: 16px;
  background: var(--bg-base);
  color: var(--text);
}

.clipboard-preview-window__header,
.clipboard-preview-window__footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.clipboard-preview-window__header {
  margin-bottom: 12px;
  cursor: move;
  user-select: none;
}

.clipboard-preview-window__title {
  margin: 0;
  color: var(--text);
  font-size: var(--fs-lg);
  line-height: 1.2;
}

.clipboard-preview-window__meta {
  margin: 6px 0 0;
}

.clipboard-preview-window__body {
  display: flex;
  flex: 1;
  align-items: center;
  justify-content: center;
  min-height: 0;
  overflow: auto;
  border: 1px solid var(--line);
  border-radius: var(--radius-lg);
  background: var(--bg-elevated);
}

.clipboard-preview-window__image {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.clipboard-preview-window__text {
  box-sizing: border-box;
  width: 100%;
  height: 100%;
  margin: 0;
  padding: 16px;
  overflow: auto;
  white-space: pre-wrap;
  word-break: break-word;
  color: var(--text);
  font-family: inherit;
  font-size: var(--fs-sm);
  line-height: 1.6;
}

.clipboard-preview-window__files {
  box-sizing: border-box;
  width: 100%;
  padding: 16px;
  color: var(--text);
  font-size: var(--fs-sm);
  line-height: 1.8;
  word-break: break-all;
}

.clipboard-preview-window__footer {
  margin-top: 12px;
}

.clipboard-preview-window__footer > span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
