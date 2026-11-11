import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import {
  canSignWithCatalogKey,
  parsePluginCatalog,
  parseTrustRoot
} from '@attson/plugin-sdk';

import { verifyCatalogState } from './publish-catalog.mjs';
import { writeStatusCeremony } from './rotate-catalog-keys.mjs';

const CODEC_APP_ID = 'com.attson.codec';
const PUBLISHER_ID = 'com.attson';
const USAGE = 'Usage: pnpm rotate:catalog-keys:retire:prod -- <root-offline-directory> <public-output-directory>';

export function parseProductionRetirementArguments(argv) {
  if (argv[0] === '--') argv = argv.slice(1);
  if (argv.length !== 2 || argv.some((value) => !value)) throw new Error(USAGE);
  return {
    rootOfflineDirectory: argv[0],
    outputDirectory: argv[1]
  };
}

export function createProductionRetirementPlan({
  root: rootInput,
  catalog: catalogInput,
  now = new Date()
}) {
  const root = parseTrustRoot(rootInput);
  const catalog = parsePluginCatalog(catalogInput);
  const issuedAt = productionRetirementTime(now);

  const replacementCatalogKey = root.catalogKeys.find(
    ({ keyId }) => keyId === catalog.signingKeyId
  );
  if (!replacementCatalogKey || !canSignWithCatalogKey(replacementCatalogKey, issuedAt)) {
    throw new Error(`${catalog.signingKeyId} must be an active Catalog key at ${issuedAt}`);
  }
  const retiredCatalogKeys = root.catalogKeys.filter(
    (key) => key.keyId !== replacementCatalogKey.keyId && canSignWithCatalogKey(key, issuedAt)
  );
  if (retiredCatalogKeys.length !== 1) {
    throw new Error('Production retirement requires exactly one other active Catalog key');
  }

  const codec = catalog.apps.find(({ appId }) => appId === CODEC_APP_ID);
  if (!codec) throw new Error(`Catalog does not contain ${CODEC_APP_ID}`);
  if (codec.publisherId !== PUBLISHER_ID) {
    throw new Error(`${CODEC_APP_ID} must be published by ${PUBLISHER_ID}`);
  }
  const replacementPublisherKeyId = codec.channels.stable.publisherKeyId;
  const publisher = root.publishers.find(({ publisherId }) => publisherId === PUBLISHER_ID);
  if (!publisher) throw new Error(`Publisher ${PUBLISHER_ID} is not declared by Root`);
  const replacementPublisherKey = publisher.keys.find(
    ({ keyId }) => keyId === replacementPublisherKeyId
  );
  if (!replacementPublisherKey || !canSignWithCatalogKey(replacementPublisherKey, issuedAt)) {
    throw new Error(`${replacementPublisherKeyId} must be an active ${PUBLISHER_ID} key at ${issuedAt}`);
  }
  const retiredPublisherKeys = publisher.keys.filter(
    (key) => key.keyId !== replacementPublisherKey.keyId && canSignWithCatalogKey(key, issuedAt)
  );
  if (retiredPublisherKeys.length !== 1) {
    throw new Error(`Production retirement requires exactly one other active ${PUBLISHER_ID} key`);
  }

  return {
    catalogKeyId: retiredCatalogKeys[0].keyId,
    publisherId: PUBLISHER_ID,
    publisherKeyId: retiredPublisherKeys[0].keyId,
    issuedAt
  };
}

export async function runProductionRetirement({
  rootOfflineDirectory,
  outputDirectory,
  metadataDirectory = path.resolve(import.meta.dirname, '../../catalog/v1'),
  now = new Date()
}) {
  assertSeparateDirectories(rootOfflineDirectory, outputDirectory);
  const [
    rootSource,
    rootSignature,
    catalogSource,
    catalogSignature,
    rootPublicKey,
    rootPrivateKey
  ] = await Promise.all([
    readFile(path.join(metadataDirectory, 'root.json'), 'utf8'),
    readFile(path.join(metadataDirectory, 'root.json.sig'), 'utf8'),
    readFile(path.join(metadataDirectory, 'catalog.json'), 'utf8'),
    readFile(path.join(metadataDirectory, 'catalog.json.sig'), 'utf8'),
    readFile(path.join(rootOfflineDirectory, 'root-public.txt'), 'utf8'),
    readFile(path.join(rootOfflineDirectory, 'root-private.pem'), 'utf8')
  ]);
  const root = JSON.parse(rootSource);
  const catalog = JSON.parse(catalogSource);
  const issuedAt = productionRetirementTime(now);
  const verified = verifyCatalogState({
    root,
    rootSignature,
    rootPublicKey: rootPublicKey.trim(),
    catalog,
    catalogSignature,
    at: issuedAt
  });
  const plan = createProductionRetirementPlan({
    root: verified.root,
    catalog: verified.catalog,
    now: issuedAt
  });
  const result = await writeStatusCeremony({
    root: verified.root,
    rootSignature,
    rootPublicKey: rootPublicKey.trim(),
    rootPrivateKey,
    outputDirectory,
    ...plan,
    status: 'retired'
  });
  return { ...result, plan };
}

function productionRetirementTime(now) {
  const issuedAt = new Date(now);
  if (!Number.isFinite(issuedAt.getTime())) throw new Error('Retirement time must be valid');
  issuedAt.setUTCMilliseconds(0);
  return issuedAt.toISOString();
}

function assertSeparateDirectories(firstDirectory, secondDirectory) {
  const first = path.resolve(firstDirectory);
  const second = path.resolve(secondDirectory);
  const firstToSecond = path.relative(first, second);
  const secondToFirst = path.relative(second, first);
  if (
    first === second ||
    (!firstToSecond.startsWith('..') && !path.isAbsolute(firstToSecond)) ||
    (!secondToFirst.startsWith('..') && !path.isAbsolute(secondToFirst))
  ) {
    throw new Error('Root offline and public output directories must be separate and non-nested');
  }
}

export async function main(argv = process.argv.slice(2), options = {}) {
  const args = parseProductionRetirementArguments(argv);
  const result = await runProductionRetirement({ ...args, ...options });
  console.log(`Created production retired Root sequence ${result.root.sequence}`);
  console.log(`Retired Catalog Key ID: ${result.plan.catalogKeyId}`);
  console.log(`Retired Codec Publisher Key ID: ${result.plan.publisherKeyId}`);
  console.log(`Public metadata: ${path.resolve(args.outputDirectory)}`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  await main();
}
