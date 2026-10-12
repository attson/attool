<script setup lang="ts">
import { NButton, useMessage } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';

const props = withDefaults(defineProps<{
  command: string;
  copySuccess?: string;
}>(), {
  copySuccess: '命令已复制',
});

const message = useMessage();

async function copyCommand() {
  try {
    await writeText(props.command);
    message.success(props.copySuccess);
  } catch (error) {
    message.error(String(error));
  }
}

async function openSettings() {
  try {
    await invoke('open_system_shortcut_settings');
  } catch (error) {
    message.error(String(error));
  }
}
</script>

<template>
  <div class="system-shortcut">
    <code class="mono">{{ command }}</code>
    <div class="system-shortcut__actions">
      <n-button size="small" secondary @click="copyCommand">复制命令</n-button>
      <n-button size="small" secondary @click="openSettings">打开键盘设置</n-button>
    </div>
  </div>
</template>

<style scoped>
.system-shortcut {
  display: grid;
  gap: 10px;
  padding-top: 2px;
}
.system-shortcut code {
  display: block;
  padding: 8px 10px;
  overflow-wrap: anywhere;
  color: var(--text);
  background: var(--bg-elev);
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  font-size: var(--fs-xs);
}
.system-shortcut__actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
</style>
