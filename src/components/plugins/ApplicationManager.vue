<script setup lang="ts">
import { computed, onMounted, ref, watchEffect } from 'vue';
import { open } from '@tauri-apps/plugin-dialog';
import {
  NAlert,
  NButton,
  NCheckbox,
  NEmpty,
  NInput,
  NModal,
  NProgress,
  NSpin,
  NTag,
  NTooltip,
  useMessage
} from 'naive-ui';

import type { PluginStoreController } from '../../composables/usePluginStore';
import type { ManagedCatalogApp } from './types';
import type { Tool } from '../../types/tool';
import PluginInstallProbe from './PluginInstallProbe.vue';

type View = 'catalog' | 'installed' | 'history';
type ManagedApp = {
  key: string;
  id: string;
  name: string;
  description: string;
  source: 'builtin' | 'plugin';
  version: string;
  enabled: boolean;
  publisher: string;
  permissions: string[];
  installedAt: string;
  toolId?: string;
};

const props = defineProps<{
  builtinTools: Tool[];
  store: PluginStoreController;
}>();

const emit = defineEmits<{
  open: [id: string];
}>();

const message = useMessage();
const view = ref<View>('catalog');
const query = ref('');
const selectedCatalogId = ref('');
const selectedInstalledKey = ref('');
const installPath = ref('');
const previewOpen = ref(false);
const permissionsAccepted = ref(false);
const installing = ref(false);
const catalogInstallOpen = ref(false);
const catalogOperationMode = ref<'install' | 'update'>('install');
const catalogPermissionsAccepted = ref(false);
const remoteInstalling = ref(false);
const uninstallOpen = ref(false);
const deleteData = ref(false);
const deleteDataConfirmOpen = ref(false);

const installedApps = computed<ManagedApp[]>(() => [
  ...props.builtinTools
    .filter((tool) => tool.source !== 'platform')
    .map((tool) => ({
      key: `builtin:${tool.id}`,
      id: tool.id,
      name: tool.name,
      description: tool.description,
      source: 'builtin' as const,
      version: '随平台',
      enabled: true,
      publisher: 'AT Tool',
      permissions: [],
      installedAt: '',
      toolId: tool.id
    })),
  ...props.store.apps.value.map((app) => ({
    key: `plugin:${app.appId}`,
    id: app.appId,
    name: app.name,
    description: `${app.publisherName} 提供的独立应用`,
    source: 'plugin' as const,
    version: app.version,
    enabled: app.enabled,
    publisher: app.publisherName,
    permissions: app.permissions,
    installedAt: app.installedAt,
    toolId: `plugin:${app.appId}`
  }))
]);

const searchText = computed(() => query.value.trim().toLocaleLowerCase());
const filteredCatalogApps = computed(() =>
  props.store.catalogApps.value.filter((app) =>
    !searchText.value ||
    `${app.name}\n${app.appId}`.toLocaleLowerCase().includes(searchText.value)
  )
);
const filteredInstalledApps = computed(() =>
  installedApps.value.filter((app) =>
    !searchText.value ||
    `${app.name}\n${app.id}`.toLocaleLowerCase().includes(searchText.value)
  )
);
const selectedCatalog = computed<ManagedCatalogApp | null>(() =>
  props.store.catalogApps.value.find((app) => app.appId === selectedCatalogId.value) ?? null
);
const selectedInstalled = computed(() =>
  installedApps.value.find((app) => app.key === selectedInstalledKey.value) ?? null
);
const preview = computed(() => props.store.preview.value);
const storeError = computed(() => props.store.error.value);
const catalogError = computed(() => props.store.catalogError.value);
const catalog = computed(() => props.store.catalog.value);
const operation = computed(() => props.store.operation.value);
const selectedOperation = computed(() =>
  operation.value?.appId === selectedCatalog.value?.appId ? operation.value : null
);
const installProgress = computed(() => {
  const current = selectedOperation.value;
  if (!current?.totalBytes) return 0;
  return Math.min(100, Math.round((current.downloadedBytes / current.totalBytes) * 100));
});
const busy = computed(() =>
  selectedInstalled.value?.source === 'plugin' &&
  props.store.busyAppIds.value.has(selectedInstalled.value.id)
);

watchEffect(() => {
  if (
    !selectedCatalogId.value ||
    !filteredCatalogApps.value.some((app) => app.appId === selectedCatalogId.value)
  ) {
    selectedCatalogId.value = filteredCatalogApps.value[0]?.appId ?? '';
  }
});

watchEffect(() => {
  if (
    !selectedInstalledKey.value ||
    !filteredInstalledApps.value.some((app) => app.key === selectedInstalledKey.value)
  ) {
    selectedInstalledKey.value = filteredInstalledApps.value[0]?.key ?? '';
  }
});

onMounted(() => {
  void refreshCatalog(false);
});

async function refreshCatalog(notify = true) {
  try {
    await props.store.refreshCatalog();
    if (notify) message.success('应用目录已刷新');
  } catch {
    // The stable command error is rendered in-page.
  }
}

async function choosePackage() {
  const path = await open({
    multiple: false,
    filters: [{ name: 'AT Tool 应用包', extensions: ['atpkg'] }]
  });
  if (typeof path !== 'string') return;
  installPath.value = path;
  permissionsAccepted.value = false;
  try {
    await props.store.inspect(path);
    previewOpen.value = true;
  } catch {
    // The stable command error is rendered in-page.
  }
}

async function confirmInstall() {
  if (!permissionsAccepted.value || !installPath.value) return;
  installing.value = true;
  try {
    const installed = await props.store.install(installPath.value);
    previewOpen.value = false;
    selectedInstalledKey.value = `plugin:${installed.appId}`;
    view.value = 'installed';
    message.success(`${installed.name} 已安装`);
  } catch {
    // The stable command error is rendered in-page.
  } finally {
    installing.value = false;
  }
}

function requestCatalogInstall() {
  if (!selectedCatalog.value?.remoteActionsAllowed) return;
  catalogOperationMode.value = 'install';
  catalogPermissionsAccepted.value = false;
  catalogInstallOpen.value = true;
}

function requestCatalogUpdate() {
  const app = selectedCatalog.value;
  if (!app?.remoteActionsAllowed || !app.updateAvailable) return;
  catalogOperationMode.value = 'update';
  catalogPermissionsAccepted.value = app.addedPermissions.length === 0;
  catalogInstallOpen.value = true;
}

async function confirmCatalogInstall() {
  const app = selectedCatalog.value;
  if (!app || !catalogPermissionsAccepted.value) return;
  remoteInstalling.value = true;
  try {
    if (catalogOperationMode.value === 'update') {
      await props.store.updateCatalog(app.appId, [...app.addedPermissions]);
    } else {
      await props.store.installCatalog(app.appId, [...app.permissions]);
    }
  } catch {
    remoteInstalling.value = false;
  }
}

async function completeCatalogInstall() {
  const current = operation.value;
  if (!current || current.stage !== 'health-check') return;
  try {
    const installed = await props.store.completeCatalogInstall(current.operationId);
    catalogInstallOpen.value = false;
    selectedInstalledKey.value = `plugin:${installed.appId}`;
    view.value = 'installed';
    message.success(`${installed.name} 已${catalogOperationMode.value === 'update' ? '升级' : '安装'}`);
  } catch {
    // The stable command error is rendered in-page.
  } finally {
    remoteInstalling.value = false;
  }
}

async function failCatalogInstall() {
  const current = operation.value;
  if (!current || current.stage !== 'health-check') return;
  try {
    await props.store.failCatalogInstall(current.operationId);
  } catch {
    // The stable command error is rendered in-page.
  } finally {
    remoteInstalling.value = false;
  }
}

function openCatalogApp() {
  const app = selectedCatalog.value;
  if (app?.installedVersion && app.enabled) emit('open', `plugin:${app.appId}`);
}

async function toggleSelected() {
  const app = selectedInstalled.value;
  if (!app || app.source !== 'plugin') return;
  try {
    await props.store.setEnabled(app.id, !app.enabled);
    message.success(app.enabled ? '应用已停用' : '应用已启用');
  } catch {
    // The stable command error is rendered in-page.
  }
}

function requestUninstall() {
  deleteData.value = false;
  uninstallOpen.value = true;
}

function continueUninstall() {
  if (deleteData.value) {
    uninstallOpen.value = false;
    deleteDataConfirmOpen.value = true;
    return;
  }
  void performUninstall(false);
}

async function performUninstall(removeData: boolean) {
  const app = selectedInstalled.value;
  if (!app || app.source !== 'plugin') return;
  try {
    await props.store.uninstall(app.id, removeData);
    uninstallOpen.value = false;
    deleteDataConfirmOpen.value = false;
    message.success(removeData ? '应用和用户数据已删除' : '应用已卸载，用户数据已保留');
  } catch {
    // The stable command error is rendered in-page.
  }
}

function switchView(next: View) {
  view.value = next;
  query.value = '';
}

function actionLabel(action: string) {
  return { install: '安装', update: '升级', rollback: '自动回滚', enable: '启用', disable: '停用', uninstall: '卸载' }[action] ?? action;
}

function operationStageLabel(stage: string) {
  return {
    download: '下载制品',
    verify: '验证签名与完整性',
    staging: '准备隔离环境',
    'health-check': 'Host API 健康检查',
    activate: '激活应用',
    complete: '安装完成',
    'rolled-back': '已自动回滚',
    error: '安装失败'
  }[stage] ?? stage;
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}
</script>

<template>
  <section class="manager">
    <header class="manager-header">
      <div>
        <h2>应用管理</h2>
        <p>发现官方应用并管理本机安装状态。</p>
      </div>
      <div class="header-actions">
        <n-tooltip>
          <template #trigger>
            <n-button
              secondary
              circle
              aria-label="刷新应用目录"
              :loading="store.catalogRefreshing.value"
              @click="refreshCatalog()"
            >
              <template #icon>
                <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
                  <path d="M20 6v5h-5" />
                  <path d="M4 18v-5h5" />
                  <path d="M6.1 9A7 7 0 0 1 18.4 6L20 8" />
                  <path d="M17.9 15A7 7 0 0 1 5.6 18L4 16" />
                </svg>
              </template>
            </n-button>
          </template>
          刷新应用目录
        </n-tooltip>
        <n-button secondary @click="choosePackage">
          <template #icon>
            <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
              <path d="M12 3v12" />
              <path d="m7 10 5 5 5-5" />
              <path d="M5 21h14" />
            </svg>
          </template>
          安装本地应用包
        </n-button>
      </div>
    </header>

    <n-alert v-if="storeError" type="error" :bordered="false" closable @close="store.error.value = null">
      <strong>{{ storeError.code }}</strong> · {{ storeError.message }}
      <span v-if="storeError.correlationId" class="correlation">{{ storeError.correlationId }}</span>
    </n-alert>

    <nav class="view-tabs" role="tablist" aria-label="应用管理视图">
      <button :class="{ active: view === 'catalog' }" type="button" @click="switchView('catalog')">
        应用目录
      </button>
      <button :class="{ active: view === 'installed' }" type="button" @click="switchView('installed')">
        已安装 <span>{{ installedApps.length }}</span>
      </button>
      <button :class="{ active: view === 'history' }" type="button" @click="switchView('history')">
        操作记录
      </button>
    </nav>

    <div v-if="view !== 'history'" class="toolbar">
      <n-input
        v-model:value="query"
        clearable
        :placeholder="view === 'catalog' ? '搜索应用目录' : '搜索已安装应用'"
      />
      <span v-if="view === 'catalog' && catalog" class="catalog-meta">
        Catalog #{{ catalog.catalogSequence }} · {{ new Date(catalog.generatedAt).toLocaleString() }}
      </span>
    </div>

    <div v-if="view === 'catalog'" class="tab-content">
      <n-alert
        v-if="catalogError && catalog"
        type="warning"
        :bordered="false"
        closable
        @close="store.catalogError.value = null"
      >
        <strong>{{ catalogError.code }}</strong> · {{ catalogError.message }}，当前继续显示上次有效目录。
      </n-alert>

      <div v-if="(store.catalogLoading.value || store.catalogRefreshing.value) && !catalog" class="state-box">
        <n-spin size="small" />
        <span>正在验证应用目录</span>
      </div>

      <div v-else-if="catalogError && !catalog" class="state-box">
        <n-empty description="应用目录暂时不可用">
          <template #extra>
            <div class="state-actions">
              <span class="error-code">{{ catalogError.code }}</span>
              <n-button size="small" secondary @click="refreshCatalog()">重试</n-button>
            </div>
          </template>
        </n-empty>
      </div>

      <div v-else-if="catalog?.expired" class="state-box">
        <n-empty description="应用目录已过期">
          <template #extra>
            <div class="state-actions">
              <n-button size="small" secondary @click="refreshCatalog()">刷新目录</n-button>
              <n-button size="small" @click="switchView('installed')">查看已安装</n-button>
            </div>
          </template>
        </n-empty>
      </div>

      <div v-else-if="catalog && !filteredCatalogApps.length" class="state-box">
        <n-empty :description="query ? '没有匹配的应用' : '应用目录为空'" />
      </div>

      <div v-else-if="selectedCatalog" class="workspace">
        <aside class="app-list">
          <div class="rows">
            <button
              v-for="app in filteredCatalogApps"
              :key="app.appId"
              type="button"
              class="app-row"
              :class="{ active: selectedCatalogId === app.appId }"
              @click="selectedCatalogId = app.appId"
            >
              <span class="app-glyph">{{ app.name.slice(0, 1).toUpperCase() }}</span>
              <span class="row-main">
                <strong>{{ app.name }}</strong>
                <span class="app-description">{{ app.description }}</span>
              </span>
              <span class="row-meta">
                <n-tag size="small" :type="app.installedVersion ? 'success' : 'default'" :bordered="false">
                  {{ app.installedVersion ? '已安装' : '可获取' }}
                </n-tag>
                <span class="mono">{{ app.latestVersion }}</span>
              </span>
            </button>
          </div>
        </aside>

        <article class="detail">
          <div class="detail-title">
            <span class="detail-glyph">{{ selectedCatalog.name.slice(0, 1).toUpperCase() }}</span>
            <div>
              <h3>{{ selectedCatalog.name }}</h3>
              <p>{{ selectedCatalog.description }}</p>
            </div>
            <n-tag type="success" :bordered="false">官方签名</n-tag>
          </div>

          <dl class="metadata">
            <div><dt>应用 ID</dt><dd class="mono">{{ selectedCatalog.appId }}</dd></div>
            <div><dt>发布者</dt><dd>{{ selectedCatalog.publisherName }}</dd></div>
            <div><dt>最新版本</dt><dd class="mono">{{ selectedCatalog.latestVersion }}</dd></div>
            <div><dt>已安装版本</dt><dd class="mono">{{ selectedCatalog.installedVersion ?? '未安装' }}</dd></div>
            <div><dt>Host API</dt><dd class="mono">{{ selectedCatalog.hostApiMin }} - {{ selectedCatalog.hostApiMax }}</dd></div>
            <div><dt>发布时间</dt><dd>{{ new Date(selectedCatalog.publishedAt).toLocaleString() }}</dd></div>
          </dl>

          <section v-if="selectedCatalog.target" class="detail-section">
            <h4>当前系统制品</h4>
            <dl class="target-grid">
              <div><dt>Target</dt><dd class="mono">{{ selectedCatalog.target.os }} / {{ selectedCatalog.target.arch }}</dd></div>
              <div><dt>文件</dt><dd class="mono">{{ selectedCatalog.target.fileName }}</dd></div>
              <div><dt>大小</dt><dd>{{ formatBytes(selectedCatalog.target.size) }}</dd></div>
              <div><dt>SHA-256</dt><dd class="mono hash">{{ selectedCatalog.target.sha256 }}</dd></div>
            </dl>
          </section>
          <n-alert v-else type="warning" :bordered="false">当前系统没有可用制品。</n-alert>

          <section class="detail-section">
            <h4>版本说明</h4>
            <p>{{ selectedCatalog.notes || '无版本说明' }}</p>
          </section>

          <section class="detail-section">
            <h4>权限</h4>
            <div class="permission-list">
              <span v-for="permission in selectedCatalog.permissions" :key="permission" class="permission mono">
                {{ permission }}
              </span>
            </div>
          </section>

          <section v-if="selectedOperation" class="operation-state">
            <div>
              <strong>{{ operationStageLabel(selectedOperation.stage) }}</strong>
              <span class="mono">{{ selectedOperation.operationId }}</span>
            </div>
            <n-progress
              v-if="selectedOperation.stage === 'download'"
              type="line"
              :percentage="installProgress"
              :show-indicator="false"
            />
            <span v-if="selectedOperation.stage === 'download'">
              {{ formatBytes(selectedOperation.downloadedBytes) }} / {{ formatBytes(selectedOperation.totalBytes) }}
            </span>
            <n-alert v-if="selectedOperation.errorCode" type="error" :bordered="false">
              <strong>{{ selectedOperation.errorCode }}</strong> · {{ selectedOperation.message }}
              <span class="correlation">{{ selectedOperation.correlationId }}</span>
            </n-alert>
          </section>

          <div class="actions">
            <template v-if="selectedCatalog.installedVersion">
              <n-button
                type="primary"
                :disabled="!selectedCatalog.enabled"
                @click="openCatalogApp"
              >
                {{ selectedCatalog.enabled ? '打开' : '已停用' }}
              </n-button>
              <n-button
                v-if="selectedCatalog.updateAvailable"
                secondary
                :disabled="!selectedCatalog.remoteActionsAllowed"
                :loading="remoteInstalling || store.busyAppIds.value.has(selectedCatalog.appId)"
                @click="requestCatalogUpdate"
              >升级</n-button>
            </template>
            <n-button
              v-else
              type="primary"
              :disabled="!selectedCatalog.remoteActionsAllowed"
              :loading="remoteInstalling || store.busyAppIds.value.has(selectedCatalog.appId)"
              @click="requestCatalogInstall"
            >安装</n-button>
          </div>
        </article>
      </div>

      <div v-else class="state-box">
        <n-empty description="尚未获取应用目录" />
      </div>
    </div>

    <div v-else-if="view === 'installed'" class="tab-content">
      <div v-if="store.loading.value" class="state-box"><n-spin size="small" /> 正在读取应用</div>
      <div v-else-if="!filteredInstalledApps.length" class="state-box">
        <n-empty description="没有匹配的应用" />
      </div>
      <div v-else-if="selectedInstalled" class="workspace">
        <aside class="app-list">
          <div class="rows">
            <button
              v-for="app in filteredInstalledApps"
              :key="app.key"
              type="button"
              class="app-row"
              :class="{ active: selectedInstalledKey === app.key }"
              @click="selectedInstalledKey = app.key"
            >
              <span class="app-glyph">{{ app.name.slice(0, 1).toUpperCase() }}</span>
              <span class="row-main">
                <strong>{{ app.name }}</strong>
                <span class="app-id">{{ app.id }}</span>
              </span>
              <span class="row-meta">
                <n-tag size="small" :bordered="false">{{ app.source === 'builtin' ? '内置' : '应用' }}</n-tag>
                <span :class="['state-dot', { off: !app.enabled }]">{{ app.enabled ? '启用' : '停用' }}</span>
              </span>
            </button>
          </div>
        </aside>

        <article class="detail">
          <div class="detail-title">
            <span class="detail-glyph">{{ selectedInstalled.name.slice(0, 1).toUpperCase() }}</span>
            <div>
              <h3>{{ selectedInstalled.name }}</h3>
              <p>{{ selectedInstalled.description }}</p>
            </div>
            <n-tag :type="selectedInstalled.enabled ? 'success' : 'default'" :bordered="false">
              {{ selectedInstalled.enabled ? '已启用' : '已停用' }}
            </n-tag>
          </div>

          <dl class="metadata">
            <div><dt>应用 ID</dt><dd class="mono">{{ selectedInstalled.id }}</dd></div>
            <div><dt>来源</dt><dd>{{ selectedInstalled.source === 'builtin' ? '平台内置' : '本地签名包' }}</dd></div>
            <div><dt>版本</dt><dd class="mono">{{ selectedInstalled.version }}</dd></div>
            <div><dt>发布者</dt><dd>{{ selectedInstalled.publisher }}</dd></div>
            <div v-if="selectedInstalled.installedAt"><dt>安装时间</dt><dd>{{ new Date(selectedInstalled.installedAt).toLocaleString() }}</dd></div>
          </dl>

          <section class="detail-section">
            <h4>权限</h4>
            <div v-if="selectedInstalled.permissions.length" class="permission-list">
              <span v-for="permission in selectedInstalled.permissions" :key="permission" class="permission mono">
                {{ permission }}
              </span>
            </div>
            <p v-else>平台内置能力，随主程序权限运行。</p>
          </section>

          <div class="actions">
            <n-button
              type="primary"
              :disabled="!selectedInstalled.enabled"
              @click="selectedInstalled.toolId && emit('open', selectedInstalled.toolId)"
            >
              打开
            </n-button>
            <template v-if="selectedInstalled.source === 'plugin'">
              <n-button secondary :loading="busy" @click="toggleSelected">
                {{ selectedInstalled.enabled ? '停用' : '启用' }}
              </n-button>
              <n-button type="error" secondary :loading="busy" @click="requestUninstall">卸载</n-button>
            </template>
          </div>
        </article>
      </div>
    </div>

    <div v-else class="history tab-content">
      <div class="history-head">
        <span>应用 / 动作</span><span>版本</span><span>检查步骤</span><span>时间 / 结果</span>
      </div>
      <div v-if="store.history.value.length" class="history-rows">
        <div v-for="record in store.history.value" :key="record.id" class="history-row">
          <div>
            <strong class="mono">{{ record.appId }}</strong>
            <span>{{ actionLabel(record.action) }}<template v-if="record.errorCode"> · {{ record.errorCode }}</template></span>
            <span v-if="record.correlationId" class="mono">{{ record.correlationId }}</span>
          </div>
          <span class="mono">{{ record.version }}</span>
          <span class="steps">{{ record.steps.join(' · ') }}</span>
          <div>
            <span>{{ new Date(record.occurredAt).toLocaleString() }}</span>
            <n-tag size="small" :type="record.result === 'success' ? 'success' : record.result === 'rolled-back' ? 'warning' : 'error'" :bordered="false">
              {{ record.result === 'success' ? '成功' : record.result === 'rolled-back' ? '已回滚' : '失败' }}
            </n-tag>
          </div>
        </div>
      </div>
      <n-empty v-else description="暂无操作记录" />
    </div>

    <PluginInstallProbe
      v-if="operation?.stage === 'health-check' && operation.entry"
      :operation="operation"
      @ready="completeCatalogInstall"
      @failed="failCatalogInstall"
    />

    <n-modal
      v-model:show="catalogInstallOpen"
      preset="card"
      :title="catalogOperationMode === 'update' ? '升级官方应用' : '安装官方应用'"
      class="install-modal"
      :mask-closable="!remoteInstalling"
      :closable="!remoteInstalling"
    >
      <template v-if="selectedCatalog">
        <div class="package-heading">
          <strong>{{ selectedCatalog.name }}</strong>
          <span class="mono">{{ selectedCatalog.latestVersion }}</span>
        </div>
        <dl class="preview-grid">
          <div><dt>发布者</dt><dd>{{ selectedCatalog.publisherName }}</dd></div>
          <div><dt>签名 Key</dt><dd class="mono">{{ selectedCatalog.publisherKeyId }}</dd></div>
          <div><dt>目标平台</dt><dd class="mono">{{ selectedCatalog.target?.os }} / {{ selectedCatalog.target?.arch }}</dd></div>
          <div><dt>制品大小</dt><dd>{{ formatBytes(selectedCatalog.target?.size ?? 0) }}</dd></div>
        </dl>
        <section class="permission-confirm">
          <h4>{{ catalogOperationMode === 'update' ? '新增权限' : '请求权限' }}</h4>
          <div class="permission-list">
            <span v-for="permission in catalogOperationMode === 'update' ? selectedCatalog.addedPermissions : selectedCatalog.permissions" :key="permission" class="permission mono">
              {{ permission }}
            </span>
          </div>
          <n-alert v-if="catalogOperationMode === 'update' && !selectedCatalog.addedPermissions.length" type="info" :bordered="false">
            权限未增加，将保留当前授权与启停状态。
          </n-alert>
          <n-checkbox v-else v-model:checked="catalogPermissionsAccepted" :disabled="remoteInstalling">
            我已查看并同意授予以上权限
          </n-checkbox>
        </section>
        <section v-if="selectedOperation" class="operation-state modal-operation">
          <div>
            <strong>{{ operationStageLabel(selectedOperation.stage) }}</strong>
            <span class="mono">关联 ID {{ selectedOperation.correlationId }}</span>
          </div>
          <n-progress
            v-if="selectedOperation.stage === 'download'"
            type="line"
            :percentage="installProgress"
          />
          <n-alert v-if="selectedOperation.errorCode" type="error" :bordered="false">
            <strong>{{ selectedOperation.errorCode }}</strong> · {{ selectedOperation.message }}
          </n-alert>
        </section>
      </template>
      <template #footer>
        <div class="modal-actions">
          <n-button :disabled="remoteInstalling" @click="catalogInstallOpen = false">取消</n-button>
          <n-button
            type="primary"
            :disabled="!catalogPermissionsAccepted || remoteInstalling"
            :loading="remoteInstalling"
            @click="confirmCatalogInstall"
          >
            {{ selectedOperation?.stage === 'error' ? '重试' : catalogOperationMode === 'update' ? '下载并升级' : '下载并安装' }}
          </n-button>
        </div>
      </template>
    </n-modal>

    <n-modal v-model:show="previewOpen" preset="card" title="确认安装应用" class="install-modal" :mask-closable="!installing">
      <template v-if="preview">
        <div class="package-heading"><strong>{{ preview.name }}</strong><span class="mono">{{ preview.version }}</span></div>
        <dl class="preview-grid">
          <div><dt>应用 ID</dt><dd class="mono">{{ preview.appId }}</dd></div>
          <div><dt>发布者</dt><dd>{{ preview.publisherName }} · <span class="mono">{{ preview.publisherKeyId }}</span></dd></div>
          <div><dt>目标平台</dt><dd class="mono">{{ preview.targetOs }} / {{ preview.targetArch }}</dd></div>
          <div><dt>Host API</dt><dd class="mono">{{ preview.hostApiMin }} - {{ preview.hostApiMax }}</dd></div>
        </dl>
        <div class="checks">
          <span>签名有效</span><span>{{ Object.keys(preview.files).length }} 个文件 hash 有效</span><span>兼容性通过</span>
        </div>
        <section class="permission-confirm">
          <h4>请求权限</h4>
          <div class="permission-list">
            <span v-for="permission in preview.permissions" :key="permission" class="permission mono">{{ permission }}</span>
          </div>
          <n-checkbox v-model:checked="permissionsAccepted">我已查看并同意授予以上权限</n-checkbox>
        </section>
        <n-alert v-if="installing" type="info" :bordered="false">正在执行 staging、入口健康检查与原子激活</n-alert>
      </template>
      <template #footer>
        <div class="modal-actions">
          <n-button :disabled="installing" @click="previewOpen = false">取消</n-button>
          <n-button type="primary" :disabled="!permissionsAccepted" :loading="installing" @click="confirmInstall">安装</n-button>
        </div>
      </template>
    </n-modal>

    <n-modal v-model:show="uninstallOpen" preset="card" title="卸载应用" class="confirm-modal">
      <p>将移除应用程序包和导航入口。用户数据默认保留，可供以后重装或独立应用继续使用。</p>
      <n-checkbox v-model:checked="deleteData">同时删除用户数据</n-checkbox>
      <template #footer>
        <div class="modal-actions">
          <n-button @click="uninstallOpen = false">取消</n-button>
          <n-button type="error" @click="continueUninstall">继续</n-button>
        </div>
      </template>
    </n-modal>

    <n-modal v-model:show="deleteDataConfirmOpen" preset="card" title="确认删除用户数据" class="confirm-modal" @keydown.enter.prevent>
      <n-alert type="error" :bordered="false">此操作不可恢复，独立应用也将失去这些数据。</n-alert>
      <template #footer>
        <div class="modal-actions">
          <n-button @click="deleteDataConfirmOpen = false">返回</n-button>
          <n-button type="error" :loading="busy" @click="performUninstall(true)">删除应用和数据</n-button>
        </div>
      </template>
    </n-modal>
  </section>
</template>

<style scoped>
.manager { height: 100%; min-height: 0; display: flex; flex-direction: column; gap: 12px; overflow: hidden; }
.manager-header { flex: none; display: flex; align-items: center; justify-content: space-between; gap: 16px; }
.manager-header h2 { margin: 0; font-size: var(--fs-xl); font-weight: 600; letter-spacing: 0; }
.manager-header p, .detail-title p, .detail-section p { margin: 3px 0 0; color: var(--text-muted); font-size: var(--fs-xs); }
.header-actions, .actions, .state-actions, .modal-actions { display: flex; align-items: center; gap: 8px; }
.correlation { display: block; margin-top: 4px; font-family: var(--font-mono); font-size: var(--fs-xxs); }
.view-tabs { flex: none; display: flex; gap: 18px; border-bottom: 1px solid var(--line); }
.view-tabs button { position: relative; padding: 7px 1px 9px; border: 0; background: transparent; color: var(--text-muted); cursor: pointer; }
.view-tabs button.active { color: var(--text); }
.view-tabs button.active::after { position: absolute; right: 0; bottom: -1px; left: 0; height: 2px; background: var(--accent); content: ""; }
.view-tabs span { margin-left: 4px; color: var(--text-muted); font-size: var(--fs-xxs); }
.toolbar { flex: none; display: flex; align-items: center; gap: 12px; }
.toolbar :deep(.n-input) { width: min(360px, 55%); }
.catalog-meta { margin-left: auto; color: var(--text-muted); font-family: var(--font-mono); font-size: var(--fs-xxs); }
.tab-content { flex: 1; min-height: 0; display: flex; flex-direction: column; gap: 10px; overflow: hidden; }
.workspace { flex: 1; min-height: 0; display: grid; grid-template-columns: minmax(280px, .72fr) minmax(420px, 1.28fr); border: 1px solid var(--line); border-radius: var(--radius-md); overflow: hidden; background: var(--bg-elevated); }
.app-list { min-width: 0; min-height: 0; overflow: hidden; border-right: 1px solid var(--line); }
.rows { height: 100%; min-height: 0; overflow: auto; padding: 6px; }
.app-row { width: 100%; min-height: 68px; display: grid; grid-template-columns: 36px minmax(0, 1fr) auto; align-items: center; gap: 10px; padding: 8px; border: 0; border-radius: var(--radius); background: transparent; color: var(--text); text-align: left; cursor: pointer; }
.app-row:hover { background: var(--bg-elev-2); }
.app-row.active { background: var(--accent-soft); }
.app-glyph, .detail-glyph { display: grid; place-items: center; flex: none; border: 1px solid var(--line-strong); border-radius: var(--radius); background: var(--bg-elev-2); color: var(--accent); font-family: var(--font-mono); font-weight: 700; }
.app-glyph { width: 34px; height: 34px; }
.detail-glyph { width: 46px; height: 46px; font-size: var(--fs-lg); }
.row-main { min-width: 0; display: grid; gap: 3px; }
.row-main strong { font-size: var(--fs-md); }
.app-id, .app-description { overflow: hidden; color: var(--text-muted); font-size: var(--fs-xxs); text-overflow: ellipsis; white-space: nowrap; }
.app-id { font-family: var(--font-mono); }
.row-meta { display: grid; justify-items: end; gap: 5px; color: var(--text-muted); font-size: var(--fs-xxs); }
.state-dot { color: var(--accent); }
.state-dot.off { color: var(--text-muted); }
.detail { min-width: 0; min-height: 0; padding: 20px 22px; overflow: auto; }
.detail-title { display: flex; align-items: center; gap: 12px; padding-bottom: 16px; border-bottom: 1px solid var(--line); }
.detail-title > div { flex: 1; min-width: 0; }
.detail-title h3 { margin: 0; font-size: var(--fs-lg); letter-spacing: 0; }
.metadata, .preview-grid { display: grid; grid-template-columns: 1fr 1fr; margin: 16px 0; border: 1px solid var(--line); border-radius: var(--radius); overflow: hidden; }
.metadata > div, .preview-grid > div { min-width: 0; padding: 10px 12px; border-bottom: 1px solid var(--line); }
.metadata > div:nth-child(odd), .preview-grid > div:nth-child(odd) { border-right: 1px solid var(--line); }
.metadata dt, .preview-grid dt, .target-grid dt { color: var(--text-muted); font-size: var(--fs-xxs); }
.metadata dd, .preview-grid dd, .target-grid dd { margin: 4px 0 0; overflow-wrap: anywhere; font-size: var(--fs-xs); }
.detail-section { padding: 14px 0; border-bottom: 1px solid var(--line); }
.detail-section h4, .permission-confirm h4 { margin: 0 0 8px; font-size: var(--fs-xs); }
.target-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; margin: 0; }
.hash { word-break: break-all; }
.permission-list { display: flex; flex-wrap: wrap; gap: 6px; margin-bottom: 10px; }
.permission { padding: 4px 7px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--bg-elev-2); font-size: var(--fs-xxs); }
.operation-state { display: grid; gap: 8px; margin-top: 14px; padding: 12px; border: 1px solid var(--line); border-radius: var(--radius); background: var(--bg-elev-2); color: var(--text-muted); font-size: var(--fs-xxs); }
.operation-state > div { display: flex; justify-content: space-between; gap: 12px; }
.operation-state strong { color: var(--text); }
.modal-operation { margin-top: 16px; }
.actions { margin-top: 20px; }
.state-box { flex: 1; min-height: 0; display: flex; align-items: center; justify-content: center; gap: 8px; border: 1px solid var(--line); border-radius: var(--radius-md); background: var(--bg-elevated); color: var(--text-muted); }
.state-actions { justify-content: center; margin-top: 10px; }
.error-code { font-family: var(--font-mono); font-size: var(--fs-xxs); }
.history { border: 1px solid var(--line); border-radius: var(--radius-md); overflow: auto; background: var(--bg-elevated); }
.history-head, .history-row { display: grid; grid-template-columns: minmax(220px, 1fr) 140px minmax(220px, 1fr) 210px; gap: 12px; align-items: center; padding: 10px 14px; }
.history-head { position: sticky; top: 0; z-index: 1; border-bottom: 1px solid var(--line); background: var(--bg-elev-2); color: var(--text-muted); font-size: var(--fs-xxs); }
.history-row { border-bottom: 1px solid var(--line); font-size: var(--fs-xs); }
.history-row > div { display: grid; gap: 3px; }
.history-row > div:last-child { grid-template-columns: 1fr auto; align-items: center; }
.history-row span { color: var(--text-muted); }
.steps { overflow-wrap: anywhere; }
.package-heading { display: flex; justify-content: space-between; gap: 12px; }
.checks { display: flex; flex-wrap: wrap; gap: 6px; margin: 12px 0; }
.checks span { padding: 4px 7px; border-radius: var(--radius-sm); background: var(--accent-soft); color: var(--accent); font-size: var(--fs-xxs); }
.permission-confirm { display: grid; gap: 8px; margin-top: 14px; }
.confirm-modal p { color: var(--text-muted); line-height: 1.65; }
.mono { font-family: var(--font-mono); font-feature-settings: "tnum"; }
:global(.install-modal) { width: min(640px, 92vw); }
:global(.confirm-modal) { width: min(480px, 92vw); }
@media (max-width: 900px) {
  .manager-header { align-items: flex-start; }
  .header-actions { flex-wrap: wrap; justify-content: flex-end; }
  .workspace { grid-template-columns: 1fr; grid-template-rows: minmax(180px, .8fr) minmax(240px, 1.2fr); }
  .app-list { border-right: 0; border-bottom: 1px solid var(--line); }
  .detail { overflow: auto; }
  .history-head { display: none; }
  .history-row { grid-template-columns: 1fr 1fr; }
}
@media (max-width: 620px) {
  .manager-header { display: grid; }
  .header-actions { justify-content: flex-start; }
  .toolbar { align-items: stretch; flex-direction: column; }
  .toolbar :deep(.n-input) { width: 100%; }
  .catalog-meta { margin-left: 0; overflow-wrap: anywhere; }
  .metadata, .preview-grid, .target-grid { grid-template-columns: 1fr; }
  .metadata > div:nth-child(odd), .preview-grid > div:nth-child(odd) { border-right: 0; }
  .history-row { grid-template-columns: 1fr; }
}
</style>
