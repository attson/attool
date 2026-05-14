<script setup lang="ts">
import type { ClipboardHistoryItem } from '../../types/clipboard';

const props = defineProps<{ item: ClipboardHistoryItem }>();
const emit = defineEmits<{
  restore: [item: ClipboardHistoryItem];
  delete: [id: string];
  pin: [id: string, isPinned: boolean];
}>();

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
      <img
        v-if="props.item.kind === 'image' && props.item.assetUrl"
        :src="props.item.assetUrl"
        alt="剪贴板图片预览"
        class="clipboard-card__image"
      />
      <template v-else-if="props.item.kind === 'files'">
        <strong>{{ props.item.preview }}</strong><br />
        <span>{{ props.item.filePaths[0] }}</span>
      </template>
      <template v-else>{{ props.item.preview }}</template>
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
.clipboard-card__image {
  max-width: 100%;
  max-height: 72px;
  border-radius: var(--radius-md);
  object-fit: cover;
}

.clipboard-card__actions { margin-top: 12px; }
.clipboard-card__actions span:not(:first-child) { color: var(--accent); }
</style>
