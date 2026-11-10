import { describe, expect, it, vi } from 'vitest';

import { usePluginStore } from './usePluginStore';
import type { InstalledPlugin, PluginApi } from '../components/plugins/types';

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

function fakeApi(): PluginApi {
  return {
    inspectPackage: vi.fn(),
    installPackage: vi.fn(),
    listInstalled: vi.fn().mockResolvedValue([{ ...codec }]),
    listHistory: vi.fn().mockResolvedValue([]),
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
});
