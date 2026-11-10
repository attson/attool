<script setup lang="ts">
import { computed, ref, watchEffect } from 'vue';
import { open } from '@tauri-apps/plugin-dialog';
import {
  NAlert,
  NButton,
  NCheckbox,
  NEmpty,
  NInput,
  NModal,
  NSelect,
  NSpin,
  NTag,
  useMessage
} from 'naive-ui';

import type { PluginStoreController } from '../../composables/usePluginStore';
import type { Tool } from '../../types/tool';

type SourceFilter = 'all' | 'builtin' | 'plugin';
type StateFilter = 'all' | 'enabled' | 'disabled';
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
const view = ref<'apps' | 'history'>('apps');
const query = ref('');
const sourceFilter = ref<SourceFilter>('all');
const stateFilter = ref<StateFilter>('all');
const selectedKey = ref('');
const installPath = ref('');
const previewOpen = ref(false);
const permissionsAccepted = ref(false);
const installing = ref(false);
const uninstallOpen = ref(false);
const deleteData = ref(false);
const deleteDataConfirmOpen = ref(false);

const sourceOptions = [
  { label: '全部来源', value: 'all' },
  { label: '内置应用', value: 'builtin' },
  { label: '插件应用', value: 'plugin' }
];
const stateOptions = [
  { label: '全部状态', value: 'all' },
  { label: '已启用', value: 'enabled' },
  { label: '已停用', value: 'disabled' }
];

const apps = computed<ManagedApp[]>(() => [
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

const filteredApps = computed(() => {
  const text = query.value.trim().toLocaleLowerCase();
  return apps.value.filter((app) => {
    if (sourceFilter.value !== 'all' && app.source !== sourceFilter.value) return false;
    if (stateFilter.value === 'enabled' && !app.enabled) return false;
    if (stateFilter.value === 'disabled' && app.enabled) return false;
    return !text || `${app.name}\n${app.id}`.toLocaleLowerCase().includes(text);
  });
});

const selected = computed(() =>
  apps.value.find((app) => app.key === selectedKey.value) ?? null
);
const preview = computed(() => props.store.preview.value);
const storeError = computed(() => props.store.error.value);
const busy = computed(() =>
  selected.value?.source === 'plugin' && props.store.busyAppIds.value.has(selected.value.id)
);

watchEffect(() => {
  if (!selectedKey.value || !apps.value.some((app) => app.key === selectedKey.value)) {
    selectedKey.value = filteredApps.value[0]?.key ?? apps.value[0]?.key ?? '';
  }
});

function clearFilters() {
  query.value = '';
  sourceFilter.value = 'all';
  stateFilter.value = 'all';
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
    selectedKey.value = `plugin:${installed.appId}`;
    message.success(`${installed.name} 已安装`);
  } catch {
    // The stable command error is rendered in-page.
  } finally {
    installing.value = false;
  }
}

async function toggleSelected() {
  const app = selected.value;
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
  const app = selected.value;
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

function actionLabel(action: string) {
  return { install: '安装', enable: '启用', disable: '停用', uninstall: '卸载' }[action] ?? action;
}
</script>

<template>
  <section class="manager">
    <header class="manager-header">
      <div>
        <h2>应用管理</h2>
        <p>管理内置工具与本地安装的应用包。</p>
      </div>
      <n-button type="primary" @click="choosePackage">
        <template #icon>
          <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
            <path d="M12 3v12" />
            <path d="m7 10 5 5 5-5" />
            <path d="M5 21h14" />
          </svg>
        </template>
        安装应用包
      </n-button>
    </header>

    <n-alert v-if="storeError" type="error" :bordered="false" closable @close="store.error.value = null">
      <strong>{{ storeError.code }}</strong> · {{ storeError.message }}
      <span v-if="storeError.correlationId" class="correlation">{{ storeError.correlationId }}</span>
    </n-alert>

    <div class="view-tabs" role="tablist" aria-label="应用管理视图">
      <button :class="{ active: view === 'apps' }" type="button" @click="view = 'apps'">应用</button>
      <button :class="{ active: view === 'history' }" type="button" @click="view = 'history'">安装记录</button>
    </div>

    <div v-if="view === 'apps'" class="workspace">
      <aside class="app-list">
        <div class="filters">
          <n-input v-model:value="query" clearable placeholder="搜索名称或 app ID" />
          <div class="filter-row">
            <n-select v-model:value="sourceFilter" :options="sourceOptions" />
            <n-select v-model:value="stateFilter" :options="stateOptions" />
          </div>
        </div>

        <div v-if="store.loading.value" class="loading"><n-spin size="small" /> 正在读取应用</div>
        <div v-else-if="filteredApps.length" class="rows">
          <button
            v-for="app in filteredApps"
            :key="app.key"
            type="button"
            class="app-row"
            :class="{ active: selectedKey === app.key }"
            @click="selectedKey = app.key"
          >
            <span class="app-glyph">{{ app.name.slice(0, 1).toUpperCase() }}</span>
            <span class="row-main">
              <strong>{{ app.name }}</strong>
              <span class="app-id">{{ app.id }}</span>
            </span>
            <span class="row-meta">
              <n-tag size="small" :bordered="false">{{ app.source === 'builtin' ? '内置' : '插件' }}</n-tag>
              <span :class="['state-dot', { off: !app.enabled }]">{{ app.enabled ? '启用' : '停用' }}</span>
            </span>
          </button>
        </div>
        <div v-else class="empty-filter">
          <n-empty description="没有匹配的应用" />
          <n-button size="small" secondary @click="clearFilters">清除筛选</n-button>
        </div>
      </aside>

      <article v-if="selected" class="detail">
        <div class="detail-title">
          <span class="detail-glyph">{{ selected.name.slice(0, 1).toUpperCase() }}</span>
          <div>
            <h3>{{ selected.name }}</h3>
            <p>{{ selected.description }}</p>
          </div>
          <n-tag :type="selected.enabled ? 'success' : 'default'" :bordered="false">
            {{ selected.enabled ? '已启用' : '已停用' }}
          </n-tag>
        </div>

        <dl class="metadata">
          <div><dt>应用 ID</dt><dd class="mono">{{ selected.id }}</dd></div>
          <div><dt>来源</dt><dd>{{ selected.source === 'builtin' ? '平台内置' : '本地签名包' }}</dd></div>
          <div><dt>版本</dt><dd class="mono">{{ selected.version }}</dd></div>
          <div><dt>发布者</dt><dd>{{ selected.publisher }}</dd></div>
          <div v-if="selected.installedAt"><dt>安装时间</dt><dd>{{ new Date(selected.installedAt).toLocaleString() }}</dd></div>
        </dl>

        <section class="permissions">
          <h4>权限</h4>
          <div v-if="selected.permissions.length" class="permission-list">
            <span v-for="permission in selected.permissions" :key="permission" class="permission mono">{{ permission }}</span>
          </div>
          <p v-else>平台内置能力，随主程序权限运行。</p>
        </section>

        <div class="actions">
          <n-button type="primary" :disabled="!selected.enabled" @click="selected.toolId && emit('open', selected.toolId)">打开</n-button>
          <template v-if="selected.source === 'plugin'">
            <n-button secondary :loading="busy" @click="toggleSelected">{{ selected.enabled ? '停用' : '启用' }}</n-button>
            <n-button type="error" secondary :loading="busy" @click="requestUninstall">卸载</n-button>
          </template>
        </div>
      </article>
    </div>

    <div v-else class="history">
      <div class="history-head">
        <span>应用 / 动作</span><span>版本</span><span>检查步骤</span><span>时间 / 结果</span>
      </div>
      <div v-if="store.history.value.length" class="history-rows">
        <div v-for="record in store.history.value" :key="record.id" class="history-row">
          <div><strong class="mono">{{ record.appId }}</strong><span>{{ actionLabel(record.action) }}</span></div>
          <span class="mono">{{ record.version }}</span>
          <span class="steps">{{ record.steps.join(' · ') }}</span>
          <div><span>{{ new Date(record.occurredAt).toLocaleString() }}</span><n-tag size="small" type="success" :bordered="false">成功</n-tag></div>
        </div>
      </div>
      <n-empty v-else description="暂无安装记录" />
    </div>

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
      <template #footer><div class="modal-actions"><n-button @click="uninstallOpen = false">取消</n-button><n-button type="error" @click="continueUninstall">继续</n-button></div></template>
    </n-modal>

    <n-modal v-model:show="deleteDataConfirmOpen" preset="card" title="确认删除用户数据" class="confirm-modal" @keydown.enter.prevent>
      <n-alert type="error" :bordered="false">此操作不可恢复，独立应用也将失去这些数据。</n-alert>
      <template #footer><div class="modal-actions"><n-button @click="deleteDataConfirmOpen = false">返回</n-button><n-button type="error" :loading="busy" @click="performUninstall(true)">删除应用和数据</n-button></div></template>
    </n-modal>
  </section>
</template>

<style scoped>
.manager { height: 100%; min-height: 560px; display: flex; flex-direction: column; gap: 14px; }
.manager-header { display: flex; align-items: center; justify-content: space-between; gap: 16px; }
.manager-header h2 { margin: 0; font-size: var(--fs-xl); font-weight: 600; letter-spacing: 0; }
.manager-header p, .detail-title p, .permissions p { margin: 3px 0 0; color: var(--text-muted); font-size: var(--fs-xs); }
.correlation { display: block; margin-top: 4px; font-family: var(--font-mono); font-size: var(--fs-xxs); }
.view-tabs { display: inline-flex; align-self: flex-start; padding: 3px; background: var(--bg-elev-2); border: 1px solid var(--line); border-radius: var(--radius); }
.view-tabs button { min-width: 92px; padding: 5px 12px; border: 0; border-radius: var(--radius-sm); background: transparent; color: var(--text-muted); cursor: pointer; }
.view-tabs button.active { background: var(--bg-elevated); color: var(--text); }
.workspace { flex: 1; min-height: 0; display: grid; grid-template-columns: minmax(280px, 0.72fr) minmax(420px, 1.28fr); border: 1px solid var(--line); border-radius: var(--radius-md); overflow: hidden; background: var(--bg-elevated); }
.app-list { min-width: 0; display: flex; flex-direction: column; border-right: 1px solid var(--line); }
.filters { padding: 12px; display: grid; gap: 8px; border-bottom: 1px solid var(--line); }
.filter-row { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
.rows { overflow: auto; padding: 6px; }
.app-row { width: 100%; min-height: 64px; display: grid; grid-template-columns: 34px minmax(0, 1fr) auto; align-items: center; gap: 10px; padding: 8px; border: 0; border-radius: var(--radius); background: transparent; color: var(--text); text-align: left; cursor: pointer; }
.app-row:hover { background: var(--bg-elev-2); }
.app-row.active { background: var(--accent-soft); }
.app-glyph, .detail-glyph { display: grid; place-items: center; flex: 0 0 auto; background: var(--bg-elev-2); border: 1px solid var(--line-strong); border-radius: var(--radius); color: var(--accent); font-family: var(--font-mono); font-weight: 700; }
.app-glyph { width: 32px; height: 32px; }
.detail-glyph { width: 44px; height: 44px; font-size: var(--fs-lg); }
.row-main { min-width: 0; display: grid; gap: 2px; }
.row-main strong { font-size: var(--fs-md); }
.app-id { overflow: hidden; color: var(--text-muted); font-family: var(--font-mono); font-size: var(--fs-xxs); text-overflow: ellipsis; white-space: nowrap; }
.row-meta { display: grid; justify-items: end; gap: 4px; }
.state-dot { color: var(--accent); font-size: var(--fs-xxs); }
.state-dot.off { color: var(--text-muted); }
.loading, .empty-filter { min-height: 180px; display: flex; align-items: center; justify-content: center; gap: 8px; color: var(--text-muted); }
.empty-filter { flex-direction: column; }
.detail { min-width: 0; padding: 22px; overflow: auto; }
.detail-title { display: flex; align-items: center; gap: 12px; padding-bottom: 18px; border-bottom: 1px solid var(--line); }
.detail-title > div { flex: 1; min-width: 0; }
.detail-title h3 { margin: 0; font-size: var(--fs-lg); letter-spacing: 0; }
.metadata, .preview-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 0; margin: 16px 0; border: 1px solid var(--line); border-radius: var(--radius); overflow: hidden; }
.metadata > div, .preview-grid > div { padding: 10px 12px; border-bottom: 1px solid var(--line); }
.metadata > div:nth-child(odd), .preview-grid > div:nth-child(odd) { border-right: 1px solid var(--line); }
.metadata dt, .preview-grid dt { color: var(--text-muted); font-size: var(--fs-xxs); }
.metadata dd, .preview-grid dd { margin: 4px 0 0; overflow-wrap: anywhere; font-size: var(--fs-xs); }
.permissions { margin-top: 16px; }
.permissions h4, .permission-confirm h4 { margin: 0 0 8px; font-size: var(--fs-xs); }
.permission-list { display: flex; flex-wrap: wrap; gap: 6px; margin-bottom: 12px; }
.permission { padding: 4px 7px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--bg-elev-2); font-size: var(--fs-xxs); }
.actions { display: flex; gap: 8px; margin-top: 22px; }
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
.modal-actions { display: flex; justify-content: flex-end; gap: 8px; }
.confirm-modal p { color: var(--text-muted); line-height: 1.65; }
.mono { font-family: var(--font-mono); font-feature-settings: "tnum"; }
:global(.install-modal) { width: min(640px, 92vw); }
:global(.confirm-modal) { width: min(480px, 92vw); }
@media (max-width: 900px) {
  .workspace { grid-template-columns: 1fr; overflow: auto; }
  .app-list { min-height: 300px; border-right: 0; border-bottom: 1px solid var(--line); }
  .detail { overflow: visible; }
  .history-head { display: none; }
  .history-row { grid-template-columns: 1fr 1fr; }
}
</style>
