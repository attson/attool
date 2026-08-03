<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { NButton, NDrawer, NDrawerContent, NInput } from 'naive-ui';
import { useAiChat } from './useAiChat';

const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{ (e: 'update:show', v: boolean): void }>();

const chat = useAiChat();

const draft = ref('');

// Seed from the current session only when the drawer opens, not on every
// remote refresh — otherwise an in-flight edit would get clobbered by a
// background openSession() call (e.g. from a streaming reply finishing).
watch(
  () => props.show,
  (v) => {
    if (v) draft.value = chat.currentSession.value?.session.systemPrompt ?? '';
  }
);

const title = computed(() => {
  const t = chat.currentSession.value?.session.title;
  return t ? `System Prompt · ${t}` : 'System Prompt';
});

function close() {
  emit('update:show', false);
}

async function save() {
  const sid = chat.currentSessionId.value;
  if (!sid) { close(); return; }
  await chat.updateSystemPrompt(sid, draft.value);
  close();
}

function cancel() {
  close();
}
</script>

<template>
  <n-drawer :show="show" placement="right" :width="440" @update:show="(v: boolean) => emit('update:show', v)">
    <n-drawer-content :title="title" closable>
      <n-input
        v-model:value="draft"
        type="textarea"
        placeholder="给这个会话设置 system prompt……"
        :autosize="{ minRows: 10, maxRows: 30 }"
      />
      <template #footer>
        <div class="footer">
          <n-button size="small" @click="cancel">取消</n-button>
          <n-button size="small" type="primary" @click="save">保存</n-button>
        </div>
      </template>
    </n-drawer-content>
  </n-drawer>
</template>

<style scoped>
.footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  width: 100%;
}
</style>
