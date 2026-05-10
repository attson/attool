#!/usr/bin/env node
// Generate Tauri updater latest.json from a flat dir of staged + renamed bundles.
//
// Usage: node build-latest-json.mjs <staging-dir> <tag> <repo>
//   staging-dir: directory holding renamed bundle files + their .sig siblings
//   tag:         git tag (e.g. v0.2.0)
//   repo:        owner/repo (e.g. attson/attool)
//
// Looks for bundles by suffix and emits the JSON to stdout.

import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

const [, , staging, tag, repo] = process.argv;
if (!staging || !tag || !repo) {
  console.error('usage: build-latest-json.mjs <staging-dir> <tag> <repo>');
  process.exit(1);
}

const version = tag.replace(/^v/, '');
const files = readdirSync(staging).filter((f) =>
  statSync(join(staging, f)).isFile()
);

function urlFor(filename) {
  return `https://github.com/${repo}/releases/download/${tag}/${encodeURIComponent(
    filename
  )}`;
}

function readSig(filename) {
  return readFileSync(join(staging, `${filename}.sig`), 'utf8').trim();
}

function findBySuffix(suffix) {
  return files.find((f) => f.endsWith(suffix));
}

const platforms = {};

const macArm = findBySuffix('_arm64.app.tar.gz');
if (macArm) {
  platforms['darwin-aarch64'] = { signature: readSig(macArm), url: urlFor(macArm) };
}

const macX64 = findBySuffix('_amd64.app.tar.gz');
if (macX64) {
  platforms['darwin-x86_64'] = { signature: readSig(macX64), url: urlFor(macX64) };
}

const linuxX64 = findBySuffix('_amd64.deb');
if (linuxX64) {
  platforms['linux-x86_64'] = { signature: readSig(linuxX64), url: urlFor(linuxX64) };
}

const linuxArm = findBySuffix('_arm64.deb');
if (linuxArm) {
  platforms['linux-aarch64'] = { signature: readSig(linuxArm), url: urlFor(linuxArm) };
}

const winX64 = findBySuffix('_amd64.exe');
if (winX64) {
  platforms['windows-x86_64'] = { signature: readSig(winX64), url: urlFor(winX64) };
}

const output = {
  version,
  notes: '',
  pub_date: new Date().toISOString(),
  platforms,
};

console.log(JSON.stringify(output, null, 2));
