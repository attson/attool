import {
  createHash,
  createPrivateKey,
  createPublicKey
} from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { createReadStream } from 'node:fs';
import {
  mkdtemp,
  mkdir,
  readFile,
  readdir,
  rm,
  stat,
  writeFile
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { isDeepStrictEqual } from 'node:util';

import {
  canSignWithCatalogKey,
  canVerifyWithCatalogKey,
  canonicalTrustMetadata,
  parseManifest,
  parsePluginCatalog,
  parseTrustRoot
} from '@attson/plugin-sdk';
import {
  signTrustMetadata,
  verifyManifestSignature,
  verifyTrustMetadataSignature
} from '@attson/plugin-sdk/node';

const EXPECTED_TARGETS = [
  ['linux', 'aarch64'],
  ['linux', 'x86_64'],
  ['macos', 'aarch64'],
  ['macos', 'x86_64'],
  ['windows', 'x86_64']
];
const DEFAULT_APPLICATION_REGISTRY = path.resolve(import.meta.dirname, '../catalog/apps.json');

export async function loadCatalogApplication(
  appId,
  registryPath = DEFAULT_APPLICATION_REGISTRY
) {
  const applications = await loadCatalogApplications(registryPath);
  const application = applications.find((candidate) => candidate.appId === appId);
  if (!application) throw new Error(`Unknown Catalog application: ${appId}`);
  return application;
}

export async function loadCatalogApplicationBySlug(
  slug,
  registryPath = DEFAULT_APPLICATION_REGISTRY
) {
  const applications = await loadCatalogApplications(registryPath);
  const application = applications.find((candidate) => candidate.slug === slug);
  if (!application) throw new Error(`Unknown Catalog application slug: ${slug}`);
  return application;
}

async function loadCatalogApplications(registryPath) {
  const registry = JSON.parse(await readFile(registryPath, 'utf8'));
  if (registry.schemaVersion !== 1 || !Array.isArray(registry.apps)) {
    throw new Error('Catalog application registry must use schema version 1');
  }
  for (const candidate of registry.apps) requireApplicationPolicy(candidate);
  const uniqueFields = [
    ['appId', 'app IDs'],
    ['slug', 'slugs'],
    ['repository', 'repositories'],
    ['artifactPrefix', 'artifact prefixes'],
    ['publisherKeySecret', 'Publisher key secrets']
  ];
  for (const [field, label] of uniqueFields) {
    if (new Set(registry.apps.map((candidate) => candidate[field])).size !== registry.apps.length) {
      throw new Error(`Catalog application registry contains duplicate ${label}`);
    }
  }
  return registry.apps;
}

export function createCatalog({
  root: rootInput,
  previousCatalog: previousCatalogInput,
  application,
  release,
  catalogKeyId,
  publisherKeyId,
  generatedAt,
  expiresAt
}) {
  const root = parseTrustRoot(rootInput);
  const previousCatalog = parsePluginCatalog(previousCatalogInput);
  const signingKey = root.catalogKeys.find((key) => key.keyId === catalogKeyId);
  if (!signingKey || !canSignWithCatalogKey(signingKey, generatedAt)) {
    throw new Error(`${catalogKeyId} must be an active Catalog signing key at ${generatedAt}`);
  }

  requireApplicationPolicy(application);
  const previousApp = previousCatalog.apps.find((app) => app.appId === application.appId);

  const publisher = root.publishers.find((candidate) => candidate.publisherId === application.publisherId);
  const publisherKey = publisher?.keys.find((key) => key.keyId === publisherKeyId);
  if (!publisherKey || !canSignWithCatalogKey(publisherKey, release.publishedAt)) {
    throw new Error(`${publisherKeyId} must be an active Publisher key at ${release.publishedAt}`);
  }

  const version = versionFromTag(release.tagName, application.name);
  if (previousApp && compareSemver(version, previousApp.channels.stable.version) <= 0) {
    throw new Error(
      `${application.name} version ${version} must be strictly newer than ${previousApp.channels.stable.version}`
    );
  }

  const stable = {
    version,
    publishedAt: release.publishedAt,
    notes: release.notes,
    publisherKeyId,
    hostApi: application.hostApi,
    permissions: application.permissions,
    artifacts: validateArtifacts(application, release.assets, release.tagName, version)
  };
  const nextApp = {
    appId: application.appId,
    name: application.name,
    description: application.description,
    publisherId: application.publisherId,
    icon: application.icon,
    channels: { stable }
  };
  const catalog = {
    schemaVersion: 1,
    sequence: previousCatalog.sequence + 1,
    generatedAt,
    expiresAt,
    signingKeyId: catalogKeyId,
    apps: previousApp
      ? previousCatalog.apps.map((app) => app.appId === application.appId ? nextApp : app)
      : [...previousCatalog.apps, nextApp]
  };

  return parsePluginCatalog(catalog);
}

export function renewCatalog({
  root: rootInput,
  previousCatalog: previousCatalogInput,
  catalogKeyId,
  generatedAt,
  expiresAt
}) {
  const root = parseTrustRoot(rootInput);
  const previousCatalog = parsePluginCatalog(previousCatalogInput);
  const signingKey = root.catalogKeys.find((key) => key.keyId === catalogKeyId);
  if (!signingKey || !canSignWithCatalogKey(signingKey, generatedAt)) {
    throw new Error(`${catalogKeyId} must be an active Catalog signing key at ${generatedAt}`);
  }
  if (Date.parse(generatedAt) <= Date.parse(previousCatalog.generatedAt)) {
    throw new Error('Renewed Catalog generatedAt must be later than the previous Catalog');
  }
  if (Date.parse(expiresAt) <= Date.parse(previousCatalog.expiresAt)) {
    throw new Error('Renewed Catalog expiresAt must be later than the previous Catalog');
  }
  if (Date.parse(expiresAt) <= Date.parse(generatedAt)) {
    throw new Error('Renewed Catalog expiresAt must be later than generatedAt');
  }

  return parsePluginCatalog({
    ...previousCatalog,
    sequence: previousCatalog.sequence + 1,
    generatedAt,
    expiresAt,
    signingKeyId: catalogKeyId,
    apps: previousCatalog.apps
  });
}

export function signCatalog({ root: rootInput, catalog: catalogInput, privateKey: privateKeyInput }) {
  const root = parseTrustRoot(rootInput);
  const catalog = parsePluginCatalog(catalogInput);
  const delegatedKey = root.catalogKeys.find((key) => key.keyId === catalog.signingKeyId);
  if (!delegatedKey || !canSignWithCatalogKey(delegatedKey, catalog.generatedAt)) {
    throw new Error(`${catalog.signingKeyId} must be an active Catalog signing key at ${catalog.generatedAt}`);
  }

  let privateKey;
  try {
    privateKey = privateKeyInput?.type === 'private'
      ? privateKeyInput
      : createPrivateKey(privateKeyInput);
  } catch {
    throw new Error('Catalog signing key must be an Ed25519 private key');
  }
  if (privateKey.asymmetricKeyType !== 'ed25519') {
    throw new Error('Catalog signing key must be an Ed25519 private key');
  }
  const publicKey = createPublicKey(privateKey);
  if (rawPublicKey(publicKey) !== delegatedKey.publicKey) {
    throw new Error(`Catalog private key does not match Root key ${catalog.signingKeyId}`);
  }

  const signature = signTrustMetadata(catalog, privateKey);
  if (!verifyTrustMetadataSignature(catalog, signature, publicKey)) {
    throw new Error('Generated Catalog signature could not be verified');
  }
  return signature;
}

export function verifyCatalogState({
  root: rootInput,
  rootSignature,
  rootPublicKey,
  catalog: catalogInput,
  catalogSignature,
  at
}) {
  const root = verifyRootState({ root: rootInput, rootSignature, rootPublicKey, at });

  const catalog = parsePluginCatalog(catalogInput);
  const verificationKey = root.catalogKeys.find(
    (key) => key.keyId === catalog.signingKeyId
  );
  if (!verificationKey || !canVerifyWithCatalogKey(verificationKey, catalog.generatedAt)) {
    throw new Error(
      `${catalog.signingKeyId} must be a trusted Catalog verification key at ${catalog.generatedAt}`
    );
  }
  if (!verifyTrustMetadataSignature(
    catalog,
    catalogSignature.trim(),
    publicKeyFromRaw(verificationKey.publicKey)
  )) {
    throw new Error('Previous Catalog signature is invalid');
  }
  return { root, catalog };
}

export function verifyRootState({ root: rootInput, rootSignature, rootPublicKey, at }) {
  const root = parseTrustRoot(rootInput);
  if (!verifyTrustMetadataSignature(root, rootSignature.trim(), publicKeyFromRaw(rootPublicKey))) {
    throw new Error('Root signature is invalid');
  }
  const verifiedAt = Date.parse(at);
  if (
    !Number.isFinite(verifiedAt) ||
    verifiedAt < Date.parse(root.issuedAt) ||
    verifiedAt >= Date.parse(root.expiresAt)
  ) {
    throw new Error(`Root is not valid at ${at}`);
  }
  return root;
}

export async function prepareCatalogRelease({
  root: rootInput,
  rootSignature,
  rootPublicKey,
  previousCatalog: previousCatalogInput,
  previousCatalogSignature,
  application,
  release,
  assetDirectory,
  catalogKeyId,
  catalogPrivateKey,
  publisherKeyId,
  generatedAt,
  expiresAt
}) {
  const hasPreviousCatalog = previousCatalogInput !== undefined || previousCatalogSignature !== undefined;
  if (hasPreviousCatalog && (previousCatalogInput === undefined || previousCatalogSignature === undefined)) {
    throw new Error('Previous Catalog and signature must be provided together');
  }
  const verifiedState = hasPreviousCatalog
    ? verifyCatalogState({
        root: rootInput,
        rootSignature,
        rootPublicKey,
        catalog: previousCatalogInput,
        catalogSignature: previousCatalogSignature,
        at: generatedAt
      })
    : {
        root: verifyRootState({ root: rootInput, rootSignature, rootPublicKey, at: generatedAt }),
        catalog: undefined
      };
  const { root, catalog: previousCatalog } = verifiedState;
  requireApplicationPolicy(application);
  const version = versionFromTag(release.tagName, application.name);
  if (!Array.isArray(release.assets) || release.assets.length !== EXPECTED_TARGETS.length) {
    throw new Error(`${application.name} release must contain exactly the five supported plugin targets`);
  }

  const inspectedAssets = [];
  for (const [os, arch] of EXPECTED_TARGETS) {
    const fileName = `${application.artifactPrefix}_${version}_${os}_${arch}.atpkg`;
    const releaseAsset = release.assets.find((asset) => asset.fileName === fileName);
    if (!releaseAsset) {
      throw new Error(`${application.name} release must contain exactly the five supported plugin targets`);
    }
    const inspected = await inspectPluginPackage({
      packagePath: path.join(assetDirectory, fileName),
      application,
      root,
      publishedAt: release.publishedAt,
      expectedVersion: version,
      expectedPublisherKeyId: publisherKeyId
    });
    if (releaseAsset.size !== inspected.size) {
      throw new Error(`${fileName} size does not match GitHub Release metadata`);
    }
    inspectedAssets.push({
      os,
      arch,
      fileName,
      size: inspected.size,
      sha256: inspected.sha256,
      url: releaseAsset.url
    });
  }

  const verifiedRelease = { ...release, assets: inspectedAssets };
  const catalog = previousCatalog
    ? createCatalog({
        root,
        previousCatalog,
        application,
        release: verifiedRelease,
        catalogKeyId,
        publisherKeyId,
        generatedAt,
        expiresAt
      })
    : createInitialCatalog({
        application,
        release: verifiedRelease,
        catalogKeyId,
        publisherKeyId,
        generatedAt,
        expiresAt
      });
  return {
    catalog,
    signature: signCatalog({ root, catalog, privateKey: catalogPrivateKey })
  };
}

export function prepareCatalogRenewal({
  root: rootInput,
  rootSignature,
  rootPublicKey,
  previousCatalog: previousCatalogInput,
  previousCatalogSignature,
  catalogKeyId,
  catalogPrivateKey,
  generatedAt,
  expiresAt
}) {
  const { root, catalog: previousCatalog } = verifyCatalogState({
    root: rootInput,
    rootSignature,
    rootPublicKey,
    catalog: previousCatalogInput,
    catalogSignature: previousCatalogSignature,
    at: generatedAt
  });
  const catalog = renewCatalog({
    root,
    previousCatalog,
    catalogKeyId,
    generatedAt,
    expiresAt
  });
  return {
    catalog,
    signature: signCatalog({ root, catalog, privateKey: catalogPrivateKey })
  };
}

function createInitialCatalog({ application, release, catalogKeyId, publisherKeyId, generatedAt, expiresAt }) {
  const version = versionFromTag(release.tagName, application.name);
  return parsePluginCatalog({
    schemaVersion: 1,
    sequence: 1,
    generatedAt,
    expiresAt,
    signingKeyId: catalogKeyId,
    apps: [{
      appId: application.appId,
      name: application.name,
      description: application.description,
      publisherId: application.publisherId,
      icon: application.icon,
      channels: {
        stable: {
          version,
          publishedAt: release.publishedAt,
          notes: release.notes,
          publisherKeyId,
          hostApi: application.hostApi,
          permissions: application.permissions,
          artifacts: validateArtifacts(application, release.assets, release.tagName, version)
        }
      }
    }]
  });
}

export async function inspectPluginPackage({
  packagePath,
  application,
  root: rootInput,
  publishedAt,
  expectedVersion,
  expectedPublisherKeyId
}) {
  const root = parseTrustRoot(rootInput);
  const extracted = await mkdtemp(path.join(tmpdir(), 'attool-catalog-package-'));
  try {
    const entries = listArchiveEntries(packagePath);
    if (!entries.includes('manifest.json') || !entries.includes('manifest.sig')) {
      throw new Error('Plugin package must contain manifest.json and manifest.sig');
    }
    if (new Set(entries).size !== entries.length) {
      throw new Error('Plugin package contains a duplicate entry');
    }
    for (const entry of entries) requireSafePackagePath(entry);
    extractArchive(packagePath, extracted);

    const manifest = parseManifest(JSON.parse(
      await readFile(path.join(extracted, 'manifest.json'), 'utf8')
    ));
    requireApplicationPolicy(application);
    const expectedFileName =
      `${application.artifactPrefix}_${expectedVersion}_${manifest.target.os}_${manifest.target.arch}.atpkg`;
    requireEqual(path.basename(packagePath), expectedFileName, 'file name');
    requireEqual(manifest.appId, application.appId, 'app ID');
    requireEqual(manifest.name, application.name, 'application name');
    requireEqual(manifest.version, expectedVersion, 'version');
    requireEqual(manifest.publisher.id, application.publisherId, 'publisher ID');
    requireEqual(manifest.publisher.keyId, expectedPublisherKeyId, 'Publisher Key ID');
    requireEqual(manifest.hostApi, application.hostApi, 'Host API range');
    requireEqual(manifest.permissions, application.permissions, 'permissions');

    const publisher = root.publishers.find(
      (candidate) => candidate.publisherId === manifest.publisher.id
    );
    const publisherKey = publisher?.keys.find(
      (candidate) => candidate.keyId === manifest.publisher.keyId
    );
    if (!publisherKey || !canSignWithCatalogKey(publisherKey, publishedAt)) {
      throw new Error(
        `${manifest.publisher.keyId} must be an active Publisher key at ${publishedAt}`
      );
    }
    const signature = (await readFile(path.join(extracted, 'manifest.sig'), 'utf8')).trim();
    if (!verifyManifestSignature(manifest, signature, publicKeyFromRaw(publisherKey.publicKey))) {
      throw new Error('Plugin manifest signature is invalid');
    }

    const actualFiles = (await collectFiles(extracted))
      .filter((entry) => entry !== 'manifest.json' && entry !== 'manifest.sig');
    const declaredFiles = Object.keys(manifest.files).sort();
    if (JSON.stringify(actualFiles) !== JSON.stringify(declaredFiles)) {
      throw new Error('Plugin package files do not match manifest');
    }
    for (const relative of declaredFiles) {
      const actualHash = await hashFile(path.join(extracted, relative));
      if (actualHash !== manifest.files[relative]) {
        throw new Error(`Plugin package hash mismatch for ${relative}`);
      }
    }

    const packageStat = await stat(packagePath);
    return {
      os: manifest.target.os,
      arch: manifest.target.arch,
      fileName: path.basename(packagePath),
      size: packageStat.size,
      sha256: await hashFile(packagePath),
      manifest
    };
  } finally {
    await rm(extracted, { recursive: true, force: true });
  }
}

export async function inspectCodecPackage(options) {
  return inspectPluginPackage({
    application: await loadCatalogApplication('com.attson.codec'),
    ...options
  });
}

function validateArtifacts(application, assets, tagName, version) {
  if (!Array.isArray(assets) || assets.length !== EXPECTED_TARGETS.length) {
    throw new Error(`${application.name} release must contain exactly the five supported plugin targets`);
  }

  const byTarget = new Map(assets.map((asset) => [`${asset.os}/${asset.arch}`, asset]));
  if (byTarget.size !== EXPECTED_TARGETS.length) {
    throw new Error(`${application.name} release must contain exactly the five supported plugin targets`);
  }

  return EXPECTED_TARGETS.map(([os, arch]) => {
    const asset = byTarget.get(`${os}/${arch}`);
    if (!asset) {
      throw new Error(`${application.name} release must contain exactly the five supported plugin targets`);
    }
    const fileName = `${application.artifactPrefix}_${version}_${os}_${arch}.atpkg`;
    const expectedUrl = `https://github.com/${application.repository}/releases/download/${tagName}/${fileName}`;
    if (asset.fileName !== fileName || asset.url !== expectedUrl) {
      throw new Error(`${fileName} must use an immutable ${application.repository} Release URL`);
    }
    return {
      os,
      arch,
      url: asset.url,
      fileName: asset.fileName,
      size: asset.size,
      sha256: asset.sha256
    };
  });
}

function versionFromTag(tagName, applicationName = 'Application') {
  if (typeof tagName !== 'string' || !tagName.startsWith('v')) {
    throw new Error(`${applicationName} release tag must start with v`);
  }
  return parseSemver(tagName.slice(1)).raw;
}

function compareSemver(left, right) {
  const a = parseSemver(left);
  const b = parseSemver(right);
  for (const field of ['major', 'minor', 'patch']) {
    if (a[field] !== b[field]) return a[field] < b[field] ? -1 : 1;
  }
  return comparePrerelease(a.prerelease, b.prerelease);
}

function comparePrerelease(left, right) {
  if (left.length === 0 || right.length === 0) {
    if (left.length === right.length) return 0;
    return left.length === 0 ? 1 : -1;
  }
  for (let index = 0; index < Math.max(left.length, right.length); index += 1) {
    if (left[index] === undefined || right[index] === undefined) {
      return left[index] === undefined ? -1 : 1;
    }
    if (left[index] === right[index]) continue;
    const leftNumber = /^\d+$/.test(left[index]);
    const rightNumber = /^\d+$/.test(right[index]);
    if (leftNumber && rightNumber) return Number(left[index]) < Number(right[index]) ? -1 : 1;
    if (leftNumber !== rightNumber) return leftNumber ? -1 : 1;
    return left[index] < right[index] ? -1 : 1;
  }
  return 0;
}

function parseSemver(version) {
  const match = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|[0-9]*[A-Za-z-][0-9A-Za-z-]*)(?:\.(?:0|[1-9]\d*|[0-9]*[A-Za-z-][0-9A-Za-z-]*))*))?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/.exec(version);
  if (!match) throw new Error(`Invalid SemVer: ${version}`);
  return {
    raw: version,
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
    prerelease: match[4]?.split('.') ?? []
  };
}

function rawPublicKey(publicKey) {
  const jwk = publicKey.export({ format: 'jwk' });
  return Buffer.from(jwk.x, 'base64url').toString('base64');
}

function publicKeyFromRaw(encodedKey) {
  const raw = Buffer.from(encodedKey, 'base64');
  if (raw.byteLength !== 32) throw new Error('Root contains an invalid Ed25519 public key');
  return createPublicKey({
    key: {
      kty: 'OKP',
      crv: 'Ed25519',
      x: raw.toString('base64url')
    },
    format: 'jwk'
  });
}

function listArchiveEntries(packagePath) {
  const listed = spawnSync('unzip', ['-Z1', packagePath], { encoding: 'utf8' });
  if (listed.status !== 0) throw new Error(listed.stderr || 'Unable to list plugin package');
  return listed.stdout.split(/\r?\n/).filter(Boolean).sort();
}

function requireSafePackagePath(relative) {
  if (
    relative.startsWith('/') ||
    relative.includes('\\') ||
    relative.split('/').some((part) => part === '' || part === '.' || part === '..')
  ) {
    throw new Error(`Unsafe plugin package path: ${relative}`);
  }
}

function extractArchive(packagePath, destination) {
  const extracted = spawnSync('unzip', ['-q', packagePath, '-d', destination], {
    encoding: 'utf8'
  });
  if (extracted.status !== 0) throw new Error(extracted.stderr || 'Unable to extract plugin package');
}

async function collectFiles(directory, prefix = '') {
  const output = [];
  for (const entry of await readdir(path.join(directory, prefix), { withFileTypes: true })) {
    const relative = path.posix.join(prefix, entry.name);
    if (entry.isDirectory()) output.push(...await collectFiles(directory, relative));
    else if (entry.isFile()) output.push(relative);
    else throw new Error(`Plugin package contains unsupported entry: ${relative}`);
  }
  return output.sort();
}

async function hashFile(filePath) {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(filePath)) hash.update(chunk);
  return hash.digest('hex');
}

function requireEqual(actual, expected, label) {
  if (!isDeepStrictEqual(actual, expected)) {
    throw new Error(
      `Plugin package ${label} mismatch: expected ${JSON.stringify(expected)}, received ${JSON.stringify(actual)}`
    );
  }
}

function requireApplicationPolicy(application) {
  const requiredStrings = [
    'appId',
    'slug',
    'repository',
    'artifactPrefix',
    'name',
    'description',
    'publisherId',
    'publisherKeySecret',
    'icon'
  ];
  if (!application || requiredStrings.some((field) => typeof application[field] !== 'string' || !application[field])) {
    throw new Error('Catalog application policy is invalid');
  }
  if (!/^[a-z0-9]+(?:\.[a-z0-9]+)+$/.test(application.appId)) {
    throw new Error(`Catalog application ID is invalid: ${application.appId}`);
  }
  if (!/^[a-z0-9-]+$/.test(application.slug)) {
    throw new Error(`Catalog application slug is invalid: ${application.slug}`);
  }
  if (!/^attsontools\/[a-z0-9-]+$/.test(application.repository)) {
    throw new Error(`Catalog application must use an official attsontools repository: ${application.repository}`);
  }
  if (!/^AT\.[A-Za-z0-9.]+$/.test(application.artifactPrefix)) {
    throw new Error(`Catalog artifact prefix is invalid: ${application.artifactPrefix}`);
  }
  if (!/^[A-Z][A-Z0-9_]*_PUBLISHER_KEY_ID$/.test(application.publisherKeySecret)) {
    throw new Error(`Catalog Publisher secret name is invalid: ${application.publisherKeySecret}`);
  }
  if ([application.name, application.description, application.icon].some((value) => /[\r\n]/.test(value))) {
    throw new Error(`Catalog application display metadata is invalid for ${application.appId}`);
  }
  if (
    !application.hostApi ||
    typeof application.hostApi.min !== 'string' ||
    typeof application.hostApi.max !== 'string' ||
    !Array.isArray(application.permissions)
  ) {
    throw new Error(`Catalog application policy is invalid for ${application.appId}`);
  }
}

async function main(argv = process.argv.slice(2), env = process.env) {
  const args = parseArguments(argv);
  const application = args.mode === 'release'
    ? await loadCatalogApplication(args.app)
    : undefined;
  const root = JSON.parse(await readFile(args.root, 'utf8'));
  const rootSignature = await readFile(args.rootSignature, 'utf8');
  const previousCatalog = args.catalog
    ? JSON.parse(await readFile(args.catalog, 'utf8'))
    : undefined;
  const previousCatalogSignature = args.catalogSignature
    ? await readFile(args.catalogSignature, 'utf8')
    : undefined;
  const common = {
    root,
    rootSignature,
    rootPublicKey: requiredEnvironment(env, 'ATTOOL_CATALOG_ROOT_PUBLIC_KEY'),
    previousCatalog,
    previousCatalogSignature,
    catalogKeyId: requiredEnvironment(env, 'ATTOOL_CATALOG_SIGNING_KEY_ID'),
    catalogPrivateKey: requiredEnvironment(env, 'ATTOOL_CATALOG_SIGNING_PRIVATE_KEY'),
    generatedAt: args.generatedAt,
    expiresAt: args.expiresAt
  };
  const prepared = args.mode === 'renew'
    ? prepareCatalogRenewal(common)
    : await prepareCatalogRelease({
        ...common,
        application,
        release: JSON.parse(await readFile(args.release, 'utf8')),
        assetDirectory: args.assets,
        publisherKeyId: requiredEnvironment(env, 'ATTOOL_PUBLISHER_KEY_ID')
      });

  await mkdir(args.output, { recursive: true });
  await writeFile(
    path.join(args.output, 'catalog.json'),
    `${canonicalTrustMetadata(prepared.catalog)}\n`,
    { mode: 0o644 }
  );
  await writeFile(
    path.join(args.output, 'catalog.json.sig'),
    `${prepared.signature}\n`,
    { mode: 0o644 }
  );
  const version = application
    ? prepared.catalog.apps.find((app) => app.appId === application.appId)?.channels.stable.version
    : undefined;
  const suffix = application ? ` for ${application.name} ${version}` : '';
  console.log(`${args.mode === 'renew' ? 'Renewed' : 'Prepared'} Catalog sequence ${prepared.catalog.sequence}${suffix}`);
}

function parseArguments(argv) {
  if (argv[0] === '--') argv = argv.slice(1);
  const values = {};
  for (let index = 0; index < argv.length; index += 2) {
    const name = argv[index];
    const value = argv[index + 1];
    if (!name?.startsWith('--') || value === undefined) {
      throw new Error(`Invalid argument near ${name ?? '<end>'}`);
    }
    values[name.slice(2)] = value;
  }
  const required = [
    'root',
    'root-signature',
    'output',
    'generated-at',
    'expires-at'
  ];
  const mode = values.mode ?? 'release';
  if (!['release', 'renew'].includes(mode)) {
    throw new Error('--mode must be release or renew');
  }
  if (mode === 'release') required.push('app', 'release', 'assets');
  if (mode === 'renew') required.push('catalog', 'catalog-signature');
  const missing = required.find((name) => !values[name]);
  if (missing) throw new Error(`--${missing} is required`);
  if (Boolean(values.catalog) !== Boolean(values['catalog-signature'])) {
    throw new Error('--catalog and --catalog-signature must be provided together');
  }
  return {
    mode,
    app: values.app,
    root: values.root,
    rootSignature: values['root-signature'],
    catalog: values.catalog,
    catalogSignature: values['catalog-signature'],
    release: values.release,
    assets: values.assets,
    output: values.output,
    generatedAt: values['generated-at'],
    expiresAt: values['expires-at']
  };
}

function requiredEnvironment(env, name) {
  const value = env[name]?.trim();
  if (!value) throw new Error(`${name} is required`);
  return value;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  await main();
}
