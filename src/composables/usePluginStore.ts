import { computed, ref } from 'vue';

import { createPluginApi } from '../components/plugins/pluginApi';
import type {
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
  const loading = ref(false);
  const busyAppIds = ref<Set<string>>(new Set());
  const error = ref<PluginCommandError | null>(null);

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

  async function load() {
    loading.value = true;
    error.value = null;
    try {
      [apps.value, history.value] = await Promise.all([
        api.listInstalled(),
        api.listHistory()
      ]);
    } catch (reason) {
      error.value = commandError(reason);
    } finally {
      loading.value = false;
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
      history.value = await api.listHistory();
      preview.value = null;
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
      history.value = await api.listHistory();
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
      history.value = await api.listHistory();
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
    preview,
    loading,
    busyAppIds,
    error,
    navigationTools,
    load,
    inspect,
    install,
    setEnabled,
    uninstall
  };
}

export type PluginStoreController = ReturnType<typeof usePluginStore>;
