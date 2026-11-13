import { describe, expect, it } from 'vitest';

import { createPluginProbeConfig } from './pluginProbe';
import type { NativeService, PluginOperationSnapshot } from './types';

const nativeService: NativeService = {
  entry: 'native/athttp-service',
  version: '0.1.0-alpha.1',
  sha256: 'a'.repeat(64),
  protocolVersion: 1
};

function operation(service: NativeService | null): PluginOperationSnapshot {
  return {
    operationId: 'operation-1',
    correlationId: 'correlation-1',
    appId: service ? 'com.attson.http' : 'com.attson.codec',
    name: service ? 'HTTP 请求' : '编码转换',
    version: '0.1.0-alpha.1',
    publisherId: 'com.attson',
    publisherName: 'Attson',
    publisherKeyId: 'publisher-2026-01',
    entry: 'ui/index.html',
    permissions: service
      ? ['host.theme.read', 'host.http.request', 'host.http.stream', 'host.http.data']
      : ['host.theme.read'],
    nativeService: service,
    stage: 'health-check',
    steps: ['download', 'verify', 'staging', 'health-check'],
    downloadedBytes: 4096,
    totalBytes: 4096,
    errorCode: null,
    message: null,
    finalAvailableVersion: null
  };
}

describe('createPluginProbeConfig', () => {
  it('uses Host API 2.0 for a native service package', () => {
    const config = createPluginProbeConfig(operation(nativeService));

    expect(config.apiVersion).toBe('2.0');
    expect(config.app.nativeService).toEqual(nativeService);
  });

  it('keeps pure frontend packages on Host API 1.0', () => {
    const config = createPluginProbeConfig(operation(null));

    expect(config.apiVersion).toBe('1.0');
    expect(config.app.nativeService).toBeNull();
  });
});
