import { appendFileSync, readdirSync, statSync } from 'node:fs';
import { basename, resolve } from 'node:path';

const [label, stageValue, baselineValue] = process.argv.slice(2);
if (!label || !stageValue || !baselineValue) {
  throw new Error('usage: check-package-size.mjs <matrix-label> <stage-dir> <baseline-dir>');
}

const budgetsMiB = { macos: 55, windows: 55, linux: 75 };
const platform = label.split('-')[0];
const budgetMiB = budgetsMiB[platform];
if (!budgetMiB) throw new Error(`unsupported package-size label: ${label}`);

const suffix = packageSuffix(label);
const current = findAsset(stageValue, suffix);
const baseline = findAsset(baselineValue, suffix);
const currentBytes = statSync(current).size;
const baselineBytes = statSync(baseline).size;
const deltaBytes = currentBytes - baselineBytes;
const deltaMiB = deltaBytes / 1024 / 1024;
const line = `${label}: ${formatMiB(baselineBytes)} MiB -> ${formatMiB(currentBytes)} MiB (${deltaMiB >= 0 ? '+' : ''}${deltaMiB.toFixed(2)} MiB; budget +${budgetMiB} MiB)`;

console.log(line);
if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, `${line}\n`);
if (deltaBytes > budgetMiB * 1024 * 1024) {
  throw new Error(`${label} exceeds its package-size budget by ${(deltaMiB - budgetMiB).toFixed(2)} MiB`);
}

function packageSuffix(matrixLabel) {
  if (matrixLabel === 'macos-arm64') return '_arm64.dmg';
  if (matrixLabel === 'macos-x64') return '_amd64.dmg';
  if (matrixLabel === 'windows-x64') return '_amd64.exe';
  if (matrixLabel === 'linux-arm64') return '_arm64.deb';
  if (matrixLabel === 'linux-x64') return '_amd64.deb';
  throw new Error(`unsupported package-size label: ${matrixLabel}`);
}

function findAsset(directory, suffix) {
  const matches = readdirSync(resolve(directory))
    .filter((name) => name.startsWith('AT.Tool_') && name.endsWith(suffix))
    .map((name) => resolve(directory, name));
  if (matches.length !== 1) {
    throw new Error(`expected one *${suffix} in ${resolve(directory)}, found ${matches.map(basename).join(', ') || 'none'}`);
  }
  return matches[0];
}

function formatMiB(bytes) {
  return (bytes / 1024 / 1024).toFixed(2);
}
