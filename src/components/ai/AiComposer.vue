<script setup lang="ts">
import { computed, ref } from 'vue';
import { NButton, NInput, NTooltip, useMessage } from 'naive-ui';
import { createAiApi } from './aiApi';
import { useAiChat } from './useAiChat';
import { parseCapabilities } from '../../types/ai';
import type { AiContentPart } from '../../types/ai';

const chat = useAiChat();
const api = createAiApi();
const message = useMessage();

function errText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

const text = ref('');
const attachments = ref<AiContentPart[]>([]);
const fileInput = ref<HTMLInputElement | null>(null);

const currentModel = computed(() => {
  const modelId = chat.currentSession.value?.session.currentModelId;
  if (!modelId) return null;
  return chat.models.value.find((m) => m.id === modelId) ?? null;
});

const capabilities = computed(() =>
  currentModel.value ? parseCapabilities(currentModel.value.capabilities) : []
);
const canImage = computed(() => capabilities.value.includes('image'));
const canVideo = computed(() => capabilities.value.includes('video'));
const canAudio = computed(() => capabilities.value.includes('audio'));

const isStreaming = computed(() => chat.streamingMessageId.value !== null);
const inputDisabled = computed(() => !chat.currentSessionId.value || !currentModel.value);
const hintText = computed(() => {
  if (!chat.currentSessionId.value) return '请先选中一个会话';
  if (!currentModel.value) return '请先选择模型';
  return null;
});

// While streaming the button always shows "取消" and stays clickable;
// the "no session / no model / empty" checks only gate the "发送" state.
const sendDisabled = computed(() => {
  if (isStreaming.value) return false;
  if (!chat.currentSessionId.value || !currentModel.value) return true;
  if (!text.value.trim() && attachments.value.length === 0) return true;
  return false;
});

function pickImage() {
  fileInput.value?.click();
}

async function onImageSelected(e: Event) {
  const input = e.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = ''; // allow re-selecting the same file later
  if (!file) return;
  try {
    const buf = await file.arrayBuffer();
    const bytes = new Uint8Array(buf);
    const mime = file.type || 'image/png';
    const path = await api.saveAttachment(bytes, mime);
    attachments.value = [...attachments.value, { type: 'image', path, mime }];
  } catch (err) {
    message.error(errText(err));
  }
}

function removeAttachment(i: number) {
  attachments.value = attachments.value.filter((_, idx) => idx !== i);
}

async function doSend() {
  const t = text.value.trim();
  const atts = attachments.value.slice();
  try {
    await chat.sendMessage(t, atts);
    // Only clear the draft once the send actually succeeds, so a rejected
    // ai_send (missing session/model, provider error, DB error) leaves the
    // user's text and attachment chips intact for retry.
    text.value = '';
    attachments.value = [];
  } catch (err) {
    message.error(errText(err));
  }
}

async function onSendClick() {
  if (isStreaming.value) {
    await chat.cancelStreaming();
    return;
  }
  if (sendDisabled.value) return;
  await doSend();
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
    e.preventDefault();
    onSendClick();
    return;
  }
  if (e.key === 'Enter' && e.shiftKey) {
    return; // let the textarea insert its own newline
  }
  if (e.key === 'Escape' && isStreaming.value) {
    chat.cancelStreaming();
  }
}
</script>

<template>
  <div class="ai-composer">
    <div v-if="hintText" class="hint">{{ hintText }}</div>

    <n-input
      v-model:value="text"
      type="textarea"
      placeholder="输入消息…"
      :autosize="{ minRows: 2, maxRows: 8 }"
      :disabled="inputDisabled"
      @keydown="onKeydown"
    />

    <div v-if="attachments.length" class="attachments">
      <div v-for="(a, i) in attachments" :key="i" class="chip">
        <img v-if="a.type === 'image'" :src="a.path" class="chip-thumb" />
        <button class="chip-remove" title="移除" @click="removeAttachment(i)">✕</button>
      </div>
    </div>

    <div class="toolbar">
      <input
        ref="fileInput"
        type="file"
        accept="image/*"
        class="hidden-input"
        @change="onImageSelected"
      />
      <n-button v-if="canImage" size="small" quaternary @click="pickImage">图片</n-button>

      <n-tooltip v-if="canVideo" trigger="hover">
        <template #trigger>
          <span class="disabled-btn-wrap">
            <n-button size="small" quaternary disabled>视频</n-button>
          </span>
        </template>
        暂未接入
      </n-tooltip>

      <n-tooltip v-if="canAudio" trigger="hover">
        <template #trigger>
          <span class="disabled-btn-wrap">
            <n-button size="small" quaternary disabled>音频</n-button>
          </span>
        </template>
        暂未接入
      </n-tooltip>

      <div class="spacer"></div>

      <n-button
        size="small"
        type="primary"
        :disabled="sendDisabled"
        @click="onSendClick"
      >
        {{ isStreaming ? '取消' : '发送' }}
      </n-button>
    </div>
  </div>
</template>

<style scoped>
.ai-composer {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 10px 12px;
  border-top: 1px solid var(--line);
  background: var(--bg-elevated);
}
.hint {
  font-size: var(--fs-xxs);
  color: var(--text-muted);
}
.attachments {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.chip {
  position: relative;
  width: 48px;
  height: 48px;
  border-radius: var(--radius-sm);
  overflow: hidden;
  border: 1px solid var(--line);
}
.chip-thumb {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}
.chip-remove {
  position: absolute;
  top: 0;
  right: 0;
  width: 16px;
  height: 16px;
  line-height: 16px;
  text-align: center;
  padding: 0;
  border: none;
  border-radius: 0 0 0 var(--radius-sm);
  background: var(--bg-overlay);
  color: var(--text-muted);
  font-size: var(--fs-xxs);
  cursor: pointer;
}
.chip-remove:hover {
  color: var(--error);
}
.toolbar {
  display: flex;
  align-items: center;
  gap: 4px;
}
.spacer {
  flex: 1;
}
.hidden-input {
  display: none;
}
.disabled-btn-wrap {
  display: inline-flex;
}
</style>
