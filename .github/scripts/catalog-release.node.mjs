import assert from 'node:assert/strict';
import { createHash, generateKeyPairSync } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { describe, it } from 'node:test';

import { parseManifest } from '@attson/plugin-sdk';
import {
  signManifest,
  signTrustMetadata,
  verifyTrustMetadataSignature
} from '@attson/plugin-sdk/node';

import {
  createCatalog,
  inspectCodecPackage,
  inspectPluginPackage,
  loadCatalogApplication,
  loadCatalogApplicationBySlug,
  prepareCatalogRelease,
  prepareCatalogRenewal,
  renewCatalog,
  signCatalog,
  verifyCatalogState
} from './publish-catalog.mjs';

const generatedAt = '2026-11-11T00:00:00Z';
const expiresAt = '2026-11-18T00:00:00Z';
const version = '0.1.0-alpha.1';
const tagName = `v${version}`;
const catalogKeyId = 'catalog-2026-01';
const publisherKeyId = 'attson-codec-2026-01';
const textPublisherKeyId = 'attson-text-2026-01';
const codecApplication = await loadCatalogApplication('com.attson.codec');
const textApplication = await loadCatalogApplication('com.attson.text');

const root = {
  schemaVersion: 1,
  sequence: 1,
  issuedAt: '2026-11-01T00:00:00Z',
  expiresAt: '2027-11-01T00:00:00Z',
  catalogKeys: [trustKey(catalogKeyId)],
  publishers: [{
    publisherId: 'com.attson',
    name: 'Attson',
    keys: [trustKey(publisherKeyId), trustKey(textPublisherKeyId)]
  }]
};

const previousCatalog = {
  schemaVersion: 1,
  sequence: 7,
  generatedAt: '2026-11-10T00:00:00Z',
  expiresAt: '2026-11-17T00:00:00Z',
  signingKeyId: catalogKeyId,
  apps: [{
    appId: 'com.attson.codec',
    name: '编码转换',
    description: 'Base64、URL、Unicode、Hex、Hash 与 JWT',
    publisherId: 'com.attson',
    icon: 'codec',
    channels: {
      stable: {
        version: '0.1.0-alpha.0',
        publishedAt: '2026-11-10T00:00:00Z',
        notes: 'Initial release',
        publisherKeyId,
        hostApi: { min: '1.0.0', max: '1.x' },
        permissions: ['host.theme.read', 'host.file.save', 'host.notification.show'],
        artifacts: [artifact('linux', 'x86_64')]
      }
    }
  }]
};

describe('Catalog release generation', () => {
  it('loads JSON release policy by app ID and slug', async () => {
    const expected = {
      appId: 'com.attson.json',
      slug: 'json',
      repository: 'attsontools/atjson',
      artifactPrefix: 'AT.JSON',
      name: 'JSON 工具',
      description: '格式化 / 查询 / 对比 / 转换',
      publisherId: 'com.attson',
      publisherKeySecret: 'ATJSON_PUBLISHER_KEY_ID',
      icon: 'code',
      hostApi: { min: '1.0.0', max: '1.x' },
      permissions: ['host.theme.read']
    };

    assert.deepEqual(await loadCatalogApplication('com.attson.json'), expected);
    assert.deepEqual(await loadCatalogApplicationBySlug('json'), expected);
  });

  it('loads Time release policy by app ID and slug', async () => {
    const expected = {
      appId: 'com.attson.time',
      slug: 'time',
      repository: 'attsontools/attime',
      artifactPrefix: 'AT.Time',
      name: '时间工具',
      description: '时间戳 / 时区 / Cron / Duration',
      publisherId: 'com.attson',
      publisherKeySecret: 'ATTIME_PUBLISHER_KEY_ID',
      icon: 'clock',
      hostApi: { min: '1.0.0', max: '1.x' },
      permissions: ['host.theme.read']
    };

    assert.deepEqual(await loadCatalogApplication('com.attson.time'), expected);
    assert.deepEqual(await loadCatalogApplicationBySlug('time'), expected);
  });

  it('loads Generator release policy by app ID and slug', async () => {
    const expected = {
      appId: 'com.attson.generator',
      slug: 'generator',
      repository: 'attsontools/atgenerator',
      artifactPrefix: 'AT.Generator',
      name: '生成器',
      description: '密码 / UUID / QR / Lorem / 假数据 / 骰子',
      publisherId: 'com.attson',
      publisherKeySecret: 'ATGENERATOR_PUBLISHER_KEY_ID',
      icon: 'dice',
      hostApi: { min: '1.0.0', max: '1.x' },
      permissions: ['host.theme.read', 'host.file.save']
    };

    assert.deepEqual(await loadCatalogApplication('com.attson.generator'), expected);
    assert.deepEqual(await loadCatalogApplicationBySlug('generator'), expected);
  });

  it('loads Text release policy from the versioned application registry', async () => {
    const application = await loadCatalogApplication('com.attson.text');

    assert.deepEqual(application, {
      appId: 'com.attson.text',
      slug: 'text',
      repository: 'attsontools/attext',
      artifactPrefix: 'AT.Text',
      name: '文本工具',
      description: '整理 / 排序 / 大小写 / 拆合 / 抽取 / 对比',
      publisherId: 'com.attson',
      publisherKeySecret: 'ATTEXT_PUBLISHER_KEY_ID',
      icon: 'type',
      hostApi: { min: '1.0.0', max: '1.x' },
      permissions: ['host.theme.read']
    });
  });

  it('rejects ambiguous or out-of-scope application registry entries', async () => {
    const directory = await mkdtemp(path.join(tmpdir(), 'attool-application-registry-'));
    const registryPath = path.join(directory, 'apps.json');
    try {
      await writeFile(registryPath, JSON.stringify({
        schemaVersion: 1,
        apps: [
          codecApplication,
          {
            ...codecApplication,
            appId: 'com.attson.other',
            repository: 'example/other'
          }
        ]
      }));

      await assert.rejects(
        () => loadCatalogApplication('com.attson.other', registryPath),
        /official attsontools repository/
      );

      await writeFile(registryPath, JSON.stringify({
        schemaVersion: 1,
        apps: [
          codecApplication,
          {
            ...codecApplication,
            appId: 'com.attson.other',
            repository: 'attsontools/atother'
          }
        ]
      }));
      await assert.rejects(
        () => loadCatalogApplication('com.attson.other', registryPath),
        /duplicate slugs/
      );
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });

  it('creates the next signed-release payload from all five immutable Codec assets', () => {
    const catalog = createCatalog({
      root,
      previousCatalog,
      application: codecApplication,
      release: release(),
      catalogKeyId,
      publisherKeyId,
      generatedAt,
      expiresAt
    });

    assert.equal(catalog.sequence, 8);
    assert.equal(catalog.signingKeyId, catalogKeyId);
    assert.equal(catalog.apps[0].name, '编码转换');
    assert.equal(catalog.apps[0].channels.stable.version, version);
    assert.deepEqual(
      catalog.apps[0].channels.stable.artifacts.map(({ os, arch }) => `${os}/${arch}`),
      ['linux/aarch64', 'linux/x86_64', 'macos/aarch64', 'macos/x86_64', 'windows/x86_64']
    );
  });

  it('adds Text without changing the existing Codec application', async () => {
    const application = await loadCatalogApplication('com.attson.text');
    const catalog = createCatalog({
      root,
      previousCatalog,
      application,
      release: textRelease(),
      catalogKeyId,
      publisherKeyId: textPublisherKeyId,
      generatedAt,
      expiresAt
    });

    assert.equal(catalog.sequence, previousCatalog.sequence + 1);
    assert.deepEqual(catalog.apps[0], previousCatalog.apps[0]);
    assert.deepEqual(catalog.apps[1], {
      appId: 'com.attson.text',
      name: '文本工具',
      description: '整理 / 排序 / 大小写 / 拆合 / 抽取 / 对比',
      publisherId: 'com.attson',
      icon: 'type',
      channels: {
        stable: {
          version,
          publishedAt: '2026-11-11T00:00:00Z',
          notes: 'Text production drill',
          publisherKeyId: textPublisherKeyId,
          hostApi: { min: '1.0.0', max: '1.x' },
          permissions: ['host.theme.read'],
          artifacts: [
            textArtifact('linux', 'aarch64'),
            textArtifact('linux', 'x86_64'),
            textArtifact('macos', 'aarch64'),
            textArtifact('macos', 'x86_64'),
            textArtifact('windows', 'x86_64')
          ]
        }
      }
    });
  });

  it('renews Catalog metadata without changing the published applications', () => {
    const catalog = renewCatalog({
      root,
      previousCatalog,
      catalogKeyId,
      generatedAt,
      expiresAt
    });

    assert.equal(catalog.sequence, 8);
    assert.equal(catalog.generatedAt, generatedAt);
    assert.equal(catalog.expiresAt, expiresAt);
    assert.equal(catalog.signingKeyId, catalogKeyId);
    assert.deepEqual(catalog.apps, previousCatalog.apps);
  });

  it('rejects a Catalog renewal that moves time backward or does not extend expiry', () => {
    assert.throws(
      () => renewCatalog({
        root,
        previousCatalog,
        catalogKeyId,
        generatedAt: '2026-11-09T00:00:00Z',
        expiresAt
      }),
      /generatedAt must be later/
    );
    assert.throws(
      () => renewCatalog({
        root,
        previousCatalog,
        catalogKeyId,
        generatedAt,
        expiresAt: previousCatalog.expiresAt
      }),
      /expiresAt must be later/
    );

    const retiredRoot = structuredClone(root);
    retiredRoot.catalogKeys[0].status = 'retired';
    assert.throws(
      () => renewCatalog({
        root: retiredRoot,
        previousCatalog,
        catalogKeyId,
        generatedAt,
        expiresAt
      }),
      /active Catalog signing key/
    );
  });

  it('rejects incomplete, mutable, stale, or untrusted releases', () => {
    const incomplete = release();
    incomplete.assets.pop();
    assert.throws(
      () => createCatalog({ root, previousCatalog, application: codecApplication, release: incomplete, catalogKeyId, publisherKeyId, generatedAt, expiresAt }),
      /exactly the five supported plugin targets/
    );

    const redirected = release();
    redirected.assets[0].url = 'https://example.com/AT.Codec.atpkg';
    assert.throws(
      () => createCatalog({ root, previousCatalog, application: codecApplication, release: redirected, catalogKeyId, publisherKeyId, generatedAt, expiresAt }),
      /immutable attsontools\/atcodec Release URL/
    );

    assert.throws(
      () => createCatalog({
        root,
        previousCatalog: {
          ...previousCatalog,
          apps: [{
            ...previousCatalog.apps[0],
            channels: { stable: { ...previousCatalog.apps[0].channels.stable, version } }
          }]
        },
        application: codecApplication,
        release: release(),
        catalogKeyId,
        publisherKeyId,
        generatedAt,
        expiresAt
      }),
      /strictly newer/
    );

    const retiredRoot = structuredClone(root);
    retiredRoot.catalogKeys[0].status = 'retired';
    assert.throws(
      () => createCatalog({ root: retiredRoot, previousCatalog, application: codecApplication, release: release(), catalogKeyId, publisherKeyId, generatedAt, expiresAt }),
      /active Catalog signing key/
    );
  });

  it('signs only with the Catalog private key delegated by Root', () => {
    const catalogKeys = generateKeyPairSync('ed25519');
    const signingRoot = structuredClone(root);
    signingRoot.catalogKeys[0].publicKey = rawPublicKey(catalogKeys.publicKey);
    const catalog = createCatalog({
      root: signingRoot,
      previousCatalog,
      application: codecApplication,
      release: release(),
      catalogKeyId,
      publisherKeyId,
      generatedAt,
      expiresAt
    });

    const signature = signCatalog({
      root: signingRoot,
      catalog,
      privateKey: catalogKeys.privateKey
    });
    assert.equal(
      verifyTrustMetadataSignature(catalog, signature, catalogKeys.publicKey),
      true
    );

    const unrelated = generateKeyPairSync('ed25519');
    assert.throws(
      () => signCatalog({ root: signingRoot, catalog, privateKey: unrelated.privateKey }),
      /does not match Root key/
    );
  });

  it('inspects a Codec package before admitting it to Catalog', async () => {
    const directory = await mkdtemp(path.join(tmpdir(), 'attool-catalog-test-'));
    const publisherKeys = generateKeyPairSync('ed25519');
    const packageRoot = structuredClone(root);
    packageRoot.publishers[0].keys[0].publicKey = rawPublicKey(publisherKeys.publicKey);
    try {
      const packagePath = await createPluginPackage(directory, publisherKeys.privateKey);
      const inspected = await inspectCodecPackage({
        packagePath,
        root: packageRoot,
        publishedAt: generatedAt,
        expectedVersion: version,
        expectedPublisherKeyId: publisherKeyId
      });

      assert.deepEqual(
        { os: inspected.os, arch: inspected.arch, size: inspected.size },
        { os: 'linux', arch: 'x86_64', size: (await readFile(packagePath)).byteLength }
      );
      assert.match(inspected.sha256, /^[a-f0-9]{64}$/);

      await rm(packagePath, { force: true });
      zipPackageWithDuplicate(path.join(directory, 'stage'), packagePath);
      await assert.rejects(
        () => inspectCodecPackage({
          packagePath,
          root: packageRoot,
          publishedAt: generatedAt,
          expectedVersion: version,
          expectedPublisherKeyId: publisherKeyId
        }),
        /duplicate entry/
      );

      await writeFile(path.join(directory, 'stage', 'manifest.sig'), 'invalid\n');
      await rm(packagePath, { force: true });
      zipPackage(path.join(directory, 'stage'), packagePath);
      await assert.rejects(
        () => inspectCodecPackage({
          packagePath,
          root: packageRoot,
          publishedAt: generatedAt,
          expectedVersion: version,
          expectedPublisherKeyId: publisherKeyId
        }),
        /manifest signature is invalid/
      );
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });

  it('inspects a Text package using its independent identity and permissions', async () => {
    const directory = await mkdtemp(path.join(tmpdir(), 'attool-text-catalog-test-'));
    const publisherKeys = generateKeyPairSync('ed25519');
    const packageRoot = structuredClone(root);
    packageRoot.publishers[0].keys[1].publicKey = rawPublicKey(publisherKeys.publicKey);
    try {
      const packagePath = await createPluginPackage(
        directory,
        publisherKeys.privateKey,
        'linux',
        'x86_64',
        {
          application: textApplication,
          publisherKeyId: textPublisherKeyId
        }
      );
      const inspected = await inspectPluginPackage({
        packagePath,
        application: textApplication,
        root: packageRoot,
        publishedAt: generatedAt,
        expectedVersion: version,
        expectedPublisherKeyId: textPublisherKeyId
      });

      assert.equal(inspected.manifest.appId, 'com.attson.text');
      assert.deepEqual(inspected.manifest.permissions, ['host.theme.read']);
      assert.equal(inspected.fileName, `AT.Text_${version}_linux_x86_64.atpkg`);
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });

  it('verifies the offline Root and previous Catalog before publishing', () => {
    const rootKeys = generateKeyPairSync('ed25519');
    const catalogKeys = generateKeyPairSync('ed25519');
    const signedRoot = structuredClone(root);
    signedRoot.catalogKeys[0].publicKey = rawPublicKey(catalogKeys.publicKey);
    const rootSignature = signTrustMetadata(signedRoot, rootKeys.privateKey);
    const catalogSignature = signTrustMetadata(previousCatalog, catalogKeys.privateKey);

    const verified = verifyCatalogState({
      root: signedRoot,
      rootSignature,
      rootPublicKey: rawPublicKey(rootKeys.publicKey),
      catalog: previousCatalog,
      catalogSignature,
      at: generatedAt
    });
    assert.equal(verified.catalog.sequence, 7);

    assert.throws(
      () => verifyCatalogState({
        root: signedRoot,
        rootSignature: catalogSignature,
        rootPublicKey: rawPublicKey(rootKeys.publicKey),
        catalog: previousCatalog,
        catalogSignature,
        at: generatedAt
      }),
      /Root signature is invalid/
    );

    const revokedRoot = structuredClone(signedRoot);
    revokedRoot.catalogKeys[0].status = 'revoked';
    const revokedRootSignature = signTrustMetadata(revokedRoot, rootKeys.privateKey);
    assert.throws(
      () => verifyCatalogState({
        root: revokedRoot,
        rootSignature: revokedRootSignature,
        rootPublicKey: rawPublicKey(rootKeys.publicKey),
        catalog: previousCatalog,
        catalogSignature,
        at: generatedAt
      }),
      /trusted Catalog verification key/
    );
  });

  it('prepares a signed renewal from the verified previous Catalog', () => {
    const rootKeys = generateKeyPairSync('ed25519');
    const catalogKeys = generateKeyPairSync('ed25519');
    const signedRoot = structuredClone(root);
    signedRoot.catalogKeys[0].publicKey = rawPublicKey(catalogKeys.publicKey);
    const renewed = prepareCatalogRenewal({
      root: signedRoot,
      rootSignature: signTrustMetadata(signedRoot, rootKeys.privateKey),
      rootPublicKey: rawPublicKey(rootKeys.publicKey),
      previousCatalog,
      previousCatalogSignature: signTrustMetadata(previousCatalog, catalogKeys.privateKey),
      catalogKeyId,
      catalogPrivateKey: catalogKeys.privateKey,
      generatedAt,
      expiresAt
    });

    assert.equal(renewed.catalog.sequence, 8);
    assert.deepEqual(renewed.catalog.apps, previousCatalog.apps);
    assert.equal(
      verifyTrustMetadataSignature(renewed.catalog, renewed.signature, catalogKeys.publicKey),
      true
    );
  });

  it('prepares a verifiable Catalog from five inspected Codec packages', async () => {
    const directory = await mkdtemp(path.join(tmpdir(), 'attool-catalog-release-'));
    const rootKeys = generateKeyPairSync('ed25519');
    const catalogKeys = generateKeyPairSync('ed25519');
    const publisherKeys = generateKeyPairSync('ed25519');
    const signedRoot = structuredClone(root);
    signedRoot.catalogKeys[0].publicKey = rawPublicKey(catalogKeys.publicKey);
    signedRoot.publishers[0].keys[0].publicKey = rawPublicKey(publisherKeys.publicKey);
    try {
      const assets = [];
      for (const [os, arch] of [
        ['linux', 'aarch64'],
        ['linux', 'x86_64'],
        ['macos', 'aarch64'],
        ['macos', 'x86_64'],
        ['windows', 'x86_64']
      ]) {
        const packagePath = await createPluginPackage(
          directory,
          publisherKeys.privateKey,
          os,
          arch
        );
        const fileName = path.basename(packagePath);
        assets.push({
          fileName,
          size: (await readFile(packagePath)).byteLength,
          url: `https://github.com/attsontools/atcodec/releases/download/${tagName}/${fileName}`
        });
      }

      const prepared = await prepareCatalogRelease({
        root: signedRoot,
        rootSignature: signTrustMetadata(signedRoot, rootKeys.privateKey),
        rootPublicKey: rawPublicKey(rootKeys.publicKey),
        previousCatalog,
        previousCatalogSignature: signTrustMetadata(previousCatalog, catalogKeys.privateKey),
        application: codecApplication,
        release: { tagName, publishedAt: generatedAt, notes: 'Production drill', assets },
        assetDirectory: directory,
        catalogKeyId,
        catalogPrivateKey: catalogKeys.privateKey,
        publisherKeyId,
        generatedAt,
        expiresAt
      });

      assert.equal(prepared.catalog.sequence, 8);
      assert.equal(
        verifyTrustMetadataSignature(prepared.catalog, prepared.signature, catalogKeys.publicKey),
        true
      );

      const initial = await prepareCatalogRelease({
        root: signedRoot,
        rootSignature: signTrustMetadata(signedRoot, rootKeys.privateKey),
        rootPublicKey: rawPublicKey(rootKeys.publicKey),
        application: codecApplication,
        release: { tagName, publishedAt: generatedAt, notes: 'Initial production release', assets },
        assetDirectory: directory,
        catalogKeyId,
        catalogPrivateKey: catalogKeys.privateKey,
        publisherKeyId,
        generatedAt,
        expiresAt
      });
      assert.equal(initial.catalog.sequence, 1);
      assert.equal(initial.catalog.apps[0].name, '编码转换');
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });

  it('accepts the pnpm argument separator in the Catalog publisher CLI', () => {
    const missingRoot = path.join(tmpdir(), 'attool-missing-catalog-root.json');
    const result = spawnSync('pnpm', [
      'publish:catalog',
      '--',
      '--app', 'com.attson.codec',
      '--root', missingRoot,
      '--root-signature', 'missing-root-signature',
      '--release', 'missing-release',
      '--assets', 'missing-assets',
      '--output', 'missing-output',
      '--generated-at', generatedAt,
      '--expires-at', expiresAt
    ], {
      cwd: path.resolve(import.meta.dirname, '../..'),
      encoding: 'utf8'
    });
    const output = `${result.stdout}\n${result.stderr}`;

    assert.equal(result.status, 1);
    assert.doesNotMatch(output, /Invalid argument/);
    assert.match(output, /ENOENT/);
    assert.match(output, /attool-missing-catalog-root\.json/);
  });

  it('rejects applications that are not in the release registry', () => {
    const result = spawnSync('pnpm', [
      'publish:catalog',
      '--',
      '--app', 'com.example.untrusted',
      '--root', 'missing-root',
      '--root-signature', 'missing-root-signature',
      '--release', 'missing-release',
      '--assets', 'missing-assets',
      '--output', 'missing-output',
      '--generated-at', generatedAt,
      '--expires-at', expiresAt
    ], {
      cwd: path.resolve(import.meta.dirname, '../..'),
      encoding: 'utf8'
    });

    assert.equal(result.status, 1);
    assert.match(`${result.stdout}\n${result.stderr}`, /Unknown Catalog application: com\.example\.untrusted/);
  });

  it('dispatches Catalog releases by registered application ID', async () => {
    const workflow = await readFile(
      path.resolve(import.meta.dirname, '../workflows/catalog.yml'),
      'utf8'
    );

    assert.match(workflow, /^\s+app_id:\s*$/m);
    assert.match(workflow, /^\s+release_tag:\s*$/m);
    assert.match(workflow, /\.github\/catalog\/apps\.json/);
    assert.match(workflow, /--app "\$APP_ID"/);
    assert.doesNotMatch(workflow, /^\s+codec_tag:\s*$/m);
  });

  it('does not require Codec release inputs in renewal mode', () => {
    const missingRoot = path.join(tmpdir(), 'attool-missing-renewal-root.json');
    const result = spawnSync('pnpm', [
      'publish:catalog',
      '--',
      '--mode', 'renew',
      '--root', missingRoot,
      '--root-signature', 'missing-root-signature',
      '--catalog', 'missing-catalog',
      '--catalog-signature', 'missing-catalog-signature',
      '--output', 'missing-output',
      '--generated-at', generatedAt,
      '--expires-at', expiresAt
    ], {
      cwd: path.resolve(import.meta.dirname, '../..'),
      encoding: 'utf8'
    });
    const output = `${result.stdout}\n${result.stderr}`;

    assert.equal(result.status, 1);
    assert.doesNotMatch(output, /--release is required|--assets is required/);
    assert.match(output, /ENOENT/);
    assert.match(output, /attool-missing-renewal-root\.json/);
  });
});

function trustKey(keyId) {
  return {
    keyId,
    algorithm: 'ed25519',
    publicKey: 'ERERERERERERERERERERERERERERERERERERERERERE=',
    status: 'active',
    notBefore: '2026-11-01T00:00:00Z',
    notAfter: '2027-05-01T00:00:00Z'
  };
}

function release() {
  return {
    tagName,
    publishedAt: '2026-11-11T00:00:00Z',
    notes: 'Production drill',
    assets: [
      artifact('linux', 'aarch64'),
      artifact('linux', 'x86_64'),
      artifact('macos', 'aarch64'),
      artifact('macos', 'x86_64'),
      artifact('windows', 'x86_64')
    ]
  };
}

function artifact(os, arch) {
  const fileName = `AT.Codec_${version}_${os}_${arch}.atpkg`;
  return {
    os,
    arch,
    fileName,
    size: 4096,
    sha256: 'a'.repeat(64),
    url: `https://github.com/attsontools/atcodec/releases/download/${tagName}/${fileName}`
  };
}

function textRelease() {
  return {
    tagName,
    publishedAt: '2026-11-11T00:00:00Z',
    notes: 'Text production drill',
    assets: [
      textArtifact('linux', 'aarch64'),
      textArtifact('linux', 'x86_64'),
      textArtifact('macos', 'aarch64'),
      textArtifact('macos', 'x86_64'),
      textArtifact('windows', 'x86_64')
    ]
  };
}

function textArtifact(os, arch) {
  const fileName = `AT.Text_${version}_${os}_${arch}.atpkg`;
  return {
    os,
    arch,
    fileName,
    size: 4096,
    sha256: 'b'.repeat(64),
    url: `https://github.com/attsontools/attext/releases/download/${tagName}/${fileName}`
  };
}

function rawPublicKey(publicKey) {
  const jwk = publicKey.export({ format: 'jwk' });
  return Buffer.from(jwk.x, 'base64url').toString('base64');
}

async function createPluginPackage(
  directory,
  privateKey,
  os = 'linux',
  arch = 'x86_64',
  {
    application = codecApplication,
    publisherKeyId: manifestPublisherKeyId = publisherKeyId
  } = {}
) {
  const stage = path.join(directory, 'stage');
  const ui = path.join(stage, 'ui');
  await mkdir(ui, { recursive: true });
  const content = Buffer.from(`<main>${application.name}</main>`);
  await writeFile(path.join(ui, 'index.html'), content);
  const manifest = parseManifest({
    schemaVersion: 1,
    appId: application.appId,
    name: application.name,
    version,
    publisher: { id: application.publisherId, name: 'Attson', keyId: manifestPublisherKeyId },
    target: { os, arch },
    hostApi: application.hostApi,
    ui: { entry: 'ui/index.html' },
    permissions: application.permissions,
    files: { 'ui/index.html': createHash('sha256').update(content).digest('hex') }
  });
  await writeFile(path.join(stage, 'manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`);
  await writeFile(path.join(stage, 'manifest.sig'), `${signManifest(manifest, privateKey)}\n`);
  const packagePath = path.join(
    directory,
    `${application.artifactPrefix}_${version}_${os}_${arch}.atpkg`
  );
  zipPackage(stage, packagePath);
  return packagePath;
}

function zipPackage(stage, output) {
  const zipped = spawnSync('zip', ['-X', '-D', '-q', output, 'manifest.json', 'manifest.sig', 'ui/index.html'], {
    cwd: stage,
    encoding: 'utf8'
  });
  if (zipped.status !== 0) throw new Error(zipped.stderr || 'zip failed');
}

function zipPackageWithDuplicate(stage, output) {
  const script = [
    'import os, sys, zipfile',
    'stage, output = sys.argv[1:3]',
    "names = ['manifest.json', 'manifest.sig', 'ui/index.html', 'ui/index.html']",
    "with zipfile.ZipFile(output, 'w') as archive:",
    '    for name in names:',
    '        archive.write(os.path.join(stage, name), name)'
  ].join('\n');
  const zipped = spawnSync('python3', ['-c', script, stage, output], { encoding: 'utf8' });
  if (zipped.status !== 0) throw new Error(zipped.stderr || 'duplicate zip fixture failed');
}
