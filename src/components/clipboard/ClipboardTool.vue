<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from 'vue';
import { NAlert, NButton, NInput, NInputNumber, NPopconfirm, NSelect, NSwitch, useMessage } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import Panel from '../ui/Panel.vue';
import ClipboardItemCard from './ClipboardItemCard.vue';
import { useClipboardHistory } from '../../composables/useClipboardHistory';
import { DEFAULT_CLIPBOARD_SHORTCUT, formatShortcutLabel } from '../../utils/clipboardHistory';
import { formatShortcutForDisplay, keyEventToShortcut } from '../image/shortcut';
import type { ClipboardHistoryItem, ClipboardHistorySettings } from '../../types/clipboard';

const history = useClipboardHistory();
const message = useMessage();
const registerError = ref('');
const settings = ref<ClipboardHistorySettings>({
  captureEnabled: true,
  retentionLimit: 500,
  shortcut: DEFAULT_CLIPBOARD_SHORTCUT,
});

// 快捷键展示 / 录制
const shortcutLabel = computed(() => formatShortcutLabel(settings.value.shortcut));
const editing = ref(false);
const recordingRaw = ref('');
const recordError = ref('');
const recorderEl = ref<HTMLDivElement | null>(null);
const recordingLabel = computed(() =>
  recordingRaw.value ? formatShortcutForDisplay(recordingRaw.value) : ''
);
const isChanged = computed(
  () => !!recordingRaw.value && recordingRaw.value !== settings.value.shortcut
);
const isDefault = computed(() => settings.value.shortcut === DEFAULT_CLIPBOARD_SHORTCUT);

async function restore(item: ClipboardHistoryItem) {
  try {
    await history.restoreItem(item.id);
    message.success('已复制到剪贴板');
  } catch (error) {
    message.error(`复制失败：${error}`);
  }
}

async function loadSettings() {
  settings.value = await invoke<ClipboardHistorySettings>('get_clipboard_settings');
}

async function loadRegisterError() {
  try {
    const errors = await invoke<{ clipboard: string | null }>('get_shortcut_register_errors');
    registerError.value = errors.clipboard ?? '';
  } catch {
    registerError.value = '';
  }
}

async function saveSettings() {
  settings.value = await invoke<ClipboardHistorySettings>('save_clipboard_settings', { settings: settings.value });
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
    recordError.value = '需要至少一个修饰键（⌘ / Ctrl / Shift / Alt）+ 一个字母/数字/功能键';
    return;
  }
  recordingRaw.value = combo;
  recordError.value = '';
}

// 用给定快捷键保存并触发后端重新注册；成功则清除错误状态
async function saveShortcut(next: string) {
  const previous = settings.value.shortcut;
  settings.value = {
    ...settings.value,
    shortcut: next,
  };
  try {
    settings.value = await invoke<ClipboardHistorySettings>('save_clipboard_settings', {
      settings: settings.value,
    });
    registerError.value = ''; // 保存成功即代表新快捷键注册成功
    editing.value = false;
    recordingRaw.value = '';
    message.success(`已切换到 ${formatShortcutForDisplay(next)}`);
  } catch (err) {
    settings.value = { ...settings.value, shortcut: previous }; // 回滚本地状态
    recordError.value = String(err);
    throw err;
  }
}

async function applyShortcut() {
  if (!recordingRaw.value) return;
  try {
    await saveShortcut(recordingRaw.value);
  } catch {
    // 错误已在 saveShortcut 里写入 recordError
  }
}

async function restoreDefault() {
  try {
    await saveShortcut(DEFAULT_CLIPBOARD_SHORTCUT);
  } catch (err) {
    message.error(String(err));
  }
}

onMounted(() => {
  history.refresh();
  loadSettings();
  loadRegisterError();
});
</script>

<template>
  <div class="tool-page">
    <Panel title="剪贴板历史">
      <p class="clipboard-muted clipboard-intro">
        AT Tool 运行时记录文本、图片和文件路径；按 {{ shortcutLabel }} 打开快捷面板。
      </p>
      <n-alert
        v-if="registerError"
        type="error"
        :bordered="false"
        style="margin-bottom: 12px"
      >
        {{ registerError }}（该快捷键已被占用，常见占用者：系统/桌面环境、输入法、远程桌面等，请点“修改”换一个）
      </n-alert>

      <div class="clipboard-shortcut">
        <div v-if="!editing" class="clipboard-shortcut__view">
          <span class="clipboard-muted">唤起快捷键</span>
          <span class="clipboard-shortcut__key mono">{{ shortcutLabel || '未设置' }}</span>
          <n-button size="tiny" secondary @click="startEdit">修改</n-button>
          <n-button v-if="!isDefault" size="tiny" secondary @click="restoreDefault">恢复默认</n-button>
        </div>
        <div v-else class="clipboard-shortcut__edit">
          <div
            ref="recorderEl"
            class="clipboard-shortcut__recorder"
            tabindex="0"
            @keydown="onRecordKeydown"
          >
            <span v-if="recordingRaw" class="mono">{{ recordingLabel }}</span>
            <span v-else class="clipboard-muted">按下你想设置的组合（例如 Ctrl+Alt+V）...</span>
          </div>
          <n-button size="tiny" secondary @click="cancelEdit">取消</n-button>
          <n-button size="tiny" type="primary" :disabled="!isChanged" @click="applyShortcut">保存</n-button>
        </div>
        <n-alert v-if="recordError" type="warning" :bordered="false" style="margin-top: 8px">
          {{ recordError }}
        </n-alert>
      </div>

      <div class="clipboard-toolbar">
        <n-switch v-model:value="settings.captureEnabled" @update:value="saveSettings" />
        <span class="clipboard-muted">{{ settings.captureEnabled ? '正在记录剪贴板' : '已暂停记录剪贴板' }}</span>
        <n-input-number
          v-model:value="settings.retentionLimit"
          class="clipboard-limit-input"
          :min="50"
          :max="5000"
          @blur="saveSettings"
        />
        <span class="clipboard-muted">最多保留条数</span>
      </div>
      <div class="clipboard-toolbar">
        <n-input v-model:value="history.query.value" placeholder="搜索内容、文件名或路径" clearable />
        <n-select
          v-model:value="history.kind.value"
          class="clipboard-kind-select"
          :options="[
            { label: '全部', value: 'all' },
            { label: '文本', value: 'text' },
            { label: '图片', value: 'image' },
            { label: '文件', value: 'files' },
          ]"
        />
        <n-button secondary :loading="history.loading.value" @click="history.refresh">刷新</n-button>
        <n-popconfirm @positive-click="history.clearHistory">
          <template #trigger><n-button secondary type="error">清空未收藏</n-button></template>
          确认清空所有未收藏的剪贴板历史？
        </n-popconfirm>
      </div>
      <p v-if="history.error.value" class="clipboard-muted">{{ history.error.value }}</p>
      <div class="clipboard-grid">
        <ClipboardItemCard
          v-for="item in history.filteredItems.value"
          :key="item.id"
          :item="item"
          @restore="restore"
          @delete="history.deleteItem"
          @pin="history.setPinned"
        />
      </div>
    </Panel>
  </div>
</template>

<style scoped>
.clipboard-intro { margin: 0 0 14px; }
.clipboard-kind-select { width: 140px; }
.clipboard-limit-input { width: 140px; }

.clipboard-shortcut { margin-bottom: 14px; }
.clipboard-shortcut__view,
.clipboard-shortcut__edit {
  display: flex;
  align-items: center;
  gap: 10px;
}
.clipboard-shortcut__key {
  padding: 2px 8px;
  border-radius: var(--radius-md);
  background: var(--bg-elevated, rgba(255, 255, 255, 0.06));
}
.clipboard-shortcut__recorder {
  min-width: 220px;
  padding: 4px 10px;
  border: 1px dashed var(--border, rgba(255, 255, 255, 0.2));
  border-radius: var(--radius-md);
  cursor: text;
  outline: none;
}
.clipboard-shortcut__recorder:focus {
  border-color: var(--accent, #63e2b7);
}
</style>
