<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import { useAiChat } from './useAiChat';
import AiMessageBubble from './AiMessageBubble.vue';
import type { AiMessage } from '../../types/ai';

const chat = useAiChat();

const scrollEl = ref<HTMLDivElement | null>(null);

// "Near bottom" threshold mirrors chat-UI convention: if the user has
// scrolled up past this slack, respect it and stop auto-following new output.
function isNearBottom(): boolean {
  const el = scrollEl.value;
  if (!el) return true;
  return el.scrollTop >= el.scrollHeight - el.clientHeight - 100;
}

function scrollToBottom() {
  const el = scrollEl.value;
  if (el) el.scrollTop = el.scrollHeight;
}

const visibleMessages = computed<AiMessage[]>(() =>
  (chat.currentSession.value?.messages ?? []).filter((m) => m.role !== 'system')
);

function modelNameFor(m: AiMessage): string | null {
  if (!m.modelId) return null;
  return chat.models.value.find((x) => x.id === m.modelId)?.displayName ?? null;
}

function onRetry(assistantId: string) {
  chat.retryMessage(assistantId);
}

onMounted(async () => {
  await nextTick();
  scrollToBottom();
});

// Session switch always jumps to bottom (fresh view); mid-session growth
// (streaming deltas, new turns) only follows if the user hasn't scrolled up.
watch(() => chat.currentSessionId.value, async () => {
  await nextTick();
  scrollToBottom();
});

watch(() => chat.currentSession.value?.messages.length, async (_len, prevLen) => {
  if (prevLen === undefined) return;
  const shouldFollow = isNearBottom();
  await nextTick();
  if (shouldFollow) scrollToBottom();
});
</script>

<template>
  <div ref="scrollEl" class="ai-message-list">
    <div v-if="chat.currentSessionId.value === null" class="empty">从左侧选择或新建一个会话</div>
    <div v-else-if="visibleMessages.length === 0" class="empty">在下方输入框开始你的对话吧</div>
    <template v-else>
      <AiMessageBubble
        v-for="m in visibleMessages"
        :key="m.id"
        :message="m"
        :model-name="modelNameFor(m)"
        @retry="onRetry"
      />
    </template>
  </div>
</template>

<style scoped>
.ai-message-list {
  flex: 1;
  overflow-y: auto;
  padding: 8px 0;
}
.empty {
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
  font-size: var(--fs-md);
}
</style>
