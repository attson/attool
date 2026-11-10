import { describe, expect, it, vi } from 'vitest';

import { usePluginStore } from './usePluginStore';
import type {
  CatalogSnapshot,
  InstalledPlugin,
  PluginApi
} from '../components/plugins/types';

const codec: InstalledPlugin = {
  appId: 'com.attson.codec',
  name: '编码转换',
  version: '0.1.0-alpha.0',
  publisherId: 'com.attson',
  publisherName: 'Attson',
  publisherKeyId: 'attool-p0-test',
  entry: 'ui/index.html',
  permissions: ['host.theme.read'],
  enabled: true,
  installedAt: '2026-11-10T00:00:00Z'
};

const catalog: CatalogSnapshot = {
  rootSequence: 3,
  catalogSequence: 18,
  generatedAt: '2026-11-10T08:00:00Z',
  expiresAt: '2026-11-17T08:00:00Z',
  expired: false,
  apps: [
    {
      appId: codec.appId,
      name: '编码转换',
      description: 'Base64、URL、Unicode、Hex、Hash 与 JWT',
      publisherId: 'com.attson',
      publisherName: 'Attson',
      icon: 'codec',
      latestVersion: '1.1.0',
      publishedAt: '2026-11-10T07:00:00Z',
      notes: '新增一键复制转换结果',
      publisherKeyId: 'codec-2026-01',
      hostApiMin: '1.0.0',
      hostApiMax: '1.x',
      permissions: ['host.theme.read'],
      target: {
        os: 'linux',
        arch: 'x86_64',
        url: 'https://github.com/attsontools/atcodec/releases/download/v1.1.0/AT.Codec_1.1.0_linux_x86_64.atpkg',
        fileName: 'AT.Codec_1.1.0_linux_x86_64.atpkg',
        size: 4096,
        sha256: 'a'.repeat(64)
      }
    }
  ]
};

function fakeApi(): PluginApi {
  return {
    inspectPackage: vi.fn(),
    installPackage: vi.fn(),
    listInstalled: vi.fn().mockResolvedValue([{ ...codec }]),
    listHistory: vi.fn().mockResolvedValue([]),
    listCatalog: vi.fn().mockResolvedValue({ ...catalog }),
    refreshCatalog: vi.fn().mockResolvedValue({ ...catalog }),
    installCatalogApp: vi.fn(),
    completeCatalogInstall: vi.fn(),
    failCatalogInstall: vi.fn(),
    getOperation: vi.fn(),
    listenOperations: vi.fn().mockResolvedValue(() => {}),
    setEnabled: vi.fn().mockResolvedValue(undefined),
    uninstall: vi.fn().mockResolvedValue(undefined)
  };
}

describe('usePluginStore', () => {
  it('keeps a disabled plugin in management but removes it from navigation', async () => {
    const api = fakeApi();
    const store = usePluginStore(api);
    await store.load();

    expect(store.navigationTools.value.map((tool) => tool.id)).toEqual([
      'plugin:com.attson.codec'
    ]);

    await store.setEnabled('com.attson.codec', false);

    expect(api.setEnabled).toHaveBeenCalledWith('com.attson.codec', false);
    expect(store.apps.value).toHaveLength(1);
    expect(store.apps.value[0].enabled).toBe(false);
    expect(store.navigationTools.value).toEqual([]);
  });

  it('does not report a completed install as failed when history refresh fails', async () => {
    const api = fakeApi();
    vi.mocked(api.installPackage).mockResolvedValue({ ...codec });
    vi.mocked(api.listHistory).mockRejectedValue(new Error('history unavailable'));
    const store = usePluginStore(api);

    await expect(store.install('/tmp/codec.atpkg')).resolves.toEqual(codec);
    expect(store.apps.value).toEqual([codec]);
  });

  it('does not report a completed state change as failed when history refresh fails', async () => {
    const api = fakeApi();
    const store = usePluginStore(api);
    await store.load();
    vi.mocked(api.listHistory).mockRejectedValue(new Error('history unavailable'));

    await expect(store.setEnabled(codec.appId, false)).resolves.toBeUndefined();
    expect(store.apps.value[0].enabled).toBe(false);
  });

  it('does not report a completed uninstall as failed when history refresh fails', async () => {
    const api = fakeApi();
    const store = usePluginStore(api);
    await store.load();
    vi.mocked(api.listHistory).mockRejectedValue(new Error('history unavailable'));

    await expect(store.uninstall(codec.appId, false)).resolves.toBeUndefined();
    expect(store.apps.value).toEqual([]);
  });

  it('loads the cached catalog and merges the installed version', async () => {
    const store = usePluginStore(fakeApi());

    await store.load();

    expect(store.catalogApps.value).toHaveLength(1);
    expect(store.catalogApps.value[0]).toMatchObject({
      name: '编码转换',
      latestVersion: '1.1.0',
      installedVersion: '0.1.0-alpha.0'
    });
  });

  it('loads installed apps when operation event subscription is unavailable', async () => {
    const api = fakeApi();
    vi.mocked(api.listenOperations).mockRejectedValue(new Error('event bridge unavailable'));
    const store = usePluginStore(api);

    await store.load();

    expect(store.apps.value).toEqual([codec]);
    expect(store.catalog.value?.catalogSequence).toBe(18);
    expect(store.error.value).toMatchObject({
      code: 'PLUGIN_UNKNOWN_ERROR',
      message: 'Error: event bridge unavailable'
    });
  });

  it('preserves the cached catalog when refresh fails', async () => {
    const api = fakeApi();
    const store = usePluginStore(api);
    await store.load();
    vi.mocked(api.refreshCatalog).mockRejectedValue({
      code: 'CATALOG_SIGNATURE_INVALID',
      message: 'Catalog signature verification failed',
      correlationId: 'catalog-correlation'
    });

    await expect(store.refreshCatalog()).rejects.toMatchObject({
      code: 'CATALOG_SIGNATURE_INVALID'
    });
    expect(store.catalog.value?.catalogSequence).toBe(18);
    expect(store.catalogError.value).toMatchObject({
      code: 'CATALOG_SIGNATURE_INVALID',
      correlationId: 'catalog-correlation'
    });
  });

  it('keeps a remote install out of navigation until health check activation completes', async () => {
    const api = fakeApi();
    const pending = {
      operationId: 'operation-1',
      correlationId: 'correlation-1',
      appId: codec.appId,
      name: codec.name,
      version: codec.version,
      publisherId: codec.publisherId,
      publisherName: codec.publisherName,
      publisherKeyId: codec.publisherKeyId,
      entry: codec.entry,
      permissions: codec.permissions,
      stage: 'health-check' as const,
      downloadedBytes: 4096,
      totalBytes: 4096,
      errorCode: null,
      message: null
    };
    vi.mocked(api.listInstalled).mockResolvedValue([]);
    vi.mocked(api.installCatalogApp).mockResolvedValue(pending);
    vi.mocked(api.completeCatalogInstall).mockResolvedValue({
      operation: { ...pending, stage: 'complete' },
      installed: codec
    });
    const store = usePluginStore(api);
    await store.load();

    await store.installCatalog(codec.appId, codec.permissions);
    expect(store.navigationTools.value).toEqual([]);
    expect(store.operation.value?.stage).toBe('health-check');

    await store.completeCatalogInstall('operation-1');
    expect(store.navigationTools.value.map((tool) => tool.id)).toEqual([
      'plugin:com.attson.codec'
    ]);
  });
});
