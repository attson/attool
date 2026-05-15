<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { NModal, NButton } from 'naive-ui';
import { getVersion } from '@tauri-apps/api/app';
import type { UpdaterState } from '../../composables/useUpdater';

const props = defineProps<{
  show: boolean;
  state: UpdaterState;
  autoCheck: boolean;
}>();

const emit = defineEmits<{
  'update:show': [v: boolean];
  check: [];
  install: [];
  relaunch: [];
  'update:autoCheck': [v: boolean];
}>();

const currentVersion = ref<string>('');

onMounted(async () => {
  try {
    currentVersion.value = await getVersion();
  } catch {
    currentVersion.value = 'unknown';
  }
});

const latestText = computed(() => {
  switch (props.state.status) {
    case 'idle': return '未检查';
    case 'checking': return '检查中...';
    case 'up-to-date': return '已是最新版本';
    case 'available': return `v${props.state.available?.version ?? ''}`;
    case 'downloading': return `下载中 ${props.state.downloadPercent ?? 0}%`;
    case 'ready': return '已下载，待重启';
    case 'error': return '检查失败';
  }
});

const showProxy = computed({
  get: () => props.show,
  set: (v) => emit('update:show', v)
});
</script>

<template>
  <n-modal v-model:show="showProxy" preset="card" title="设置" style="max-width: 480px">
    <section class="block">
      <h3>软件更新</h3>
      <div class="row">
        <span class="key">当前版本</span>
        <span class="val mono">v{{ currentVersion }}</span>
      </div>
      <div class="row">
        <span class="key">最新版本</span>
        <span class="val">{{ latestText }}</span>
      </div>
      <div class="actions">
        <n-button :loading="state.status === 'checking'" @click="emit('check')">立即检查更新</n-button>
        <n-button
          v-if="state.status === 'available'"
          type="primary"
          @click="emit('install')"
        >下载并安装</n-button>
        <n-button
          v-else-if="state.status === 'downloading'"
          :loading="true"
          disabled
        >下载中 {{ state.downloadPercent ?? 0 }}%</n-button>
        <n-button
          v-else-if="state.status === 'ready'"
          type="primary"
          @click="emit('relaunch')"
        >立即重启</n-button>
      </div>
      <label class="toggle-row">
        <input
          type="checkbox"
          :checked="autoCheck"
          @change="(e) => emit('update:autoCheck', (e.target as HTMLInputElement).checked)"
        />
        <span>启动时自动检查更新</span>
      </label>
    </section>
  </n-modal>
</template>

<style scoped>
.block { display: grid; gap: 10px; }
.block h3 {
  margin: 0 0 6px;
  font-size: var(--fs-xxs);
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.08em;
  font-weight: 600;
}
.row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: var(--fs-sm);
  padding: 4px 0;
  border-bottom: 1px solid var(--line);
}
.row .key { color: var(--text-muted); }
.row .val { color: var(--text); }
.row .val.mono { font-family: var(--font-mono); }

.actions { padding-top: 4px; }

.toggle-row {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: var(--fs-sm);
  cursor: pointer;
  padding-top: 6px;
}
.toggle-row input { cursor: pointer; }
</style>
