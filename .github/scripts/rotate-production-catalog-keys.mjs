import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import { parseTrustRoot } from '@attson/plugin-sdk';

import { writeOverlapCeremony } from './rotate-catalog-keys.mjs';

const PUBLISHER_ID = 'com.attson';
const USAGE = 'Usage: pnpm rotate:catalog-keys:prod -- <root-offline-directory> <new-offline-directory> <new-transfer-directory>';

export function parseProductionArguments(argv) {
  if (argv[0] === '--') argv = argv.slice(1);
  if (argv.length !== 3 || argv.some((value) => !value)) throw new Error(USAGE);
  return {
    rootOfflineDirectory: argv[0],
    offlineDirectory: argv[1],
    transferDirectory: argv[2]
  };
}

export function createProductionRotationPlan({ root: rootInput, now = new Date() }) {
  const root = parseTrustRoot(rootInput);
  const issuedAt = new Date(now);
  if (!Number.isFinite(issuedAt.getTime())) throw new Error('Rotation time must be valid');
  issuedAt.setUTCMilliseconds(0);
  const notAfter = new Date(issuedAt);
  notAfter.setUTCFullYear(notAfter.getUTCFullYear() + 1);

  const publisher = root.publishers.find((candidate) => candidate.publisherId === PUBLISHER_ID);
  if (!publisher) throw new Error(`Publisher ${PUBLISHER_ID} is not declared by Root`);
  const year = issuedAt.getUTCFullYear();
  return {
    catalogKeyId: nextKeyId(root.catalogKeys, `catalog-${year}-`),
    publisherId: PUBLISHER_ID,
    publisherKeyId: nextKeyId(publisher.keys, `attson-codec-${year}-`),
    issuedAt: issuedAt.toISOString(),
    notAfter: notAfter.toISOString()
  };
}

export async function runProductionRotation({
  rootOfflineDirectory,
  offlineDirectory,
  transferDirectory,
  metadataDirectory = path.resolve(import.meta.dirname, '../../catalog/v1'),
  now = new Date()
}) {
  assertSeparateDirectories([
    rootOfflineDirectory,
    offlineDirectory,
    transferDirectory
  ]);
  const [rootSource, rootSignature, rootPublicKey, rootPrivateKey] = await Promise.all([
    readFile(path.join(metadataDirectory, 'root.json'), 'utf8'),
    readFile(path.join(metadataDirectory, 'root.json.sig'), 'utf8'),
    readFile(path.join(rootOfflineDirectory, 'root-public.txt'), 'utf8'),
    readFile(path.join(rootOfflineDirectory, 'root-private.pem'), 'utf8')
  ]);
  const root = JSON.parse(rootSource);
  const plan = createProductionRotationPlan({ root, now });
  const result = await writeOverlapCeremony({
    root,
    rootSignature,
    rootPublicKey: rootPublicKey.trim(),
    rootPrivateKey,
    offlineDirectory,
    transferDirectory,
    ...plan
  });
  return { ...result, plan };
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
        throw new Error('Root offline, new offline, and transfer directories must be separate and non-nested');
      }
    }
  }
}

export async function main(argv = process.argv.slice(2), options = {}) {
  const args = parseProductionArguments(argv);
  const result = await runProductionRotation({ ...args, ...options });
  console.log(`Created production overlap Root sequence ${result.root.sequence}`);
  console.log(`Catalog Key ID: ${result.plan.catalogKeyId}`);
  console.log(`Codec Publisher Key ID: ${result.plan.publisherKeyId}`);
  console.log(`Public metadata: ${path.join(path.resolve(args.transferDirectory), 'public')}`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  await main();
}
