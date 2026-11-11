import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import type {
  CatalogSnapshot,
  InstalledPlugin,
  InspectedPluginPackage,
  PluginApi,
  PluginHistoryRecord,
  PluginOperationSnapshot,
  CatalogInstallResult
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
    installCatalogApp: (appId, acceptedPermissions) =>
      invoke<PluginOperationSnapshot>('plugin_install_catalog_app', { appId, acceptedPermissions }),
    updateCatalogApp: (appId, acceptedAddedPermissions) =>
      invoke<PluginOperationSnapshot>('plugin_update_catalog_app', { appId, acceptedAddedPermissions }),
    completeCatalogInstall: (operationId) =>
      invoke<CatalogInstallResult>('plugin_complete_catalog_install', { operationId }),
    failCatalogInstall: (operationId) =>
      invoke<PluginOperationSnapshot>('plugin_fail_catalog_install', { operationId }),
    getOperation: (operationId) =>
      invoke<PluginOperationSnapshot>('plugin_get_operation', { operationId }),
    getLatestOperation: () =>
      invoke<PluginOperationSnapshot | null>('plugin_get_latest_operation'),
    listenOperations: async (listener) =>
      listen<PluginOperationSnapshot>('plugin://operation-state', (event) => listener(event.payload)),
    confirmPluginLaunch: (appId, version) =>
      invoke<boolean>('plugin_confirm_plugin_launch', { appId, version }),
    rollbackPluginLaunch: (appId, version) =>
      invoke<InstalledPlugin | null>('plugin_rollback_plugin_launch', { appId, version }),
    setEnabled: (appId, enabled) =>
      invoke<void>('plugin_set_enabled', { appId, enabled }),
    uninstall: (appId, deleteData) =>
      invoke<void>('plugin_uninstall', { appId, deleteData })
  };
}
