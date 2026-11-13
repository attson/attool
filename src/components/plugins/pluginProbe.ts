import type { InstalledPlugin, PluginOperationSnapshot } from './types';

export function createPluginProbeConfig(operation: PluginOperationSnapshot): {
  app: InstalledPlugin;
  apiVersion: '1.0' | '2.0';
} {
  return {
    app: {
      appId: operation.appId,
      name: operation.name,
      version: operation.version,
      publisherId: operation.publisherId,
      publisherName: operation.publisherName,
      publisherKeyId: operation.publisherKeyId,
      entry: operation.entry ?? '',
      permissions: operation.permissions,
      nativeService: operation.nativeService,
      enabled: false,
      installedAt: ''
    },
    apiVersion: operation.nativeService ? '2.0' : '1.0'
  };
}
