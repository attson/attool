<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue';

import { createPluginHostSession, type PluginHostSession } from './pluginHost';
import { createPluginProbeConfig } from './pluginProbe';
import type { PluginOperationSnapshot } from './types';

const props = defineProps<{
  operation: PluginOperationSnapshot;
}>();

const emit = defineEmits<{
  ready: [];
  failed: [];
}>();

const frame = ref<HTMLIFrameElement | null>(null);
let session: PluginHostSession | null = null;
let timer: ReturnType<typeof setTimeout> | null = null;
let settled = false;

const source = computed(() => {
  const entry = (props.operation.entry ?? '')
    .split('/')
    .map((segment) => encodeURIComponent(segment))
    .join('/');
  return `attool-plugin-probe://${props.operation.operationId}/${entry}`;
});

const config = computed(() => createPluginProbeConfig(props.operation));

function clear() {
  session?.terminate();
  session = null;
  if (timer) clearTimeout(timer);
  timer = null;
}

function fail() {
  if (settled) return;
  settled = true;
  clear();
  emit('failed');
}

function handleLoad() {
  clear();
  const contentWindow = frame.value?.contentWindow;
  if (!contentWindow) {
    fail();
    return;
  }
  const channel = new MessageChannel();
  session = createPluginHostSession({
    app: config.value.app,
    port: channel.port1,
    correlationId: props.operation.correlationId,
    theme: () => 'dark',
    notify: async () => {},
    saveFile: async () => ({ cancelled: true }),
    onHandshake: () => {
      if (settled) return;
      settled = true;
      clear();
      emit('ready');
    }
  });
  contentWindow.postMessage(
    {
      type: 'attool:connect',
      apiVersion: config.value.apiVersion,
      appId: props.operation.appId
    },
    '*',
    [channel.port2]
  );
  timer = setTimeout(fail, 8_000);
}

onBeforeUnmount(clear);
</script>

<template>
  <iframe
    ref="frame"
    class="probe-frame"
    :src="source"
    :title="`${operation.name} 安装检查`"
    sandbox="allow-scripts allow-same-origin"
    referrerpolicy="no-referrer"
    @load="handleLoad"
    @error="fail"
  />
</template>

<style scoped>
.probe-frame {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  border: 0;
  opacity: 0;
  pointer-events: none;
}
</style>
