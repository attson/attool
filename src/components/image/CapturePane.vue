<script setup lang="ts">
import { inject, ref } from 'vue';
import { NAlert, NButton, NInputNumber } from 'naive-ui';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import Panel from '../ui/Panel.vue';
import { setPendingAnnotateImage } from './imageBus';

type CaptureMode = 'region' | 'window' | 'full';

const delay = ref(0);
const capturing = ref(false);
const notice = ref('');
const lastPath = ref('');
const previewSrc = ref('');

const switchTab = inject<(name: string) => void>('image-tool:switchTab');

async function capture(mode: CaptureMode) {
  notice.value = '';
  capturing.value = true;
  try {
    const response = await invoke<{ outputPath: string }>('capture_screen', {
      request: { mode, delaySeconds: delay.value }
    });
    lastPath.value = response.outputPath;
    previewSrc.value = `${convertFileSrc(response.outputPath)}?t=${Date.now()}`;
  } catch (err) {
    notice.value = String(err);
  } finally {
    capturing.value = false;
  }
}

function openInAnnotate() {
  if (!lastPath.value) return;
  setPendingAnnotateImage(lastPath.value);
  switchTab?.('annotate');
}

function basename(path: string): string {
  const idx = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
  return idx >= 0 ? path.slice(idx + 1) : path;
}
</script>

<template>
  <div class="capture-pane">
    <Panel title="截图">
      <div class="form">
        <div class="btn-row">
          <n-button block secondary :loading="capturing" @click="capture('region')">
            选区截图
          </n-button>
          <n-button block secondary :loading="capturing" @click="capture('window')">
            窗口截图
          </n-button>
          <n-button block secondary :loading="capturing" @click="capture('full')">
            全屏截图
          </n-button>
        </div>

        <label class="field">
          <span class="lbl">延时（秒，0-10）</span>
          <n-input-number v-model:value="delay" :min="0" :max="10" style="width: 100%" />
        </label>

        <p class="note">
          目前仅 macOS：底层 <code>screencapture</code>，选区/窗口时按 <kbd>Esc</kbd> 取消。
          图片临时存到系统 temp 目录，重启后会清。
        </p>

        <n-alert v-if="notice" type="error" :bordered="false">{{ notice }}</n-alert>
      </div>
    </Panel>

    <Panel v-if="previewSrc" title="预览">
      <template #right>
        <span class="mono">{{ basename(lastPath) }}</span>
      </template>
      <div class="preview-wrap">
        <img :src="previewSrc" alt="capture preview" />
      </div>
      <div class="actions">
        <n-button type="primary" :disabled="!lastPath" @click="openInAnnotate">
          在标注 tab 中编辑
        </n-button>
      </div>
    </Panel>
  </div>
</template>

<style scoped>
.capture-pane {
  display: grid;
  gap: 16px;
  padding: 16px;
  height: 100%;
  overflow: auto;
}
.form { display: grid; gap: 12px; }
.btn-row {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
}
@media (max-width: 700px) {
  .btn-row { grid-template-columns: 1fr; }
}
.field { display: grid; gap: 6px; max-width: 200px; }
.lbl { font-size: var(--fs-xxs); color: var(--text-muted); }
.note {
  margin: 0;
  color: var(--text-muted);
  font-size: var(--fs-xs);
  line-height: 1.6;
}
.note code, .note kbd {
  padding: 1px 6px;
  background: var(--bg-elev);
  border-radius: 3px;
  font-family: var(--font-mono, monospace);
}
.preview-wrap {
  display: grid;
  place-items: center;
  padding: 12px;
  background: var(--bg-elev);
  border-radius: var(--radius-md);
  min-height: 200px;
}
.preview-wrap img {
  max-width: 100%;
  max-height: 500px;
  border-radius: 4px;
}
.actions {
  display: flex;
  justify-content: flex-end;
  margin-top: 12px;
}
.mono { font-family: var(--font-mono, monospace); font-size: var(--fs-xs); }
</style>
