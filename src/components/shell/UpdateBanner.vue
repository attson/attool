<script setup lang="ts">
import { computed } from 'vue';
import type { UpdaterState } from '../../composables/useUpdater';

const props = defineProps<{ state: UpdaterState }>();

const emit = defineEmits<{
  install: [];
  skip: [];
  relaunch: [];
  dismiss: [];
}>();

const visible = computed(() => {
  const s = props.state.status;
  if (s === 'available' || s === 'downloading' || s === 'ready') return true;
  if (s === 'error' && props.state.trigger === 'manual') return true;
  return false;
});

const versionText = computed(() => props.state.available?.version ?? '');
</script>

<template>
  <div v-if="visible" class="update-banner" :data-state="state.status">
    <span class="msg">
      <template v-if="state.status === 'available'">
        新版本 <strong>v{{ versionText }}</strong> 可用
      </template>
      <template v-else-if="state.status === 'downloading'">
        正在下载 v{{ versionText }}... {{ state.downloadPercent ?? 0 }}%
      </template>
      <template v-else-if="state.status === 'ready'">
        下载完成，重启以应用更新
      </template>
      <template v-else-if="state.status === 'error'">
        更新失败：{{ state.error }}
      </template>
    </span>

    <div class="actions">
      <template v-if="state.status === 'available'">
        <button class="btn primary" type="button" @click="emit('install')">现在安装</button>
        <button class="btn ghost" type="button" @click="emit('skip')">稍后</button>
      </template>
      <template v-else-if="state.status === 'ready'">
        <button class="btn primary" type="button" @click="emit('relaunch')">立即重启</button>
        <button class="btn ghost" type="button" @click="emit('dismiss')">稍后重启</button>
      </template>
      <template v-else-if="state.status === 'error'">
        <button class="btn ghost" type="button" @click="emit('dismiss')">关闭</button>
      </template>
    </div>

    <div v-if="state.status === 'downloading'" class="progress">
      <i :style="{ width: `${state.downloadPercent ?? 0}%` }"></i>
    </div>
  </div>
</template>

<style scoped>
.update-banner {
  position: relative;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 6px 22px;
  background: var(--accent-soft);
  color: var(--text);
  border-bottom: 1px solid var(--line);
  font-size: var(--fs-xs);
  min-height: 32px;
}
.update-banner[data-state="error"] {
  background: color-mix(in srgb, var(--error) 18%, transparent);
  color: var(--error);
}

.msg { flex: 1; min-width: 0; }
.msg strong { font-weight: 600; }

.actions { display: flex; gap: 6px; flex-shrink: 0; }

.btn {
  padding: 3px 10px;
  border-radius: var(--radius);
  border: 0;
  font-size: var(--fs-xxs);
  font-weight: 500;
  cursor: pointer;
  line-height: 1.4;
}
.btn.primary { background: var(--accent); color: var(--accent-fg); }
.btn.ghost {
  background: transparent;
  color: inherit;
  border: 1px solid var(--line-strong);
}
.btn.primary:hover { filter: brightness(1.05); }
.btn.ghost:hover { border-color: var(--text-muted); }

.progress {
  position: absolute;
  left: 0; right: 0; bottom: 0;
  height: 3px;
  background: transparent;
  overflow: hidden;
}
.progress > i {
  display: block;
  height: 100%;
  background: var(--accent);
  transition: width var(--motion-fast);
}
</style>
