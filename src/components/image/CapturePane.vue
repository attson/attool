<script setup lang="ts">
import { computed, inject, nextTick, onMounted, ref } from 'vue';
import { NAlert, NButton, NInputNumber, useMessage } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import Panel from '../ui/Panel.vue';
import { setPendingAnnotateImage } from './imageBus';
import { formatShortcutForDisplay, keyEventToShortcut } from './shortcut';

type CaptureMode = 'region' | 'window' | 'full';

interface CaptureShortcutInfo {
  shortcut: string;
  defaultShortcut: string;
}

const delay = ref(0);
const capturing = ref(false);
const notice = ref('');

const message = useMessage();
const switchTab = inject<(name: string) => void>('image-tool:switchTab');

const shortcutInfo = ref<CaptureShortcutInfo>({
  shortcut: '',
  defaultShortcut: 'CommandOrControl+Shift+A'
});
const editing = ref(false);
const recordingRaw = ref('');
const recordError = ref('');
const registerError = ref('');
const recorderEl = ref<HTMLDivElement | null>(null);

const shortcutLabel = computed(() => formatShortcutForDisplay(shortcutInfo.value.shortcut));
const recordingLabel = computed(() =>
  recordingRaw.value ? formatShortcutForDisplay(recordingRaw.value) : ''
);
const isChanged = computed(
  () => !!recordingRaw.value && recordingRaw.value !== shortcutInfo.value.shortcut
);
const isDefault = computed(
  () => shortcutInfo.value.shortcut === shortcutInfo.value.defaultShortcut
);

async function loadShortcut() {
  try {
    shortcutInfo.value = await invoke<CaptureShortcutInfo>('get_capture_shortcut');
  } catch (err) {
    console.warn('[capture] load shortcut failed', err);
  }
  try {
    const errors = await invoke<{ capture: string | null }>('get_shortcut_register_errors');
    registerError.value = errors.capture ?? '';
  } catch {
    registerError.value = '';
  }
}

async function capture(mode: CaptureMode) {
  notice.value = '';
  capturing.value = true;
  try {
    if (mode === 'region') {
      // Snipaste-style overlay: draw + annotate + confirm all inside a floating window
      await invoke('open_capture_overlay');
      return;
    }
    const response = await invoke<{ outputPath: string }>('capture_screen', {
      request: { mode, delaySeconds: delay.value }
    });
    setPendingAnnotateImage(response.outputPath);
    switchTab?.('annotate');
  } catch (err) {
    notice.value = String(err);
  } finally {
    capturing.value = false;
  }
}

function startEdit() {
  editing.value = true;
  recordingRaw.value = '';
  recordError.value = '';
  nextTick(() => recorderEl.value?.focus());
}

function cancelEdit() {
  editing.value = false;
  recordingRaw.value = '';
  recordError.value = '';
}

function onRecordKeydown(event: KeyboardEvent) {
  event.preventDefault();
  event.stopPropagation();
  const combo = keyEventToShortcut(event);
  if (!combo) {
    // Only modifiers held so far, or invalid key — wait for the real combo
    recordingRaw.value = '';
    recordError.value = '需要至少一个修饰键（⌘ / Ctrl / Shift / Alt）+ 一个字母/数字/功能键';
    return;
  }
  recordingRaw.value = combo;
  recordError.value = '';
}

async function applyShortcut() {
  if (!recordingRaw.value) return;
  try {
    shortcutInfo.value = await invoke<CaptureShortcutInfo>('set_capture_shortcut', {
      shortcut: recordingRaw.value
    });
    editing.value = false;
    recordingRaw.value = '';
    registerError.value = ''; // 切换成功即代表新快捷键注册成功
    message.success(`已切换到 ${shortcutLabel.value}`);
  } catch (err) {
    recordError.value = String(err);
  }
}

async function restoreDefault() {
  try {
    shortcutInfo.value = await invoke<CaptureShortcutInfo>('set_capture_shortcut', {
      shortcut: shortcutInfo.value.defaultShortcut
    });
    registerError.value = ''; // 恢复默认成功即代表注册成功
    message.success(`已恢复默认 ${shortcutLabel.value}`);
  } catch (err) {
    message.error(String(err));
  }
}

onMounted(loadShortcut);
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
            <strong>选区截图</strong> 进入 Snipaste 式浮层：拖框 → 就地画矩形/箭头/文字 → <kbd>⌘↩</kbd> 完成、<kbd>Esc</kbd> 取消。
            完成后自动复制到剪贴板 + 存文件 + 送标注 tab。
          </p>
          <p>
            <strong>窗口 / 全屏截图</strong> 走 macOS 原生 <code>screencapture</code>，截完直接送标注 tab。
          </p>
          <p class="muted">
            图片存 <code>~/Library/Caches/attool/captures/</code>。
          </p>
        </div>

        <n-alert v-if="notice" type="error" :bordered="false">{{ notice }}</n-alert>
      </div>
    </Panel>

    <Panel title="全局快捷键">
      <template #right>
        <span v-if="!editing" class="mono shortcut-display">{{ shortcutLabel || '未设置' }}</span>
      </template>

      <n-alert
        v-if="registerError"
        type="error"
        :bordered="false"
        style="margin-bottom: 10px"
      >
        {{ registerError }}（该快捷键已被占用，常见占用者：系统/桌面环境、输入法、远程桌面等，请点“修改”换一个）
      </n-alert>

      <div v-if="!editing" class="shortcut-view">
        <p class="tips-inline">
          任何窗口下按快捷键都直接开始选区截图，截完自动送入标注 tab。
        </p>
        <div class="btn-inline">
          <n-button size="small" secondary @click="startEdit">修改</n-button>
          <n-button v-if="!isDefault" size="small" secondary @click="restoreDefault">
            恢复默认
          </n-button>
        </div>
      </div>

      <div v-else class="shortcut-edit">
        <div
          class="recorder"
          tabindex="0"
          @keydown="onRecordKeydown"
          ref="recorderEl"
        >
          <div v-if="recordingRaw" class="captured mono">{{ recordingLabel }}</div>
          <div v-else class="hint">按下你想设置的组合（例如 ⌘⇧A）...</div>
        </div>
        <n-alert v-if="recordError" type="warning" :bordered="false">{{ recordError }}</n-alert>
        <div class="btn-inline">
          <n-button size="small" secondary @click="cancelEdit">取消</n-button>
          <n-button size="small" type="primary" :disabled="!isChanged" @click="applyShortcut">
            保存
          </n-button>
        </div>
        <p class="tips-inline muted">
          若系统里已有 app 占用该组合，Tauri 注册会失败，会显示错误让你换一个。
        </p>
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
.tips p, .tips-inline { margin: 0; color: var(--text); font-size: var(--fs-xs); line-height: 1.6; }
.tips p.muted, .tips-inline.muted { color: var(--text-muted); font-size: var(--fs-xxs); }
.tips code, .tips kbd, .shortcut-edit kbd {
  padding: 1px 6px;
  background: var(--bg-base);
  border: 1px solid var(--line);
  border-radius: 3px;
  font-family: var(--font-mono, monospace);
  font-size: var(--fs-xxs);
}

.shortcut-view {
  display: grid;
  gap: 10px;
}
.shortcut-display {
  padding: 2px 10px;
  border: 1px solid var(--line-strong);
  border-radius: var(--radius-sm);
  font-size: var(--fs-sm);
}
.btn-inline {
  display: flex;
  gap: 8px;
}

.shortcut-edit {
  display: grid;
  gap: 10px;
}
.recorder {
  display: grid;
  place-items: center;
  min-height: 80px;
  border: 2px dashed var(--line-strong);
  border-radius: var(--radius-md);
  outline: none;
  cursor: pointer;
  transition: border-color 0.15s;
}
.recorder:focus {
  border-color: var(--accent, #10b981);
  background: var(--bg-elev);
}
.captured {
  font-size: var(--fs-xl);
  color: var(--text);
  padding: 8px 20px;
  border-radius: var(--radius-sm);
  background: var(--bg-elev);
}
.hint { color: var(--text-muted); font-size: var(--fs-sm); }
.mono { font-family: var(--font-mono, monospace); }
</style>
