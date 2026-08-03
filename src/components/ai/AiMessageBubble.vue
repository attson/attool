<script setup lang="ts">
import { computed } from 'vue';
import { NButton, NTag } from 'naive-ui';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { renderMarkdown } from './markdown';
import { parseContent } from '../../types/ai';
import type { AiMessage } from '../../types/ai';

const props = defineProps<{
  message: AiMessage;
  modelName: string | null;
}>();

const emit = defineEmits<{ (e: 'retry', assistantId: string): void }>();

const parts = computed(() => parseContent(props.message.contentJson));

const imageParts = computed(() => parts.value.filter((p) => p.type === 'image'));

// Concatenated text across parts; user turns may interleave text+images,
// assistant turns are effectively always a single text part while streaming.
const text = computed(() =>
  parts.value.filter((p) => p.type === 'text').map((p) => p.text).join('')
);

const isStreaming = computed(() => props.message.status === 'streaming');
const isThinking = computed(() => isStreaming.value && text.value.length === 0);

const renderedHtml = computed(() => renderMarkdown(text.value));

const tokensLabel = computed(() => {
  const p = props.message.promptTokens ?? '?';
  const c = props.message.completionTokens ?? '?';
  return `${p} → ${c} tokens`;
});

// v-html'd markdown has no Vue event bindings, so code-block copy buttons
// (emitted by markdown.ts as plain <button class="md-copy-btn">) are wired
// via delegated click on the container rather than per-element listeners.
async function onMarkdownClick(ev: MouseEvent) {
  const target = ev.target as HTMLElement | null;
  const btn = target?.closest('.md-copy-btn') as HTMLButtonElement | null;
  if (!btn) return;
  const code = btn.closest('pre')?.querySelector('code')?.textContent ?? '';
  await writeText(code);
  const original = btn.textContent;
  btn.textContent = '已复制';
  btn.disabled = true;
  setTimeout(() => { btn.textContent = original; btn.disabled = false; }, 1000);
}
</script>

<template>
  <div v-if="message.role === 'user'" class="bubble-row user">
    <div class="bubble user-bubble">
      <img v-for="(p, i) in imageParts" :key="i" :src="(p as any).path" class="attachment-img" />
      <div class="text">{{ text }}</div>
    </div>
  </div>

  <div v-else-if="message.role === 'assistant'" class="bubble-row assistant">
    <div class="bubble assistant-bubble">
      <div v-if="isThinking" class="thinking">思考中…</div>
      <template v-else>
        <div class="markdown" v-html="renderedHtml" @click="onMarkdownClick"></div>
        <span v-if="isStreaming" class="cursor">▂</span>
      </template>

      <div v-if="message.status === 'error'" class="error-box">
        <span class="error-text">{{ message.errorMessage }}</span>
        <n-button size="tiny" secondary @click="emit('retry', message.id)">重试</n-button>
      </div>
      <n-tag v-else-if="message.status === 'cancelled'" size="small" round>已取消</n-tag>

      <div v-if="!isThinking" class="meta">
        <span>{{ modelName ?? '(模型已删除)' }}</span>
        <span>·</span>
        <span>{{ tokensLabel }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.bubble-row { display: flex; padding: 4px 12px; }
.bubble-row.user { justify-content: flex-end; }
.bubble-row.assistant { justify-content: flex-start; }
.bubble {
  max-width: 78%;
  border-radius: var(--radius);
  padding: 8px 12px;
  font-size: var(--fs-md);
  line-height: 1.5;
}
.user-bubble { background: var(--bg-elev-2); color: var(--text); }
.assistant-bubble { background: var(--bg-base); color: var(--text); max-width: 88%; }
.text { white-space: pre-wrap; word-break: break-word; }
.attachment-img {
  display: block;
  max-width: 240px;
  max-height: 240px;
  border-radius: var(--radius-sm);
  margin-bottom: 6px;
}
.thinking { color: var(--text-muted); font-size: var(--fs-sm); }
.cursor { color: var(--accent); animation: blink 1s step-start infinite; }
@keyframes blink { 50% { opacity: 0; } }
.markdown :deep(p) { margin: 0 0 8px; }
.markdown :deep(p:last-child) { margin-bottom: 0; }
.markdown :deep(pre) {
  position: relative;
  background: var(--bg-elev-2);
  border-radius: var(--radius-sm);
  padding: 8px 10px;
  overflow-x: auto;
  font-size: var(--fs-xs);
}
.markdown :deep(.md-copy-btn) {
  position: absolute;
  top: 6px;
  right: 6px;
  border: 1px solid var(--line);
  background: var(--bg-elevated);
  color: var(--text-muted);
  font-size: var(--fs-xxs);
  padding: 2px 6px;
  border-radius: var(--radius-sm);
  cursor: pointer;
}
.markdown :deep(.md-copy-btn:hover) { color: var(--text); border-color: var(--line-strong); }
.markdown :deep(code) {
  font-family: var(--font-mono);
  font-size: 0.92em;
}
.markdown :deep(pre code) { font-size: 1em; }
.markdown :deep(a) { color: var(--accent); }
.markdown :deep(ul), .markdown :deep(ol) { margin: 0 0 8px; padding-left: 20px; }
.markdown :deep(blockquote) {
  margin: 0 0 8px;
  padding-left: 10px;
  border-left: 2px solid var(--line-strong);
  color: var(--text-muted);
}
.error-box {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 6px;
  padding: 6px 10px;
  border: 1px solid var(--error);
  background: var(--bg-elev-2);
  border-radius: var(--radius-sm);
}
.error-text { flex: 1; color: var(--error); font-size: var(--fs-xs); word-break: break-word; }
.meta {
  display: flex;
  gap: 6px;
  margin-top: 6px;
  color: var(--text-faint);
  font-size: var(--fs-xxs);
}
</style>
