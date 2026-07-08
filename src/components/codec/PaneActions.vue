<script setup lang="ts">
import { ref } from 'vue';
import { NButton } from 'naive-ui';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';

const props = defineProps<{ value: string }>();
const state = ref<'idle' | 'ok' | 'fail'>('idle');

async function copy() {
  if (!props.value) return;
  try {
    await writeText(props.value);
    state.value = 'ok';
  } catch {
    state.value = 'fail';
  }
  setTimeout(() => (state.value = 'idle'), 1200);
}

const label = () => (state.value === 'ok' ? '已复制' : state.value === 'fail' ? '失败' : '复制');
</script>

<template>
  <n-button size="tiny" secondary :disabled="!value" @click="copy">
    {{ label() }}
  </n-button>
</template>
