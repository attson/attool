<script setup lang="ts">
import { onMounted, ref } from 'vue';
import AiSessionList from './AiSessionList.vue';
import AiModelPicker from './AiModelPicker.vue';
import AiMessageList from './AiMessageList.vue';
import AiComposer from './AiComposer.vue';
import AiSettingsModal from './AiSettingsModal.vue';
import AiSystemPromptDrawer from './AiSystemPromptDrawer.vue';
import { useAiChat } from './useAiChat';

const chat = useAiChat();
const showSettings = ref(false);
const showPrompt = ref(false);

onMounted(async () => {
  await Promise.all([chat.loadProviders(), chat.loadModels(), chat.loadSessions()]);
  if (!chat.currentSessionId.value && chat.sessions.value.length) {
    await chat.openSession(chat.sessions.value[0].id);
  }
});
</script>

<template>
  <div class="ai-chat">
    <aside class="ai-chat__sidebar">
      <AiSessionList />
    </aside>
    <section class="ai-chat__main">
      <header class="ai-chat__header">
        <AiModelPicker @open-settings="showSettings = true" @open-prompt="showPrompt = true" />
      </header>
      <AiMessageList />
      <AiComposer />
    </section>
    <AiSettingsModal v-model:show="showSettings" />
    <AiSystemPromptDrawer v-model:show="showPrompt" />
  </div>
</template>

<style scoped>
.ai-chat {
  display: grid;
  grid-template-columns: 260px 1fr;
  height: 100%;
  min-height: 0;
  background: var(--bg-base);
  color: var(--text);
}
.ai-chat__sidebar { border-right: 1px solid var(--line); min-height: 0; overflow: hidden; }
.ai-chat__main { display: flex; flex-direction: column; min-height: 0; }
.ai-chat__header {
  padding: 12px 16px;
  border-bottom: 1px solid var(--line);
  display: flex; align-items: center; gap: 12px;
}
</style>
