<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue';
import {
  NButton, NCheckbox, NCheckboxGroup, NDivider, NInput, NInputNumber,
  NModal, NPopconfirm, NScrollbar, NSelect, useMessage,
} from 'naive-ui';
import { createAiApi } from './aiApi';
import { useAiChat } from './useAiChat';
import { isDuplicateAiModelId, parseCapabilities } from '../../types/ai';
import type { AiCapability, AiModel, AiProvider, AiProviderKind, ProviderModelInfo } from '../../types/ai';

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

function errText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

// ---- providers ----

const selectedProviderId = ref<string | null>(null);
// 未保存的新 provider 单独存放：left 列表和 providers store 都还没有它，
// 保存成功后清空并回落到 providers store 里的同 id 记录。
const draftProvider = ref<AiProvider | null>(null);
const isDraftProvider = computed(() => draftProvider.value !== null);

const selectedProvider = computed<AiProvider | null>(() =>
  draftProvider.value ?? providers.value.find((p) => p.id === selectedProviderId.value) ?? null
);

const providerForm = reactive({
  name: '', kind: 'openai' as AiProviderKind, baseUrl: '', apiKey: '',
});

watch(selectedProvider, (p) => {
  providerForm.name = p?.name ?? '';
  providerForm.kind = p?.kind ?? 'openai';
  providerForm.baseUrl = p?.baseUrl ?? '';
  providerForm.apiKey = p?.apiKey ?? '';
}, { immediate: true });

function selectProvider(id: string) {
  draftProvider.value = null;
  selectedProviderId.value = id;
}

function addProvider() {
  const now = Date.now();
  const id = crypto.randomUUID();
  draftProvider.value = {
    id, name: '', kind: 'openai', baseUrl: '', apiKey: '', extraJson: '{}',
    sortOrder: providers.value.length, createdAt: now, updatedAt: now,
  };
  selectedProviderId.value = id;
}

async function saveProvider() {
  const base = selectedProvider.value;
  if (!base) return;
  const payload: AiProvider = {
    id: base.id,
    name: providerForm.name.trim(),
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
    await loadProviders();
    syncModelDrafts();
    message.success('已保存 provider');
  } catch (e) {
    message.error(errText(e));
  }
}

async function removeProvider(id: string) {
  try {
    await api.deleteProvider(id);
    if (selectedProviderId.value === id) {
      selectedProviderId.value = null;
      draftProvider.value = null;
    }
    await loadProviders();
    await loadModels();
    syncModelDrafts();
    message.success('已删除 provider');
  } catch (e) {
    message.error(errText(e));
  }
}

// ---- models ----

interface ModelDraft {
  id: string; modelId: string; displayName: string;
  capabilities: AiCapability[]; temperature: number | null; maxTokens: number | null;
  sortOrder: number; createdAt: number; updatedAt: number;
  persisted: boolean;
}

function toDraft(m: AiModel): ModelDraft {
  return {
    id: m.id, modelId: m.modelId, displayName: m.displayName,
    capabilities: parseCapabilities(m.capabilities),
    temperature: m.temperature, maxTokens: m.maxTokens,
    sortOrder: m.sortOrder, createdAt: m.createdAt, updatedAt: m.updatedAt,
    persisted: true,
  };
}

const modelDrafts = ref<ModelDraft[]>([]);

function syncModelDrafts() {
  const pid = isDraftProvider.value ? null : selectedProvider.value?.id ?? null;
  if (!pid) { modelDrafts.value = modelDrafts.value.filter((d) => !d.persisted); return; }
  const unsaved = modelDrafts.value.filter((d) => !d.persisted);
  const persisted = models.value.filter((m) => m.providerId === pid).map(toDraft);
  modelDrafts.value = [...persisted, ...unsaved];
}

watch(selectedProviderId, () => { modelDrafts.value = []; syncModelDrafts(); });

function addModelDraft() {
  if (!selectedProvider.value || isDraftProvider.value) return;
  const now = Date.now();
  modelDrafts.value.push({
    id: crypto.randomUUID(), modelId: '', displayName: '', capabilities: ['text'],
    temperature: null, maxTokens: null, sortOrder: modelDrafts.value.length,
    createdAt: now, updatedAt: now, persisted: false,
  });
}

async function saveModelDraft(draft: ModelDraft) {
  const provider = selectedProvider.value;
  if (!provider || isDraftProvider.value) return;
  const modelId = draft.modelId.trim();
  if (!modelId) {
    message.error('请输入模型 ID');
    return;
  }
  if (isDuplicateAiModelId(models.value, provider.id, modelId, draft.id)) {
    message.error(`该 provider 下已存在模型「${modelId}」`);
    return;
  }
  const payload: AiModel = {
    id: draft.id, providerId: provider.id,
    modelId, displayName: draft.displayName.trim() || modelId,
    capabilities: JSON.stringify(draft.capabilities.length ? draft.capabilities : ['text']),
    temperature: draft.temperature, maxTokens: draft.maxTokens,
    sortOrder: draft.sortOrder, createdAt: draft.createdAt, updatedAt: Date.now(),
  };
  try {
    await api.upsertModel(payload);
    await loadModels();
    syncModelDrafts();
    message.success('已保存模型');
  } catch (e) {
    message.error(errText(e));
  }
}

async function removeModelDraft(draft: ModelDraft) {
  if (!draft.persisted) {
    modelDrafts.value = modelDrafts.value.filter((d) => d.id !== draft.id);
    return;
  }
  try {
    await api.deleteModel(draft.id);
    await loadModels();
    syncModelDrafts();
    message.success('已删除模型');
  } catch (e) {
    message.error(errText(e));
  }
}

// ---- sync models from server ----

const syncing = ref(false);
const syncCandidates = ref<ProviderModelInfo[] | null>(null);
const syncSelected = ref<Set<string>>(new Set());

async function syncFromServer() {
  const provider = selectedProvider.value;
  if (!provider || isDraftProvider.value) return;
  syncing.value = true;
  try {
    syncCandidates.value = await api.fetchProviderModels(provider.id);
    syncSelected.value = new Set(syncCandidates.value.map((m) => m.id));
  } catch (e) {
    message.error(errText(e));
  } finally {
    syncing.value = false;
  }
}

function toggleSyncSelected(id: string, checked: boolean) {
  const next = new Set(syncSelected.value);
  if (checked) next.add(id); else next.delete(id);
  syncSelected.value = next;
}

async function importSelectedModels() {
  const provider = selectedProvider.value;
  if (!provider || !syncCandidates.value) return;
  const toImport = syncCandidates.value.filter((m) => syncSelected.value.has(m.id));
  const now = Date.now();
  // 按 (providerId, modelId) 复用已有行的 id：ai_models 的唯一约束在 id 上，不在
  // (provider_id, model_id) 上，重新同步时若每次都发新 id 会插出重复行。
  const existingByModelId = new Map<string, string>();
  for (const existing of models.value) {
    if (existing.providerId === provider.id) existingByModelId.set(existing.modelId, existing.id);
  }
  try {
    for (const m of toImport) {
      const rowId = existingByModelId.get(m.id) ?? crypto.randomUUID();
      await api.upsertModel({
        id: rowId, providerId: provider.id,
        modelId: m.id, displayName: m.displayName || m.id,
        capabilities: JSON.stringify(['text']), temperature: null, maxTokens: null,
        sortOrder: modelDrafts.value.length, createdAt: now, updatedAt: now,
      });
    }
    await loadModels();
    syncModelDrafts();
    syncCandidates.value = null;
    message.success(`已导入 ${toImport.length} 个模型`);
  } catch (e) {
    message.error(errText(e));
  }
}

// ---- import / export config ----

const exportText = ref('');
const exporting = ref(false);

async function doExport(includeKeys: boolean) {
  exporting.value = true;
  try {
    exportText.value = await api.exportConfig(includeKeys);
  } catch (e) {
    message.error(errText(e));
  } finally {
    exporting.value = false;
  }
}

const importText = ref('');
const importing = ref(false);

async function doImport() {
  const json = importText.value.trim();
  if (!json) return;
  importing.value = true;
  try {
    const summary = await api.importConfig(json);
    message.success(`导入完成：provider ${summary.providersUpserted} 个，model ${summary.modelsUpserted} 个`);
    importText.value = '';
    await loadProviders();
    await loadModels();
    syncModelDrafts();
  } catch (e) {
    message.error(errText(e));
  } finally {
    importing.value = false;
  }
}

// ---- modal lifecycle ----

watch(() => props.show, async (v) => {
  if (!v) {
    // 关闭时清掉未保存/一次性状态，避免 T13 反复 toggle show 时旧草稿重新浮现。
    draftProvider.value = null;
    syncCandidates.value = null;
    syncSelected.value = new Set();
    importText.value = '';
    exportText.value = '';
    return;
  }
  try {
    await loadProviders();
    await loadModels();
    syncModelDrafts();
  } catch (e) {
    message.error(errText(e));
  }
});

function updateShow(v: boolean) {
  emit('update:show', v);
}
</script>

<template>
  <n-modal
    :show="show"
    preset="card"
    style="width: min(960px, 94vw)"
    title="AI 设置"
    @update:show="updateShow"
  >
    <div class="settings-body">
      <div class="panel left">
        <n-scrollbar style="max-height: 52vh">
          <div class="provider-list">
            <div
              v-for="p in providers"
              :key="p.id"
              class="provider-row"
              :class="{ active: p.id === selectedProviderId && !isDraftProvider }"
              @click="selectProvider(p.id)"
            >
              <div class="p-main">
                <span class="p-name">{{ p.name || '(未命名)' }}</span>
                <span class="p-kind">{{ p.kind }}</span>
              </div>
              <n-popconfirm @positive-click="removeProvider(p.id)">
                <template #trigger>
                  <n-button size="tiny" quaternary @click.stop>删除</n-button>
                </template>
                确认删除该 provider？其下所有模型也会一并删除。
              </n-popconfirm>
            </div>
            <div v-if="draftProvider" class="provider-row active">
              <div class="p-main">
                <span class="p-name">{{ draftProvider.name || '新 Provider' }}</span>
                <span class="p-kind unsaved">未保存</span>
              </div>
            </div>
            <div v-if="providers.length === 0 && !draftProvider" class="empty small">暂无 provider</div>
          </div>
        </n-scrollbar>
        <n-button block secondary size="small" @click="addProvider">+ 新增 provider</n-button>
      </div>

      <div class="panel right">
        <n-scrollbar style="max-height: 52vh">
          <template v-if="selectedProvider">
            <div class="provider-form">
              <label>
                <span>名称</span>
                <n-input v-model:value="providerForm.name" size="small" placeholder="例如 OpenAI 官方" />
              </label>
              <label>
                <span>类型</span>
                <n-select v-model:value="providerForm.kind" :options="kindOptions" size="small" />
              </label>
              <label>
                <span>Base URL</span>
                <n-input v-model:value="providerForm.baseUrl" size="small" placeholder="https://api.openai.com/v1" />
              </label>
              <label>
                <span>API Key</span>
                <n-input
                  v-model:value="providerForm.apiKey"
                  size="small"
                  type="password"
                  show-password-on="click"
                  placeholder="sk-..."
                />
              </label>
              <div class="form-actions">
                <n-button size="small" type="primary" :disabled="!providerForm.name.trim()" @click="saveProvider">
                  保存 provider
                </n-button>
              </div>
            </div>

            <n-divider />

            <div v-if="isDraftProvider" class="empty small">先保存 provider，才能管理模型。</div>
            <div v-else class="models-section">
              <div class="models-head">
                <h4>模型</h4>
                <div class="models-actions">
                  <n-button size="tiny" secondary @click="addModelDraft">+ 新增模型</n-button>
                  <n-button size="tiny" secondary :loading="syncing" @click="syncFromServer">从服务器同步</n-button>
                </div>
              </div>

              <div v-if="syncCandidates" class="sync-panel">
                <div class="sync-head">
                  <span>从服务器获取到 {{ syncCandidates.length }} 个模型</span>
                  <n-button size="tiny" quaternary @click="syncCandidates = null">取消</n-button>
                </div>
                <n-scrollbar style="max-height: 160px">
                  <div v-for="m in syncCandidates" :key="m.id" class="sync-row">
                    <n-checkbox
                      :checked="syncSelected.has(m.id)"
                      @update:checked="(v: boolean) => toggleSyncSelected(m.id, v)"
                    >
                      {{ m.displayName }} <span class="mono muted">({{ m.id }})</span>
                    </n-checkbox>
                  </div>
                </n-scrollbar>
                <n-button
                  size="small" type="primary" :disabled="syncSelected.size === 0"
                  @click="importSelectedModels"
                >
                  导入选中（{{ syncSelected.size }}）
                </n-button>
              </div>

              <div v-if="modelDrafts.length === 0" class="empty small">
                还没有模型 —— 手动添加，或点右上「从服务器同步」
              </div>
              <div v-for="draft in modelDrafts" :key="draft.id" class="model-card">
                <div class="model-row">
                  <n-input v-model:value="draft.modelId" size="small" placeholder="model id，例如 gpt-4o" />
                  <n-input v-model:value="draft.displayName" size="small" placeholder="显示名称" />
                </div>
                <n-checkbox-group v-model:value="draft.capabilities" class="cap-group">
                  <n-checkbox value="text">文本</n-checkbox>
                  <n-checkbox value="image">图片</n-checkbox>
                  <n-checkbox value="audio">音频</n-checkbox>
                  <n-checkbox value="video">视频</n-checkbox>
                </n-checkbox-group>
                <div class="model-row">
                  <label class="inline">
                    <span>temperature</span>
                    <n-input-number
                      v-model:value="draft.temperature" size="small" :precision="1"
                      :min="0" :max="2" :step="0.1" placeholder="默认" clearable
                    />
                  </label>
                  <label class="inline">
                    <span>maxTokens</span>
                    <n-input-number v-model:value="draft.maxTokens" size="small" :min="1" placeholder="默认" clearable />
                  </label>
                </div>
                <div class="model-actions">
                  <n-button
                    size="tiny" type="primary" :disabled="!draft.modelId.trim()"
                    @click="saveModelDraft(draft)"
                  >
                    保存
                  </n-button>
                  <n-popconfirm v-if="draft.persisted" @positive-click="removeModelDraft(draft)">
                    <template #trigger>
                      <n-button size="tiny" quaternary type="error">删除</n-button>
                    </template>
                    确认删除该模型？
                  </n-popconfirm>
                  <n-button v-else size="tiny" quaternary @click="removeModelDraft(draft)">移除</n-button>
                </div>
              </div>
            </div>
          </template>
          <div v-else class="empty">选择一个提供商，或新建一个</div>
        </n-scrollbar>
      </div>
    </div>

    <n-divider />

    <div class="io-section">
      <div class="io-col">
        <h4>导出配置</h4>
        <div class="io-actions">
          <n-button size="tiny" secondary :loading="exporting" @click="doExport(true)">含 Key</n-button>
          <n-button size="tiny" secondary :loading="exporting" @click="doExport(false)">脱敏</n-button>
        </div>
        <n-input
          v-model:value="exportText" type="textarea" readonly placeholder="点击上方按钮生成导出内容"
          :autosize="{ minRows: 4, maxRows: 8 }"
        />
      </div>
      <div class="io-col">
        <h4>导入配置</h4>
        <n-input
          v-model:value="importText" type="textarea" placeholder="粘贴导出的 JSON"
          :autosize="{ minRows: 4, maxRows: 8 }"
        />
        <div class="io-actions">
          <n-button size="small" type="primary" :loading="importing" :disabled="!importText.trim()" @click="doImport">
            导入
          </n-button>
        </div>
      </div>
    </div>
  </n-modal>
</template>

<style scoped>
.settings-body {
  display: grid;
  grid-template-columns: 30% 1fr;
  gap: 14px;
  min-height: 320px;
}
.panel { display: flex; flex-direction: column; gap: 8px; min-width: 0; }

.provider-list { display: grid; gap: 4px; padding-right: 4px; }
.provider-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
  padding: 6px 8px;
  border-radius: var(--radius-sm);
  border: 1px solid transparent;
  cursor: pointer;
}
.provider-row:hover { background: var(--bg-elev-2); }
.provider-row.active { background: var(--bg-elev-2); border-color: var(--accent); }
.p-main { display: flex; align-items: center; gap: 6px; overflow: hidden; }
.p-name {
  color: var(--text);
  font-size: var(--fs-xs);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.p-kind {
  color: var(--text-muted);
  font-size: var(--fs-xxs);
  background: var(--bg-elevated);
  border: 1px solid var(--line);
  border-radius: var(--radius-pill);
  padding: 1px 7px;
  flex: none;
}
.p-kind.unsaved { color: var(--accent); border-color: var(--accent-line); }

.empty {
  padding: 24px 12px;
  text-align: center;
  color: var(--text-muted);
  font-size: var(--fs-xs);
}
.empty.small { padding: 10px; }

.provider-form { display: grid; gap: 10px; }
.provider-form label { display: grid; gap: 5px; font-size: var(--fs-xs); color: var(--text-muted); }
.form-actions { display: flex; justify-content: flex-end; }

.models-section { display: grid; gap: 10px; }
.models-head { display: flex; align-items: center; justify-content: space-between; }
.models-head h4 { margin: 0; font-size: var(--fs-sm); color: var(--text); font-weight: 600; }
.models-actions { display: flex; gap: 6px; }

.sync-panel {
  display: grid;
  gap: 8px;
  padding: 8px;
  border: 1px solid var(--line);
  border-radius: var(--radius);
  background: var(--bg-elevated);
}
.sync-head { display: flex; align-items: center; justify-content: space-between; font-size: var(--fs-xs); color: var(--text-muted); }
.sync-row { padding: 2px 0; }
.mono { font-family: var(--font-mono, monospace); }
.muted { color: var(--text-faint); }

.model-card {
  display: grid;
  gap: 8px;
  padding: 10px;
  border: 1px solid var(--line);
  border-radius: var(--radius);
}
.model-row { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
.model-row label.inline { display: grid; gap: 4px; font-size: var(--fs-xxs); color: var(--text-muted); }
.cap-group { display: flex; gap: 12px; flex-wrap: wrap; }
.model-actions { display: flex; justify-content: flex-end; gap: 8px; }

.io-section { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; }
.io-col { display: grid; gap: 6px; }
.io-col h4 { margin: 0; font-size: var(--fs-sm); color: var(--text); font-weight: 600; }
.io-actions { display: flex; justify-content: flex-end; gap: 6px; }
</style>
