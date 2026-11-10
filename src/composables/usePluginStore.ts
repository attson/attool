import { computed, ref } from 'vue';

import { createPluginApi } from '../components/plugins/pluginApi';
import { mergeCatalogApps } from '../components/plugins/catalogMerge';
import type {
  CatalogSnapshot,
  InstalledPlugin,
  InspectedPluginPackage,
  PluginApi,
  PluginCommandError,
  PluginHistoryRecord
} from '../components/plugins/types';
import type { Tool } from '../types/tool';

function commandError(error: unknown): PluginCommandError {
  if (typeof error === 'object' && error !== null) {
    const value = error as Partial<PluginCommandError>;
    if (typeof value.code === 'string' && typeof value.message === 'string') {
      return {
        code: value.code,
        message: value.message,
        correlationId: value.correlationId ?? ''
      };
    }
  }
  return {
    code: 'PLUGIN_UNKNOWN_ERROR',
    message: String(error),
    correlationId: ''
  };
}

export function usePluginStore(api: PluginApi = createPluginApi()) {
  const apps = ref<InstalledPlugin[]>([]);
  const preview = ref<InspectedPluginPackage | null>(null);
  const history = ref<PluginHistoryRecord[]>([]);
  const catalog = ref<CatalogSnapshot | null>(null);
  const loading = ref(false);
  const catalogLoading = ref(false);
  const catalogRefreshing = ref(false);
  const busyAppIds = ref<Set<string>>(new Set());
  const error = ref<PluginCommandError | null>(null);
  const catalogError = ref<PluginCommandError | null>(null);

  const navigationTools = computed<Tool[]>(() =>
    apps.value
      .filter((app) => app.enabled)
      .map((app) => ({
        id: `plugin:${app.appId}`,
        name: app.name,
        description: `${app.publisherName} 提供的独立应用`,
        status: 'ready',
        icon: 'apps',
        group: 'utility',
        source: 'plugin',
        pluginAppId: app.appId
      }))
  );
  const catalogApps = computed(() =>
    catalog.value ? mergeCatalogApps(catalog.value, apps.value) : []
  );

  async function refreshHistory() {
    try {
      history.value = await api.listHistory();
    } catch (reason) {
      error.value = commandError(reason);
    }
  }

  async function load() {
    loading.value = true;
    error.value = null;
    try {
      apps.value = await api.listInstalled();
      await refreshHistory();
      catalogLoading.value = true;
      try {
        catalog.value = await api.listCatalog();
        catalogError.value = null;
      } catch (reason) {
        catalogError.value = commandError(reason);
      } finally {
        catalogLoading.value = false;
      }
    } catch (reason) {
      error.value = commandError(reason);
    } finally {
      loading.value = false;
    }
  }

  async function refreshCatalog() {
    catalogRefreshing.value = true;
    catalogError.value = null;
    try {
      const refreshed = await api.refreshCatalog();
      catalog.value = refreshed;
      return refreshed;
    } catch (reason) {
      catalogError.value = commandError(reason);
      throw reason;
    } finally {
      catalogRefreshing.value = false;
    }
  }

  async function inspect(path: string) {
    error.value = null;
    try {
      preview.value = await api.inspectPackage(path);
      return preview.value;
    } catch (reason) {
      error.value = commandError(reason);
      throw reason;
    }
  }

  async function install(path: string) {
    error.value = null;
    try {
      const installed = await api.installPackage(path);
      apps.value = [...apps.value.filter((app) => app.appId !== installed.appId), installed];
      preview.value = null;
      await refreshHistory();
      return installed;
    } catch (reason) {
      error.value = commandError(reason);
      throw reason;
    }
  }

  async function setEnabled(appId: string, enabled: boolean) {
    busyAppIds.value = new Set(busyAppIds.value).add(appId);
    error.value = null;
    try {
      await api.setEnabled(appId, enabled);
      apps.value = apps.value.map((app) =>
        app.appId === appId ? { ...app, enabled } : app
      );
      await refreshHistory();
    } catch (reason) {
      error.value = commandError(reason);
      throw reason;
    } finally {
      const next = new Set(busyAppIds.value);
      next.delete(appId);
      busyAppIds.value = next;
    }
  }

  async function uninstall(appId: string, deleteData: boolean) {
    busyAppIds.value = new Set(busyAppIds.value).add(appId);
    error.value = null;
    try {
      await api.uninstall(appId, deleteData);
      apps.value = apps.value.filter((app) => app.appId !== appId);
      await refreshHistory();
    } catch (reason) {
      error.value = commandError(reason);
      throw reason;
    } finally {
      const next = new Set(busyAppIds.value);
      next.delete(appId);
      busyAppIds.value = next;
    }
  }

  return {
    apps,
    history,
    catalog,
    preview,
    loading,
    catalogLoading,
    catalogRefreshing,
    busyAppIds,
    error,
    catalogError,
    navigationTools,
    catalogApps,
    load,
    inspect,
    install,
    refreshCatalog,
    setEnabled,
    uninstall
  };
}

export type PluginStoreController = ReturnType<typeof usePluginStore>;
