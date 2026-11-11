import {
  createPrivateKey,
  createPublicKey,
  generateKeyPairSync
} from 'node:crypto';
import { access, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import { canSignWithCatalogKey, parseTrustRoot } from '@attson/plugin-sdk';
import {
  signTrustMetadata,
  verifyTrustMetadataSignature
} from '@attson/plugin-sdk/node';

export function parseRotationArguments(argv) {
  if (argv[0] === '--') argv = argv.slice(1);
  const mode = argv[0];
  if (!['overlap', 'status'].includes(mode)) {
    throw new Error('First argument must be overlap or status');
  }
  const values = {};
  for (let index = 1; index < argv.length; index += 2) {
    const name = argv[index];
    const value = argv[index + 1];
    if (!name?.startsWith('--') || value === undefined) {
      throw new Error(`Invalid argument near ${name ?? '<end>'}`);
    }
    const key = name.slice(2);
    if (values[key] !== undefined) throw new Error(`Duplicate argument: ${name}`);
    values[key] = value;
  }

  const required = [
    'root',
    'root-signature',
    'root-private-key',
    'catalog-key-id',
    'publisher-id',
    'publisher-key-id',
    'issued-at'
  ];
  if (mode === 'overlap') {
    required.push('offline-directory', 'transfer-directory', 'not-after');
  } else {
    required.push('output-directory', 'status');
  }
  const missing = required.find((name) => !values[name]);
  if (missing) throw new Error(`--${missing} is required`);
  if (mode === 'status' && !['retired', 'revoked'].includes(values.status)) {
    throw new Error('--status must be retired or revoked');
  }

  const common = {
    mode,
    root: values.root,
    rootSignature: values['root-signature'],
    rootPrivateKey: values['root-private-key'],
    catalogKeyId: values['catalog-key-id'],
    publisherId: values['publisher-id'],
    publisherKeyId: values['publisher-key-id'],
    issuedAt: values['issued-at']
  };
  return mode === 'overlap'
    ? {
        ...common,
        offlineDirectory: values['offline-directory'],
        transferDirectory: values['transfer-directory'],
        notAfter: values['not-after']
      }
    : {
        ...common,
        outputDirectory: values['output-directory'],
        status: values.status
      };
}

export async function writeOverlapCeremony({
  root,
  rootSignature,
  rootPublicKey,
  rootPrivateKey,
  offlineDirectory,
  transferDirectory,
  catalogKeyId,
  publisherId,
  publisherKeyId,
  issuedAt,
  notAfter
}) {
  const offlinePath = path.resolve(offlineDirectory);
  const transferPath = path.resolve(transferDirectory);
  assertSeparateDirectories(offlinePath, transferPath);
  await assertMissing(offlinePath);
  await assertMissing(transferPath);

  const catalogKeys = generateKeyPairSync('ed25519');
  const publisherKeys = generateKeyPairSync('ed25519');
  const rotated = createOverlapRoot({
    root,
    rootSignature,
    rootPublicKey,
    rootPrivateKey,
    catalogKey: {
      keyId: catalogKeyId,
      publicKey: rawPublicKey(catalogKeys.publicKey)
    },
    publisherId,
    publisherKey: {
      keyId: publisherKeyId,
      publicKey: rawPublicKey(publisherKeys.publicKey)
    },
    issuedAt,
    notAfter
  });

  let offlineCreated = false;
  let transferCreated = false;
  try {
    await mkdir(offlinePath, { mode: 0o700 });
    offlineCreated = true;
    await mkdir(transferPath, { mode: 0o700 });
    transferCreated = true;

    const publicDirectory = path.join(transferPath, 'public');
    const attoolSecrets = path.join(transferPath, 'github-secrets', 'attool');
    const atcodecSecrets = path.join(transferPath, 'github-secrets', 'atcodec');
    await Promise.all([
      mkdir(publicDirectory, { recursive: true, mode: 0o700 }),
      mkdir(attoolSecrets, { recursive: true, mode: 0o700 }),
      mkdir(atcodecSecrets, { recursive: true, mode: 0o700 })
    ]);

    const catalogPrivateKey = exportPrivateKey(catalogKeys.privateKey);
    const publisherPrivateKey = exportPrivateKey(publisherKeys.privateKey);
    const catalogPublicKey = rawPublicKey(catalogKeys.publicKey);
    const publisherPublicKey = rawPublicKey(publisherKeys.publicKey);
    await Promise.all([
      writeSecret(path.join(offlinePath, 'catalog-private.pem'), catalogPrivateKey),
      writeSecret(path.join(offlinePath, 'codec-publisher-private.pem'), publisherPrivateKey),
      writePublic(path.join(publicDirectory, 'root.json'), `${JSON.stringify(rotated.root, null, 2)}\n`),
      writePublic(path.join(publicDirectory, 'root.json.sig'), `${rotated.signature}\n`),
      writePublic(path.join(publicDirectory, 'catalog-public-key.txt'), `${catalogPublicKey}\n`),
      writePublic(path.join(publicDirectory, 'codec-publisher-public-key.txt'), `${publisherPublicKey}\n`),
      writeSecret(path.join(attoolSecrets, 'ATTOOL_CATALOG_SIGNING_KEY_ID'), `${catalogKeyId}\n`),
      writeSecret(path.join(attoolSecrets, 'ATTOOL_CATALOG_SIGNING_PRIVATE_KEY'), catalogPrivateKey),
      writeSecret(path.join(attoolSecrets, 'ATCODEC_PUBLISHER_KEY_ID'), `${publisherKeyId}\n`),
      writeSecret(path.join(atcodecSecrets, 'ATCODEC_PUBLISHER_KEY_ID'), `${publisherKeyId}\n`),
      writeSecret(path.join(atcodecSecrets, 'ATCODEC_PUBLISHER_PRIVATE_KEY'), publisherPrivateKey),
      writeSecret(path.join(atcodecSecrets, 'ATCODEC_PUBLISHER_PUBLIC_KEY'), `${publisherPublicKey}\n`)
    ]);
  } catch (error) {
    if (offlineCreated) await rm(offlinePath, { recursive: true, force: true });
    if (transferCreated) await rm(transferPath, { recursive: true, force: true });
    throw error;
  }

  return rotated;
}

export async function writePublisherAuthorizationCeremony({
  root,
  rootSignature,
  rootPublicKey,
  rootPrivateKey,
  offlineDirectory,
  transferDirectory,
  publisherId,
  publisherKeyId,
  issuedAt,
  notAfter,
  appSlug,
  repositorySlug,
  publisherKeySecret
}) {
  const offlinePath = path.resolve(offlineDirectory);
  const transferPath = path.resolve(transferDirectory);
  assertSeparateDirectories(offlinePath, transferPath);
  await assertMissing(offlinePath);
  await assertMissing(transferPath);
  if (!/^[a-z0-9-]+$/.test(appSlug) || !/^[a-z0-9-]+$/.test(repositorySlug)) {
    throw new Error('Application and repository slugs must use lowercase ASCII');
  }
  if (!/^[A-Z][A-Z0-9_]*_KEY_ID$/.test(publisherKeySecret)) {
    throw new Error('Publisher key secret name is invalid');
  }

  const publisherKeys = generateKeyPairSync('ed25519');
  const authorized = createPublisherAuthorizationRoot({
    root,
    rootSignature,
    rootPublicKey,
    rootPrivateKey,
    publisherId,
    publisherKey: {
      keyId: publisherKeyId,
      publicKey: rawPublicKey(publisherKeys.publicKey)
    },
    issuedAt,
    notAfter
  });

  let offlineCreated = false;
  let transferCreated = false;
  try {
    await mkdir(offlinePath, { mode: 0o700 });
    offlineCreated = true;
    await mkdir(transferPath, { mode: 0o700 });
    transferCreated = true;
    const publicDirectory = path.join(transferPath, 'public');
    const attoolSecrets = path.join(transferPath, 'github-secrets', 'attool');
    const applicationSecrets = path.join(transferPath, 'github-secrets', repositorySlug);
    await Promise.all([
      mkdir(publicDirectory, { recursive: true, mode: 0o700 }),
      mkdir(attoolSecrets, { recursive: true, mode: 0o700 }),
      mkdir(applicationSecrets, { recursive: true, mode: 0o700 })
    ]);

    const privateKey = exportPrivateKey(publisherKeys.privateKey);
    const publicKey = rawPublicKey(publisherKeys.publicKey);
    const secretPrefix = publisherKeySecret.slice(0, -'_KEY_ID'.length);
    await Promise.all([
      writeSecret(path.join(offlinePath, 'publisher-private.pem'), privateKey),
      writePublic(path.join(publicDirectory, 'root.json'), `${JSON.stringify(authorized.root, null, 2)}\n`),
      writePublic(path.join(publicDirectory, 'root.json.sig'), `${authorized.signature}\n`),
      writePublic(path.join(publicDirectory, `${appSlug}-publisher-public-key.txt`), `${publicKey}\n`),
      writeSecret(path.join(attoolSecrets, publisherKeySecret), `${publisherKeyId}\n`),
      writeSecret(path.join(applicationSecrets, publisherKeySecret), `${publisherKeyId}\n`),
      writeSecret(path.join(applicationSecrets, `${secretPrefix}_PRIVATE_KEY`), privateKey),
      writeSecret(path.join(applicationSecrets, `${secretPrefix}_PUBLIC_KEY`), `${publicKey}\n`)
    ]);
  } catch (error) {
    if (offlineCreated) await rm(offlinePath, { recursive: true, force: true });
    if (transferCreated) await rm(transferPath, { recursive: true, force: true });
    throw error;
  }

  return authorized;
}

export async function writeStatusCeremony({
  root,
  rootSignature,
  rootPublicKey,
  rootPrivateKey,
  outputDirectory,
  catalogKeyId,
  publisherId,
  publisherKeyId,
  status,
  issuedAt
}) {
  const outputPath = path.resolve(outputDirectory);
  await assertMissing(outputPath);
  const updated = createStatusRoot({
    root,
    rootSignature,
    rootPublicKey,
    rootPrivateKey,
    catalogKeyId,
    publisherId,
    publisherKeyId,
    status,
    issuedAt
  });

  let outputCreated = false;
  try {
    await mkdir(outputPath, { mode: 0o700 });
    outputCreated = true;
    await Promise.all([
      writePublic(path.join(outputPath, 'root.json'), `${JSON.stringify(updated.root, null, 2)}\n`),
      writePublic(path.join(outputPath, 'root.json.sig'), `${updated.signature}\n`)
    ]);
  } catch (error) {
    if (outputCreated) await rm(outputPath, { recursive: true, force: true });
    throw error;
  }
  return updated;
}

export function createOverlapRoot({
  root: rootInput,
  rootSignature,
  rootPublicKey,
  rootPrivateKey: rootPrivateKeyInput,
  catalogKey,
  publisherId,
  publisherKey,
  issuedAt,
  notAfter
}) {
  const root = verifyRootAuthority({
    root: rootInput,
    rootSignature,
    rootPublicKey,
    rootPrivateKey: rootPrivateKeyInput
  });
  if (Date.parse(issuedAt) <= Date.parse(root.issuedAt)) {
    throw new Error('Rotated Root issuedAt must be later than the current Root');
  }
  if (Date.parse(notAfter) <= Date.parse(issuedAt) || Date.parse(notAfter) > Date.parse(root.expiresAt)) {
    throw new Error('Delegated key validity must end after issuedAt and before Root expiry');
  }
  const publisher = root.publishers.find((candidate) => candidate.publisherId === publisherId);
  if (!publisher) throw new Error(`Publisher ${publisherId} is not declared by Root`);
  if (root.catalogKeys.some((key) => key.keyId === catalogKey.keyId)) {
    throw new Error(`Catalog key ID already exists: ${catalogKey.keyId}`);
  }
  if (publisher.keys.some((key) => key.keyId === publisherKey.keyId)) {
    throw new Error(`Publisher key ID already exists: ${publisherKey.keyId}`);
  }

  const nextRoot = parseTrustRoot({
    ...root,
    sequence: root.sequence + 1,
    issuedAt,
    catalogKeys: [
      ...root.catalogKeys,
      delegatedKey(catalogKey, issuedAt, notAfter)
    ],
    publishers: root.publishers.map((candidate) => candidate.publisherId === publisherId
      ? {
          ...candidate,
          keys: [...candidate.keys, delegatedKey(publisherKey, issuedAt, notAfter)]
        }
      : candidate)
  });
  return signRoot(nextRoot, rootPrivateKeyInput);
}

export function createPublisherAuthorizationRoot({
  root: rootInput,
  rootSignature,
  rootPublicKey,
  rootPrivateKey: rootPrivateKeyInput,
  publisherId,
  publisherKey,
  issuedAt,
  notAfter
}) {
  const root = verifyRootAuthority({
    root: rootInput,
    rootSignature,
    rootPublicKey,
    rootPrivateKey: rootPrivateKeyInput
  });
  if (Date.parse(issuedAt) <= Date.parse(root.issuedAt)) {
    throw new Error('Authorized Root issuedAt must be later than the current Root');
  }
  if (Date.parse(notAfter) <= Date.parse(issuedAt) || Date.parse(notAfter) > Date.parse(root.expiresAt)) {
    throw new Error('Delegated key validity must end after issuedAt and before Root expiry');
  }
  const publisher = root.publishers.find((candidate) => candidate.publisherId === publisherId);
  if (!publisher) throw new Error(`Publisher ${publisherId} is not declared by Root`);
  if (publisher.keys.some((key) => key.keyId === publisherKey.keyId)) {
    throw new Error(`Publisher key ID already exists: ${publisherKey.keyId}`);
  }

  const nextRoot = parseTrustRoot({
    ...root,
    sequence: root.sequence + 1,
    issuedAt,
    catalogKeys: root.catalogKeys,
    publishers: root.publishers.map((candidate) => candidate.publisherId === publisherId
      ? {
          ...candidate,
          keys: [...candidate.keys, delegatedKey(publisherKey, issuedAt, notAfter)]
        }
      : candidate)
  });
  return signRoot(nextRoot, rootPrivateKeyInput);
}

export function createStatusRoot({
  root: rootInput,
  rootSignature,
  rootPublicKey,
  rootPrivateKey: rootPrivateKeyInput,
  catalogKeyId,
  publisherId,
  publisherKeyId,
  status,
  issuedAt
}) {
  if (!['retired', 'revoked'].includes(status)) {
    throw new Error('Delegated key status must be retired or revoked');
  }
  const root = verifyRootAuthority({
    root: rootInput,
    rootSignature,
    rootPublicKey,
    rootPrivateKey: rootPrivateKeyInput
  });
  if (Date.parse(issuedAt) <= Date.parse(root.issuedAt)) {
    throw new Error('Updated Root issuedAt must be later than the current Root');
  }
  const publisher = root.publishers.find((candidate) => candidate.publisherId === publisherId);
  if (!publisher) throw new Error(`Publisher ${publisherId} is not declared by Root`);
  const catalogKey = root.catalogKeys.find((key) => key.keyId === catalogKeyId);
  if (!catalogKey) {
    throw new Error(`Catalog key ${catalogKeyId} is not declared by Root`);
  }
  const publisherKey = publisher.keys.find((key) => key.keyId === publisherKeyId);
  if (!publisherKey) {
    throw new Error(`Publisher key ${publisherKeyId} is not declared by Root`);
  }
  if (catalogKey.status === 'revoked' || publisherKey.status === 'revoked') {
    throw new Error('Revoked keys cannot change status');
  }

  const nextRoot = parseTrustRoot({
    ...root,
    sequence: root.sequence + 1,
    issuedAt,
    catalogKeys: root.catalogKeys.map((key) => key.keyId === catalogKeyId
      ? { ...key, status }
      : key),
    publishers: root.publishers.map((candidate) => candidate.publisherId === publisherId
      ? {
          ...candidate,
          keys: candidate.keys.map((key) => key.keyId === publisherKeyId
            ? { ...key, status }
            : key)
        }
      : candidate)
  });
  if (!nextRoot.catalogKeys.some((key) => canSignWithCatalogKey(key, issuedAt))) {
    throw new Error('Updated Root must retain a usable active Catalog key');
  }
  const nextPublisher = nextRoot.publishers.find((candidate) => candidate.publisherId === publisherId);
  if (!nextPublisher.keys.some((key) => canSignWithCatalogKey(key, issuedAt))) {
    throw new Error(`Updated Root must retain a usable active key for ${publisherId}`);
  }
  return signRoot(nextRoot, rootPrivateKeyInput);
}

function verifyRootAuthority({ root: rootInput, rootSignature, rootPublicKey, rootPrivateKey }) {
  const root = parseTrustRoot(rootInput);
  const publicKey = publicKeyFromRaw(rootPublicKey);
  if (!verifyTrustMetadataSignature(root, rootSignature.trim(), publicKey)) {
    throw new Error('Current Root signature is invalid');
  }
  const privateKey = privateKeyFromInput(rootPrivateKey);
  if (rawPublicKey(createPublicKey(privateKey)) !== rootPublicKey) {
    throw new Error('Root private key does not match the trusted Root public key');
  }
  return root;
}

function signRoot(root, privateKeyInput) {
  const privateKey = privateKeyFromInput(privateKeyInput);
  const signature = signTrustMetadata(root, privateKey);
  if (!verifyTrustMetadataSignature(root, signature, createPublicKey(privateKey))) {
    throw new Error('Generated Root signature could not be verified');
  }
  return { root, signature };
}

function delegatedKey(key, notBefore, notAfter) {
  return {
    keyId: key.keyId,
    algorithm: 'ed25519',
    publicKey: key.publicKey,
    status: 'active',
    notBefore,
    notAfter
  };
}

function assertSeparateDirectories(first, second) {
  const firstToSecond = path.relative(first, second);
  const secondToFirst = path.relative(second, first);
  if (
    first === second ||
    (!firstToSecond.startsWith('..') && !path.isAbsolute(firstToSecond)) ||
    (!secondToFirst.startsWith('..') && !path.isAbsolute(secondToFirst))
  ) {
    throw new Error('Offline and transfer directories must be separate and non-nested');
  }
}

async function assertMissing(target) {
  try {
    await access(target);
  } catch (error) {
    if (error.code === 'ENOENT') return;
    throw error;
  }
  throw new Error(`Output directory already exists: ${target}`);
}

function exportPrivateKey(privateKey) {
  return privateKey.export({ type: 'pkcs8', format: 'pem' });
}

function writeSecret(target, content) {
  return writeFile(target, content, { flag: 'wx', mode: 0o600 });
}

function writePublic(target, content) {
  return writeFile(target, content, { flag: 'wx', mode: 0o644 });
}

function privateKeyFromInput(input) {
  let key;
  try {
    key = input?.type === 'private' ? input : createPrivateKey(input);
  } catch {
    throw new Error('Root private key must be an Ed25519 private key');
  }
  if (key.asymmetricKeyType !== 'ed25519') {
    throw new Error('Root private key must be an Ed25519 private key');
  }
  return key;
}

function rawPublicKey(publicKey) {
  const jwk = publicKey.export({ format: 'jwk' });
  return Buffer.from(jwk.x, 'base64url').toString('base64');
}

function publicKeyFromRaw(encodedKey) {
  const raw = Buffer.from(encodedKey, 'base64');
  if (raw.byteLength !== 32) throw new Error('Root public key must be a raw Ed25519 public key');
  return createPublicKey({
    key: {
      kty: 'OKP',
      crv: 'Ed25519',
      x: raw.toString('base64url')
    },
    format: 'jwk'
  });
}

export async function main(argv = process.argv.slice(2), env = process.env) {
  const args = parseRotationArguments(argv);
  const root = JSON.parse(await readFile(args.root, 'utf8'));
  const rootSignature = await readFile(args.rootSignature, 'utf8');
  const rootPrivateKey = await readFile(args.rootPrivateKey, 'utf8');
  const common = {
    root,
    rootSignature,
    rootPublicKey: requiredEnvironment(env, 'ATTOOL_CATALOG_ROOT_PUBLIC_KEY'),
    rootPrivateKey,
    catalogKeyId: args.catalogKeyId,
    publisherId: args.publisherId,
    publisherKeyId: args.publisherKeyId,
    issuedAt: args.issuedAt
  };
  const result = args.mode === 'overlap'
    ? await writeOverlapCeremony({
        ...common,
        offlineDirectory: args.offlineDirectory,
        transferDirectory: args.transferDirectory,
        notAfter: args.notAfter
      })
    : await writeStatusCeremony({
        ...common,
        outputDirectory: args.outputDirectory,
        status: args.status
      });
  const publicOutput = args.mode === 'overlap'
    ? path.join(path.resolve(args.transferDirectory), 'public')
    : path.resolve(args.outputDirectory);
  console.log(`Created Root sequence ${result.root.sequence} in ${publicOutput}`);
}

function requiredEnvironment(env, name) {
  const value = env[name]?.trim();
  if (!value) throw new Error(`${name} is required`);
  return value;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  await main();
}
