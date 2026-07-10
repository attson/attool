<script setup lang="ts">
import { computed, h, ref, Fragment } from 'vue';
import { convertFileSrc } from '@tauri-apps/api/core';
import { NImage, NModal, NButton } from 'naive-ui';
import type { ImageInst, ImageRenderToolbarProps } from 'naive-ui';
import type { ClipboardHistoryItem } from '../../types/clipboard';

const props = defineProps<{ item: ClipboardHistoryItem }>();
const emit = defineEmits<{
  restore: [item: ClipboardHistoryItem];
  delete: [id: string];
  pin: [id: string, isPinned: boolean];
}>();

const imageSrc = computed(() => props.item.assetUrl ?? (props.item.assetPath ? convertFileSrc(props.item.assetPath) : null));

const imageRef = ref<ImageInst | null>(null);

function openPreview() {
  imageRef.value?.showPreview();
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

// 在 NImage 内置工具栏后追加“复制到剪贴板”按钮
function renderToolbar({ nodes }: ImageRenderToolbarProps) {
  const copyNode = h(
    'div',
    {
      class: 'clipboard-preview__toolbar-copy',
      role: 'button',
      title: '复制到剪贴板',
      onClick: () => emit('restore', props.item),
    },
    h(
      'svg',
      { viewBox: '0 0 24 24', width: 20, height: 20, fill: 'none', stroke: 'currentColor', 'stroke-width': 2, 'stroke-linecap': 'round', 'stroke-linejoin': 'round' },
      [
        h('rect', { x: 9, y: 9, width: 11, height: 11, rx: 2 }),
        h('path', { d: 'M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1' }),
      ],
    ),
  );
  return h(Fragment, null, [
    nodes.prev,
    nodes.next,
    nodes.rotateCounterclockwise,
    nodes.rotateClockwise,
    nodes.resizeToOriginalSize,
    nodes.zoomOut,
    nodes.zoomIn,
    copyNode,
    nodes.close,
  ]);
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
        <!-- 仅借用 NImage 的预览层(工具栏含缩放/旋转 + 复制),缩略图本身隐藏 -->
        <n-image
          ref="imageRef"
          :src="imageSrc"
          :render-toolbar="renderToolbar"
          class="clipboard-card__previewer"
        />
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

/* 只借用 NImage 的预览层,隐藏它自身的缩略图占位 */
.clipboard-card__previewer { display: none; }

/* 文本卡:相对定位以容纳右上角悬浮的“查看”图标 */
.clipboard-card__text { position: relative; }

.clipboard-card__text .clipboard-card__zoom {
  cursor: pointer;
}

.clipboard-card__actions { margin-top: 12px; }
.clipboard-card__actions span:not(:first-child) { color: var(--accent); }
</style>

<style>
/* 预览工具栏内的复制按钮:与 NImage 内置工具栏图标风格保持一致(非 scoped,作用于 teleport 到 body 的预览层) */
.clipboard-preview__toolbar-copy {
  display: flex;
  align-items: center;
  justify-content: center;
  margin-left: 12px;
  cursor: pointer;
  transition: color 0.2s;
}

.clipboard-preview__toolbar-copy:hover {
  color: var(--n-toolbar-icon-color-hover, #fff);
}

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
</style>
