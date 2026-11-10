import { describe, expect, it } from 'vitest';

import { mergeCatalogApps } from './catalogMerge';
import type { CatalogSnapshot, InstalledPlugin } from './types';

const installedCodec: InstalledPlugin = {
  appId: 'com.attson.codec',
  name: '编码转换',
  version: '1.0.0',
  publisherId: 'com.attson',
  publisherName: 'Attson',
  publisherKeyId: 'codec-2026-01',
  entry: 'ui/index.html',
  permissions: ['host.theme.read'],
  enabled: true,
  installedAt: '2026-11-10T00:00:00Z'
};

const snapshot: CatalogSnapshot = {
  rootSequence: 3,
  catalogSequence: 18,
  generatedAt: '2026-11-10T08:00:00Z',
  expiresAt: '2026-11-17T08:00:00Z',
  expired: false,
  apps: [
    {
      appId: 'com.attson.codec',
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

describe('mergeCatalogApps', () => {
  it('joins the trusted catalog with the installed version by appId', () => {
    const [codec] = mergeCatalogApps(snapshot, [installedCodec]);

    expect(codec.name).toBe('编码转换');
    expect(codec.latestVersion).toBe('1.1.0');
    expect(codec.installedVersion).toBe('1.0.0');
    expect(codec.enabled).toBe(true);
    expect(codec.remoteActionsAllowed).toBe(true);
    expect(codec.updateAvailable).toBe(true);
    expect(codec.addedPermissions).toEqual([]);
  });

  it('does not offer an update when the catalog version is not strictly newer', () => {
    const sameVersion = {
      ...snapshot,
      apps: [{ ...snapshot.apps[0], latestVersion: installedCodec.version }]
    };

    const [codec] = mergeCatalogApps(sameVersion, [installedCodec]);

    expect(codec.updateAvailable).toBe(false);
  });

  it('orders prerelease identifiers that contain hyphens', () => {
    const prereleaseCatalog = {
      ...snapshot,
      apps: [{ ...snapshot.apps[0], latestVersion: '1.0.0-alpha-2' }]
    };
    const prereleaseInstalled = [{ ...installedCodec, version: '1.0.0-alpha-1' }];

    const [codec] = mergeCatalogApps(prereleaseCatalog, prereleaseInstalled);

    expect(codec.updateAvailable).toBe(true);
  });

  it('keeps installed state visible but disables remote actions for expired metadata', () => {
    const [codec] = mergeCatalogApps({ ...snapshot, expired: true }, [installedCodec]);

    expect(codec.installedVersion).toBe('1.0.0');
    expect(codec.remoteActionsAllowed).toBe(false);
  });
});
