import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { describe, expect, it } from 'vitest';

const script = resolve('.github/scripts/build-latest-json.mjs');

function write(path, contents = 'bundle') {
  writeFileSync(path, contents);
}

function buildLatest(files) {
  const dir = mkdtempSync(join(tmpdir(), 'attool-latest-'));
  for (const [name, contents] of Object.entries(files)) {
    write(join(dir, name), contents);
  }
  const result = spawnSync('node', [script, dir, 'v0.2.0', 'attson/attool'], {
    encoding: 'utf8',
  });
  if (result.status !== 0) {
    throw new Error(`${result.stdout}\n${result.stderr}`);
  }
  return JSON.parse(result.stdout);
}

describe('build-latest-json.mjs', () => {
  it('emits updater platforms for signed mac and Windows artifacts only', () => {
    const latest = buildLatest({
      'AT Tool_arm64.app.tar.gz': 'mac arm',
      'AT Tool_arm64.app.tar.gz.sig': 'mac arm sig\n',
      'AT Tool_amd64.app.tar.gz': 'mac x64',
      'AT Tool_amd64.app.tar.gz.sig': 'mac x64 sig\n',
      'AT Tool_0.2.0_amd64.exe': 'win installer',
      'AT Tool_0.2.0_amd64.exe.sig': 'win sig\n',
      'AT Tool_0.2.0_amd64.deb': 'linux x64',
      'AT Tool_0.2.0_amd64.deb.sig': 'linux x64 sig\n',
      'AT Tool_0.2.0_arm64.deb': 'linux arm',
      'AT Tool_0.2.0_arm64.deb.sig': 'linux arm sig\n',
    });

    expect(Object.keys(latest.platforms).sort()).toEqual([
      'darwin-aarch64',
      'darwin-x86_64',
      'windows-x86_64',
    ]);
    expect(latest.platforms['darwin-aarch64'].signature).toBe('mac arm sig');
    expect(latest.platforms['windows-x86_64'].url).toBe(
      'https://github.com/attson/attool/releases/download/v0.2.0/AT%20Tool_0.2.0_amd64.exe'
    );
  });
});
