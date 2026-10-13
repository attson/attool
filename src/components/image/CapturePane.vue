<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import { NAlert, NButton, useMessage } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import RecordingTool from '../recording/RecordingTool.vue';
import SystemShortcutNotice from '../ui/SystemShortcutNotice.vue';
import SystemShortcutSetup from '../ui/SystemShortcutSetup.vue';
import { formatShortcutForDisplay, keyEventToShortcut } from './shortcut';
import { useRequestedCaptureMode, type CaptureMode } from './imageBus';

interface CaptureShortcutInfo {
  shortcut: string;
  defaultShortcut: string;
  unavailableReason: string | null;
  systemShortcutCommand: string | null;
}

const message = useMessage();
const mode = ref<CaptureMode>('screenshot');
const recordingBusy = ref(false);
const recordingShortcutTrigger = ref(0);
const shortcutInfo = ref<CaptureShortcutInfo>({
  shortcut: '',
  defaultShortcut: 'CommandOrControl+Shift+A',
  unavailableReason: null,
  systemShortcutCommand: null
});
const editing = ref(false);
const recordingRaw = ref('');
const recordError = ref('');
const registerError = ref('');
const capturing = ref(false);
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
const modeLocked = computed(() => capturing.value || recordingBusy.value);
const captureModeRequest = useRequestedCaptureMode();

async function loadShortcut() {
  try {
    shortcutInfo.value = await invoke<CaptureShortcutInfo>('get_capture_shortcut');
  } catch (error) {
    console.warn('[capture] load shortcut failed', error);
  }
  try {
    const errors = await invoke<{ capture: string | null }>('get_shortcut_register_errors');
    registerError.value = errors.capture ?? '';
  } catch {
    registerError.value = '';
  }
}

function selectMode(nextMode: CaptureMode) {
  if (modeLocked.value) return;
  mode.value = nextMode;
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
    recordingRaw.value = '';
    recordError.value = '需要至少一个修饰键（⌘ / Ctrl / Shift / Alt）和一个字母、数字或功能键';
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
    registerError.value = '';
    message.success(`已切换到 ${shortcutLabel.value}`);
  } catch (error) {
    recordError.value = String(error);
  }
}

async function restoreDefault() {
  try {
    shortcutInfo.value = await invoke<CaptureShortcutInfo>('set_capture_shortcut', {
      shortcut: shortcutInfo.value.defaultShortcut
    });
    registerError.value = '';
    message.success(`已恢复默认 ${shortcutLabel.value}`);
  } catch (error) {
    message.error(String(error));
  }
}

async function startCapture() {
  capturing.value = true;
  try {
    await invoke('open_capture_overlay');
  } catch (error) {
    message.error(String(error), { duration: 8000, closable: true });
  } finally {
    capturing.value = false;
  }
}

async function drainCaptureModeRequest() {
  const request = captureModeRequest.consume();
  if (!request) return;
  mode.value = request.mode;
  if (request.toggle) {
    await nextTick();
    recordingShortcutTrigger.value += 1;
  }
}

onMounted(() => {
  void loadShortcut();
  void drainCaptureModeRequest();
});
watch(captureModeRequest.requested, (request) => {
  if (request) void drainCaptureModeRequest();
});
</script>

<template>
  <div class="capture-workspace">
    <header class="mode-toolbar">
      <div class="mode-switch" role="tablist" aria-label="捕获模式">
        <button
          class="mode-button"
          :class="{ active: mode === 'screenshot' }"
          :disabled="modeLocked"
          role="tab"
          :aria-selected="mode === 'screenshot'"
          @click="selectMode('screenshot')"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M4 7h3l1.5-2h7L17 7h3a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V9a2 2 0 0 1 2-2Z" />
            <circle cx="12" cy="13" r="4" />
          </svg>
          截图
        </button>
        <button
          class="mode-button"
          :class="{ active: mode === 'recording' }"
          :disabled="modeLocked"
          role="tab"
          :aria-selected="mode === 'recording'"
          @click="selectMode('recording')"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <rect x="3" y="5" width="14" height="14" rx="2" />
            <path d="m17 10 4-2v8l-4-2Z" />
          </svg>
          录屏
        </button>
      </div>

      <div v-if="mode === 'screenshot'" class="mode-status">
        <span class="status-dot" aria-hidden="true"></span>
        <span>{{ capturing ? '正在打开截图区域' : '准备截图' }}</span>
        <span v-if="shortcutLabel" class="shortcut-hint mono">{{ shortcutLabel }}</span>
      </div>
    </header>

    <RecordingTool
      v-show="mode === 'recording'"
      class="embedded-recording"
      :shortcut-trigger="recordingShortcutTrigger"
      @busy-change="recordingBusy = $event"
    />

    <div v-show="mode === 'screenshot'" class="screenshot-layout">
      <main class="capture-main">
        <div class="capture-stage">
          <div class="desktop-visual" aria-hidden="true">
            <div class="visual-topbar"><span></span><span></span><span></span></div>
            <div class="visual-body">
              <div class="visual-rail"><i class="active"></i><i></i><i></i><i></i></div>
              <div class="visual-window">
                <div class="visual-window-head"><i></i><i></i><i></i></div>
                <div class="visual-window-body">
                  <div class="visual-sidebar"><b></b><i></i><i></i><i></i></div>
                  <div class="visual-content">
                    <b></b><i></i>
                    <div class="visual-cards"><span></span><span></span><span></span></div>
                    <div class="visual-chart"></div>
                  </div>
                </div>
              </div>
            </div>
            <div class="visual-dock"><i></i><i class="active"></i><i></i><i></i><i></i></div>
          </div>

          <div class="source-badge">
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M4 9V4h5M15 4h5v5M20 15v5h-5M9 20H4v-5" />
            </svg>
            区域截图
          </div>
        </div>

        <n-alert
          v-if="registerError && !shortcutInfo.unavailableReason"
          class="capture-alert"
          type="error"
          :bordered="false"
        >
          {{ registerError }}。该快捷键可能已被系统、输入法或其他应用占用。
        </n-alert>

        <div class="capture-launchbar">
          <div class="source-option active">
            <span class="source-icon">
              <svg viewBox="0 0 24 24" aria-hidden="true">
                <path d="M4 9V4h5M15 4h5v5M20 15v5h-5M9 20H4v-5" />
              </svg>
            </span>
            <span class="source-copy"><strong>区域</strong><small>拖动选择范围</small></span>
          </div>
          <n-button type="primary" :loading="capturing" @click="startCapture">
            <template #icon>
              <svg viewBox="0 0 24 24" aria-hidden="true">
                <path d="M4 9V4h5M15 4h5v5M20 15v5h-5M9 20H4v-5" />
              </svg>
            </template>
            选择区域
          </n-button>
        </div>
      </main>

      <aside class="capture-settings">
        <section class="setting-section">
          <h3>截图设置</h3>
          <div class="setting-row"><span>来源</span><span class="setting-value">选定区域</span></div>
          <div class="setting-row"><span>格式</span><span class="setting-value mono">PNG</span></div>
          <div class="setting-row"><span>完成后</span><span class="setting-value">标注与复制</span></div>
        </section>

        <section class="setting-section shortcut-section">
          <div class="section-heading">
            <h3>全局快捷键</h3>
            <span v-if="!editing && shortcutLabel" class="shortcut-key mono">{{ shortcutLabel }}</span>
          </div>

          <SystemShortcutNotice
            v-if="shortcutInfo.unavailableReason"
            :title="shortcutInfo.unavailableReason"
          >
            <SystemShortcutSetup
              v-if="shortcutInfo.systemShortcutCommand"
              :command="shortcutInfo.systemShortcutCommand"
              copy-success="截图命令已复制"
            />
          </SystemShortcutNotice>

          <template v-else-if="!editing">
            <div class="shortcut-actions">
              <n-button size="small" secondary @click="startEdit">修改</n-button>
              <n-button v-if="!isDefault" size="small" secondary @click="restoreDefault">
                恢复默认
              </n-button>
            </div>
          </template>

          <div v-else class="shortcut-editor">
            <div
              ref="recorderEl"
              class="shortcut-recorder"
              tabindex="0"
              @keydown="onRecordKeydown"
            >
              <span v-if="recordingRaw" class="mono">{{ recordingLabel }}</span>
              <span v-else>请按下组合键</span>
            </div>
            <n-alert v-if="recordError" type="warning" :bordered="false">{{ recordError }}</n-alert>
            <div class="shortcut-actions">
              <n-button size="small" secondary @click="cancelEdit">取消</n-button>
              <n-button size="small" type="primary" :disabled="!isChanged" @click="applyShortcut">
                保存
              </n-button>
            </div>
          </div>
        </section>
      </aside>
    </div>
  </div>
</template>

<style scoped>
.capture-workspace {
  height: 100%;
  min-height: 0;
  display: grid;
  grid-template-rows: 52px minmax(0, 1fr);
  background: var(--bg-base);
}
.mode-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 8px 16px;
  border-bottom: 1px solid var(--line);
}
.mode-switch {
  height: 34px;
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 3px;
  padding: 3px;
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
}
.mode-button {
  min-width: 100px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  border: 0;
  border-radius: var(--radius);
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
  transition: color var(--motion-fast), background var(--motion-fast);
}
.mode-button:hover:not(:disabled) { color: var(--text); }
.mode-button.active {
  background: var(--bg-elev-2);
  color: var(--text);
  box-shadow: inset 0 0 0 1px var(--line-strong);
}
.mode-button:disabled { cursor: not-allowed; opacity: 0.65; }
.mode-button svg,
.source-badge svg,
.source-icon svg,
.capture-launchbar :deep(.n-button__icon svg) {
  width: 16px;
  height: 16px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.8;
  stroke-linecap: round;
  stroke-linejoin: round;
}
.mode-status {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 9px;
  color: var(--text-muted);
  font-size: var(--fs-xs);
}
.status-dot { width: 7px; height: 7px; border-radius: var(--radius-pill); background: var(--text-faint); }
.shortcut-hint { color: var(--text-faint); }
.embedded-recording { min-height: 0; }

.screenshot-layout {
  min-width: 0;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 280px;
}
.capture-main {
  min-width: 0;
  min-height: 0;
  display: grid;
  grid-template-rows: minmax(0, 1fr) auto auto;
  overflow: auto;
}
.capture-stage {
  position: relative;
  min-height: 280px;
  margin: 16px 16px 0;
  overflow: hidden;
  border: 1px solid var(--line-strong);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
}
.desktop-visual {
  position: absolute;
  inset: 0;
  display: grid;
  grid-template-rows: 26px minmax(0, 1fr) 30px;
  background: var(--bg-elev-2);
  color: var(--text-muted);
  opacity: 0.92;
}
.visual-topbar { display: flex; align-items: center; justify-content: flex-end; gap: 7px; padding: 0 10px; border-bottom: 1px solid var(--line); background: var(--bg-base); }
.visual-topbar span { width: 20px; height: 4px; border-radius: var(--radius-sm); background: var(--line-strong); }
.visual-body { min-height: 0; display: grid; grid-template-columns: 44px minmax(0, 1fr); }
.visual-rail { display: grid; align-content: start; justify-items: center; gap: 9px; padding-top: 12px; border-right: 1px solid var(--line); background: var(--bg-elevated); }
.visual-rail i { width: 25px; height: 25px; border-radius: var(--radius); background: var(--line-strong); }
.visual-rail i.active { background: var(--accent); }
.visual-window { margin: clamp(18px, 4vh, 38px) clamp(24px, 7vw, 80px); overflow: hidden; border: 1px solid var(--line-strong); border-radius: var(--radius-md); background: var(--bg-base); box-shadow: var(--shadow-canvas); }
.visual-window-head { height: 30px; display: flex; align-items: center; gap: 5px; padding: 0 10px; border-bottom: 1px solid var(--line); background: var(--bg-elevated); }
.visual-window-head i { width: 7px; height: 7px; border-radius: var(--radius-pill); background: var(--text-faint); }
.visual-window-body { height: calc(100% - 30px); display: grid; grid-template-columns: 116px minmax(0, 1fr); }
.visual-sidebar { padding: 18px 12px; border-right: 1px solid var(--line); background: var(--bg-elevated); }
.visual-sidebar b, .visual-sidebar i, .visual-content > b, .visual-content > i { display: block; border-radius: var(--radius-sm); background: var(--line-strong); }
.visual-sidebar b { width: 58px; height: 8px; margin-bottom: 20px; background: var(--text-faint); }
.visual-sidebar i { width: 72px; height: 6px; margin-top: 10px; }
.visual-content { padding: clamp(16px, 3vw, 28px); }
.visual-content > b { width: 42%; height: 12px; background: var(--text-muted); }
.visual-content > i { width: 66%; height: 7px; margin-top: 9px; }
.visual-cards { display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px; margin-top: 24px; }
.visual-cards span { height: 70px; border: 1px solid var(--line); border-radius: var(--radius); background: var(--bg-elevated); }
.visual-chart { height: 74px; margin-top: 12px; border: 1px solid var(--line); border-radius: var(--radius); background: var(--bg-elevated); }
.visual-dock { display: flex; align-items: center; justify-content: center; gap: 7px; border-top: 1px solid var(--line); background: var(--bg-base); }
.visual-dock i { width: 18px; height: 18px; border-radius: var(--radius-sm); background: var(--line-strong); }
.visual-dock i.active { background: var(--accent); }
.source-badge {
  position: absolute;
  top: 12px;
  left: 12px;
  display: flex;
  align-items: center;
  gap: 6px;
  height: 28px;
  padding: 0 9px;
  border: 1px solid var(--line-strong);
  border-radius: var(--radius);
  background: var(--bg-overlay);
  color: var(--text);
  font-size: var(--fs-xxs);
  box-shadow: var(--shadow-pop);
}
.capture-alert { margin: 12px 16px 0; }
.capture-launchbar {
  min-height: 76px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 12px 16px;
}
.capture-launchbar :deep(.n-button) { min-width: 126px; }
.source-option {
  min-width: 138px;
  height: 48px;
  display: grid;
  grid-template-columns: 28px minmax(0, 1fr);
  align-items: center;
  gap: 8px;
  padding: 0 10px;
  border: 1px solid var(--accent-line);
  border-radius: var(--radius);
  background: var(--accent-soft);
}
.source-icon { width: 27px; height: 27px; display: grid; place-items: center; border-radius: var(--radius-sm); background: var(--bg-elev-2); color: var(--accent); }
.source-copy { min-width: 0; display: grid; gap: 2px; }
.source-copy strong { font-size: var(--fs-xs); font-weight: 600; }
.source-copy small { color: var(--text-faint); font-size: var(--fs-xxs); }

.capture-settings { min-width: 0; overflow: auto; border-left: 1px solid var(--line); background: var(--bg-elevated); }
.setting-section { display: grid; gap: 13px; padding: 16px; border-bottom: 1px solid var(--line); }
.setting-section h3 { margin: 0; color: var(--text); font-size: var(--fs-sm); font-weight: 600; }
.setting-row { min-height: 28px; display: flex; align-items: center; justify-content: space-between; gap: 12px; color: var(--text-muted); font-size: var(--fs-xs); }
.setting-value { min-width: 0; overflow: hidden; color: var(--text); text-overflow: ellipsis; white-space: nowrap; }
.section-heading { display: flex; align-items: center; justify-content: space-between; gap: 10px; }
.shortcut-key { padding: 3px 7px; border: 1px solid var(--line-strong); border-bottom-width: 2px; border-radius: var(--radius-sm); background: var(--bg-base); color: var(--text-muted); font-size: var(--fs-xxs); }
.shortcut-actions { display: flex; flex-wrap: wrap; gap: 8px; }
.shortcut-editor { display: grid; gap: 10px; }
.shortcut-recorder { min-height: 68px; display: grid; place-items: center; padding: 10px; border: 1px dashed var(--line-strong); border-radius: var(--radius); outline: none; color: var(--text-muted); cursor: text; }
.shortcut-recorder:focus { border-color: var(--accent); background: var(--accent-soft); color: var(--text); }

@media (max-width: 820px) {
  .screenshot-layout { grid-template-columns: 1fr; overflow: auto; }
  .capture-main { min-height: 460px; overflow: visible; }
  .capture-settings { overflow: visible; border-top: 1px solid var(--line); border-left: 0; }
  .visual-window { margin-right: 24px; margin-left: 24px; }
  .mode-toolbar { padding-right: 10px; padding-left: 10px; }
}
@media (max-width: 560px) {
  .mode-status .shortcut-hint { display: none; }
  .mode-button { min-width: 80px; }
  .capture-stage { min-height: 300px; margin: 10px 10px 0; }
  .capture-launchbar { padding-right: 10px; padding-left: 10px; }
  .visual-window-body { grid-template-columns: 78px minmax(0, 1fr); }
  .visual-sidebar { padding-right: 8px; padding-left: 8px; }
  .visual-sidebar i { width: 54px; }
}
</style>
