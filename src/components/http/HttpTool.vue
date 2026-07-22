<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { NButton, NSelect } from 'naive-ui';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { useHttpStore } from '../../composables/useHttpStore';
import type { HttpEnvVar, HttpRequestSpec, TabKind } from './types';
import { toCurl } from './curl';
import HttpSidebar from './HttpSidebar.vue';
import HttpTabBar from './HttpTabBar.vue';
import HttpRequestEditor from './HttpRequestEditor.vue';
import HttpResponseView from './HttpResponseView.vue';
import HttpEnvModal from './HttpEnvModal.vue';
import HttpOpenApiImportModal from './HttpOpenApiImportModal.vue';
import SseTool from './SseTool.vue';
import WsTool from './WsTool.vue';
import type { ImportedOpenApiCollection } from './openapiImport';

const store = useHttpStore();
const collapsed = ref(false);
const envModalOpen = ref(false);
const envModalTab = ref<'env' | 'vars'>('vars');
const openApiImportOpen = ref(false);

const envOptions = computed(() => [
  { label: '不使用环境', value: '__none__' },
  ...store.state.envs.map((e) => ({ label: e.name, value: e.id })),
  { label: '管理环境…', value: '__manage__' }
]);

const currentEnvValue = computed(() => store.state.activeEnvId ?? '__none__');

async function onEnvSelect(value: string) {
  if (value === '__manage__') {
    envModalTab.value = 'env';
    envModalOpen.value = true;
    return;
  }
  await store.setActiveEnv(value === '__none__' ? null : value);
}

function openVarsModal() {
  envModalTab.value = 'vars';
  envModalOpen.value = true;
}

function updateSpec(next: HttpRequestSpec) {
  const active = store.activeTab.value;
  if (!active || active.kind !== 'http') return;
  active.spec = next;
  const url = next.url;
  if (url) {
    try {
      const u = new URL(url);
      const p = u.pathname.length > 1 ? u.pathname : u.host;
      active.title = `${next.method} ${p}`.slice(0, 40);
    } catch {
      active.title = `${next.method} ${url || '新请求'}`.slice(0, 40);
    }
  }
}

async function copyCurrentCurl(template: boolean) {
  const active = store.activeTab.value;
  if (!active || active.kind !== 'http') return;
  const text = toCurl(active.spec as HttpRequestSpec, template ? null : store.varContext.value);
  try { await writeText(text); } catch {}
}

function onLoadHistory(item: import('./types').HttpHistoryItem, mode: 'active' | 'new') {
  store.loadIntoTab(JSON.parse(JSON.stringify(item.spec)), mode);
}

async function onImportOpenApi(imported: ImportedOpenApiCollection) {
  await store.importCollection(imported);
}

// ---- 快捷键 ----
function onKey(ev: KeyboardEvent) {
  const meta = ev.metaKey || ev.ctrlKey;
  if (!meta) return;
  const k = ev.key.toLowerCase();
  if (k === 'enter') {
    ev.preventDefault();
    if (store.activeTab.value?.sending) store.cancel();
    else void store.send();
  } else if (k === 't') {
    ev.preventDefault();
    void store.newTab();
  } else if (k === 'w') {
    ev.preventDefault();
    if (store.state.activeTabId) void store.closeTab(store.state.activeTabId);
  } else if (k === 'b') {
    ev.preventDefault();
    collapsed.value = !collapsed.value;
  } else if (k === 'e') {
    ev.preventDefault();
    envModalTab.value = 'vars';
    envModalOpen.value = true;
  }
}

function onOpenVarsEvent() {
  envModalTab.value = 'vars';
  envModalOpen.value = true;
}

onMounted(async () => {
  await store.init();
  window.addEventListener('keydown', onKey);
  window.addEventListener('attool:http-open-vars', onOpenVarsEvent);
});
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKey);
  window.removeEventListener('attool:http-open-vars', onOpenVarsEvent);
  void store.flushDirtyNow();
});
</script>

<template>
  <div class="http-root">
    <HttpSidebar
      :items="store.state.history"
      :collections="store.state.collections"
      :folders="store.state.collectionFolders"
      :requests="store.state.collectionRequests"
      :collapsed="collapsed"
      @load="onLoadHistory"
      @open-request="store.openCollectionRequest"
      @delete-collection="store.deleteCollection"
      @delete-request="store.deleteCollectionRequest"
      @import-openapi="openApiImportOpen = true"
      @delete="(id: string) => store.deleteHistory(id)"
      @clear="() => store.clearHistory()"
      @toggle-collapse="collapsed = !collapsed"
    />
    <div class="main">
      <div class="topbar">
        <div class="topbar-left">
          <span class="label">环境</span>
          <n-select
            :value="currentEnvValue"
            :options="envOptions"
            size="small"
            style="width: 180px"
            @update:value="onEnvSelect"
          />
        </div>
        <n-button size="tiny" secondary @click="openVarsModal">
          <span class="mono">{{ '{}' }}</span>&nbsp;变量
        </n-button>
      </div>

      <HttpTabBar
        :tabs="store.state.tabs"
        :active-id="store.state.activeTabId"
        @activate="(id: string) => store.setActiveTab(id)"
        @close="(id: string) => store.closeTab(id)"
        @new="(kind: TabKind) => store.newTab(undefined, undefined, kind)"
      />

      <template v-if="store.activeTab.value">
        <template v-if="store.activeTab.value.kind === 'http'">
          <HttpRequestEditor
            :spec="store.activeTab.value.spec as any"
            :sending="store.activeTab.value.sending"
            :var-context="store.varContext.value"
            @update:spec="updateSpec"
            @send="() => store.send()"
            @cancel="() => store.cancel()"
            @copy-curl="copyCurrentCurl"
            @apply-spec="(s: HttpRequestSpec) => store.loadIntoTab(s, 'active')"
          />
          <HttpResponseView
            :response="store.activeTab.value.lastResponse"
            :error="store.activeTab.value.lastError"
            :sending="store.activeTab.value.sending"
          />
        </template>
        <SseTool v-else-if="store.activeTab.value.kind === 'sse'" :tab="store.activeTab.value" />
        <WsTool v-else-if="store.activeTab.value.kind === 'ws'" :tab="store.activeTab.value" />
      </template>
    </div>

    <HttpEnvModal
      v-model:show="envModalOpen"
      :envs="store.state.envs"
      :active-env-id="store.state.activeEnvId"
      :active-env-vars="store.state.activeEnvVars"
      :global-vars="store.state.globalVars"
      :default-tab="envModalTab"
      @add-env="(name: string) => store.addEnv(name)"
      @rename-env="(id: string, name: string) => store.renameEnv(id, name)"
      @delete-env="(id: string) => store.deleteEnv(id)"
      @set-active-env="(id: string | null) => store.setActiveEnv(id)"
      @add-var="(envId: string) => store.upsertVar(store.makeVar(envId))"
      @update-var="(v: HttpEnvVar) => store.upsertVar(v)"
      @delete-var="(id: string, envId: string) => store.deleteVar(id, envId)"
    />
    <HttpOpenApiImportModal
      v-model:show="openApiImportOpen"
      @import="onImportOpenApi"
    />
  </div>
</template>

<style scoped>
.http-root {
  display: flex;
  height: 100%;
  overflow: hidden;
  background: var(--bg-base);
}
.main {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.topbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 6px 12px;
  border-bottom: 1px solid var(--line);
  gap: 12px;
}
.topbar-left { display: flex; align-items: center; gap: 6px; }
.label { color: var(--text-muted); font-size: var(--fs-xs); }
.mono { font-family: var(--font-mono, ui-monospace, monospace); }
</style>
