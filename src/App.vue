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
  NPageHeader,
  NSelect,
  NSpace,
  NTag,
  NText
} from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import TaskCard from './components/TaskCard.vue';
import type { DownloadEventPayload, DownloadTask, StartDownloadRequest } from './types/download';

type ToolEntry = {
  id: string;
  name: string;
  description: string;
  badge: string;
  active: boolean;
};

const themeOverrides = {
  common: {
    primaryColor: '#56715d',
    primaryColorHover: '#68866f',
    primaryColorPressed: '#263c2d',
    borderRadius: '14px',
    borderRadiusSmall: '10px',
    fontFamily: '"Avenir Next", "Gill Sans", "PingFang SC", "Microsoft YaHei", sans-serif'
  },
  Card: {
    borderRadius: '24px'
  },
  Button: {
    borderRadiusMedium: '14px',
    borderRadiusSmall: '12px'
  }
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
    id: 'image',
    name: '图片工具',
    description: '压缩、格式转换、水印与尺寸处理',
    badge: 'Soon',
    active: false
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
</script>

<template>
  <n-config-provider :theme-overrides="themeOverrides">
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
              <n-space :size="10">
                <n-tag round type="info">进行中 {{ activeCount }}</n-tag>
                <n-tag round type="success">已完成 {{ completedCount }}</n-tag>
              </n-space>
            </n-flex>

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
          </n-space>
        </n-card>
      </main>
    </n-message-provider>
  </n-config-provider>
</template>
