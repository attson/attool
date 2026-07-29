<script setup lang="ts">
import { computed, ref } from 'vue';
import { convertFileSrc } from '@tauri-apps/api/core';
import { NModal, NButton } from 'naive-ui';
import type { ClipboardHistoryItem } from '../../types/clipboard';

const props = defineProps<{ item: ClipboardHistoryItem }>();
const emit = defineEmits<{
  restore: [item: ClipboardHistoryItem];
  delete: [id: string];
  pin: [id: string, isPinned: boolean];
  previewOpen: [];
  previewClose: [];
}>();

const imageSrc = computed(() => props.item.assetUrl ?? (props.item.assetPath ? convertFileSrc(props.item.assetPath) : null));

const showImagePreview = ref(false);

function openPreview() {
  showImagePreview.value = true;
  emit('previewOpen');
}

function handleImagePreviewShow(show: boolean) {
  showImagePreview.value = show;
  emit(show ? 'previewOpen' : 'previewClose');
}

const showTextPreview = ref(false);
const createdAtText = computed(() => props.item.createdAt.slice(0, 16).replace('T', ' '));
const charCount = computed(() => [...props.item.contentText].length);

function openTextPreview() {
  showTextPreview.value = true;
}

function copyText() {
  emit('restore', props.item);
}

const KIND_LABEL: Record<ClipboardHistoryItem['kind'], string> = {
  text: '文本',
  image: '图片',
  files: '文件',
};
</script>

<template>
  <button class="clipboard-card" type="button" @click="emit('restore', props.item)">
    <div class="clipboard-card__meta">
      <span>{{ KIND_LABEL[props.item.kind] }}</span>
      <span>{{ props.item.isPinned ? '已收藏' : '历史' }}</span>
    </div>
    <div class="clipboard-card__preview">
      <div v-if="props.item.kind === 'image' && imageSrc" class="clipboard-card__thumb">
        <img
          :src="imageSrc"
          alt="剪贴板图片预览"
          class="clipboard-card__image"
        />
        <span
          class="clipboard-card__zoom"
          title="放大预览"
          @click.stop="openPreview"
        >
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="11" cy="11" r="7" />
            <line x1="16.5" y1="16.5" x2="21" y2="21" />
            <line x1="11" y1="8" x2="11" y2="14" />
            <line x1="8" y1="11" x2="14" y2="11" />
          </svg>
        </span>
      </div>
      <template v-else-if="props.item.kind === 'files'">
        <strong>{{ props.item.preview }}</strong><br />
        <span>{{ props.item.filePaths[0] }}</span>
      </template>
      <div v-else class="clipboard-card__text">
        <span>{{ props.item.preview }}</span>
        <span
          class="clipboard-card__zoom"
          title="查看完整内容"
          @click.stop="openTextPreview"
        >
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M1 12s4-7 11-7 11 7 11 7-4 7-11 7-11-7-11-7z" />
            <circle cx="12" cy="12" r="3" />
          </svg>
        </span>
      </div>
    </div>
    <div class="clipboard-card__meta clipboard-card__actions">
      <span>{{ props.item.createdAt.slice(0, 16).replace('T', ' ') }}</span>
      <span @click.stop="emit('pin', props.item.id, !props.item.isPinned)">
        {{ props.item.isPinned ? '取消收藏' : '收藏' }}
      </span>
      <span @click.stop="emit('delete', props.item.id)">删除</span>
    </div>
  </button>

  <n-modal
    v-if="props.item.kind === 'text'"
    v-model:show="showTextPreview"
    preset="card"
    title="剪贴板文本"
    class="clipboard-text-modal"
    :auto-focus="false"
    :bordered="false"
  >
    <pre class="clipboard-text-modal__body">{{ props.item.contentText }}</pre>
    <template #footer>
      <div class="clipboard-text-modal__footer">
        <span class="clipboard-text-modal__info">{{ charCount }} 字 · {{ createdAtText }}</span>
        <n-button size="small" type="primary" @click="copyText">复制到剪贴板</n-button>
      </div>
    </template>
  </n-modal>

  <n-modal
    v-if="props.item.kind === 'image' && imageSrc"
    v-model:show="showImagePreview"
    preset="card"
    title="剪贴板图片"
    class="clipboard-image-modal"
    :auto-focus="false"
    :bordered="false"
    @update:show="handleImagePreviewShow"
  >
    <div class="clipboard-image-modal__body">
      <img :src="imageSrc" alt="剪贴板图片预览" class="clipboard-image-modal__image" />
    </div>
    <template #footer>
      <div class="clipboard-image-modal__footer">
        <span class="clipboard-text-modal__info">{{ createdAtText }}</span>
        <div class="clipboard-image-modal__actions">
          <n-button size="small" secondary @click="handleImagePreviewShow(false)">关闭</n-button>
          <n-button size="small" type="primary" @click="copyText">复制到剪贴板</n-button>
        </div>
      </div>
    </template>
  </n-modal>
</template>

<style scoped>
.clipboard-card__thumb {
  position: relative;
  display: inline-block;
  max-width: 100%;
}

.clipboard-card__image {
  max-width: 100%;
  max-height: 72px;
  border-radius: var(--radius-md);
  object-fit: cover;
}

.clipboard-card__zoom {
  position: absolute;
  top: 4px;
  right: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border-radius: var(--radius-md);
  color: #fff;
  background: rgba(0, 0, 0, 0.55);
  opacity: 0;
  transition: opacity 0.15s ease;
  cursor: zoom-in;
}

.clipboard-card:hover .clipboard-card__zoom { opacity: 1; }

/* 文本卡:相对定位以容纳右上角悬浮的“查看”图标 */
.clipboard-card__text { position: relative; }

.clipboard-card__text .clipboard-card__zoom {
  cursor: pointer;
}

.clipboard-card__actions { margin-top: 12px; }
.clipboard-card__actions span:not(:first-child) { color: var(--accent); }
</style>

<style>
/* 文本预览弹窗(NModal teleport 到 body,需非 scoped) */
.clipboard-text-modal {
  width: 640px;
  max-width: 90vw;
}

.clipboard-text-modal__body {
  margin: 0;
  max-height: 60vh;
  overflow: auto;
  white-space: pre-wrap;
  word-break: break-word;
  font-family: inherit;
  font-size: var(--fs-sm, 13px);
  line-height: 1.6;
}

.clipboard-text-modal__footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.clipboard-text-modal__info {
  color: var(--n-text-color-3, rgba(255, 255, 255, 0.52));
  font-size: var(--fs-sm, 13px);
}

.clipboard-image-modal {
  width: calc(100vw - 32px);
  max-width: 1480px;
}

.clipboard-image-modal .n-card__content {
  padding: 0;
}

.clipboard-image-modal__body {
  display: flex;
  align-items: center;
  justify-content: center;
  height: calc(100vh - 150px);
  min-height: 360px;
  overflow: hidden;
  background: var(--bg-base);
}

.clipboard-image-modal__image {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.clipboard-image-modal__footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.clipboard-image-modal__actions {
  display: flex;
  gap: 8px;
}
</style>
