<script setup lang="ts">
import { computed } from 'vue';
import { NButton, NDropdown, NSelect, useMessage } from 'naive-ui';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { createAiApi } from './aiApi';
import { useAiChat } from './useAiChat';

const emit = defineEmits<{
  (e: 'open-settings'): void;
  (e: 'open-prompt'): void;
}>();

const chat = useAiChat();
const api = createAiApi();
const message = useMessage();

function errText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

const noProviders = computed(() => chat.providers.value.length === 0);

const modelOptions = computed(() =>
  chat.providers.value.map((p) => ({
    type: 'group' as const,
    key: p.id,
    label: p.name,
    children: chat.models.value
      .filter((m) => m.providerId === p.id)
      .map((m) => ({ label: m.displayName || m.modelId, value: m.id })),
  }))
);

const selectedModelId = computed(() => chat.currentSession.value?.session.currentModelId ?? null);
const selectDisabled = computed(() => !chat.currentSessionId.value);

async function onModelChange(modelId: string | null) {
  const sid = chat.currentSessionId.value;
  if (!sid || !modelId) return;
  try {
    await chat.switchSessionModel(sid, modelId);
  } catch (e) {
    message.error(errText(e));
  }
}

const exportOptions = [
  { label: 'Markdown', key: 'markdown' },
  { label: 'JSON', key: 'json' },
];

async function onExport(key: string) {
  const sid = chat.currentSessionId.value;
  if (!sid) return;
  try {
    const text = await api.exportSession(sid, key as 'markdown' | 'json');
    await writeText(text);
    message.success('已复制到剪贴板');
  } catch (e) {
    message.error(errText(e));
  }
}
</script>

<template>
  <div class="ai-model-picker">
    <n-select
      :value="selectedModelId"
      :options="modelOptions"
      :disabled="selectDisabled"
      size="small"
      filterable
      placeholder="选择模型"
      class="model-select"
      @update:value="onModelChange"
    />
    <span v-if="noProviders" class="hint">点右侧齿轮先配置提供商</span>

    <div class="actions">
      <n-button size="small" quaternary @click="emit('open-prompt')">System Prompt</n-button>
      <n-dropdown :options="exportOptions" trigger="click" @select="onExport">
        <n-button size="small" quaternary :disabled="!chat.currentSessionId.value">导出</n-button>
      </n-dropdown>
      <n-button size="small" quaternary @click="emit('open-settings')">⚙ 设置</n-button>
    </div>
  </div>
</template>

<style scoped>
.ai-model-picker {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}
.model-select {
  width: 220px;
  min-width: 0;
}
.hint {
  color: var(--text-muted);
  font-size: var(--fs-xxs);
  white-space: nowrap;
}
.actions {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-left: auto;
}
</style>
