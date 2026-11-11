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

const root = {
  schemaVersion: 1,
  sequence: 1,
  issuedAt: '2026-11-01T00:00:00Z',
  expiresAt: '2027-11-01T00:00:00Z',
  catalogKeys: [trustKey(catalogKeyId)],
  publishers: [{
    publisherId: 'com.attson',
    name: 'Attson',
    keys: [trustKey(publisherKeyId)]
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
  it('creates the next signed-release payload from all five immutable Codec assets', () => {
    const catalog = createCatalog({
      root,
      previousCatalog,
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
      () => createCatalog({ root, previousCatalog, release: incomplete, catalogKeyId, publisherKeyId, generatedAt, expiresAt }),
      /exactly the five supported plugin targets/
    );

    const redirected = release();
    redirected.assets[0].url = 'https://example.com/AT.Codec.atpkg';
    assert.throws(
      () => createCatalog({ root, previousCatalog, release: redirected, catalogKeyId, publisherKeyId, generatedAt, expiresAt }),
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
      () => createCatalog({ root: retiredRoot, previousCatalog, release: release(), catalogKeyId, publisherKeyId, generatedAt, expiresAt }),
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

function rawPublicKey(publicKey) {
  const jwk = publicKey.export({ format: 'jwk' });
  return Buffer.from(jwk.x, 'base64url').toString('base64');
}

async function createPluginPackage(directory, privateKey, os = 'linux', arch = 'x86_64') {
  const stage = path.join(directory, 'stage');
  const ui = path.join(stage, 'ui');
  await mkdir(ui, { recursive: true });
  const content = Buffer.from('<main>Codec</main>');
  await writeFile(path.join(ui, 'index.html'), content);
  const manifest = parseManifest({
    schemaVersion: 1,
    appId: 'com.attson.codec',
    name: '编码转换',
    version,
    publisher: { id: 'com.attson', name: 'Attson', keyId: publisherKeyId },
    target: { os, arch },
    hostApi: { min: '1.0.0', max: '1.x' },
    ui: { entry: 'ui/index.html' },
    permissions: ['host.theme.read', 'host.file.save', 'host.notification.show'],
    files: { 'ui/index.html': createHash('sha256').update(content).digest('hex') }
  });
  await writeFile(path.join(stage, 'manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`);
  await writeFile(path.join(stage, 'manifest.sig'), `${signManifest(manifest, privateKey)}\n`);
  const packagePath = path.join(directory, `AT.Codec_${version}_${os}_${arch}.atpkg`);
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
