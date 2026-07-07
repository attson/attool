<script setup lang="ts">
import { inject, ref } from 'vue';
import { NAlert, NButton, NInputNumber } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import Panel from '../ui/Panel.vue';
import { setPendingAnnotateImage } from './imageBus';

type CaptureMode = 'region' | 'window' | 'full';

const delay = ref(0);
const capturing = ref(false);
const notice = ref('');

const switchTab = inject<(name: string) => void>('image-tool:switchTab');

const shortcutLabel = navigator.userAgent.includes('Mac') ? '⌘⇧A' : 'Ctrl+Shift+A';

async function capture(mode: CaptureMode) {
  notice.value = '';
  capturing.value = true;
  try {
    const response = await invoke<{ outputPath: string }>('capture_screen', {
      request: { mode, delaySeconds: delay.value }
    });
    // Direct-to-annotation, WeChat-style
    setPendingAnnotateImage(response.outputPath);
    switchTab?.('annotate');
  } catch (err) {
    notice.value = String(err);
  } finally {
    capturing.value = false;
  }
}
</script>

<template>
  <div class="capture-pane">
    <Panel title="截图">
      <div class="form">
        <div class="btn-row">
          <n-button block type="primary" :loading="capturing" @click="capture('region')">
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

        <div class="tips">
          <p>
            <strong>全局快捷键 <kbd>{{ shortcutLabel }}</kbd></strong>：任何窗口下按都直接开始选区截图。
          </p>
          <p>
            截图完成后自动跳到 <strong>标注 tab</strong> 打开图片（微信截图那种流程）。
            选区模式按 <kbd>Space</kbd> 切窗口、<kbd>Esc</kbd> 取消。
          </p>
          <p class="muted">
            底层 macOS <code>screencapture</code>，图片存 <code>~/Library/Caches/attool/captures/</code>。
          </p>
        </div>

        <n-alert v-if="notice" type="error" :bordered="false">{{ notice }}</n-alert>
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
.form { display: grid; gap: 14px; }
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
.tips {
  display: grid;
  gap: 6px;
  padding: 12px 14px;
  background: var(--bg-elev);
  border-radius: var(--radius-md);
  font-size: var(--fs-xs);
  line-height: 1.6;
}
.tips p { margin: 0; color: var(--text); }
.tips p.muted { color: var(--text-muted); font-size: var(--fs-xxs); }
.tips code, .tips kbd {
  padding: 1px 6px;
  background: var(--bg-base);
  border: 1px solid var(--line);
  border-radius: 3px;
  font-family: var(--font-mono, monospace);
  font-size: var(--fs-xxs);
}
</style>
