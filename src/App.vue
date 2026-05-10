<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import {
  NAlert,
  NButton,
  NCard,
  NConfigProvider,
  NEllipsis,
  NEmpty,
  NFlex,
  NForm,
  NFormItem,
  NGrid,
  NGridItem,
  NInput,
  NInputGroup,
  NInputNumber,
  NMessageProvider,
  NModal,
  NSelect,
  NSpace,
  NTag,
  NText,
  darkTheme
} from 'naive-ui';
import { darkOverrides } from './theme';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import TemplateTool from './components/ecommerce/TemplateTool.vue';
import AppShell from './components/shell/AppShell.vue';
import Dashboard from './components/shell/Dashboard.vue';
import StatPill from './components/ui/StatPill.vue';
import Panel from './components/ui/Panel.vue';
import TaskRow from './components/ui/TaskRow.vue';
import { useSidebarState } from './composables/useSidebarState';
import { useLastTool } from './composables/useLastTool';
import type { DownloadEventPayload, DownloadTask, StartDownloadRequest } from './types/download';
import type { Tool } from './types/tool';

const tools: Tool[] = [
  { id: 'aria2',     name: 'Aria2 下载',     description: 'HTTP / HTTPS / FTP / BT 多连接下载', status: 'ready' },
  { id: 'template',  name: '主图模板',       description: 'PSD 导入、字段替换、批量生成主图',   status: 'ready' },
  { id: 'image',     name: '电商图片处理',   description: '批量加 Logo、商品图处理',             status: 'ready' },
  { id: 'clipboard', name: '剪贴板工具',     description: '剪贴板历史、清洗与批量转换',         status: 'soon' },
  { id: 'text',      name: '文本工具',       description: '去重、排序、分割、大小写转换',       status: 'soon' },
  { id: 'network',   name: '网络工具',       description: 'Ping、端口检查、URL 分析',           status: 'soon' },
  { id: 'codec',     name: '编码转换',       description: 'Base64、URL Encode、Hash 摘要',      status: 'soon' }
];

const minSplitOptions = [
  { label: '1M', value: '1M' },
  { label: '4M', value: '4M' },
  { label: '8M', value: '8M' },
  { label: '16M', value: '16M' }
];


type BatchLogoResult = {
  total: number;
  succeeded: number;
  outputs: string[];
  failed: Array<{ path: string; message: string }>;
};

type LogoPreset = {
  id: number;
  name: string;
  logoPath: string;
  outputDir: string;
  logoXPercent: number;
  logoYPercent: number;
  logoWidthPercent: number;
  updatedAt: string;
};

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
const initialToolId = (() => {
  const id = lastToolId.value;
  if (!id) return null;
  const t = tools.find((x) => x.id === id);
  return t && t.status === 'ready' ? id : null;
})();
const selectedToolId = ref<string | null>(initialToolId);
const imagePaths = ref<string[]>([]);
const selectedPreviewIndex = ref(0);
const logoPath = ref('');
const imageOutputDir = ref('');
const logoXPercent = ref(68);
const logoYPercent = ref(68);
const logoWidthPercent = ref(18);
const previewFrame = ref<HTMLElement | null>(null);
const previewBaseImage = ref<HTMLImageElement | null>(null);
const draggingLogo = ref(false);
const resizingLogo = ref(false);
const pointerStart = ref({ x: 0, y: 0, logoX: 0, logoY: 0, logoWidth: 18 });
const imageProcessing = ref(false);
const imageNotice = ref('');
const imageResult = ref<BatchLogoResult | null>(null);
const logoPresets = ref<LogoPreset[]>([]);
const selectedPresetId = ref<number | null>(null);
const presetName = ref('');
const savingPreset = ref(false);
const showPresetModal = ref(false);

let unlistenProgress: Promise<UnlistenFn> | undefined;

onMounted(() => {
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

  invoke<LogoPreset[]>('list_logo_presets')
    .then((records) => {
      logoPresets.value = records;
    })
    .catch((error) => {
      imageNotice.value = String(error);
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

  window.addEventListener('keydown', handleHotkey);
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
  window.removeEventListener('keydown', handleHotkey);
});

const activeCount = computed(
  () => tasks.value.filter((task) => task.status === 'queued' || task.status === 'running').length
);

const completedCount = computed(() => tasks.value.filter((task) => task.status === 'completed').length);
const selectedTool = computed(() => tools.find((tool) => tool.id === selectedToolId.value) ?? null);

const previewImagePath = computed(() => imagePaths.value[selectedPreviewIndex.value] ?? imagePaths.value[0] ?? '');
const previewImageSrc = computed(() => (previewImagePath.value ? convertFileSrc(previewImagePath.value) : ''));
const previewLogoSrc = computed(() => (logoPath.value ? convertFileSrc(logoPath.value) : ''));
const logoStyle = computed(() => ({
  left: `${logoXPercent.value}%`,
  top: `${logoYPercent.value}%`,
  width: `${logoWidthPercent.value}%`
}));
const logoPresetOptions = computed(() =>
  logoPresets.value.map((preset) => ({
    label: preset.name,
    value: preset.id
  }))
);

function selectTool(id: string) {
  const tool = tools.find((t) => t.id === id);
  if (!tool || tool.status !== 'ready') return;
  selectedToolId.value = id;
  rememberLastTool(id);
}

function goHome() {
  selectedToolId.value = null;
}

function openSearch() {
  alert('命令面板敬请期待');
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
        createdAt: new Date().toLocaleTimeString()
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

async function chooseImages() {
  imageNotice.value = '';
  try {
    const selected = await open({
      multiple: true,
      filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp', 'bmp', 'tif', 'tiff'] }]
    });

    if (Array.isArray(selected)) {
      imagePaths.value = selected;
    } else if (typeof selected === 'string') {
      imagePaths.value = [selected];
    }
    selectedPreviewIndex.value = 0;

    if (!imageOutputDir.value && imagePaths.value.length > 0) {
      const firstPath = imagePaths.value[0];
      imageOutputDir.value = firstPath.slice(0, Math.max(firstPath.lastIndexOf('/'), firstPath.lastIndexOf('\\')));
    }
  } catch (error) {
    imageNotice.value = String(error);
  }
}

function selectPreviewImage(index: number) {
  selectedPreviewIndex.value = index;
}

function removeImage(index: number) {
  imagePaths.value = imagePaths.value.filter((_, itemIndex) => itemIndex !== index);

  if (selectedPreviewIndex.value >= imagePaths.value.length) {
    selectedPreviewIndex.value = Math.max(imagePaths.value.length - 1, 0);
  }
}

function applyLogoPreset(presetId: number | null) {
  if (presetId === null) {
    return;
  }

  const preset = logoPresets.value.find((item) => item.id === presetId);
  if (!preset) {
    return;
  }

  selectedPresetId.value = preset.id;
  presetName.value = preset.name;
  logoPath.value = preset.logoPath;
  imageOutputDir.value = preset.outputDir;
  logoXPercent.value = preset.logoXPercent;
  logoYPercent.value = preset.logoYPercent;
  logoWidthPercent.value = preset.logoWidthPercent;
  clampLogoPlacement();
}

function openPresetModal() {
  imageNotice.value = '';
  showPresetModal.value = true;
}

async function saveLogoPreset() {
  imageNotice.value = '';
  if (!presetName.value.trim() || !logoPath.value || !imageOutputDir.value) {
    imageNotice.value = '请输入方案名称，并选择 Logo 和输出目录。';
    return;
  }

  savingPreset.value = true;
  try {
    const preset = await invoke<LogoPreset>('save_logo_preset', {
      request: {
        name: presetName.value.trim(),
        logoPath: logoPath.value,
        outputDir: imageOutputDir.value,
        logoXPercent: logoXPercent.value,
        logoYPercent: logoYPercent.value,
        logoWidthPercent: logoWidthPercent.value
      }
    });
    logoPresets.value = [preset, ...logoPresets.value.filter((item) => item.id !== preset.id)];
    selectedPresetId.value = preset.id;
    showPresetModal.value = false;
  } catch (error) {
    imageNotice.value = String(error);
  } finally {
    savingPreset.value = false;
  }
}

async function chooseLogo() {
  imageNotice.value = '';
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'Logo', extensions: ['png', 'jpg', 'jpeg', 'webp', 'bmp'] }]
    });

    if (typeof selected === 'string') {
      logoPath.value = selected;
    }
  } catch (error) {
    imageNotice.value = String(error);
  }
}

async function chooseImageOutputDir() {
  imageNotice.value = '';
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      defaultPath: imageOutputDir.value || undefined
    });

    if (typeof selected === 'string') {
      imageOutputDir.value = selected;
    }
  } catch (error) {
    imageNotice.value = String(error);
  }
}


function clampLogoPlacement() {
  logoWidthPercent.value = Math.min(Math.max(logoWidthPercent.value, 1), 100);
  logoXPercent.value = Math.min(Math.max(logoXPercent.value, 0), 100 - logoWidthPercent.value);
  logoYPercent.value = Math.min(Math.max(logoYPercent.value, 0), 100);
}

function startLogoDrag(event: PointerEvent) {
  if (!previewFrame.value) {
    return;
  }

  draggingLogo.value = true;
  pointerStart.value = {
    x: event.clientX,
    y: event.clientY,
    logoX: logoXPercent.value,
    logoY: logoYPercent.value,
    logoWidth: logoWidthPercent.value
  };
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}

function startLogoResize(event: PointerEvent) {
  event.stopPropagation();
  if (!previewFrame.value) {
    return;
  }

  resizingLogo.value = true;
  pointerStart.value = {
    x: event.clientX,
    y: event.clientY,
    logoX: logoXPercent.value,
    logoY: logoYPercent.value,
    logoWidth: logoWidthPercent.value
  };
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}

function moveLogo(event: PointerEvent) {
  const frame = previewFrame.value;
  if (!frame || (!draggingLogo.value && !resizingLogo.value)) {
    return;
  }

  const rect = frame.getBoundingClientRect();
  const dxPercent = ((event.clientX - pointerStart.value.x) / rect.width) * 100;
  const dyPercent = ((event.clientY - pointerStart.value.y) / rect.height) * 100;

  if (draggingLogo.value) {
    logoXPercent.value = pointerStart.value.logoX + dxPercent;
    logoYPercent.value = pointerStart.value.logoY + dyPercent;
  }

  if (resizingLogo.value) {
    logoWidthPercent.value = pointerStart.value.logoWidth + dxPercent;
  }

  clampLogoPlacement();
}

function stopLogoInteraction(event: PointerEvent) {
  draggingLogo.value = false;
  resizingLogo.value = false;
  try {
    (event.currentTarget as HTMLElement).releasePointerCapture(event.pointerId);
  } catch {
    // Pointer capture may already be released by the browser.
  }
}

async function addLogoBatch() {
  imageNotice.value = '';
  imageResult.value = null;

  if (imagePaths.value.length === 0 || !logoPath.value || !imageOutputDir.value) {
    imageNotice.value = '请选择图片、Logo 和输出目录。';
    return;
  }

  imageProcessing.value = true;
  try {
    imageResult.value = await invoke<BatchLogoResult>('batch_add_logo', {
      request: {
        imagePaths: imagePaths.value,
        logoPath: logoPath.value,
        outputDir: imageOutputDir.value,
        position: 'bottomRight',
        margin: 0,
        logoWidthPercent: logoWidthPercent.value,
        logoXPercent: logoXPercent.value,
        logoYPercent: logoYPercent.value
      }
    });
  } catch (error) {
    imageNotice.value = String(error);
  } finally {
    imageProcessing.value = false;
  }
}
</script>

<template>
  <n-config-provider :theme="darkTheme" :theme-overrides="darkOverrides">
    <n-message-provider>
      <AppShell
        :tools="tools"
        :active-id="selectedToolId"
        :collapsed="sidebarCollapsed"
        :crumb="selectedTool?.name"
        @select="selectTool"
        @toggle="toggleSidebar"
        @brand="goHome"
        @search="openSearch"
      >
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

                  <div class="row2">
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
                  </div>

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
                <template #right><span class="mono">实时</span></template>
                <div v-if="tasks.length === 0" class="empty">还没有下载任务</div>
                <div v-else class="tasks">
                  <TaskRow
                    v-for="task in tasks"
                    :key="task.id"
                    :task="task"
                    @cancel="cancelTask"
                    @open-folder="openTaskFolder"
                  />
                </div>
              </Panel>
            </div>
          </div>
        </template>

            <template v-else-if="selectedTool.id === 'template'">
              <TemplateTool />
            </template>

            <template v-else-if="selectedTool.id === 'image'">
          <div class="page">
            <header class="page-header">
              <h2>电商图片处理</h2>
              <p>左侧选择商品图与 Logo，右侧拖拽 Logo 到任意位置，拖动右下角控制点调整大小。</p>
            </header>

            <div class="image-editor-layout">
              <Panel title="素材与参数">
                <n-space vertical :size="12">
                  <n-button type="primary" block @click="chooseImages">添加图片</n-button>

                  <div class="image-list">
                    <button
                      v-for="(path, index) in imagePaths"
                      :key="path"
                      :class="['image-list-item', { active: index === selectedPreviewIndex }]"
                      type="button"
                      @click="selectPreviewImage(index)"
                    >
                      <img class="image-list-thumb" :src="convertFileSrc(path)" alt="商品图缩略图" draggable="false" />
                      <n-ellipsis :tooltip="false">{{ path }}</n-ellipsis>
                      <span class="image-remove" @click.stop="removeImage(index)">删除</span>
                    </button>
                    <div v-if="imagePaths.length === 0" class="empty">还没有图片</div>
                  </div>

                  <n-form label-placement="top" size="small">
                    <n-form-item label="已保存方案">
                      <n-select
                        v-model:value="selectedPresetId"
                        :options="logoPresetOptions"
                        clearable
                        placeholder="选择方案快速应用"
                        @update:value="applyLogoPreset"
                      />
                    </n-form-item>
                  </n-form>

                  <n-form label-placement="top" size="small">
                    <n-form-item label="Logo 图片">
                      <n-input-group>
                        <n-input v-model:value="logoPath" readonly placeholder="选择 Logo 文件" />
                        <n-button secondary @click="chooseLogo">选择 Logo</n-button>
                      </n-input-group>
                    </n-form-item>

                    <n-form-item label="输出目录">
                      <n-input-group>
                        <n-input v-model:value="imageOutputDir" readonly placeholder="选择处理后图片保存位置" />
                        <n-button secondary @click="chooseImageOutputDir">选择文件夹</n-button>
                      </n-input-group>
                    </n-form-item>

                    <n-grid responsive="screen" cols="1 m:2" :x-gap="12">
                      <n-grid-item>
                        <n-form-item label="X 坐标（%）">
                          <n-input-number v-model:value="logoXPercent" :min="0" :max="100" style="width: 100%" @update:value="clampLogoPlacement" />
                        </n-form-item>
                      </n-grid-item>
                      <n-grid-item>
                        <n-form-item label="Y 坐标（%）">
                          <n-input-number v-model:value="logoYPercent" :min="0" :max="100" style="width: 100%" @update:value="clampLogoPlacement" />
                        </n-form-item>
                      </n-grid-item>
                    </n-grid>

                    <n-form-item label="Logo 宽度占比（%）">
                      <n-input-number v-model:value="logoWidthPercent" :min="1" :max="100" style="width: 100%" @update:value="clampLogoPlacement" />
                    </n-form-item>
                  </n-form>

                  <n-alert v-if="imageNotice" type="error" :bordered="false" class="notice-alert">
                    {{ imageNotice }}
                  </n-alert>

                  <div class="image-action-row">
                    <n-button class="image-apply-action" type="primary" :loading="imageProcessing" @click="addLogoBatch">
                      {{ imageProcessing ? '正在处理...' : '应用到全部图片' }}
                    </n-button>
                    <n-button class="image-save-action" secondary @click="openPresetModal">
                      保存当前方案
                    </n-button>
                  </div>
                </n-space>
              </Panel>

              <Panel title="预览与处理结果">
                <n-space vertical :size="12">
                  <div
                    v-if="previewImageSrc"
                    ref="previewFrame"
                    class="logo-preview-frame"
                    @pointermove="moveLogo"
                    @pointerup="stopLogoInteraction"
                    @pointercancel="stopLogoInteraction"
                  >
                    <img ref="previewBaseImage" class="preview-base-image" :src="previewImageSrc" alt="商品图预览" draggable="false" />
                    <div
                      v-if="previewLogoSrc"
                      class="preview-logo-layer"
                      :style="logoStyle"
                      @pointerdown="startLogoDrag"
                    >
                      <img :src="previewLogoSrc" alt="Logo 预览" draggable="false" />
                      <span class="logo-resize-handle" @pointerdown="startLogoResize" />
                    </div>
                  </div>
                  <div v-else class="empty">请先在左侧添加图片</div>

                  <n-alert v-if="previewImageSrc && !previewLogoSrc" type="info" :bordered="false">
                    请选择 Logo 图片后，可在预览图中拖拽位置和缩放大小。
                  </n-alert>

                  <n-alert v-if="imageResult" type="success" :bordered="false">
                    共 {{ imageResult.total }} 张，成功 {{ imageResult.succeeded }} 张，失败 {{ imageResult.failed.length }} 张。
                  </n-alert>

                  <Panel v-if="imageResult?.outputs.length" title="输出文件" flush>
                    <div class="result-list">
                      <n-ellipsis v-for="output in imageResult.outputs" :key="output" :tooltip="false">
                        {{ output }}
                      </n-ellipsis>
                    </div>
                  </Panel>

                  <Panel v-if="imageResult?.failed.length" title="失败记录" flush>
                    <div class="result-list">
                      <span v-for="item in imageResult.failed" :key="item.path" class="error-line">
                        {{ item.path }}：{{ item.message }}
                      </span>
                    </div>
                  </Panel>
                </n-space>
              </Panel>
            </div>
          </div>
        </template>
      </AppShell>

      <n-modal v-model:show="showPresetModal" preset="card" title="保存当前方案" class="preset-modal">
          <n-space vertical :size="14">
            <n-form label-placement="top" size="small">
              <n-form-item label="方案名称">
                <n-input v-model:value="presetName" placeholder="例如：右下角店铺 Logo" />
              </n-form-item>
            </n-form>

            <n-alert type="info" :bordered="false">
              将保存当前 Logo 图片、坐标、宽度占比和输出文件夹。同名方案会被覆盖。
            </n-alert>

            <n-flex justify="end" :size="8">
              <n-button @click="showPresetModal = false">取消</n-button>
              <n-button type="primary" :loading="savingPreset" @click="saveLogoPreset">确认保存</n-button>
            </n-flex>
          </n-space>
        </n-modal>
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
.row2 { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
.row3 { display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px; }

.notice-alert { margin-bottom: 4px; }

.tasks { display: grid; gap: 6px; }
.empty {
  padding: 60px 20px;
  text-align: center;
  color: var(--text-muted);
  border: 1px dashed var(--line-strong);
  border-radius: var(--radius-md);
  font-size: var(--fs-sm);
}

.image-editor-layout {
  display: grid;
  grid-template-columns: 320px minmax(0, 1fr);
  gap: 16px;
  align-items: start;
}
@media (max-width: 920px) {
  .image-editor-layout { grid-template-columns: 1fr; }
}

.image-list {
  display: grid;
  gap: 6px;
  max-height: 260px;
  overflow: auto;
  border: 1px dashed var(--line-strong);
  border-radius: var(--radius-md);
  background: var(--bg-elev-2);
  padding: 8px;
}

.image-list-item {
  display: grid;
  grid-template-columns: 36px minmax(0, 1fr) auto;
  gap: 8px;
  align-items: center;
  width: 100%;
  border: 1px solid transparent;
  border-radius: var(--radius);
  background: var(--bg-elevated);
  color: var(--text);
  padding: 7px 9px;
  text-align: left;
  cursor: pointer;
}
.image-list-item:hover { border-color: var(--line-strong); }
.image-list-item.active {
  border-color: color-mix(in srgb, var(--accent) 50%, var(--line-strong));
  background: var(--accent-soft);
  color: var(--accent);
}
.image-list-thumb {
  width: 36px; height: 36px;
  border-radius: var(--radius-sm);
  object-fit: cover;
  background: var(--bg-base);
}
.image-remove {
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, var(--error) 18%, transparent);
  color: var(--error);
  padding: 3px 8px;
  font-size: var(--fs-xxs);
  font-weight: 600;
}

.logo-preview-frame {
  position: relative;
  overflow: hidden;
  width: fit-content;
  max-width: 100%;
  margin: 0 auto;
  border: 1px solid var(--line-strong);
  border-radius: var(--radius-md);
  background: #0f0f12;
  touch-action: none;
  user-select: none;
}
.preview-base-image {
  display: block;
  max-width: 100%;
  max-height: 68vh;
  object-fit: contain;
  user-select: none;
}
.preview-logo-layer {
  position: absolute;
  cursor: move;
  touch-action: none;
}
.preview-logo-layer img {
  display: block;
  width: 100%;
  height: auto;
  user-select: none;
  pointer-events: none;
}
.logo-resize-handle {
  position: absolute;
  right: -6px;
  bottom: -6px;
  width: 12px;
  height: 12px;
  border: 1.5px solid #0a0a0b;
  border-radius: 2px;
  background: var(--accent);
  cursor: nwse-resize;
}

.image-action-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 128px;
  gap: 10px;
}
.image-action-row :deep(.n-button) { width: 100%; }
@media (max-width: 420px) {
  .image-action-row { grid-template-columns: 1fr; }
}

.result-list {
  display: grid;
  gap: 4px;
  padding: 12px 14px;
  font-size: var(--fs-xs);
  color: var(--text-muted);
  font-family: var(--font-mono);
}
.error-line { color: var(--error); }
</style>
