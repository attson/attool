<script setup lang="ts">
import { computed } from 'vue';
import { convertFileSrc } from '@tauri-apps/api/core';
import type { ClipboardHistoryItem } from '../../types/clipboard';

const props = defineProps<{ item: ClipboardHistoryItem }>();
const emit = defineEmits<{
  restore: [item: ClipboardHistoryItem];
  delete: [id: string];
  pin: [id: string, isPinned: boolean];
  preview: [item: ClipboardHistoryItem];
}>();

const imageSrc = computed(() => props.item.assetUrl ?? (props.item.assetPath ? convertFileSrc(props.item.assetPath) : null));

function openPreview() {
  emit('preview', props.item);
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
          @click.stop="openPreview"
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
