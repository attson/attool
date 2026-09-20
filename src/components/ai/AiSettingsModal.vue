<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue';
import {
  NButton, NCheckbox, NCheckboxGroup, NDrawer, NDrawerContent, NInput,
  NInputNumber, NModal, NPopconfirm, NSelect, useMessage,
} from 'naive-ui';
import { createAiApi } from './aiApi';
import { useAiChat } from './useAiChat';
import { isDuplicateAiModelId, parseCapabilities } from '../../types/ai';
import type { AiCapability, AiModel, AiProvider, AiProviderKind, ProviderModelInfo } from '../../types/ai';

type SettingsPane = 'connection' | 'models';
type ConfigPane = 'export' | 'import';
type ConnectionState = 'idle' | 'testing' | 'online' | 'error';

interface ModelDraft {
  id: string;
  modelId: string;
  displayName: string;
  capabilities: AiCapability[];
  temperature: number | null;
  maxTokens: number | null;
  sortOrder: number;
  createdAt: number;
  updatedAt: number;
  persisted: boolean;
}

const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{ (e: 'update:show', v: boolean): void }>();

const { providers, models, loadProviders, loadModels } = useAiChat();
const api = createAiApi();
const message = useMessage();

const kindOptions: { label: string; value: AiProviderKind }[] = [
  { label: 'OpenAI 兼容', value: 'openai' },
  { label: 'Anthropic', value: 'anthropic' },
  { label: 'Ollama', value: 'ollama' },
];
const capabilityLabels: Record<AiCapability, string> = {
  text: '文本', image: '图片', audio: '音频', video: '视频',
};
const defaultCapabilities: AiCapability[] = ['text', 'image', 'audio', 'video'];

function errText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

function kindLabel(kind: AiProviderKind): string {
  return kindOptions.find((option) => option.value === kind)?.label ?? kind;
}

const activePane = ref<SettingsPane>('models');
const selectedProviderId = ref<string | null>(null);
const draftProvider = ref<AiProvider | null>(null);
const connectionStates = reactive<Record<string, ConnectionState>>({});
const isDraftProvider = computed(() => draftProvider.value !== null);
const selectedProvider = computed<AiProvider | null>(() =>
  draftProvider.value ?? providers.value.find((provider) => provider.id === selectedProviderId.value) ?? null
);
const totalModelCount = computed(() => models.value.length);
function providerModelCount(providerId: string): number {
  return models.value.filter((model) => model.providerId === providerId).length;
}

function providerConnectionState(providerId: string): ConnectionState {
  return connectionStates[providerId] ?? 'idle';
}

const providerStatus = computed(() => {
  if (isDraftProvider.value) return { label: '未保存', tone: 'draft' };
  const state = selectedProvider.value ? connectionStates[selectedProvider.value.id] ?? 'idle' : 'idle';
  if (state === 'testing') return { label: '正在测试', tone: 'testing' };
  if (state === 'online') return { label: '连接正常', tone: 'online' };
  if (state === 'error') return { label: '连接失败', tone: 'error' };
  return { label: '尚未测试', tone: 'idle' };
});

const providerForm = reactive({
  name: '', kind: 'openai' as AiProviderKind, baseUrl: '', apiKey: '',
});

watch(selectedProvider, (provider) => {
  providerForm.name = provider?.name ?? '';
  providerForm.kind = provider?.kind ?? 'openai';
  providerForm.baseUrl = provider?.baseUrl ?? '';
  providerForm.apiKey = provider?.apiKey ?? '';
}, { immediate: true });

function selectProvider(id: string) {
  draftProvider.value = null;
  selectedProviderId.value = id;
  activePane.value = 'models';
}

function addProvider() {
  const now = Date.now();
  const id = crypto.randomUUID();
  draftProvider.value = {
    id, name: '', kind: 'openai', baseUrl: '', apiKey: '', extraJson: '{}',
    sortOrder: providers.value.length, createdAt: now, updatedAt: now,
  };
  selectedProviderId.value = id;
  activePane.value = 'connection';
}

async function saveProvider() {
  const base = selectedProvider.value;
  const name = providerForm.name.trim();
  if (!base || !name) return;
  const payload: AiProvider = {
    id: base.id,
    name,
    kind: providerForm.kind,
    baseUrl: providerForm.baseUrl.trim(),
    apiKey: providerForm.apiKey,
    extraJson: base.extraJson,
    sortOrder: base.sortOrder,
    createdAt: base.createdAt,
    updatedAt: Date.now(),
  };
  try {
    await api.upsertProvider(payload);
    draftProvider.value = null;
    connectionStates[payload.id] = 'idle';
    await loadProviders();
    syncModelDrafts();
    message.success('Provider 已保存');
  } catch (e) {
    message.error(errText(e));
  }
}

async function removeProvider(id: string) {
  if (draftProvider.value?.id === id) {
    draftProvider.value = null;
    selectedProviderId.value = providers.value[0]?.id ?? null;
    return;
  }
  try {
    await api.deleteProvider(id);
    delete connectionStates[id];
    await loadProviders();
    await loadModels();
    if (selectedProviderId.value === id) selectedProviderId.value = providers.value[0]?.id ?? null;
    syncModelDrafts();
    message.success('Provider 已删除');
  } catch (e) {
    message.error(errText(e));
  }
}

async function testConnection() {
  const provider = selectedProvider.value;
  if (!provider || isDraftProvider.value) return;
  connectionStates[provider.id] = 'testing';
  try {
    await api.fetchProviderModels(provider.id);
    connectionStates[provider.id] = 'online';
    message.success('连接成功');
  } catch (e) {
    connectionStates[provider.id] = 'error';
    message.error(errText(e));
  }
}

function toDraft(model: AiModel): ModelDraft {
  return {
    id: model.id,
    modelId: model.modelId,
    displayName: model.displayName,
    capabilities: parseCapabilities(model.capabilities),
    temperature: model.temperature,
    maxTokens: model.maxTokens,
    sortOrder: model.sortOrder,
    createdAt: model.createdAt,
    updatedAt: model.updatedAt,
    persisted: true,
  };
}

const modelDrafts = ref<ModelDraft[]>([]);
const selectedModelId = ref<string | null>(null);
const modelSearch = ref('');
const modelError = ref('');
const selectedModel = computed(() =>
  modelDrafts.value.find((draft) => draft.id === selectedModelId.value) ?? null
);
const filteredModelDrafts = computed(() => {
  const query = modelSearch.value.trim().toLocaleLowerCase();
  if (!query) return modelDrafts.value;
  return modelDrafts.value.filter((draft) =>
    draft.modelId.toLocaleLowerCase().includes(query)
    || draft.displayName.toLocaleLowerCase().includes(query)
  );
});
const batchSelecting = ref(false);
const batchSelectedIds = ref<Set<string>>(new Set());
const deletingModels = ref(false);
const allFilteredModelsSelected = computed(() =>
  filteredModelDrafts.value.length > 0
  && filteredModelDrafts.value.every((draft) => batchSelectedIds.value.has(draft.id))
);
const someFilteredModelsSelected = computed(() =>
  filteredModelDrafts.value.some((draft) => batchSelectedIds.value.has(draft.id))
);

function syncModelDrafts() {
  const providerId = isDraftProvider.value ? null : selectedProvider.value?.id ?? null;
  if (!providerId) {
    modelDrafts.value = modelDrafts.value.filter((draft) => !draft.persisted);
  } else {
    const unsaved = modelDrafts.value.filter((draft) => !draft.persisted);
    const persisted = models.value.filter((model) => model.providerId === providerId).map(toDraft);
    modelDrafts.value = [...unsaved, ...persisted];
  }
  if (!modelDrafts.value.some((draft) => draft.id === selectedModelId.value)) {
    selectedModelId.value = modelDrafts.value[0]?.id ?? null;
  }
}

watch(selectedProviderId, () => {
  modelDrafts.value = [];
  selectedModelId.value = null;
  modelSearch.value = '';
  modelError.value = '';
  batchSelecting.value = false;
  batchSelectedIds.value = new Set();
  syncModelDrafts();
});

function selectModel(id: string) {
  if (batchSelecting.value) {
    toggleBatchModel(id, !batchSelectedIds.value.has(id));
    return;
  }
  selectedModelId.value = id;
  modelError.value = '';
}

function startBatchSelection() {
  batchSelecting.value = true;
  batchSelectedIds.value = new Set();
}

function cancelBatchSelection() {
  batchSelecting.value = false;
  batchSelectedIds.value = new Set();
}

function toggleBatchModel(id: string, checked: boolean) {
  const next = new Set(batchSelectedIds.value);
  if (checked) next.add(id); else next.delete(id);
  batchSelectedIds.value = next;
}

function toggleAllFilteredModels(checked: boolean) {
  const next = new Set(batchSelectedIds.value);
  for (const draft of filteredModelDrafts.value) {
    if (checked) next.add(draft.id); else next.delete(draft.id);
  }
  batchSelectedIds.value = next;
}

function addModelDraft() {
  if (!selectedProvider.value || isDraftProvider.value) return;
  const now = Date.now();
  const draft: ModelDraft = {
    id: crypto.randomUUID(), modelId: '', displayName: '', capabilities: [...defaultCapabilities],
    temperature: null, maxTokens: null, sortOrder: modelDrafts.value.length,
    createdAt: now, updatedAt: now, persisted: false,
  };
  modelDrafts.value.unshift(draft);
  selectedModelId.value = draft.id;
  modelSearch.value = '';
  modelError.value = '';
}

async function saveModelDraft(draft: ModelDraft) {
  const provider = selectedProvider.value;
  if (!provider || isDraftProvider.value) return;
  const modelId = draft.modelId.trim();
  modelError.value = '';
  if (!modelId) {
    modelError.value = '请输入 Model ID。';
    return;
  }
  const duplicateDraft = modelDrafts.value.some((candidate) =>
    candidate.id !== draft.id && candidate.modelId.trim() === modelId
  );
  if (duplicateDraft || isDuplicateAiModelId(models.value, provider.id, modelId, draft.id)) {
    modelError.value = `该 provider 下已存在模型「${modelId}」。`;
    return;
  }
  const payload: AiModel = {
    id: draft.id,
    providerId: provider.id,
    modelId,
    displayName: draft.displayName.trim() || modelId,
    capabilities: JSON.stringify(draft.capabilities.length ? draft.capabilities : ['text']),
    temperature: draft.temperature,
    maxTokens: draft.maxTokens,
    sortOrder: draft.sortOrder,
    createdAt: draft.createdAt,
    updatedAt: Date.now(),
  };
  try {
    await api.upsertModel(payload);
    draft.persisted = true;
    await loadModels();
    syncModelDrafts();
    message.success('模型已保存');
  } catch (e) {
    message.error(errText(e));
  }
}

async function removeModelDraft(draft: ModelDraft) {
  if (!draft.persisted) {
    modelDrafts.value = modelDrafts.value.filter((candidate) => candidate.id !== draft.id);
    selectedModelId.value = modelDrafts.value[0]?.id ?? null;
    return;
  }
  try {
    await api.deleteModel(draft.id);
    await loadModels();
    syncModelDrafts();
    message.success('模型已删除');
  } catch (e) {
    message.error(errText(e));
  }
}

async function removeSelectedModels() {
  const selectedDrafts = modelDrafts.value.filter((draft) => batchSelectedIds.value.has(draft.id));
  if (!selectedDrafts.length) return;
  deletingModels.value = true;
  try {
    await Promise.all(
      selectedDrafts.filter((draft) => draft.persisted).map((draft) => api.deleteModel(draft.id))
    );
    modelDrafts.value = modelDrafts.value.filter((draft) => !batchSelectedIds.value.has(draft.id));
    await loadModels();
    syncModelDrafts();
    cancelBatchSelection();
    message.success(`已删除 ${selectedDrafts.length} 个模型`);
  } catch (e) {
    await loadModels();
    syncModelDrafts();
    batchSelectedIds.value = new Set(
      modelDrafts.value.filter((draft) => batchSelectedIds.value.has(draft.id)).map((draft) => draft.id)
    );
    message.error(errText(e));
  } finally {
    deletingModels.value = false;
  }
}

const syncing = ref(false);
const syncCandidates = ref<ProviderModelInfo[] | null>(null);
const syncSelected = ref<Set<string>>(new Set());
const syncProviderId = ref<string | null>(null);
const syncProvider = computed(() =>
  providers.value.find((provider) => provider.id === syncProviderId.value) ?? null
);
const syncExistingIds = computed(() => new Set(
  models.value
    .filter((model) => model.providerId === syncProviderId.value)
    .map((model) => model.modelId)
));
const syncImportCount = computed(() =>
  syncCandidates.value?.filter((candidate) =>
    syncSelected.value.has(candidate.id) && !syncExistingIds.value.has(candidate.id)
  ).length ?? 0
);
const syncExistingCount = computed(() =>
  syncCandidates.value?.filter((candidate) => syncExistingIds.value.has(candidate.id)).length ?? 0
);

async function syncFromServer() {
  const provider = selectedProvider.value;
  if (!provider || isDraftProvider.value) return;
  syncing.value = true;
  syncProviderId.value = provider.id;
  try {
    const response = await api.fetchProviderModels(provider.id);
    const candidates = Array.from(new Map(
      response
        .map((candidate) => ({ ...candidate, id: candidate.id.trim() }))
        .filter((candidate) => candidate.id)
        .map((candidate) => [candidate.id, candidate])
    ).values());
    syncCandidates.value = candidates;
    syncSelected.value = new Set(
      candidates.filter((candidate) => !syncExistingIds.value.has(candidate.id)).map((candidate) => candidate.id)
    );
    connectionStates[provider.id] = 'online';
  } catch (e) {
    connectionStates[provider.id] = 'error';
    message.error(errText(e));
  } finally {
    syncing.value = false;
  }
}

function closeSyncModal() {
  syncCandidates.value = null;
  syncSelected.value = new Set();
  syncProviderId.value = null;
}

function toggleSyncSelected(id: string, checked: boolean) {
  if (syncExistingIds.value.has(id)) return;
  const next = new Set(syncSelected.value);
  if (checked) next.add(id); else next.delete(id);
  syncSelected.value = next;
}

async function importSelectedModels() {
  const provider = syncProvider.value;
  if (!provider || !syncCandidates.value) return;
  const toImport = syncCandidates.value.filter((candidate) =>
    syncSelected.value.has(candidate.id) && !syncExistingIds.value.has(candidate.id)
  );
  const now = Date.now();
  const baseSortOrder = models.value.filter((model) => model.providerId === provider.id).length;
  try {
    for (const [index, candidate] of toImport.entries()) {
      await api.upsertModel({
        id: crypto.randomUUID(),
        providerId: provider.id,
        modelId: candidate.id,
        displayName: candidate.displayName || candidate.id,
        capabilities: JSON.stringify(defaultCapabilities),
        temperature: null,
        maxTokens: null,
        sortOrder: baseSortOrder + index,
        createdAt: now,
        updatedAt: now,
      });
    }
    await loadModels();
    if (selectedProviderId.value === provider.id) syncModelDrafts();
    closeSyncModal();
    message.success(`已导入 ${toImport.length} 个模型`);
  } catch (e) {
    await loadModels();
    if (selectedProviderId.value === provider.id) syncModelDrafts();
    message.error(errText(e));
  }
}

const configDrawerOpen = ref(false);
const configPane = ref<ConfigPane>('export');
const exportText = ref('');
const exporting = ref(false);
const exportIncludesKeys = ref(false);
const importText = ref('');
const importing = ref(false);

async function doExport(includeKeys: boolean) {
  exporting.value = true;
  exportIncludesKeys.value = includeKeys;
  try {
    exportText.value = await api.exportConfig(includeKeys);
  } catch (e) {
    message.error(errText(e));
  } finally {
    exporting.value = false;
  }
}

function openConfigDrawer() {
  configDrawerOpen.value = true;
  configPane.value = 'export';
  void doExport(false);
}

function selectConfigPane(pane: ConfigPane) {
  configPane.value = pane;
  if (pane === 'export' && !exportText.value) void doExport(false);
}

async function copyExport() {
  if (!exportText.value) return;
  try {
    await navigator.clipboard.writeText(exportText.value);
    message.success('JSON 已复制');
  } catch {
    message.error('复制失败，请在文本框中手动复制');
  }
}

async function doImport() {
  const json = importText.value.trim();
  if (!json) return;
  importing.value = true;
  try {
    const summary = await api.importConfig(json);
    importText.value = '';
    await loadProviders();
    await loadModels();
    if (!selectedProviderId.value && providers.value.length) selectedProviderId.value = providers.value[0].id;
    syncModelDrafts();
    configDrawerOpen.value = false;
    message.success(`导入完成：provider ${summary.providersUpserted} 个，model ${summary.modelsUpserted} 个`);
  } catch (e) {
    message.error(errText(e));
  } finally {
    importing.value = false;
  }
}

watch(() => props.show, async (visible) => {
  if (!visible) {
    draftProvider.value = null;
    closeSyncModal();
    configDrawerOpen.value = false;
    importText.value = '';
    exportText.value = '';
    modelError.value = '';
    cancelBatchSelection();
    return;
  }
  try {
    await loadProviders();
    await loadModels();
    if (!selectedProviderId.value || !providers.value.some((provider) => provider.id === selectedProviderId.value)) {
      selectedProviderId.value = providers.value[0]?.id ?? null;
    }
    syncModelDrafts();
  } catch (e) {
    message.error(errText(e));
  }
}, { immediate: true });

function updateShow(visible: boolean) {
  emit('update:show', visible);
}

function updateSyncShow(visible: boolean) {
  if (!visible) closeSyncModal();
}
</script>

<template>
  <n-modal
    :show="show"
    :mask-closable="false"
    @update:show="updateShow"
  >
    <div class="settings-modal" role="dialog" aria-modal="true" aria-label="AI 设置">
      <header class="modal-header">
        <div class="modal-title">
          <h1>AI 设置</h1>
          <span>{{ providers.length }} providers · {{ totalModelCount }} models</span>
        </div>
        <n-button class="icon-button" quaternary title="关闭" aria-label="关闭" @click="updateShow(false)">
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 6l12 12M18 6 6 18" /></svg>
        </n-button>
      </header>

      <div class="settings-workspace">
        <aside class="provider-rail">
          <div class="rail-head">
            <p>Providers</p>
            <n-button block secondary size="small" @click="addProvider">
              <template #icon>
                <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 5v14M5 12h14" /></svg>
              </template>
              新增 Provider
            </n-button>
          </div>

          <div class="provider-list">
            <button
              v-for="provider in providers"
              :key="provider.id"
              class="provider-item"
              :class="{ active: provider.id === selectedProviderId && !isDraftProvider }"
              type="button"
              @click="selectProvider(provider.id)"
            >
              <span class="provider-mark">
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <ellipse cx="12" cy="5" rx="7" ry="3" />
                  <path d="M5 5v6c0 1.7 3.1 3 7 3s7-1.3 7-3V5M5 11v6c0 1.7 3.1 3 7 3s7-1.3 7-3v-6" />
                </svg>
              </span>
              <span class="provider-copy">
                <span class="provider-name">{{ provider.name || '未命名 Provider' }}</span>
                <span class="provider-meta">{{ kindLabel(provider.kind) }} · {{ providerModelCount(provider.id) }} 个模型</span>
              </span>
              <span
                class="status-dot"
                :class="providerConnectionState(provider.id)"
                :title="providerConnectionState(provider.id) === 'online' ? '连接正常' : providerConnectionState(provider.id) === 'error' ? '连接失败' : '尚未测试'"
              />
            </button>

            <button v-if="draftProvider" class="provider-item active" type="button">
              <span class="provider-mark">
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <ellipse cx="12" cy="5" rx="7" ry="3" />
                  <path d="M5 5v6c0 1.7 3.1 3 7 3s7-1.3 7-3V5M5 11v6c0 1.7 3.1 3 7 3s7-1.3 7-3v-6" />
                </svg>
              </span>
              <span class="provider-copy">
                <span class="provider-name">{{ providerForm.name || '新 Provider' }}</span>
                <span class="provider-meta">{{ kindLabel(providerForm.kind) }} · 未保存</span>
              </span>
              <span class="status-dot draft" title="未保存" />
            </button>

            <div v-if="providers.length === 0 && !draftProvider" class="list-empty">
              暂无 Provider
            </div>
          </div>

          <div class="rail-foot">
            <n-button quaternary size="small" @click="openConfigDrawer">
              <template #icon>
                <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 7h11l-3-3M17 17H6l3 3M18 7l-3 3M6 17l3-3" /></svg>
              </template>
              导入 / 导出配置
            </n-button>
          </div>
        </aside>

        <main v-if="selectedProvider" class="settings-content">
          <header class="provider-header">
            <div class="provider-heading">
              <span class="provider-mark provider-mark-large">
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <ellipse cx="12" cy="5" rx="7" ry="3" />
                  <path d="M5 5v6c0 1.7 3.1 3 7 3s7-1.3 7-3V5M5 11v6c0 1.7 3.1 3 7 3s7-1.3 7-3v-6" />
                </svg>
              </span>
              <span class="provider-heading-copy">
                <h2>{{ isDraftProvider ? (providerForm.name || '新 Provider') : selectedProvider.name }}</h2>
                <span>
                  <span class="provider-state" :class="providerStatus.tone">{{ providerStatus.label }}</span>
                  · {{ kindLabel(providerForm.kind) }}
                </span>
              </span>
            </div>
            <div class="header-actions">
              <n-button
                size="small"
                secondary
                :loading="providerStatus.tone === 'testing'"
                :disabled="isDraftProvider"
                @click="testConnection"
              >
                <template #icon>
                  <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 13h3l2-6 4 11 2-5h5" /></svg>
                </template>
                测试连接
              </n-button>
              <n-popconfirm @positive-click="removeProvider(selectedProvider.id)">
                <template #trigger>
                  <n-button class="icon-button danger" quaternary title="删除 Provider" aria-label="删除 Provider">
                    <svg viewBox="0 0 24 24" aria-hidden="true">
                      <path d="M4 7h16M9 7V4h6v3M7 7l1 13h8l1-13M10 11v5M14 11v5" />
                    </svg>
                  </n-button>
                </template>
                {{ isDraftProvider ? '放弃这个未保存的 Provider？' : '确认删除该 Provider？其下所有模型也会一并删除。' }}
              </n-popconfirm>
            </div>
          </header>

          <nav class="settings-tabs" aria-label="Provider 设置">
            <button
              type="button"
              :class="{ active: activePane === 'connection' }"
              :aria-pressed="activePane === 'connection'"
              @click="activePane = 'connection'"
            >
              连接配置
            </button>
            <button
              type="button"
              :class="{ active: activePane === 'models' }"
              :aria-pressed="activePane === 'models'"
              @click="activePane = 'models'"
            >
              模型
            </button>
          </nav>

          <div class="pane-host">
            <section v-if="activePane === 'connection'" class="connection-pane">
              <div class="provider-form">
                <section class="form-section">
                  <h3>基本信息</h3>
                  <div class="field-grid">
                    <label class="field">
                      <span>名称</span>
                      <n-input v-model:value="providerForm.name" size="small" placeholder="例如 OpenAI 官方" />
                    </label>
                    <label class="field">
                      <span>类型</span>
                      <n-select v-model:value="providerForm.kind" :options="kindOptions" size="small" />
                    </label>
                  </div>
                </section>
                <div class="form-divider" />
                <section class="form-section">
                  <h3>连接</h3>
                  <div class="field-grid">
                    <label class="field field-full">
                      <span>Base URL</span>
                      <n-input v-model:value="providerForm.baseUrl" size="small" placeholder="https://api.openai.com/v1" />
                    </label>
                    <label class="field field-full">
                      <span>API Key</span>
                      <n-input
                        v-model:value="providerForm.apiKey"
                        size="small"
                        type="password"
                        show-password-on="click"
                        placeholder="sk-..."
                      />
                      <small>API Key 仅保存在本机。</small>
                    </label>
                  </div>
                </section>
                <div class="form-actions">
                  <n-button type="primary" size="small" :disabled="!providerForm.name.trim()" @click="saveProvider">
                    保存更改
                  </n-button>
                </div>
              </div>
            </section>

            <section v-else-if="isDraftProvider" class="empty-state">
              <div>
                <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 3v18M3 12h18" /></svg>
                <strong>先保存 Provider</strong>
                <p>完成连接配置后，即可添加或同步模型。</p>
              </div>
            </section>

            <section v-else class="models-pane">
              <div class="models-browser">
                <div class="models-toolbar">
                  <n-input v-model:value="modelSearch" size="small" clearable placeholder="搜索模型">
                    <template #prefix>
                      <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="6" /><path d="M16 16l4 4" /></svg>
                    </template>
                  </n-input>
                  <div class="toolbar-actions">
                    <n-button
                      v-if="!batchSelecting"
                      class="icon-button"
                      quaternary
                      :disabled="modelDrafts.length === 0"
                      title="批量删除"
                      aria-label="批量删除"
                      @click="startBatchSelection"
                    >
                      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h16M9 7V4h6v3M7 7l1 13h8l1-13M10 11v5M14 11v5" /></svg>
                    </n-button>
                    <n-button
                      v-if="!batchSelecting"
                      class="icon-button"
                      quaternary
                      :loading="syncing"
                      title="从服务器同步"
                      aria-label="从服务器同步"
                      @click="syncFromServer"
                    >
                      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 7h-5V2M4 17h5v5" /><path d="M18.2 11a7 7 0 0 0-11.9-4L5 8M5.8 13a7 7 0 0 0 11.9 4L19 16" /></svg>
                    </n-button>
                    <n-button
                      v-if="!batchSelecting"
                      class="icon-button"
                      quaternary
                      title="新增模型"
                      aria-label="新增模型"
                      @click="addModelDraft"
                    >
                      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 5v14M5 12h14" /></svg>
                    </n-button>
                  </div>
                  <div v-if="batchSelecting" class="batch-toolbar">
                    <n-checkbox
                      :checked="allFilteredModelsSelected"
                      :indeterminate="someFilteredModelsSelected && !allFilteredModelsSelected"
                      :disabled="filteredModelDrafts.length === 0 || deletingModels"
                      @update:checked="toggleAllFilteredModels"
                    >
                      全选
                    </n-checkbox>
                    <span>已选 {{ batchSelectedIds.size }} 项</span>
                    <div class="batch-actions">
                      <n-button size="tiny" quaternary :disabled="deletingModels" @click="cancelBatchSelection">
                        取消
                      </n-button>
                      <n-popconfirm
                        :positive-text="`删除 ${batchSelectedIds.size} 项`"
                        negative-text="取消"
                        :disabled="batchSelectedIds.size === 0"
                        @positive-click="removeSelectedModels"
                      >
                        <template #trigger>
                          <n-button
                            size="tiny"
                            type="error"
                            :loading="deletingModels"
                            :disabled="batchSelectedIds.size === 0"
                          >
                            删除
                          </n-button>
                        </template>
                        确定删除选中的 {{ batchSelectedIds.size }} 个模型吗？
                      </n-popconfirm>
                    </div>
                  </div>
                </div>

                <div class="model-list">
                  <div
                    v-for="draft in filteredModelDrafts"
                    :key="draft.id"
                    class="model-row"
                    :class="{ selecting: batchSelecting }"
                  >
                    <n-checkbox
                      v-if="batchSelecting"
                      class="model-select"
                      :checked="batchSelectedIds.has(draft.id)"
                      :disabled="deletingModels"
                      :aria-label="`选择 ${draft.displayName || draft.modelId || '未命名模型'}`"
                      @update:checked="(checked) => toggleBatchModel(draft.id, checked)"
                    />
                    <button
                      class="model-item"
                      :class="{
                        active: !batchSelecting && draft.id === selectedModelId,
                        selected: batchSelecting && batchSelectedIds.has(draft.id),
                      }"
                      type="button"
                      :disabled="deletingModels"
                      @click="selectModel(draft.id)"
                    >
                      <span class="model-main">
                        <strong>{{ draft.displayName || draft.modelId || '未命名模型' }}</strong>
                        <span>{{ draft.modelId || '等待填写 Model ID' }}</span>
                      </span>
                      <span class="capability-list">
                        <span v-for="capability in draft.capabilities" :key="capability">
                          {{ capabilityLabels[capability] }}
                        </span>
                      </span>
                    </button>
                  </div>
                  <div v-if="filteredModelDrafts.length === 0" class="list-empty">
                    {{ modelDrafts.length ? '没有匹配的模型' : '还没有模型' }}
                  </div>
                </div>
              </div>

              <div v-if="selectedModel" class="model-editor">
                <header class="editor-heading">
                  <div>
                    <h3>{{ selectedModel.persisted ? '编辑模型' : '新增模型' }}</h3>
                    <p>Model ID 在同一 Provider 下必须唯一。</p>
                  </div>
                </header>
                <div class="editor-form">
                  <label class="field">
                    <span>Model ID</span>
                    <n-input v-model:value="selectedModel.modelId" size="small" placeholder="例如 gpt-4o" />
                  </label>
                  <label class="field">
                    <span>显示名称</span>
                    <n-input v-model:value="selectedModel.displayName" size="small" placeholder="未填写时使用 Model ID" />
                  </label>
                  <div class="field">
                    <span>能力</span>
                    <n-checkbox-group v-model:value="selectedModel.capabilities" class="capability-grid">
                      <n-checkbox v-for="(label, capability) in capabilityLabels" :key="capability" :value="capability">
                        {{ label }}
                      </n-checkbox>
                    </n-checkbox-group>
                  </div>
                  <div class="field-grid">
                    <label class="field">
                      <span>Temperature</span>
                      <n-input-number
                        v-model:value="selectedModel.temperature"
                        size="small"
                        :precision="1"
                        :min="0"
                        :max="2"
                        :step="0.1"
                        placeholder="默认"
                        clearable
                      />
                    </label>
                    <label class="field">
                      <span>Max Tokens</span>
                      <n-input-number
                        v-model:value="selectedModel.maxTokens"
                        size="small"
                        :min="1"
                        placeholder="默认"
                        clearable
                      />
                    </label>
                  </div>
                  <div v-if="modelError" class="validation-error">{{ modelError }}</div>
                  <div class="editor-actions">
                    <n-popconfirm v-if="selectedModel.persisted" @positive-click="removeModelDraft(selectedModel)">
                      <template #trigger>
                        <n-button quaternary type="error" size="small">删除模型</n-button>
                      </template>
                      确认删除该模型？
                    </n-popconfirm>
                    <n-button v-else quaternary size="small" @click="removeModelDraft(selectedModel)">取消</n-button>
                    <n-button
                      type="primary"
                      size="small"
                      :disabled="!selectedModel.modelId.trim()"
                      @click="saveModelDraft(selectedModel)"
                    >
                      保存模型
                    </n-button>
                  </div>
                </div>
              </div>
              <div v-else class="empty-state">
                <div>
                  <svg viewBox="0 0 24 24" aria-hidden="true"><ellipse cx="12" cy="5" rx="7" ry="3" /><path d="M5 5v12c0 1.7 3.1 3 7 3s7-1.3 7-3V5" /></svg>
                  <strong>选择一个模型</strong>
                  <p>从左侧列表选择模型，或新增、同步模型。</p>
                </div>
              </div>
            </section>
          </div>
        </main>

        <main v-else class="settings-content empty-content">
          <div class="empty-state">
            <div>
              <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 5v14M5 12h14" /></svg>
              <strong>添加第一个 Provider</strong>
              <p>配置服务地址和 API Key 后，即可管理模型。</p>
              <n-button type="primary" size="small" @click="addProvider">新增 Provider</n-button>
            </div>
          </div>
        </main>
      </div>
    </div>
  </n-modal>

  <n-modal
    :show="syncCandidates !== null"
    preset="card"
    class="sync-modal"
    title="从服务器同步模型"
    :mask-closable="false"
    @update:show="updateSyncShow"
  >
    <p class="sync-summary">
      {{ syncProvider?.name || '当前 Provider' }} 返回 {{ syncCandidates?.length || 0 }} 个模型，
      其中 {{ syncExistingCount }} 个已添加。
    </p>
    <div class="sync-list">
      <label
        v-for="candidate in syncCandidates || []"
        :key="candidate.id"
        class="sync-row"
        :class="{ existing: syncExistingIds.has(candidate.id) }"
      >
        <n-checkbox
          :checked="syncExistingIds.has(candidate.id) || syncSelected.has(candidate.id)"
          :disabled="syncExistingIds.has(candidate.id)"
          @update:checked="(checked: boolean) => toggleSyncSelected(candidate.id, checked)"
        />
        <span class="sync-copy">
          <strong>{{ candidate.displayName || candidate.id }}</strong>
          <span>{{ candidate.id }}</span>
        </span>
        <span v-if="syncExistingIds.has(candidate.id)" class="sync-badge">已添加</span>
      </label>
      <div v-if="syncCandidates?.length === 0" class="list-empty">服务器未返回可用模型</div>
    </div>
    <template #footer>
      <div class="sync-actions">
        <n-button size="small" @click="closeSyncModal">取消</n-button>
        <n-button type="primary" size="small" :disabled="syncImportCount === 0" @click="importSelectedModels">
          导入选中（{{ syncImportCount }}）
        </n-button>
      </div>
    </template>
  </n-modal>

  <n-drawer :show="configDrawerOpen" width="min(460px, 92vw)" placement="right" @update:show="configDrawerOpen = $event">
    <n-drawer-content title="配置迁移" closable>
      <nav class="settings-tabs drawer-tabs" aria-label="配置迁移">
        <button type="button" :class="{ active: configPane === 'export' }" @click="selectConfigPane('export')">导出</button>
        <button type="button" :class="{ active: configPane === 'import' }" @click="selectConfigPane('import')">导入</button>
      </nav>
      <section v-if="configPane === 'export'" class="config-pane">
        <p>导出所有 Provider 与模型配置。默认不包含 API Key。</p>
        <n-input
          v-model:value="exportText"
          type="textarea"
          readonly
          placeholder="正在生成导出内容"
          :autosize="{ minRows: 14, maxRows: 24 }"
        />
      </section>
      <section v-else class="config-pane">
        <p>粘贴此前导出的 JSON。导入时会合并配置，并跳过同一 Provider 下的重复模型。</p>
        <n-input
          v-model:value="importText"
          type="textarea"
          placeholder="粘贴配置 JSON"
          :autosize="{ minRows: 14, maxRows: 24 }"
        />
      </section>
      <template #footer>
        <div v-if="configPane === 'export'" class="drawer-actions">
          <n-button size="small" :loading="exporting" @click="doExport(!exportIncludesKeys)">
            {{ exportIncludesKeys ? '切换为脱敏' : '包含 API Key' }}
          </n-button>
          <n-button type="primary" size="small" :disabled="!exportText" @click="copyExport">复制 JSON</n-button>
        </div>
        <div v-else class="drawer-actions">
          <n-button size="small" @click="configDrawerOpen = false">取消</n-button>
          <n-button type="primary" size="small" :loading="importing" :disabled="!importText.trim()" @click="doImport">
            导入配置
          </n-button>
        </div>
      </template>
    </n-drawer-content>
  </n-drawer>
</template>

<style scoped>
.settings-modal {
  width: min(1080px, calc(100vw - 32px));
  height: min(720px, calc(100vh - 32px));
  display: grid;
  grid-template-rows: 56px minmax(0, 1fr);
  overflow: hidden;
  border: 1px solid var(--line);
  border-radius: var(--radius-lg);
  background: var(--bg-overlay);
  box-shadow: var(--shadow-pop);
  color: var(--text);
}

.modal-header,
.provider-header,
.header-actions,
.toolbar-actions,
.form-actions,
.editor-actions,
.sync-actions,
.drawer-actions {
  display: flex;
  align-items: center;
}

.modal-header {
  justify-content: space-between;
  gap: 16px;
  padding: 0 14px 0 20px;
  border-bottom: 1px solid var(--line);
}

.modal-title {
  min-width: 0;
  display: flex;
  align-items: baseline;
  gap: 10px;
}

.modal-title h1 {
  margin: 0;
  font-size: var(--fs-xl);
  font-weight: 650;
}

.modal-title span {
  color: var(--text-faint);
  font-size: var(--fs-xs);
}

.settings-workspace {
  min-height: 0;
  display: grid;
  grid-template-columns: 236px minmax(0, 1fr);
}

.provider-rail {
  min-height: 0;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr) auto;
  border-right: 1px solid var(--line);
  background: var(--bg-elevated);
}

.rail-head {
  padding: 16px 12px 10px;
}

.rail-head p {
  margin: 0 0 8px;
  color: var(--text-muted);
  font-size: var(--fs-xxs);
  font-weight: 650;
  text-transform: uppercase;
}

.provider-list {
  min-height: 0;
  overflow-y: auto;
  padding: 4px 8px 12px;
}

.provider-item {
  width: 100%;
  display: grid;
  grid-template-columns: 30px minmax(0, 1fr) 12px;
  align-items: center;
  gap: 9px;
  padding: 9px 8px;
  border: 1px solid transparent;
  border-radius: var(--radius-md);
  background: transparent;
  color: var(--text);
  font: inherit;
  letter-spacing: 0;
  text-align: left;
  cursor: pointer;
  transition: background var(--motion-fast), border-color var(--motion-fast);
}

.provider-item + .provider-item { margin-top: 3px; }
.provider-item:hover { background: var(--bg-elev-2); }
.provider-item.active { border-color: var(--accent-line); background: var(--accent-soft); }

.provider-mark {
  width: 30px;
  height: 30px;
  display: grid;
  place-items: center;
  border: 1px solid var(--line-strong);
  border-radius: var(--radius);
  background: var(--bg-overlay);
  color: var(--text-muted);
}

.provider-item.active .provider-mark,
.provider-mark-large {
  border-color: var(--accent-line);
  color: var(--accent);
}

.provider-mark svg,
.icon-button svg,
.models-toolbar svg,
.empty-state svg,
.n-button svg {
  width: 16px;
  height: 16px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.8;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.provider-copy,
.model-main,
.sync-copy {
  min-width: 0;
  display: grid;
}

.provider-copy { gap: 2px; }
.provider-name { overflow: hidden; font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
.provider-meta { color: var(--text-muted); font-size: var(--fs-xxs); }

.status-dot {
  width: 7px;
  height: 7px;
  border-radius: var(--radius-pill);
  background: var(--text-faint);
}

.status-dot.online { background: var(--accent); }
.status-dot.error { background: var(--error); }
.status-dot.testing,
.status-dot.draft { background: var(--warning); }

.rail-foot {
  padding: 10px 8px 12px;
  border-top: 1px solid var(--line);
}

.rail-foot :deep(.n-button) { width: 100%; justify-content: flex-start; }

.settings-content {
  min-width: 0;
  min-height: 0;
  display: grid;
  grid-template-rows: auto auto minmax(0, 1fr);
}

.provider-header {
  min-height: 76px;
  justify-content: space-between;
  gap: 16px;
  padding: 14px 18px;
}

.provider-heading {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 12px;
}

.provider-mark-large { width: 38px; height: 38px; flex: none; }
.provider-heading-copy { min-width: 0; display: grid; gap: 3px; }
.provider-heading-copy h2 { margin: 0; overflow: hidden; font-size: var(--fs-lg); font-weight: 650; text-overflow: ellipsis; white-space: nowrap; }
.provider-heading-copy > span { color: var(--text-muted); font-size: var(--fs-xs); }
.provider-state.online { color: var(--accent); }
.provider-state.error { color: var(--error); }
.provider-state.testing,
.provider-state.draft { color: var(--warning); }
.header-actions { gap: 8px; flex: none; }

.icon-button { width: 30px; min-width: 30px; padding: 0; }
.icon-button.danger:hover { color: var(--error); }

.settings-tabs {
  display: flex;
  gap: 18px;
  padding: 0 18px;
  border-bottom: 1px solid var(--line);
}

.settings-tabs button {
  position: relative;
  height: 38px;
  padding: 0 2px;
  border: 0;
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  letter-spacing: 0;
  cursor: pointer;
}

.settings-tabs button:hover,
.settings-tabs button.active { color: var(--text); }
.settings-tabs button.active { font-weight: 600; }
.settings-tabs button.active::after {
  content: "";
  position: absolute;
  right: 0;
  bottom: -1px;
  left: 0;
  height: 2px;
  background: var(--accent);
}

.pane-host { min-height: 0; overflow: hidden; }
.connection-pane { height: 100%; overflow-y: auto; padding: 20px; }
.provider-form { max-width: 760px; display: grid; gap: 18px; }
.form-section { display: grid; gap: 12px; }
.form-section h3 { margin: 0; font-size: var(--fs-xs); font-weight: 650; }
.field-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }
.field { min-width: 0; display: grid; gap: 6px; color: var(--text-muted); font-size: var(--fs-xxs); }
.field-full { grid-column: 1 / -1; }
.field small { color: var(--text-faint); font-size: var(--fs-xxs); }
.form-divider { height: 1px; background: var(--line); }
.form-actions { justify-content: flex-end; }

.models-pane {
  height: 100%;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(270px, 42%) minmax(0, 1fr);
}

.models-browser {
  min-width: 0;
  min-height: 0;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  border-right: 1px solid var(--line);
}

.models-toolbar {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 8px;
  padding: 14px;
  border-bottom: 1px solid var(--line);
}

.toolbar-actions { gap: 6px; }
.batch-toolbar {
  grid-column: 1 / -1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  padding-top: 4px;
  color: var(--text-muted);
  font-size: var(--fs-xxs);
}
.batch-actions { margin-left: auto; display: flex; align-items: center; gap: 4px; }
.model-list { min-height: 0; overflow-y: auto; padding: 6px; }
.model-row { display: grid; grid-template-columns: minmax(0, 1fr); align-items: center; }
.model-row.selecting { grid-template-columns: auto minmax(0, 1fr); gap: 4px; }
.model-row + .model-row { margin-top: 2px; }
.model-select { padding-left: 6px; }

.model-item {
  width: 100%;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 10px;
  padding: 10px;
  border: 1px solid transparent;
  border-radius: var(--radius);
  background: transparent;
  color: var(--text);
  font: inherit;
  letter-spacing: 0;
  text-align: left;
  cursor: pointer;
  transition: background var(--motion-fast), border-color var(--motion-fast);
}

.model-item:hover { background: var(--bg-elev-2); }
.model-item.active,
.model-item.selected { border-color: var(--accent-line); background: var(--accent-soft); }
.model-item:disabled { cursor: default; opacity: 0.65; }
.model-main { gap: 4px; }
.model-main strong,
.model-main > span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.model-main strong { font-weight: 600; }
.model-main > span { color: var(--text-muted); font-family: var(--font-mono); font-size: var(--fs-xxs); }

.capability-list { display: flex; align-items: center; flex-wrap: wrap; justify-content: flex-end; gap: 4px; }
.capability-list span {
  padding: 2px 6px;
  border-radius: var(--radius-sm);
  background: var(--bg-elev-2);
  color: var(--text-muted);
  font-size: var(--fs-xxs);
}

.model-item.active .capability-list span { background: var(--bg-elevated); }
.model-editor { min-width: 0; min-height: 0; overflow-y: auto; padding: 18px 20px; }
.editor-heading { margin-bottom: 18px; }
.editor-heading h3 { margin: 0 0 4px; font-size: var(--fs-md); }
.editor-heading p { margin: 0; color: var(--text-muted); font-size: var(--fs-xxs); }
.editor-form { display: grid; gap: 16px; }

.capability-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px;
}

.capability-grid :deep(.n-checkbox) {
  min-height: 34px;
  margin: 0;
  padding: 0 9px;
  border: 1px solid var(--line);
  border-radius: var(--radius);
  background: var(--bg-elevated);
}

.capability-grid :deep(.n-checkbox.n-checkbox--checked) {
  border-color: var(--accent-line);
  background: var(--accent-soft);
}

.validation-error {
  padding: 8px 10px;
  border: 1px solid var(--error);
  border-radius: var(--radius);
  background: var(--error-soft);
  color: var(--error);
  font-size: var(--fs-xxs);
}

.editor-actions { justify-content: space-between; gap: 10px; }
.list-empty { padding: 24px 12px; color: var(--text-muted); font-size: var(--fs-xs); text-align: center; }

.empty-content { grid-template-rows: minmax(0, 1fr); }
.empty-state { height: 100%; display: grid; place-items: center; padding: 30px; color: var(--text-muted); text-align: center; }
.empty-state > div { display: grid; justify-items: center; gap: 8px; }
.empty-state svg { width: 28px; height: 28px; color: var(--text-faint); }
.empty-state strong { color: var(--text); font-size: var(--fs-md); }
.empty-state p { max-width: 260px; margin: 0; font-size: var(--fs-xxs); line-height: 1.5; }

.sync-modal { width: min(520px, calc(100vw - 40px)); }
.sync-summary { margin: 0 0 10px; color: var(--text-muted); font-size: var(--fs-xs); }
.sync-list { max-height: min(420px, calc(100vh - 230px)); overflow-y: auto; border-top: 1px solid var(--line); }
.sync-row {
  min-height: 52px;
  display: grid;
  grid-template-columns: 18px minmax(0, 1fr) auto;
  align-items: center;
  gap: 10px;
  padding: 8px 2px;
  border-bottom: 1px solid var(--line);
  cursor: pointer;
}

.sync-row:hover { background: var(--bg-elev-2); }
.sync-row.existing { color: var(--text-muted); cursor: default; }
.sync-copy { gap: 3px; }
.sync-copy strong,
.sync-copy span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.sync-copy span { color: var(--text-muted); font-family: var(--font-mono); font-size: var(--fs-xxs); }
.sync-badge { padding: 2px 6px; border-radius: var(--radius-sm); background: var(--accent-soft); color: var(--accent); font-size: var(--fs-xxs); }
.sync-actions,
.drawer-actions { justify-content: flex-end; gap: 8px; width: 100%; }

.drawer-tabs { margin: -4px -24px 0; padding: 0 24px; }
.config-pane { display: grid; gap: 12px; padding-top: 18px; }
.config-pane p { margin: 0; color: var(--text-muted); font-size: var(--fs-xs); line-height: 1.6; }
.config-pane :deep(textarea) { font-family: var(--font-mono); }

@media (max-width: 820px) {
  .settings-modal { width: calc(100vw - 20px); height: calc(100vh - 20px); }
  .settings-workspace { grid-template-columns: 198px minmax(0, 1fr); }
  .provider-header { padding: 12px 14px; }
  .models-pane { grid-template-columns: minmax(220px, 40%) minmax(0, 1fr); }
  .model-editor { padding: 16px; }
  .field-grid { grid-template-columns: minmax(0, 1fr); }
  .field-full { grid-column: auto; }
  .capability-grid { grid-template-columns: minmax(0, 1fr); }
  .capability-list { display: none; }
}

@media (max-height: 560px) {
  .settings-modal { height: calc(100vh - 16px); }
  .provider-header { min-height: 64px; padding-block: 8px; }
  .connection-pane { padding-block: 14px; }
}
</style>
