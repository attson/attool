<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
import { NButton, NSpin, useMessage } from 'naive-ui';

import type { ThemeName } from '../../composables/useTheme';
import { createPluginHostSession, type HostFileSaveRequest, type PluginHostSession } from './pluginHost';
import type { InstalledPlugin } from './types';

const props = defineProps<{
  app: InstalledPlugin;
  theme: ThemeName;
}>();

const emit = defineEmits<{
  versionState: [app: InstalledPlugin];
}>();

type FrameState = 'loading' | 'connecting' | 'ready' | 'error';

const message = useMessage();
const frame = ref<HTMLIFrameElement | null>(null);
const state = ref<FrameState>('loading');
const errorMessage = ref('');
const reloadRevision = ref(0);
let session: PluginHostSession | null = null;
let handshakeTimer: ReturnType<typeof setTimeout> | null = null;
let handlingFailure = false;

const source = computed(() => {
  const entry = props.app.entry
    .split('/')
    .map((segment) => encodeURIComponent(segment))
    .join('/');
  return `attool-plugin://${props.app.appId}/${entry}?revision=${reloadRevision.value}`;
});

function clearSession() {
  session?.terminate();
  session = null;
  if (handshakeTimer) clearTimeout(handshakeTimer);
  handshakeTimer = null;
}

async function savePluginFile(input: HostFileSaveRequest) {
  const path = await save({ defaultPath: input.suggestedName });
  if (!path) return { cancelled: true };
  const base64 = input.encoding === 'base64' ? input.content : utf8ToBase64(input.content);
  await invoke('write_binary_file', { path, base64 });
  return { cancelled: false };
}

function handleLoad() {
  clearSession();
  handlingFailure = false;
  const contentWindow = frame.value?.contentWindow;
  if (!contentWindow) {
    void handleLaunchFailure('无法建立应用窗口连接');
    return;
  }
  state.value = 'connecting';
  errorMessage.value = '';
  const channel = new MessageChannel();
  session = createPluginHostSession({
    app: props.app,
    port: channel.port1,
    theme: () => props.theme,
    notify: async ({ title, message: body, kind }) => {
      message[kind](title ? `${title}: ${body}` : body);
    },
    saveFile: savePluginFile,
    onHandshake: () => {
      if (handshakeTimer) clearTimeout(handshakeTimer);
      handshakeTimer = null;
      void confirmLaunch();
    }
  });
  contentWindow.postMessage(
    {
      type: 'attool:connect',
      apiVersion: '1.0',
      appId: props.app.appId
    },
    '*',
    [channel.port2]
  );
  handshakeTimer = setTimeout(() => {
    clearSession();
    void handleLaunchFailure('应用未在规定时间内完成 Host API 握手');
  }, 8_000);
}

async function confirmLaunch() {
  try {
    await invoke<boolean>('plugin_confirm_plugin_launch', {
      appId: props.app.appId,
      version: props.app.version
    });
    state.value = 'ready';
  } catch {
    clearSession();
    await handleLaunchFailure('无法确认当前应用版本');
  }
}

async function handleLaunchFailure(reason: string) {
  if (handlingFailure) return;
  handlingFailure = true;
  try {
    const rolledBack = await invoke<InstalledPlugin | null>('plugin_rollback_plugin_launch', {
      appId: props.app.appId,
      version: props.app.version
    });
    if (rolledBack) {
      message.warning(`启动失败，已恢复 ${rolledBack.version}`);
      emit('versionState', rolledBack);
      return;
    }
  } catch {
    fail('自动回滚失败，请在应用管理中查看操作记录');
    return;
  }
  fail(reason);
}

function fail(reason: string) {
  state.value = 'error';
  errorMessage.value = reason;
}

function reload() {
  clearSession();
  state.value = 'loading';
  errorMessage.value = '';
  handlingFailure = false;
  reloadRevision.value += 1;
}

function utf8ToBase64(value: string) {
  const bytes = new TextEncoder().encode(value);
  let binary = '';
  for (let offset = 0; offset < bytes.length; offset += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000));
  }
  return btoa(binary);
}

watch(
  () => props.theme,
  (theme) => session?.notifyTheme(theme)
);

onBeforeUnmount(clearSession);
</script>

<template>
  <section class="plugin-frame">
    <iframe
      ref="frame"
      :src="source"
      :title="app.name"
      sandbox="allow-scripts allow-same-origin"
      referrerpolicy="no-referrer"
      @load="handleLoad"
      @error="handleLaunchFailure('应用资源加载失败')"
    />

    <div v-if="state === 'loading' || state === 'connecting'" class="frame-state">
      <n-spin size="small" />
      <span>{{ state === 'loading' ? '正在加载应用' : '正在建立安全连接' }}</span>
    </div>

    <div v-else-if="state === 'error'" class="frame-state error-state">
      <strong>应用无法启动</strong>
      <span>{{ errorMessage }}</span>
      <n-button size="small" secondary @click="reload">重新加载</n-button>
    </div>
  </section>
</template>

<style scoped>
.plugin-frame {
  position: relative;
  width: 100%;
  height: 100%;
  min-height: 480px;
  overflow: hidden;
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
}

iframe {
  display: block;
  width: 100%;
  height: 100%;
  min-height: 480px;
  border: 0;
  background: var(--bg-base);
}

.frame-state {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 9px;
  background: var(--bg-elevated);
  color: var(--text-muted);
  font-size: var(--fs-xs);
}

.error-state {
  flex-direction: column;
}

.error-state strong {
  color: var(--text);
  font-size: var(--fs-md);
}
</style>
