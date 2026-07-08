#!/usr/bin/env node
// Generate Tauri updater latest.json from a flat dir of staged + renamed bundles.
//
// Usage: node build-latest-json.mjs <staging-dir> <tag> <repo> [urlPrefix]
//   staging-dir: directory holding renamed bundle files + their .sig siblings
//   tag:         git tag (e.g. v0.2.0)
//   repo:        owner/repo (e.g. attson/attool)
//   urlPrefix:   optional prefix (e.g. https://ghfast.top/) — prepended to every
//                artifact URL to route downloads through a mirror. Empty string
//                (default) = plain github.com URLs.
//
// Looks for bundles by suffix and emits the JSON to stdout.

import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

const [, , staging, tag, repo, urlPrefixArg] = process.argv;
if (!staging || !tag || !repo) {
  console.error('usage: build-latest-json.mjs <staging-dir> <tag> <repo> [urlPrefix]');
  process.exit(1);
}
const urlPrefix = urlPrefixArg ?? '';

const version = tag.replace(/^v/, '');
const files = readdirSync(staging).filter((f) =>
  statSync(join(staging, f)).isFile()
);

function urlFor(filename) {
  const gh = `https://github.com/${repo}/releases/download/${tag}/${encodeURIComponent(
    filename
  )}`;
  return urlPrefix ? `${urlPrefix}${gh}` : gh;
}

function findBundleWithSig(suffix) {
  const file = files.find((f) => f.endsWith(suffix));
  if (!file) return null;
  const sigPath = join(staging, `${file}.sig`);
  if (!existsSync(sigPath)) {
    console.error(`skip ${file}: no .sig (platform excluded from updater)`);
    return null;
  }
  return {
    signature: readFileSync(sigPath, 'utf8').trim(),
    url: urlFor(file)
  };
}

const platforms = {};

const macArm = findBundleWithSig('_arm64.app.tar.gz');
if (macArm) platforms['darwin-aarch64'] = macArm;

const macX64 = findBundleWithSig('_amd64.app.tar.gz');
if (macX64) platforms['darwin-x86_64'] = macX64;

// Linux .deb assets are uploaded for manual install only; in-app updater
// metadata is intentionally limited to macOS and Windows.
const winX64 = findBundleWithSig('_amd64.exe');
if (winX64) platforms['windows-x86_64'] = winX64;

const output = {
  version,
  notes: '',
  pub_date: new Date().toISOString(),
  platforms,
};

console.log(JSON.stringify(output, null, 2));
