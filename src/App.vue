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
  NPageHeader,
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
import TaskCard from './components/TaskCard.vue';
import TemplateTool from './components/ecommerce/TemplateTool.vue';
import type { DownloadEventPayload, DownloadTask, StartDownloadRequest } from './types/download';

type ToolEntry = {
  id: string;
  name: string;
  description: string;
  badge: string;
  active: boolean;
};

const tools: ToolEntry[] = [
  {
    id: 'aria2',
    name: 'Aria2 下载',
    description: 'HTTP / HTTPS / FTP / BitTorrent 多连接下载',
    badge: 'Ready',
    active: true
  },
  {
    id: 'clipboard',
    name: '剪贴板工具',
    description: '剪贴板历史、清洗与批量转换',
    badge: 'Soon',
    active: false
  },
  {
    id: 'template',
    name: '主图模板',
    description: 'PSD 导入、字段替换、批量生成主图',
    badge: 'New',
    active: true
  },
  {
    id: 'image',
    name: '电商图片处理',
    description: '批量加 Logo、商品图处理',
    badge: 'Ready',
    active: true
  },
  {
    id: 'text',
    name: '文本工具',
    description: '去重、排序、分割、大小写转换',
    badge: 'Soon',
    active: false
  },
  {
    id: 'network',
    name: '网络工具',
    description: 'Ping、端口检查、URL 分析',
    badge: 'Soon',
    active: false
  },
  {
    id: 'codec',
    name: '编码转换',
    description: 'Base64、URL Encode、Hash 摘要',
    badge: 'Soon',
    active: false
  }
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
const selectedToolId = ref<string | null>(null);
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
});

onUnmounted(() => {
  unlistenProgress?.then((dispose) => dispose()).catch(() => undefined);
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

function openTool(tool: ToolEntry) {
  if (!tool.active) {
    return;
  }

  selectedToolId.value = tool.id;
}

function goHome() {
  selectedToolId.value = null;
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
      <main class="app-shell">
        <n-card v-if="!selectedTool" class="surface-card home-surface" :bordered="false">
          <n-flex class="home-brand" align="center" :gap="12">
            <div class="brand-mark">AT</div>
            <div>
              <div class="brand-name">AT Tool</div>
              <n-text depth="3">Personal Utility Deck</n-text>
            </div>
          </n-flex>

          <n-grid class="tool-grid" responsive="screen" cols="1 s:2 m:3 l:4" :x-gap="12" :y-gap="12">
            <n-grid-item v-for="tool in tools" :key="tool.id">
              <n-card
                class="tool-card"
                :class="{ disabled: !tool.active }"
                size="small"
                hoverable
                :bordered="false"
                role="button"
                @click="openTool(tool)"
              >
                <template #header>
                  <n-ellipsis :tooltip="false">{{ tool.name }}</n-ellipsis>
                </template>
                <template #header-extra>
                  <n-tag size="small" round :type="tool.active ? 'success' : 'default'">
                    {{ tool.badge }}
                  </n-tag>
                </template>
                <n-ellipsis :line-clamp="2" :tooltip="false">
                  <n-text depth="3">{{ tool.description }}</n-text>
                </n-ellipsis>
                <template #footer>
                  <n-button size="small" block secondary :type="tool.active ? 'primary' : 'default'" :disabled="!tool.active">
                    {{ tool.active ? '进入工具' : '等待接入' }}
                  </n-button>
                </template>
              </n-card>
            </n-grid-item>
          </n-grid>
        </n-card>

        <n-card v-else class="surface-card tool-surface" :bordered="false">
          <n-space vertical :size="18">
            <n-flex justify="space-between" align="center">
              <n-button secondary size="small" @click="goHome">返回首页</n-button>
              <n-space v-if="selectedTool.id === 'aria2'" :size="10">
                <n-tag round type="info">进行中 {{ activeCount }}</n-tag>
                <n-tag round type="success">已完成 {{ completedCount }}</n-tag>
              </n-space>
            </n-flex>

            <template v-if="selectedTool.id === 'aria2'">
              <n-page-header subtitle="本机 aria2c 引擎，支持断点续传、分片、多连接和实时进度回传。">
                <template #title>多线程下载工作台</template>
                <template #extra>
                  <n-tag round>Aria2 Engine</n-tag>
                </template>
              </n-page-header>

              <n-grid responsive="screen" cols="1 l:2" :x-gap="16" :y-gap="16">
                <n-grid-item>
                  <n-card title="新建下载" size="small" :bordered="false" class="panel-card">
                    <template #header-extra>
                      <n-tag round size="small">支持批量</n-tag>
                    </template>
                    <form @submit.prevent="startDownload">
                      <n-form label-placement="top" size="small">
                        <n-form-item label="资源链接（每行一个，或用逗号分隔）">
                          <n-input
                            v-model:value="url"
                            type="textarea"
                            placeholder="https://example.com/file-a.zip&#10;https://example.com/file-b.zip"
                            :autosize="{ minRows: 5, maxRows: 10 }"
                          />
                        </n-form-item>

                        <n-grid responsive="screen" cols="1 m:2" :x-gap="12">
                          <n-grid-item>
                            <n-form-item label="保存目录">
                              <n-input-group>
                                <n-input v-model:value="downloadDir" placeholder="/Users/you/Downloads" />
                                <n-button secondary :loading="choosingDir" @click="chooseDownloadDir">
                                  选择文件夹
                                </n-button>
                              </n-input-group>
                            </n-form-item>
                          </n-grid-item>
                          <n-grid-item>
                            <n-form-item label="文件名（仅单个链接时生效）">
                              <n-input v-model:value="fileName" placeholder="archive.zip" />
                            </n-form-item>
                          </n-grid-item>
                        </n-grid>

                        <n-grid responsive="screen" cols="1 m:3" :x-gap="12">
                          <n-grid-item>
                            <n-form-item label="单服务器连接数">
                              <n-input-number v-model:value="connections" :min="1" :max="16" style="width: 100%" />
                            </n-form-item>
                          </n-grid-item>
                          <n-grid-item>
                            <n-form-item label="分片数">
                              <n-input-number v-model:value="split" :min="1" :max="64" style="width: 100%" />
                            </n-form-item>
                          </n-grid-item>
                          <n-grid-item>
                            <n-form-item label="最小分片大小">
                              <n-select v-model:value="minSplitSize" :options="minSplitOptions" />
                            </n-form-item>
                          </n-grid-item>
                        </n-grid>
                      </n-form>

                      <n-alert v-if="notice" type="error" :bordered="false" class="notice-alert">
                        {{ notice }}
                      </n-alert>

                      <n-button type="primary" block attr-type="submit" :loading="submitting">
                        {{ submitting ? '正在创建...' : '开始下载' }}
                      </n-button>
                    </form>
                  </n-card>
                </n-grid-item>

                <n-grid-item>
                  <n-card title="任务队列" size="small" :bordered="false" class="panel-card queue-card">
                    <template #header-extra>
                      <n-text depth="3">实时解析 aria2 输出</n-text>
                    </template>

                    <n-empty v-if="tasks.length === 0" class="empty-state" description="还没有下载任务" />
                    <n-space v-else vertical :size="6">
                      <TaskCard
                        v-for="task in tasks"
                        :key="task.id"
                        :task="task"
                        @cancel="cancelTask"
                        @open-folder="openTaskFolder"
                      />
                    </n-space>
                  </n-card>
                </n-grid-item>
              </n-grid>
            </template>

            <template v-else-if="selectedTool.id === 'template'">
              <TemplateTool />
            </template>

            <template v-else-if="selectedTool.id === 'image'">
              <n-page-header subtitle="左侧选择商品图与 Logo，右侧拖拽 Logo 到任意位置，拖动右下角控制点调整大小。">
                <template #title>电商图片处理</template>
                <template #extra>
                  <n-tag round type="success">批量加 Logo</n-tag>
                </template>
              </n-page-header>

              <div class="image-editor-layout">
                <n-card title="素材与参数" size="small" :bordered="false" class="panel-card image-side-panel">
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
                      <n-empty v-if="imagePaths.length === 0" description="还没有图片" />
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
                </n-card>

                <n-card title="预览与处理结果" size="small" :bordered="false" class="panel-card preview-panel">
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
                    <n-empty v-else class="empty-state" description="请先在左侧添加图片" />

                    <n-alert v-if="previewImageSrc && !previewLogoSrc" type="info" :bordered="false">
                      请选择 Logo 图片后，可在预览图中拖拽位置和缩放大小。
                    </n-alert>

                    <n-alert v-if="imageResult" type="success" :bordered="false">
                      共 {{ imageResult.total }} 张，成功 {{ imageResult.succeeded }} 张，失败 {{ imageResult.failed.length }} 张。
                    </n-alert>

                    <n-card v-if="imageResult?.outputs.length" size="small" :bordered="false" class="result-card">
                      <template #header>输出文件</template>
                      <n-space vertical :size="6">
                        <n-ellipsis v-for="output in imageResult.outputs" :key="output" :tooltip="false">
                          {{ output }}
                        </n-ellipsis>
                      </n-space>
                    </n-card>

                    <n-card v-if="imageResult?.failed.length" size="small" :bordered="false" class="result-card">
                      <template #header>失败记录</template>
                      <n-space vertical :size="6">
                        <n-text v-for="item in imageResult.failed" :key="item.path" type="error">
                          {{ item.path }}：{{ item.message }}
                        </n-text>
                      </n-space>
                    </n-card>
                  </n-space>
                </n-card>
              </div>
            </template>
          </n-space>
        </n-card>

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
      </main>
    </n-message-provider>
  </n-config-provider>
</template>
