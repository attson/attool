import { invoke } from '@tauri-apps/api/core';

import type {
  CatalogSnapshot,
  InstalledPlugin,
  InspectedPluginPackage,
  PluginApi,
  PluginHistoryRecord
} from './types';

export function createPluginApi(): PluginApi {
  return {
    inspectPackage: (path) =>
      invoke<InspectedPluginPackage>('plugin_inspect_package', { path }),
    installPackage: (path) =>
      invoke<InstalledPlugin>('plugin_install_package', { path }),
    listInstalled: () => invoke<InstalledPlugin[]>('plugin_list_installed'),
    listHistory: () => invoke<PluginHistoryRecord[]>('plugin_list_history'),
    listCatalog: () => invoke<CatalogSnapshot | null>('plugin_catalog_list'),
    refreshCatalog: () => invoke<CatalogSnapshot>('plugin_catalog_refresh'),
    setEnabled: (appId, enabled) =>
      invoke<void>('plugin_set_enabled', { appId, enabled }),
    uninstall: (appId, deleteData) =>
      invoke<void>('plugin_uninstall', { appId, deleteData })
  };
}
