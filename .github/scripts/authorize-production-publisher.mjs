import { readFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import { parseTrustRoot } from '@attson/plugin-sdk';

import { loadCatalogApplicationBySlug } from './publish-catalog.mjs';
import { writePublisherAuthorizationCeremony } from './rotate-catalog-keys.mjs';

const USAGE = 'Usage: pnpm authorize:publisher:prod -- <app-slug> [--root-offline <directory>] [--offline <directory>] [--transfer <directory>]';

export function parseProductionPublisherArguments(argv) {
  if (argv[0] === '--') argv = argv.slice(1);
  const [appSlug, ...options] = argv;
  if (!appSlug || appSlug.startsWith('--') || !/^[a-z0-9-]+$/.test(appSlug)) {
    throw new Error(USAGE);
  }
  const names = {
    '--root-offline': 'rootOfflineDirectory',
    '--offline': 'offlineDirectory',
    '--transfer': 'transferDirectory',
    '--metadata': 'metadataDirectory'
  };
  const parsed = { appSlug };
  for (let index = 0; index < options.length; index += 2) {
    const name = options[index];
    const value = options[index + 1];
    if (!names[name] || !value) throw new Error(USAGE);
    if (parsed[names[name]]) throw new Error(`Duplicate argument: ${name}`);
    parsed[names[name]] = value;
  }
  return parsed;
}

export function createProductionPublisherAuthorizationPlan({
  root: rootInput,
  application,
  now = new Date()
}) {
  const root = parseTrustRoot(rootInput);
  const issuedAt = new Date(now);
  if (!Number.isFinite(issuedAt.getTime())) throw new Error('Authorization time must be valid');
  issuedAt.setUTCMilliseconds(0);
  const notAfter = new Date(issuedAt);
  notAfter.setUTCFullYear(notAfter.getUTCFullYear() + 1);
  const publisher = root.publishers.find(
    (candidate) => candidate.publisherId === application.publisherId
  );
  if (!publisher) {
    throw new Error(`Publisher ${application.publisherId} is not declared by Root`);
  }
  const year = issuedAt.getUTCFullYear();
  return {
    publisherId: application.publisherId,
    publisherKeyId: nextKeyId(publisher.keys, `attson-${application.slug}-${year}-`),
    issuedAt: issuedAt.toISOString(),
    notAfter: notAfter.toISOString()
  };
}

export async function runProductionPublisherAuthorization({
  application,
  rootOfflineDirectory,
  offlineDirectory,
  transferDirectory,
  metadataDirectory = path.resolve(import.meta.dirname, '../../catalog/v1'),
  now = new Date(),
  baseDirectory = path.join(homedir(), 'OFFLINE_KEY')
}) {
  const issuedAt = new Date(now);
  if (!Number.isFinite(issuedAt.getTime())) throw new Error('Authorization time must be valid');
  const year = issuedAt.getUTCFullYear();
  const resolvedRootOffline = rootOfflineDirectory ?? path.join(baseDirectory, `attool-root-${year}`);
  const [rootSource, rootSignature, rootPublicKey, rootPrivateKey] = await Promise.all([
    readFile(path.join(metadataDirectory, 'root.json'), 'utf8'),
    readFile(path.join(metadataDirectory, 'root.json.sig'), 'utf8'),
    readFile(path.join(resolvedRootOffline, 'root-public.txt'), 'utf8'),
    readFile(path.join(resolvedRootOffline, 'root-private.pem'), 'utf8')
  ]);
  const root = JSON.parse(rootSource);
  const plan = createProductionPublisherAuthorizationPlan({ root, application, now });
  const repositorySlug = application.repository.split('/').at(-1);
  const resolvedOffline = offlineDirectory ?? path.join(
    baseDirectory,
    `${repositorySlug}-${plan.publisherKeyId}-offline`
  );
  const resolvedTransfer = transferDirectory ?? path.join(
    baseDirectory,
    `${repositorySlug}-${plan.publisherKeyId}-transfer`
  );
  assertSeparateDirectories([resolvedRootOffline, resolvedOffline, resolvedTransfer]);
  const result = await writePublisherAuthorizationCeremony({
    root,
    rootSignature,
    rootPublicKey: rootPublicKey.trim(),
    rootPrivateKey,
    offlineDirectory: resolvedOffline,
    transferDirectory: resolvedTransfer,
    ...plan,
    appSlug: application.slug,
    repositorySlug,
    publisherKeySecret: application.publisherKeySecret
  });
  return {
    ...result,
    plan,
    paths: {
      rootOfflineDirectory: resolvedRootOffline,
      offlineDirectory: resolvedOffline,
      transferDirectory: resolvedTransfer
    }
  };
}

function nextKeyId(keys, prefix) {
  const sequence = keys.reduce((highest, key) => {
    if (!key.keyId.startsWith(prefix)) return highest;
    const suffix = key.keyId.slice(prefix.length);
    return /^\d+$/.test(suffix) ? Math.max(highest, Number(suffix)) : highest;
  }, 0) + 1;
  return `${prefix}${String(sequence).padStart(2, '0')}`;
}

function assertSeparateDirectories(directories) {
  const resolved = directories.map((directory) => path.resolve(directory));
  for (let first = 0; first < resolved.length; first += 1) {
    for (let second = first + 1; second < resolved.length; second += 1) {
      const relative = path.relative(resolved[first], resolved[second]);
      const reverse = path.relative(resolved[second], resolved[first]);
      if (
        relative === '' ||
        (!relative.startsWith('..') && !path.isAbsolute(relative)) ||
        (!reverse.startsWith('..') && !path.isAbsolute(reverse))
      ) {
        throw new Error('Root offline, Publisher offline, and transfer directories must be separate and non-nested');
      }
    }
  }
}

export async function main(argv = process.argv.slice(2), options = {}) {
  const args = parseProductionPublisherArguments(argv);
  const application = await loadCatalogApplicationBySlug(args.appSlug);
  const result = await runProductionPublisherAuthorization({
    application,
    rootOfflineDirectory: args.rootOfflineDirectory,
    offlineDirectory: args.offlineDirectory,
    transferDirectory: args.transferDirectory,
    metadataDirectory: args.metadataDirectory,
    ...options
  });
  console.log(`Authorized Publisher key ${result.plan.publisherKeyId} in Root sequence ${result.root.sequence}`);
  console.log(`Offline backup: ${result.paths.offlineDirectory}`);
  console.log(`Transfer bundle: ${result.paths.transferDirectory}`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  await main();
}
