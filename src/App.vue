<script setup lang="ts">
import { computed, defineAsyncComponent, onMounted, onUnmounted, ref } from 'vue';
import {
  NAlert,
  NButton,
  NConfigProvider,
  NInput,
  NInputGroup,
  NInputNumber,
  NMessageProvider,
  NSelect,
  darkTheme
} from 'naive-ui';
import { darkOverrides, lightOverrides } from './theme';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import TemplateTool from './components/ecommerce/TemplateTool.vue';
import ClipboardHistoryWindow from './components/clipboard/ClipboardHistoryWindow.vue';
import ClipboardTool from './components/clipboard/ClipboardTool.vue';
import AppShell from './components/shell/AppShell.vue';
import ShortcutErrorNotifier from './components/shell/ShortcutErrorNotifier.vue';
import Dashboard from './components/shell/Dashboard.vue';
import StatPill from './components/ui/StatPill.vue';
import Panel from './components/ui/Panel.vue';
import TaskRow from './components/ui/TaskRow.vue';
import { useSidebarState } from './composables/useSidebarState';
import { useLastTool } from './composables/useLastTool';
import { useTheme } from './composables/useTheme';
import { useAria2Handoff } from './composables/useAria2Handoff';
import UpdateBanner from './components/shell/UpdateBanner.vue';
import SettingsModal from './components/shell/SettingsModal.vue';
import { useUpdater } from './composables/useUpdater';
import { useUpdaterPrefs } from './composables/useUpdaterPrefs';
import type { DownloadEventPayload, DownloadTask, StartDownloadRequest } from './types/download';
import type { Tool } from './types/tool';

const JsonTool = defineAsyncComponent(() => import('./components/json/JsonTool.vue'));
const DouyinTool = defineAsyncComponent(() => import('./components/douyin/DouyinTool.vue'));
const ImageTool = defineAsyncComponent(() => import('./components/image/ImageTool.vue'));
const CodecTool = defineAsyncComponent(() => import('./components/codec/CodecTool.vue'));
const NetworkTool = defineAsyncComponent(() => import('./components/network/NetworkTool.vue'));
const TextTool = defineAsyncComponent(() => import('./components/text/TextTool.vue'));
const GeneratorTool = defineAsyncComponent(() => import('./components/generator/GeneratorTool.vue'));
const TimeTool = defineAsyncComponent(() => import('./components/time/TimeTool.vue'));
const HttpTool = defineAsyncComponent(() => import('./components/http/HttpTool.vue'));

const tools: Tool[] = [
  { id: 'aria2',     name: 'Aria2 下载',     description: 'HTTP / HTTPS / FTP / BT 多连接下载', status: 'ready', icon: 'download' },
  { id: 'template',  name: '主图模板',       description: 'PSD 导入、字段替换、批量生成主图',   status: 'ready', icon: 'layout' },
  { id: 'clipboard', name: '剪贴板工具',     description: 'Paste 风格剪贴板历史与快捷恢复',     status: 'ready', icon: 'clipboard' },
  { id: 'json',      name: 'JSON 工具',       description: '格式化 / 查询 / 对比 / 转换',          status: 'ready', icon: 'code' },
  { id: 'video-link', name: '视频链接抽取',  description: '抖音 / 小红书 / B站 / YouTube（后三者建设中）', status: 'ready', icon: 'video' },
  { id: 'image',     name: '图片工具',       description: '压缩 / 格式转 / EXIF / 标注 / OCR',    status: 'ready', icon: 'image' },
  { id: 'text',      name: '文本工具',       description: '整理 / 排序 / 大小写 / 拆合 / 抽取 / 对比', status: 'ready', icon: 'type' },
  { id: 'network',   name: '网络工具',       description: 'URL 分析 / Ping / 端口检查 / DNS',    status: 'ready', icon: 'wifi' },
  { id: 'codec',     name: '编码转换',       description: 'Base64 / URL / Unicode / Hex / Hash / JWT', status: 'ready', icon: 'hash' },
  { id: 'generator', name: '生成器',         description: '密码 / UUID / QR / Lorem / 假数据 / 骰子',  status: 'ready', icon: 'dice' },
  { id: 'time',      name: '时间工具',       description: '时间戳 / 时区 / Cron / Duration',      status: 'ready', icon: 'clock' },
  { id: 'http',      name: 'HTTP 请求',      description: 'GET/POST/... + headers / body / 响应',   status: 'ready', icon: 'send' }
];

const currentWindow = getCurrentWindow();
const isClipboardHistoryWindow = currentWindow.label === 'clipboard-history';
const isCaptureOverlayWindow = currentWindow.label === 'capture-overlay';
const isCapturePinWindow = currentWindow.label.startsWith('capture-pin-');
const CaptureOverlay = defineAsyncComponent(() => import('./components/image/CaptureOverlay.vue'));
const CapturePinWindow = defineAsyncComponent(() => import('./components/image/CapturePinWindow.vue'));

const minSplitOptions = [
  { label: '1M', value: '1M' },
  { label: '4M', value: '4M' },
  { label: '8M', value: '8M' },
  { label: '16M', value: '16M' }
];

const url = ref('');
const downloadDir = ref('');
const fileName = ref('');
const connections = ref(16);
const split = ref(16);
const minSplitSize = ref('1M');
const tasks = ref<DownloadTask[]>([]);
const submitting = ref(false);
const choosingDir = ref(false);
const notice = ref('');
const { collapsed: sidebarCollapsed, toggle: toggleSidebar } = useSidebarState();
const { lastToolId, remember: rememberLastTool } = useLastTool();
const { theme, toggle: toggleTheme } = useTheme();
const aria2Handoff = useAria2Handoff();
const { state: updaterState, check: updaterCheck, install: updaterInstall, relaunch: updaterRelaunch, dismiss: updaterDismiss } = useUpdater();
const { autoCheck: updaterAutoCheck, setAutoCheck: updaterSetAutoCheck, skipVersion: updaterSkipVersion, shouldSkip: updaterShouldSkip } = useUpdaterPrefs();
const settingsOpen = ref(false);
const naiveTheme = computed(() => (theme.value === 'dark' ? darkTheme : null));
const naiveOverrides = computed(() => (theme.value === 'dark' ? darkOverrides : lightOverrides));
const initialToolId = (() => {
  const id = lastToolId.value;
  if (!id) return null;
  const t = tools.find((x) => x.id === id);
  return t && t.status === 'ready' ? id : null;
})();
const selectedToolId = ref<string | null>(initialToolId);

let unlistenProgress: Promise<UnlistenFn> | undefined;
let unlistenCaptureFail: Promise<UnlistenFn> | undefined;

onMounted(() => {
  if (isClipboardHistoryWindow || isCaptureOverlayWindow || isCapturePinWindow) return;

  invoke<string>('get_default_download_dir')
    .then((dir) => {
      downloadDir.value = dir;
    })
    .catch(() => {
      downloadDir.value = '';
    });

  invoke<DownloadTask[]>('list_download_tasks')
    .then((records) => {
      tasks.value = records;
    })
    .catch((error) => {
      notice.value = String(error);
    });

  unlistenProgress = listen<DownloadEventPayload>('download-progress', (event) => {
    const payload = event.payload;
    tasks.value = tasks.value.map((task) =>
      task.id === payload.id
        ? {
            ...task,
            ...payload,
            progress:
              payload.progress > 0 || payload.status === 'completed' ? payload.progress : task.progress
          }
        : task
    );
  });

  unlistenCaptureFail = listen<string>('capture-failed', (event) => {
    // 用户按 Esc 取消不算错误，就直接静默；其它错误在控制台留个痕迹
    const msg = event.payload;
    if (!msg || msg.includes('取消')) return;
    console.warn('[capture]', msg);
  });

  window.addEventListener('keydown', handleHotkey);

  if (updaterAutoCheck.value) {
    setTimeout(() => runUpdateCheck('auto'), 5000);
  }
});

function handleHotkey(event: KeyboardEvent) {
  const meta = event.metaKey || event.ctrlKey;
  if (!meta) return;

  if (event.key === '\\') {
    event.preventDefault();
    toggleSidebar();
  } else if (event.key === 'k' || event.key === 'K') {
    event.preventDefault();
    openSearch();
  }
}

onUnmounted(() => {
  unlistenProgress?.then((dispose) => dispose()).catch(() => undefined);
  unlistenCaptureFail?.then((dispose) => dispose()).catch(() => undefined);
  window.removeEventListener('keydown', handleHotkey);
});

const activeCount = computed(
  () => tasks.value.filter((task) => task.status === 'queued' || task.status === 'running').length
);
const completedCount = computed(() => tasks.value.filter((task) => task.status === 'completed').length);
const selectedTool = computed(() => tools.find((tool) => tool.id === selectedToolId.value) ?? null);

const taskSearch = ref('');
const filteredTasks = computed(() => {
  const q = taskSearch.value.trim().toLocaleLowerCase();
  if (!q) return tasks.value;
  return tasks.value.filter((t) => {
    const haystack = [t.url, t.fileName ?? '', t.message ?? '', t.localPath ?? '']
      .join('\n')
      .toLocaleLowerCase();
    return haystack.includes(q);
  });
});
const hasCompleted = computed(() => tasks.value.some((t) => t.status === 'completed'));

function selectTool(id: string) {
  const tool = tools.find((t) => t.id === id);
  if (!tool || tool.status !== 'ready') return;
  selectedToolId.value = id;
  rememberLastTool(id);
  if (id === 'aria2') aria2Handoff.drainInto(url);
}

function handleDouyinNavigate(target: string) {
  selectTool(target);
}

function goHome() {
  selectedToolId.value = null;
}

function openSearch() {
  alert('命令面板敬请期待');
}

function handleSkip() {
  if (updaterState.value.available) {
    updaterSkipVersion(updaterState.value.available.version);
  }
  updaterDismiss();
}
function openSettings() {
  settingsOpen.value = true;
}
async function runUpdateCheck(trigger: 'auto' | 'manual') {
  await updaterCheck(trigger);
  if (updaterState.value.status === 'available' &&
      updaterShouldSkip(updaterState.value.available?.version ?? '')) {
    updaterDismiss();
  }
}
function handleSettingsCheck() {
  runUpdateCheck('manual');
}

async function chooseDownloadDir() {
  notice.value = '';
  choosingDir.value = true;
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      defaultPath: downloadDir.value || undefined
    });

    if (typeof selected === 'string') {
      downloadDir.value = selected;
    }
  } catch (error) {
    notice.value = String(error);
  } finally {
    choosingDir.value = false;
  }
}

function parseDownloadUrls(value: string) {
  return value
    .split(/[\n,]+/)
    .map((item) => item.trim())
    .filter(Boolean);
}

async function startDownload() {
  notice.value = '';

  const urls = parseDownloadUrls(url.value);
  const downloadPath = downloadDir.value.trim();
  const sharedRequest = {
    downloadDir: downloadPath,
    connections: connections.value,
    split: split.value,
    minSplitSize: minSplitSize.value
  };

  if (urls.length === 0 || !downloadPath) {
    notice.value = '请填写下载链接和保存目录。';
    return;
  }

  submitting.value = true;
  try {
    const createdTasks: DownloadTask[] = [];
    const singleFileName = urls.length === 1 ? fileName.value.trim() || undefined : undefined;

    for (const itemUrl of urls) {
      const request: StartDownloadRequest = {
        ...sharedRequest,
        url: itemUrl,
        fileName: singleFileName
      };
      const response = await invoke<{ id: string }>('start_download', { request });
      createdTasks.push({
        id: response.id,
        url: request.url,
        downloadDir: request.downloadDir,
        fileName: request.fileName,
        status: 'queued',
        progress: 0,
        speed: null,
        eta: null,
        message: '任务已提交给 aria2',
        createdAt: new Date().toLocaleTimeString(),
        startedAt: null,
        finishedAt: null,
        localPath: null
      });
    }

    tasks.value = [...createdTasks.reverse(), ...tasks.value];
    url.value = '';
    fileName.value = '';
  } catch (error) {
    notice.value = String(error);
  } finally {
    submitting.value = false;
  }
}

async function cancelTask(id: string) {
  notice.value = '';
  try {
    await invoke('cancel_download', { id });
  } catch (error) {
    notice.value = String(error);
  }
}

async function openTaskFolder(id: string) {
  notice.value = '';
  try {
    await invoke('open_download_folder', { id });
  } catch (error) {
    notice.value = String(error);
  }
}

async function retryTask(task: DownloadTask) {
  notice.value = '';
  submitting.value = true;
  try {
    const request: StartDownloadRequest = {
      url: task.url,
      downloadDir: task.downloadDir,
      fileName: task.fileName ?? undefined,
      connections: connections.value,
      split: split.value,
      minSplitSize: minSplitSize.value
    };
    const response = await invoke<{ id: string }>('start_download', { request });
    tasks.value = [
      {
        id: response.id,
        url: request.url,
        downloadDir: request.downloadDir,
        fileName: request.fileName,
        status: 'queued',
        progress: 0,
        speed: null,
        eta: null,
        message: '任务已重新提交给 aria2',
        createdAt: new Date().toLocaleTimeString(),
        startedAt: null,
        finishedAt: null,
        localPath: null
      },
      ...tasks.value
    ];
  } catch (error) {
    notice.value = String(error);
  } finally {
    submitting.value = false;
  }
}

async function deleteTask(id: string) {
  notice.value = '';
  try {
    await invoke('delete_download_task', { id });
    tasks.value = tasks.value.filter((t) => t.id !== id);
  } catch (error) {
    notice.value = String(error);
  }
}

async function clearCompleted() {
  notice.value = '';
  try {
    const count = await invoke<number>('clear_completed_downloads');
    if (count > 0) {
      tasks.value = tasks.value.filter((t) => t.status !== 'completed');
    }
  } catch (error) {
    notice.value = String(error);
  }
}
</script>

<template>
  <n-config-provider :theme="naiveTheme" :theme-overrides="naiveOverrides">
    <n-message-provider>
      <ShortcutErrorNotifier
        v-if="!isCaptureOverlayWindow && !isCapturePinWindow && !isClipboardHistoryWindow"
      />
      <CaptureOverlay v-if="isCaptureOverlayWindow" />
      <CapturePinWindow v-else-if="isCapturePinWindow" />
      <ClipboardHistoryWindow v-else-if="isClipboardHistoryWindow" />
      <AppShell
        v-else-if="!isClipboardHistoryWindow && !isCapturePinWindow"
        :tools="tools"
        :active-id="selectedToolId"
        :collapsed="sidebarCollapsed"
        :crumb="selectedTool?.name"
        :theme="theme"
        @select="selectTool"
        @toggle="toggleSidebar"
        @brand="goHome"
        @search="openSearch"
        @theme-toggle="toggleTheme"
        @settings-toggle="openSettings"
      >
        <template #banner>
          <UpdateBanner
            :state="updaterState"
            @install="updaterInstall"
            @skip="handleSkip"
            @relaunch="updaterRelaunch"
            @dismiss="updaterDismiss"
          />
        </template>
        <template #topbar-right>
          <template v-if="selectedTool?.id === 'aria2'">
            <StatPill tone="accent">进行中 {{ activeCount }}</StatPill>
            <StatPill>已完成 {{ completedCount }}</StatPill>
          </template>
        </template>

        <Dashboard
          v-if="!selectedTool"
          :tools="tools"
          :last-tool-id="lastToolId"
          @open="selectTool"
        />

        <template v-else-if="selectedTool.id === 'aria2'">
          <div class="page">
            <header class="page-header">
              <h2>多线程下载工作台</h2>
              <p>本机 aria2c 引擎，支持断点续传、分片、多连接和实时进度回传。</p>
            </header>

            <div class="aria2-grid">
              <Panel title="新建下载">
                <template #right><span>支持批量</span></template>
                <form @submit.prevent="startDownload" class="form">
                  <label class="field">
                    <span class="lbl">资源链接（每行一个，或用逗号分隔）</span>
                    <n-input
                      v-model:value="url"
                      type="textarea"
                      placeholder="https://example.com/file-a.zip&#10;https://example.com/file-b.zip"
                      :autosize="{ minRows: 5, maxRows: 10 }"
                    />
                  </label>

                  <label class="field">
                    <span class="lbl">保存目录</span>
                    <n-input-group>
                      <n-input v-model:value="downloadDir" placeholder="/Users/you/Downloads" />
                      <n-button secondary :loading="choosingDir" @click="chooseDownloadDir">选择文件夹</n-button>
                    </n-input-group>
                  </label>

                  <label class="field">
                    <span class="lbl">文件名（仅单个链接时生效）</span>
                    <n-input v-model:value="fileName" placeholder="archive.zip" />
                  </label>

                  <div class="row3">
                    <label class="field">
                      <span class="lbl">单服务器连接数</span>
                      <n-input-number v-model:value="connections" :min="1" :max="16" style="width: 100%" />
                    </label>
                    <label class="field">
                      <span class="lbl">分片数</span>
                      <n-input-number v-model:value="split" :min="1" :max="64" style="width: 100%" />
                    </label>
                    <label class="field">
                      <span class="lbl">最小分片大小</span>
                      <n-select v-model:value="minSplitSize" :options="minSplitOptions" />
                    </label>
                  </div>

                  <n-alert v-if="notice" type="error" :bordered="false" class="notice-alert">
                    {{ notice }}
                  </n-alert>

                  <n-button type="primary" block attr-type="submit" :loading="submitting">
                    {{ submitting ? '正在创建...' : '开始下载' }}
                  </n-button>
                </form>
              </Panel>

              <Panel title="任务队列">
                <template #right>
                  <div class="queue-header-right">
                    <n-button
                      v-if="hasCompleted"
                      size="tiny"
                      secondary
                      @click="clearCompleted"
                    >
                      清理已完成
                    </n-button>
                    <span class="mono">实时</span>
                  </div>
                </template>

                <div v-if="tasks.length > 0" class="queue-toolbar">
                  <n-input
                    v-model:value="taskSearch"
                    placeholder="搜索 URL / 文件名 / 路径 / 备注"
                    clearable
                    size="small"
                  />
                </div>

                <div v-if="tasks.length === 0" class="empty">还没有下载任务</div>
                <div v-else-if="filteredTasks.length === 0" class="empty">没有匹配的任务</div>
                <div v-else class="tasks">
                  <TaskRow
                    v-for="task in filteredTasks"
                    :key="task.id"
                    :task="task"
                    @cancel="cancelTask"
                    @open-folder="openTaskFolder"
                    @retry="retryTask"
                    @delete="deleteTask"
                  />
                </div>
              </Panel>
            </div>
          </div>
        </template>

        <template v-else-if="selectedTool.id === 'template'">
          <TemplateTool />
        </template>

        <template v-else-if="selectedTool.id === 'clipboard'">
          <ClipboardTool />
        </template>

        <template v-else-if="selectedTool.id === 'json'">
          <JsonTool />
        </template>

        <template v-else-if="selectedTool.id === 'video-link'">
          <DouyinTool @request-navigate="handleDouyinNavigate" />
        </template>

        <template v-else-if="selectedTool.id === 'image'">
          <ImageTool />
        </template>

        <template v-else-if="selectedTool.id === 'codec'">
          <CodecTool />
        </template>

        <template v-else-if="selectedTool.id === 'network'">
          <NetworkTool />
        </template>

        <template v-else-if="selectedTool.id === 'text'">
          <TextTool />
        </template>

        <template v-else-if="selectedTool.id === 'generator'">
          <GeneratorTool />
        </template>

        <template v-else-if="selectedTool.id === 'time'">
          <TimeTool />
        </template>

        <template v-else-if="selectedTool.id === 'http'">
          <HttpTool />
        </template>
      </AppShell>

      <SettingsModal
        v-model:show="settingsOpen"
        :state="updaterState"
        :auto-check="updaterAutoCheck"
        @check="handleSettingsCheck"
        @install="updaterInstall"
        @relaunch="updaterRelaunch"
        @update:auto-check="updaterSetAutoCheck"
      />
    </n-message-provider>
  </n-config-provider>
</template>

<style scoped>
.page { display: grid; gap: 16px; }
.page-header { display: grid; gap: 4px; }
.page-header h2 {
  margin: 0;
  font-size: var(--fs-xl);
  font-weight: 600;
  letter-spacing: -0.012em;
}
.page-header p {
  margin: 0;
  color: var(--text-muted);
  font-size: var(--fs-xs);
}

.aria2-grid {
  display: grid;
  grid-template-columns: 1.1fr 1fr;
  gap: 16px;
}
@media (max-width: 1100px) {
  .aria2-grid { grid-template-columns: 1fr; }
}

.form { display: grid; gap: 12px; }
.field { display: grid; gap: 5px; }
.field .lbl {
  font-size: var(--fs-xxs);
  color: var(--text-muted);
}
.row3 { display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px; }

.notice-alert { margin-bottom: 4px; }

.queue-header-right { display: flex; align-items: center; gap: 10px; }
.queue-toolbar { margin-bottom: 8px; }
.tasks { display: grid; gap: 6px; }
.empty {
  padding: 60px 20px;
  text-align: center;
  color: var(--text-muted);
  border: 1px dashed var(--line-strong);
  border-radius: var(--radius-md);
  font-size: var(--fs-sm);
}
</style>
