import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';

const manifest = JSON.parse(await (await import('node:fs/promises')).readFile('src-tauri/runtime-manifest.json', 'utf8'));
const python = process.env.PYTHON || (process.platform === 'win32' ? 'python' : 'python3');
const directory = mkdtempSync(resolve(tmpdir(), 'attool-yt-dlp-'));
try {
  for (const dependency of [
    { name: 'yt-dlp', version: manifest.ytDlp.version, sourceSha256: manifest.ytDlp.sha256, wheelSha256: manifest.ytDlp.wheelSha256 },
    { name: 'yt-dlp-ejs', version: manifest.ytDlp.ejs.version, sourceSha256: manifest.ytDlp.ejs.sha256, wheelSha256: manifest.ytDlp.ejs.wheelSha256 },
  ]) {
    const path = await downloadVerifiedSource(dependency, directory);
    const result = spawnSync(python, ['-m', 'pip', 'install', '--no-deps', path], { stdio: 'inherit' });
    if (result.error) throw result.error;
    if (result.status !== 0) throw new Error(`pip install ${dependency.name} failed with exit code ${result.status}`);
  }
} finally {
  rmSync(directory, { recursive: true, force: true });
}

async function downloadVerifiedSource(dependency, directory) {
  const metadataUrl = `https://pypi.org/pypi/${dependency.name}/${dependency.version}/json`;
  const metadataResponse = await fetch(metadataUrl);
  if (!metadataResponse.ok) throw new Error(`download ${metadataUrl}: HTTP ${metadataResponse.status}`);
  const metadata = await metadataResponse.json();
  const source = metadata.urls.find((file) => file.packagetype === 'sdist');
  if (!source) throw new Error(`${dependency.name} ${dependency.version} source distribution is unavailable`);
  if (source.digests?.sha256 !== dependency.sourceSha256) {
    throw new Error(`PyPI metadata checksum differs for ${dependency.name} ${dependency.version}`);
  }
  await downloadAndVerify(source.url, dependency.sourceSha256, `${dependency.name} source`);
  const wheel = metadata.urls.find((file) => file.packagetype === 'bdist_wheel' && file.filename.endsWith('-py3-none-any.whl'));
  if (!wheel || wheel.digests?.sha256 !== dependency.wheelSha256) {
    throw new Error(`PyPI wheel checksum differs for ${dependency.name} ${dependency.version}`);
  }
  const archive = await downloadAndVerify(wheel.url, dependency.wheelSha256, `${dependency.name} wheel`);
  const path = resolve(directory, wheel.filename);
  writeFileSync(path, archive);
  return path;
}

async function downloadAndVerify(url, expected, label) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`download ${url}: HTTP ${response.status}`);
  const bytes = Buffer.from(await response.arrayBuffer());
  const actual = createHash('sha256').update(bytes).digest('hex');
  if (actual !== expected) throw new Error(`${label} checksum mismatch: expected ${expected}, got ${actual}`);
  return bytes;
}
