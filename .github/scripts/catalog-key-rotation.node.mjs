import assert from 'node:assert/strict';
import { generateKeyPairSync } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { mkdir, mkdtemp, readFile, readdir, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { describe, it } from 'node:test';

import {
  signTrustMetadata,
  verifyTrustMetadataSignature
} from '@attson/plugin-sdk/node';

import {
  createOverlapRoot,
  createStatusRoot,
  parseRotationArguments,
  writeOverlapCeremony,
  writeStatusCeremony
} from './rotate-catalog-keys.mjs';
import {
  createProductionRotationPlan,
  parseProductionArguments,
  runProductionRotation
} from './rotate-production-catalog-keys.mjs';
import {
  createProductionRetirementPlan,
  parseProductionRetirementArguments,
  runProductionRetirement
} from './retire-production-catalog-keys.mjs';
import {
  createProductionPublisherAuthorizationPlan,
  parseProductionPublisherArguments,
  runProductionPublisherAuthorization
} from './authorize-production-publisher.mjs';

describe('Catalog key rotation ceremony', () => {
  it('accepts a one-argument production Publisher authorization command', () => {
    assert.deepEqual(parseProductionPublisherArguments(['--', 'text']), {
      appSlug: 'text'
    });
    assert.throws(
      () => parseProductionPublisherArguments([]),
      /Usage: pnpm authorize:publisher:prod/
    );
  });

  it('derives a Text-only Publisher authorization plan', () => {
    const fixture = createFixture();

    assert.deepEqual(
      createProductionPublisherAuthorizationPlan({
        root: fixture.root,
        application: {
          appId: 'com.attson.text',
          slug: 'text',
          publisherId: 'com.attson'
        },
        now: new Date('2026-11-12T08:09:10.987Z')
      }),
      {
        publisherId: 'com.attson',
        publisherKeyId: 'attson-text-2026-01',
        issuedAt: '2026-11-12T08:09:10.000Z',
        notAfter: '2027-11-12T08:09:10.000Z'
      }
    );
  });

  it('derives an independent Generator Publisher authorization plan', () => {
    const fixture = createFixture();

    assert.deepEqual(
      createProductionPublisherAuthorizationPlan({
        root: fixture.root,
        application: {
          appId: 'com.attson.generator',
          slug: 'generator',
          publisherId: 'com.attson'
        },
        now: new Date('2026-11-12T08:09:10.987Z')
      }),
      {
        publisherId: 'com.attson',
        publisherKeyId: 'attson-generator-2026-01',
        issuedAt: '2026-11-12T08:09:10.000Z',
        notAfter: '2027-11-12T08:09:10.000Z'
      }
    );
  });

  it('authorizes one Publisher key without rotating the Catalog key', async () => {
    const fixture = createFixture();
    const directory = await mkdtemp(path.join(tmpdir(), 'attool-publisher-authorization-'));
    const rootOfflineDirectory = path.join(directory, 'root-offline');
    const metadataDirectory = path.join(directory, 'metadata');
    const offlineDirectory = path.join(directory, 'text-offline');
    const transferDirectory = path.join(directory, 'text-transfer');
    const application = {
      appId: 'com.attson.text',
      slug: 'text',
      repository: 'attsontools/attext',
      publisherId: 'com.attson',
      publisherKeySecret: 'ATTEXT_PUBLISHER_KEY_ID'
    };
    try {
      await Promise.all([
        mkdir(rootOfflineDirectory, { mode: 0o700 }),
        mkdir(metadataDirectory)
      ]);
      await Promise.all([
        writeFile(
          path.join(rootOfflineDirectory, 'root-private.pem'),
          fixture.rootKeys.privateKey.export({ type: 'pkcs8', format: 'pem' }),
          { mode: 0o600 }
        ),
        writeFile(
          path.join(rootOfflineDirectory, 'root-public.txt'),
          `${rawPublicKey(fixture.rootKeys.publicKey)}\n`
        ),
        writeFile(path.join(metadataDirectory, 'root.json'), `${JSON.stringify(fixture.root)}\n`),
        writeFile(path.join(metadataDirectory, 'root.json.sig'), `${fixture.rootSignature}\n`)
      ]);

      const result = await runProductionPublisherAuthorization({
        application,
        rootOfflineDirectory,
        offlineDirectory,
        transferDirectory,
        metadataDirectory,
        now: new Date('2026-11-12T08:09:10Z')
      });

      assert.equal(result.root.sequence, 2);
      assert.deepEqual(result.root.catalogKeys, fixture.root.catalogKeys);
      assert.deepEqual(
        result.root.publishers[0].keys.map(({ keyId }) => keyId),
        ['attson-codec-2026-01', 'attson-text-2026-01']
      );
      assert.equal(
        JSON.parse(await readFile(path.join(transferDirectory, 'public', 'root.json'), 'utf8')).sequence,
        2
      );
      assert.equal(
        await readFile(path.join(transferDirectory, 'github-secrets', 'attool', 'ATTEXT_PUBLISHER_KEY_ID'), 'utf8'),
        'attson-text-2026-01\n'
      );
      assert.equal(
        await readFile(path.join(transferDirectory, 'github-secrets', 'attext', 'ATTEXT_PUBLISHER_KEY_ID'), 'utf8'),
        'attson-text-2026-01\n'
      );
      const privateFile = path.join(offlineDirectory, 'publisher-private.pem');
      assert.match(await readFile(privateFile, 'utf8'), /BEGIN PRIVATE KEY/);
      assert.equal((await stat(privateFile)).mode & 0o777, 0o600);
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });

  it('derives the next production key IDs and one-year validity window', () => {
    const fixture = createFixture();

    assert.deepEqual(
      createProductionRotationPlan({
        root: fixture.root,
        now: new Date('2026-11-12T08:09:10.987Z')
      }),
      {
        catalogKeyId: 'catalog-2026-02',
        publisherId: 'com.attson',
        publisherKeyId: 'attson-codec-2026-02',
        issuedAt: '2026-11-12T08:09:10.000Z',
        notAfter: '2027-11-12T08:09:10.000Z'
      }
    );
  });

  it('accepts exactly three production ceremony directories', () => {
    assert.deepEqual(
      parseProductionArguments(['--', '/root-offline', '/new-offline', '/new-transfer']),
      {
        rootOfflineDirectory: '/root-offline',
        offlineDirectory: '/new-offline',
        transferDirectory: '/new-transfer'
      }
    );
    assert.throws(
      () => parseProductionArguments(['/root-offline', '/new-offline']),
      /Usage: pnpm rotate:catalog-keys:prod/
    );
  });

  it('runs production overlap rotation from three directory arguments', async () => {
    const fixture = createFixture();
    const directory = await mkdtemp(path.join(tmpdir(), 'attool-production-rotation-'));
    const rootOfflineDirectory = path.join(directory, 'root-offline');
    const metadataDirectory = path.join(directory, 'metadata');
    const offlineDirectory = path.join(directory, 'new-offline');
    const transferDirectory = path.join(directory, 'new-transfer');
    try {
      await Promise.all([
        mkdir(rootOfflineDirectory, { mode: 0o700 }),
        mkdir(metadataDirectory)
      ]);
      await Promise.all([
        writeFile(
          path.join(rootOfflineDirectory, 'root-private.pem'),
          fixture.rootKeys.privateKey.export({ type: 'pkcs8', format: 'pem' }),
          { mode: 0o600 }
        ),
        writeFile(
          path.join(rootOfflineDirectory, 'root-public.txt'),
          `${rawPublicKey(fixture.rootKeys.publicKey)}\n`
        ),
        writeFile(path.join(metadataDirectory, 'root.json'), `${JSON.stringify(fixture.root)}\n`),
        writeFile(path.join(metadataDirectory, 'root.json.sig'), `${fixture.rootSignature}\n`)
      ]);

      await assert.rejects(
        () => runProductionRotation({
          rootOfflineDirectory,
          offlineDirectory,
          transferDirectory: path.join(rootOfflineDirectory, 'transfer'),
          metadataDirectory,
          now: new Date('2026-11-12T08:09:10Z')
        }),
        /must be separate and non-nested/
      );

      const result = await runProductionRotation({
        rootOfflineDirectory,
        offlineDirectory,
        transferDirectory,
        metadataDirectory,
        now: new Date('2026-11-12T08:09:10Z')
      });

      assert.equal(result.root.sequence, 2);
      assert.deepEqual(
        result.root.catalogKeys.map(({ keyId }) => keyId),
        ['catalog-2026-01', 'catalog-2026-02']
      );
      assert.equal(
        JSON.parse(await readFile(path.join(transferDirectory, 'public', 'root.json'), 'utf8')).sequence,
        2
      );
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });

  it('derives a production retirement plan from the keys used by the Catalog', () => {
    const fixture = createRetirementFixture();

    assert.deepEqual(
      createProductionRetirementPlan({
        root: fixture.root,
        catalog: fixture.catalog,
        now: new Date('2026-11-13T08:09:10.987Z')
      }),
      {
        catalogKeyId: 'catalog-2026-01',
        publisherId: 'com.attson',
        publisherKeyId: 'attson-codec-2026-01',
        issuedAt: '2026-11-13T08:09:10.000Z'
      }
    );
  });

  it('fails closed when production retirement candidates are ambiguous', () => {
    const fixture = createRetirementFixture();
    const ambiguousRoot = structuredClone(fixture.root);
    ambiguousRoot.catalogKeys.push({
      ...ambiguousRoot.catalogKeys[0],
      keyId: 'catalog-2025-01'
    });

    assert.throws(
      () => createProductionRetirementPlan({
        root: ambiguousRoot,
        catalog: fixture.catalog,
        now: new Date('2026-11-13T08:09:10Z')
      }),
      /exactly one other active Catalog key/
    );

    const missingReplacement = structuredClone(fixture.root);
    missingReplacement.catalogKeys.find(({ keyId }) => keyId === fixture.catalog.signingKeyId).status = 'retired';
    assert.throws(
      () => createProductionRetirementPlan({
        root: missingReplacement,
        catalog: fixture.catalog,
        now: new Date('2026-11-13T08:09:10Z')
      }),
      /must be an active Catalog key/
    );

    const ambiguousPublisherRoot = structuredClone(fixture.root);
    ambiguousPublisherRoot.publishers[0].keys.push({
      ...ambiguousPublisherRoot.publishers[0].keys[0],
      keyId: 'attson-codec-2025-01'
    });
    assert.throws(
      () => createProductionRetirementPlan({
        root: ambiguousPublisherRoot,
        catalog: fixture.catalog,
        now: new Date('2026-11-13T08:09:10Z')
      }),
      /exactly one other active com.attson key/
    );
  });

  it('accepts exactly two production retirement directories', () => {
    assert.deepEqual(
      parseProductionRetirementArguments(['--', '/root-offline', '/public-output']),
      {
        rootOfflineDirectory: '/root-offline',
        outputDirectory: '/public-output'
      }
    );
    assert.throws(
      () => parseProductionRetirementArguments(['/root-offline']),
      /Usage: pnpm rotate:catalog-keys:retire:prod/
    );
  });

  it('retires superseded production keys into a public-only directory', async () => {
    const fixture = createRetirementFixture();
    const directory = await mkdtemp(path.join(tmpdir(), 'attool-production-retirement-'));
    const rootOfflineDirectory = path.join(directory, 'root-offline');
    const metadataDirectory = path.join(directory, 'metadata');
    const outputDirectory = path.join(directory, 'public-output');
    try {
      await Promise.all([
        mkdir(rootOfflineDirectory, { mode: 0o700 }),
        mkdir(metadataDirectory)
      ]);
      await Promise.all([
        writeFile(
          path.join(rootOfflineDirectory, 'root-private.pem'),
          fixture.rootKeys.privateKey.export({ type: 'pkcs8', format: 'pem' }),
          { mode: 0o600 }
        ),
        writeFile(
          path.join(rootOfflineDirectory, 'root-public.txt'),
          `${rawPublicKey(fixture.rootKeys.publicKey)}\n`
        ),
        writeFile(path.join(metadataDirectory, 'root.json'), `${JSON.stringify(fixture.root)}\n`),
        writeFile(path.join(metadataDirectory, 'root.json.sig'), `${fixture.rootSignature}\n`),
        writeFile(path.join(metadataDirectory, 'catalog.json'), `${JSON.stringify(fixture.catalog)}\n`),
        writeFile(path.join(metadataDirectory, 'catalog.json.sig'), `${fixture.catalogSignature}\n`)
      ]);

      await assert.rejects(
        () => runProductionRetirement({
          rootOfflineDirectory,
          outputDirectory: path.join(rootOfflineDirectory, 'public-output'),
          metadataDirectory,
          now: new Date('2026-11-13T08:09:10Z')
        }),
        /must be separate and non-nested/
      );

      await writeFile(
        path.join(metadataDirectory, 'catalog.json.sig'),
        `${fixture.rootSignature}\n`
      );
      await assert.rejects(
        () => runProductionRetirement({
          rootOfflineDirectory,
          outputDirectory,
          metadataDirectory,
          now: new Date('2026-11-13T08:09:10Z')
        }),
        /Previous Catalog signature is invalid/
      );

      await writeFile(
        path.join(metadataDirectory, 'catalog.json.sig'),
        `${fixture.catalogSignature}\n`
      );

      const result = await runProductionRetirement({
        rootOfflineDirectory,
        outputDirectory,
        metadataDirectory,
        now: new Date('2026-11-13T08:09:10Z')
      });

      assert.equal(result.root.sequence, 3);
      assert.equal(result.root.catalogKeys[0].status, 'retired');
      assert.equal(result.root.catalogKeys[1].status, 'active');
      assert.equal(result.root.publishers[0].keys[0].status, 'retired');
      assert.equal(result.root.publishers[0].keys[1].status, 'active');
      assert.equal((await stat(path.join(outputDirectory, 'root.json'))).mode & 0o777, 0o644);
      assert.equal((await stat(path.join(outputDirectory, 'root.json.sig'))).mode & 0o777, 0o644);
      assert.deepEqual((await readdir(outputDirectory)).sort(), ['root.json', 'root.json.sig']);
      await assert.rejects(
        () => runProductionRetirement({
          rootOfflineDirectory,
          outputDirectory,
          metadataDirectory,
          now: new Date('2026-11-13T08:09:11Z')
        }),
        /Output directory already exists/
      );
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });

  it('parses explicit overlap and status ceremony arguments', () => {
    assert.deepEqual(
      parseRotationArguments([
        'overlap',
        '--root', 'root.json',
        '--root-signature', 'root.json.sig',
        '--root-private-key', '/offline/root.pem',
        '--offline-directory', '/offline/new-keys',
        '--transfer-directory', '/transfer/new-keys',
        '--catalog-key-id', 'catalog-2026-02',
        '--publisher-id', 'com.attson',
        '--publisher-key-id', 'attson-codec-2026-02',
        '--issued-at', '2026-11-12T00:00:00Z',
        '--not-after', '2027-11-12T00:00:00Z'
      ]),
      {
        mode: 'overlap',
        root: 'root.json',
        rootSignature: 'root.json.sig',
        rootPrivateKey: '/offline/root.pem',
        offlineDirectory: '/offline/new-keys',
        transferDirectory: '/transfer/new-keys',
        catalogKeyId: 'catalog-2026-02',
        publisherId: 'com.attson',
        publisherKeyId: 'attson-codec-2026-02',
        issuedAt: '2026-11-12T00:00:00Z',
        notAfter: '2027-11-12T00:00:00Z'
      }
    );
    assert.equal(
      parseRotationArguments([
        'status',
        '--root', 'root.json',
        '--root-signature', 'root.json.sig',
        '--root-private-key', '/offline/root.pem',
        '--output-directory', '/transfer/retirement',
        '--catalog-key-id', 'catalog-2026-01',
        '--publisher-id', 'com.attson',
        '--publisher-key-id', 'attson-codec-2026-01',
        '--status', 'retired',
        '--issued-at', '2026-11-13T00:00:00Z'
      ]).status,
      'retired'
    );
    assert.throws(
      () => parseRotationArguments(['overlap', '--root', 'root.json']),
      /--root-signature is required/
    );
  });

  it('creates a signed overlap Root while keeping existing keys active', () => {
    const fixture = createFixture();
    const nextCatalog = generateKeyPairSync('ed25519');
    const nextPublisher = generateKeyPairSync('ed25519');

    const result = createOverlapRoot({
      root: fixture.root,
      rootSignature: fixture.rootSignature,
      rootPublicKey: rawPublicKey(fixture.rootKeys.publicKey),
      rootPrivateKey: fixture.rootKeys.privateKey,
      catalogKey: {
        keyId: 'catalog-2026-02',
        publicKey: rawPublicKey(nextCatalog.publicKey)
      },
      publisherId: 'com.attson',
      publisherKey: {
        keyId: 'attson-codec-2026-02',
        publicKey: rawPublicKey(nextPublisher.publicKey)
      },
      issuedAt: '2026-11-12T00:00:00Z',
      notAfter: '2027-11-12T00:00:00Z'
    });

    assert.equal(result.root.sequence, 2);
    assert.equal(result.root.issuedAt, '2026-11-12T00:00:00Z');
    assert.deepEqual(
      result.root.catalogKeys.map(({ keyId, status }) => [keyId, status]),
      [['catalog-2026-01', 'active'], ['catalog-2026-02', 'active']]
    );
    assert.deepEqual(
      result.root.publishers[0].keys.map(({ keyId, status }) => [keyId, status]),
      [['attson-codec-2026-01', 'active'], ['attson-codec-2026-02', 'active']]
    );
    assert.equal(
      verifyTrustMetadataSignature(result.root, result.signature, fixture.rootKeys.publicKey),
      true
    );
  });

  it('rejects duplicate Catalog and Publisher key IDs', () => {
    const fixture = createFixture();
    const nextCatalog = generateKeyPairSync('ed25519');
    const nextPublisher = generateKeyPairSync('ed25519');
    const input = {
      root: fixture.root,
      rootSignature: fixture.rootSignature,
      rootPublicKey: rawPublicKey(fixture.rootKeys.publicKey),
      rootPrivateKey: fixture.rootKeys.privateKey,
      catalogKey: {
        keyId: 'catalog-2026-01',
        publicKey: rawPublicKey(nextCatalog.publicKey)
      },
      publisherId: 'com.attson',
      publisherKey: {
        keyId: 'attson-codec-2026-02',
        publicKey: rawPublicKey(nextPublisher.publicKey)
      },
      issuedAt: '2026-11-12T00:00:00Z',
      notAfter: '2027-11-12T00:00:00Z'
    };

    assert.throws(() => createOverlapRoot(input), /Catalog key ID already exists/);
    assert.throws(
      () => createOverlapRoot({
        ...input,
        catalogKey: { ...input.catalogKey, keyId: 'catalog-2026-02' },
        publisherKey: { ...input.publisherKey, keyId: 'attson-codec-2026-01' }
      }),
      /Publisher key ID already exists/
    );
  });

  it('rejects a Root private key that does not match the trust anchor', () => {
    const fixture = createFixture();
    const unrelatedRoot = generateKeyPairSync('ed25519');
    const nextCatalog = generateKeyPairSync('ed25519');
    const nextPublisher = generateKeyPairSync('ed25519');

    assert.throws(
      () => createOverlapRoot({
        root: fixture.root,
        rootSignature: fixture.rootSignature,
        rootPublicKey: rawPublicKey(fixture.rootKeys.publicKey),
        rootPrivateKey: unrelatedRoot.privateKey,
        catalogKey: { keyId: 'catalog-2026-02', publicKey: rawPublicKey(nextCatalog.publicKey) },
        publisherId: 'com.attson',
        publisherKey: { keyId: 'attson-codec-2026-02', publicKey: rawPublicKey(nextPublisher.publicKey) },
        issuedAt: '2026-11-12T00:00:00Z',
        notAfter: '2027-11-12T00:00:00Z'
      }),
      /does not match the trusted Root public key/
    );
  });

  it('rejects invalid overlap validity ordering', () => {
    const fixture = createFixture();
    const nextCatalog = generateKeyPairSync('ed25519');
    const nextPublisher = generateKeyPairSync('ed25519');
    const input = {
      root: fixture.root,
      rootSignature: fixture.rootSignature,
      rootPublicKey: rawPublicKey(fixture.rootKeys.publicKey),
      rootPrivateKey: fixture.rootKeys.privateKey,
      catalogKey: { keyId: 'catalog-2026-02', publicKey: rawPublicKey(nextCatalog.publicKey) },
      publisherId: 'com.attson',
      publisherKey: { keyId: 'attson-codec-2026-02', publicKey: rawPublicKey(nextPublisher.publicKey) },
      issuedAt: '2026-11-11T00:00:00Z',
      notAfter: '2027-11-12T00:00:00Z'
    };

    assert.throws(() => createOverlapRoot(input), /issuedAt must be later/);
    assert.throws(
      () => createOverlapRoot({
        ...input,
        issuedAt: '2026-11-12T00:00:00Z',
        notAfter: '2026-11-12T00:00:00Z'
      }),
      /validity must end after issuedAt/
    );
  });

  it('retires old keys only after replacement keys are active', () => {
    const fixture = createFixture();
    const overlap = createOverlap(fixture);

    const retired = createStatusRoot({
      root: overlap.root,
      rootSignature: overlap.signature,
      rootPublicKey: rawPublicKey(fixture.rootKeys.publicKey),
      rootPrivateKey: fixture.rootKeys.privateKey,
      catalogKeyId: 'catalog-2026-01',
      publisherId: 'com.attson',
      publisherKeyId: 'attson-codec-2026-01',
      status: 'retired',
      issuedAt: '2026-11-13T00:00:00Z'
    });

    assert.equal(retired.root.sequence, 3);
    assert.deepEqual(
      retired.root.catalogKeys.map(({ keyId, status }) => [keyId, status]),
      [['catalog-2026-01', 'retired'], ['catalog-2026-02', 'active']]
    );
    assert.deepEqual(
      retired.root.publishers[0].keys.map(({ keyId, status }) => [keyId, status]),
      [['attson-codec-2026-01', 'retired'], ['attson-codec-2026-02', 'active']]
    );
    assert.equal(
      verifyTrustMetadataSignature(retired.root, retired.signature, fixture.rootKeys.publicKey),
      true
    );
  });

  it('refuses to retire or revoke the only active keys', () => {
    const fixture = createFixture();
    for (const status of ['retired', 'revoked']) {
      assert.throws(
        () => createStatusRoot({
          root: fixture.root,
          rootSignature: fixture.rootSignature,
          rootPublicKey: rawPublicKey(fixture.rootKeys.publicKey),
          rootPrivateKey: fixture.rootKeys.privateKey,
          catalogKeyId: 'catalog-2026-01',
          publisherId: 'com.attson',
          publisherKeyId: 'attson-codec-2026-01',
          status,
          issuedAt: '2026-11-12T00:00:00Z'
        }),
        /must retain a usable active Catalog key/
      );
    }
  });

  it('requires replacement keys to be usable when the status Root is issued', () => {
    const fixture = createFixture();
    const overlap = createOverlap(fixture);

    assert.throws(
      () => createStatusRoot({
        root: overlap.root,
        rootSignature: overlap.signature,
        rootPublicKey: rawPublicKey(fixture.rootKeys.publicKey),
        rootPrivateKey: fixture.rootKeys.privateKey,
        catalogKeyId: 'catalog-2026-01',
        publisherId: 'com.attson',
        publisherKeyId: 'attson-codec-2026-01',
        status: 'retired',
        issuedAt: '2028-01-01T00:00:00Z'
      }),
      /usable active Catalog key/
    );
  });

  it('does not allow revoked keys to become trusted again', () => {
    const fixture = createFixture();
    const overlap = createOverlap(fixture);
    const revoked = createStatusRoot({
      root: overlap.root,
      rootSignature: overlap.signature,
      rootPublicKey: rawPublicKey(fixture.rootKeys.publicKey),
      rootPrivateKey: fixture.rootKeys.privateKey,
      catalogKeyId: 'catalog-2026-01',
      publisherId: 'com.attson',
      publisherKeyId: 'attson-codec-2026-01',
      status: 'revoked',
      issuedAt: '2026-11-13T00:00:00Z'
    });

    assert.throws(
      () => createStatusRoot({
        root: revoked.root,
        rootSignature: revoked.signature,
        rootPublicKey: rawPublicKey(fixture.rootKeys.publicKey),
        rootPrivateKey: fixture.rootKeys.privateKey,
        catalogKeyId: 'catalog-2026-01',
        publisherId: 'com.attson',
        publisherKeyId: 'attson-codec-2026-01',
        status: 'retired',
        issuedAt: '2026-11-14T00:00:00Z'
      }),
      /Revoked keys cannot change status/
    );
  });

  it('writes separated offline, transfer, and public rotation outputs', async () => {
    const fixture = createFixture();
    const directory = await mkdtemp(path.join(tmpdir(), 'attool-key-rotation-'));
    const offlineDirectory = path.join(directory, 'offline');
    const transferDirectory = path.join(directory, 'transfer');
    try {
      await writeOverlapCeremony({
        root: fixture.root,
        rootSignature: fixture.rootSignature,
        rootPublicKey: rawPublicKey(fixture.rootKeys.publicKey),
        rootPrivateKey: fixture.rootKeys.privateKey,
        offlineDirectory,
        transferDirectory,
        catalogKeyId: 'catalog-2026-02',
        publisherId: 'com.attson',
        publisherKeyId: 'attson-codec-2026-02',
        issuedAt: '2026-11-12T00:00:00Z',
        notAfter: '2027-11-12T00:00:00Z'
      });

      const publicRoot = JSON.parse(
        await readFile(path.join(transferDirectory, 'public', 'root.json'), 'utf8')
      );
      assert.equal(publicRoot.sequence, 2);
      assert.equal(
        await readFile(
          path.join(
            transferDirectory,
            'github-secrets',
            'attool',
            'ATTOOL_CATALOG_SIGNING_KEY_ID'
          ),
          'utf8'
        ),
        'catalog-2026-02\n'
      );
      assert.equal(
        await readFile(
          path.join(
            transferDirectory,
            'github-secrets',
            'atcodec',
            'ATCODEC_PUBLISHER_KEY_ID'
          ),
          'utf8'
        ),
        'attson-codec-2026-02\n'
      );
      const privateFile = path.join(offlineDirectory, 'catalog-private.pem');
      assert.match(await readFile(privateFile, 'utf8'), /BEGIN PRIVATE KEY/);
      assert.equal((await stat(privateFile)).mode & 0o777, 0o600);
      await assert.rejects(
        () => writeOverlapCeremony({
          root: fixture.root,
          rootSignature: fixture.rootSignature,
          rootPublicKey: rawPublicKey(fixture.rootKeys.publicKey),
          rootPrivateKey: fixture.rootKeys.privateKey,
          offlineDirectory,
          transferDirectory,
          catalogKeyId: 'catalog-2026-03',
          publisherId: 'com.attson',
          publisherKeyId: 'attson-codec-2026-03',
          issuedAt: '2026-11-13T00:00:00Z',
          notAfter: '2027-11-13T00:00:00Z'
        }),
        /already exists/
      );
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });

  it('rejects identical or nested ceremony output directories', async () => {
    const fixture = createFixture();
    const directory = await mkdtemp(path.join(tmpdir(), 'attool-key-paths-'));
    const output = path.join(directory, 'output');
    const input = {
      root: fixture.root,
      rootSignature: fixture.rootSignature,
      rootPublicKey: rawPublicKey(fixture.rootKeys.publicKey),
      rootPrivateKey: fixture.rootKeys.privateKey,
      catalogKeyId: 'catalog-2026-02',
      publisherId: 'com.attson',
      publisherKeyId: 'attson-codec-2026-02',
      issuedAt: '2026-11-12T00:00:00Z',
      notAfter: '2027-11-12T00:00:00Z'
    };
    try {
      await assert.rejects(
        () => writeOverlapCeremony({
          ...input,
          offlineDirectory: output,
          transferDirectory: output
        }),
        /separate and non-nested/
      );
      await assert.rejects(
        () => writeOverlapCeremony({
          ...input,
          offlineDirectory: output,
          transferDirectory: path.join(output, 'transfer')
        }),
        /separate and non-nested/
      );
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });

  it('writes a status-only Root without creating new private keys', async () => {
    const fixture = createFixture();
    const overlap = createOverlap(fixture);
    const directory = await mkdtemp(path.join(tmpdir(), 'attool-key-status-'));
    const outputDirectory = path.join(directory, 'retirement');
    try {
      const result = await writeStatusCeremony({
        root: overlap.root,
        rootSignature: overlap.signature,
        rootPublicKey: rawPublicKey(fixture.rootKeys.publicKey),
        rootPrivateKey: fixture.rootKeys.privateKey,
        outputDirectory,
        catalogKeyId: 'catalog-2026-01',
        publisherId: 'com.attson',
        publisherKeyId: 'attson-codec-2026-01',
        status: 'retired',
        issuedAt: '2026-11-13T00:00:00Z'
      });

      assert.equal(result.root.sequence, 3);
      assert.equal(
        JSON.parse(await readFile(path.join(outputDirectory, 'root.json'), 'utf8')).sequence,
        3
      );
      assert.equal(
        await readFile(path.join(outputDirectory, 'root.json.sig'), 'utf8'),
        `${result.signature}\n`
      );
      await assert.rejects(
        () => readFile(path.join(outputDirectory, 'catalog-private.pem')),
        /ENOENT/
      );
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });

  it('runs the overlap ceremony through the pnpm CLI without printing private keys', async () => {
    const fixture = createFixture();
    const directory = await mkdtemp(path.join(tmpdir(), 'attool-key-cli-'));
    const rootPath = path.join(directory, 'root.json');
    const signaturePath = path.join(directory, 'root.json.sig');
    const rootPrivateKeyPath = path.join(directory, 'root-private.pem');
    const offlineDirectory = path.join(directory, 'offline');
    const transferDirectory = path.join(directory, 'transfer');
    try {
      await Promise.all([
        writeFile(rootPath, `${JSON.stringify(fixture.root)}\n`),
        writeFile(signaturePath, `${fixture.rootSignature}\n`),
        writeFile(
          rootPrivateKeyPath,
          fixture.rootKeys.privateKey.export({ type: 'pkcs8', format: 'pem' }),
          { mode: 0o600 }
        )
      ]);
      const result = spawnSync(
        'corepack',
        [
          'pnpm', 'rotate:catalog-keys', '--', 'overlap',
          '--root', rootPath,
          '--root-signature', signaturePath,
          '--root-private-key', rootPrivateKeyPath,
          '--offline-directory', offlineDirectory,
          '--transfer-directory', transferDirectory,
          '--catalog-key-id', 'catalog-2026-02',
          '--publisher-id', 'com.attson',
          '--publisher-key-id', 'attson-codec-2026-02',
          '--issued-at', '2026-11-12T00:00:00Z',
          '--not-after', '2027-11-12T00:00:00Z'
        ],
        {
          cwd: path.resolve(import.meta.dirname, '../..'),
          encoding: 'utf8',
          env: {
            ...process.env,
            ATTOOL_CATALOG_ROOT_PUBLIC_KEY: rawPublicKey(fixture.rootKeys.publicKey)
          }
        }
      );

      assert.equal(result.status, 0, result.stderr);
      assert.match(result.stdout, /Created Root sequence 2/);
      assert.doesNotMatch(result.stdout, /BEGIN PRIVATE KEY/);
      assert.equal(
        JSON.parse(await readFile(path.join(transferDirectory, 'public', 'root.json'), 'utf8')).sequence,
        2
      );
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });
});

function createOverlap(fixture) {
  const nextCatalog = generateKeyPairSync('ed25519');
  const nextPublisher = generateKeyPairSync('ed25519');
  return createOverlapRoot({
    root: fixture.root,
    rootSignature: fixture.rootSignature,
    rootPublicKey: rawPublicKey(fixture.rootKeys.publicKey),
    rootPrivateKey: fixture.rootKeys.privateKey,
    catalogKey: {
      keyId: 'catalog-2026-02',
      publicKey: rawPublicKey(nextCatalog.publicKey)
    },
    publisherId: 'com.attson',
    publisherKey: {
      keyId: 'attson-codec-2026-02',
      publicKey: rawPublicKey(nextPublisher.publicKey)
    },
    issuedAt: '2026-11-12T00:00:00Z',
    notAfter: '2027-11-12T00:00:00Z'
  });
}

function createRetirementFixture() {
  const fixture = createFixture();
  const catalogKeys = generateKeyPairSync('ed25519');
  const publisherKeys = generateKeyPairSync('ed25519');
  const overlap = createOverlapRoot({
    root: fixture.root,
    rootSignature: fixture.rootSignature,
    rootPublicKey: rawPublicKey(fixture.rootKeys.publicKey),
    rootPrivateKey: fixture.rootKeys.privateKey,
    catalogKey: {
      keyId: 'catalog-2026-02',
      publicKey: rawPublicKey(catalogKeys.publicKey)
    },
    publisherId: 'com.attson',
    publisherKey: {
      keyId: 'attson-codec-2026-02',
      publicKey: rawPublicKey(publisherKeys.publicKey)
    },
    issuedAt: '2026-11-12T00:00:00Z',
    notAfter: '2027-11-12T00:00:00Z'
  });
  const catalog = {
    schemaVersion: 1,
    sequence: 2,
    generatedAt: '2026-11-12T01:00:00Z',
    expiresAt: '2026-11-19T01:00:00Z',
    signingKeyId: 'catalog-2026-02',
    apps: [{
      appId: 'com.attson.codec',
      name: '编码转换',
      description: 'Base64、URL、Unicode、Hex、Hash 与 JWT',
      publisherId: 'com.attson',
      icon: 'codec',
      channels: {
        stable: {
          version: '0.1.0-alpha.2',
          publishedAt: '2026-11-12T01:00:00Z',
          notes: 'Key transition release',
          publisherKeyId: 'attson-codec-2026-02',
          hostApi: { min: '1.0.0', max: '1.x' },
          permissions: ['host.theme.read'],
          artifacts: [{
            os: 'linux',
            arch: 'x86_64',
            fileName: 'AT.Codec_0.1.0-alpha.2_linux_x86_64.atpkg',
            url: 'https://github.com/attsontools/atcodec/releases/download/v0.1.0-alpha.2/AT.Codec_0.1.0-alpha.2_linux_x86_64.atpkg',
            sha256: 'a'.repeat(64),
            size: 1
          }]
        }
      }
    }]
  };
  return {
    ...fixture,
    root: overlap.root,
    rootSignature: overlap.signature,
    catalog,
    catalogSignature: signTrustMetadata(catalog, catalogKeys.privateKey)
  };
}

function createFixture() {
  const rootKeys = generateKeyPairSync('ed25519');
  const catalogKeys = generateKeyPairSync('ed25519');
  const publisherKeys = generateKeyPairSync('ed25519');
  const root = {
    schemaVersion: 1,
    sequence: 1,
    issuedAt: '2026-11-11T00:00:00Z',
    expiresAt: '2031-11-11T00:00:00Z',
    catalogKeys: [trustKey('catalog-2026-01', catalogKeys.publicKey)],
    publishers: [{
      publisherId: 'com.attson',
      name: 'Attson',
      keys: [trustKey('attson-codec-2026-01', publisherKeys.publicKey)]
    }]
  };
  return {
    root,
    rootKeys,
    rootSignature: signTrustMetadata(root, rootKeys.privateKey)
  };
}

function trustKey(keyId, publicKey) {
  return {
    keyId,
    algorithm: 'ed25519',
    publicKey: rawPublicKey(publicKey),
    status: 'active',
    notBefore: '2026-11-11T00:00:00Z',
    notAfter: '2027-11-11T00:00:00Z'
  };
}

function rawPublicKey(publicKey) {
  const jwk = publicKey.export({ format: 'jwk' });
  return Buffer.from(jwk.x, 'base64url').toString('base64');
}
