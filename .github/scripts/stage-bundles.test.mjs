import { mkdtempSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { describe, expect, it } from 'vitest';

const script = resolve('.github/scripts/stage-bundles.sh');

function touch(path, contents = 'bundle') {
  mkdirSync(resolve(path, '..'), { recursive: true });
  writeFileSync(path, contents);
}

function runStage(cwd, target, label, stage) {
  const result = spawnSync('bash', [script, target, label, stage], {
    cwd,
    encoding: 'utf8',
  });
  if (result.status !== 0) {
    throw new Error(`${result.stdout}\n${result.stderr}`);
  }
  return readdirSync(stage).sort();
}

describe('stage-bundles.sh', () => {
  it('renames mac app signatures to match the staged app tarball', () => {
    const cwd = mkdtempSync(join(tmpdir(), 'attool-stage-mac-'));
    const stage = join(cwd, 'stage');
    const bundle = join(cwd, 'src-tauri/target/aarch64-apple-darwin/release/bundle');
    touch(join(bundle, 'dmg/AT Tool_0.2.0_aarch64.dmg'));
    touch(join(bundle, 'macos/AT Tool.app.tar.gz'));
    touch(join(bundle, 'macos/AT Tool.app.tar.gz.sig'), 'sig');

    const files = runStage(cwd, 'aarch64-apple-darwin', 'macos-arm64', stage);

    expect(files).toContain('AT.Tool_arm64.app.tar.gz');
    expect(files).toContain('AT.Tool_arm64.app.tar.gz.sig');
  });

  it('stages Windows installer signatures for updater metadata', () => {
    const cwd = mkdtempSync(join(tmpdir(), 'attool-stage-win-'));
    const stage = join(cwd, 'stage');
    const bundle = join(cwd, 'src-tauri/target/x86_64-pc-windows-msvc/release/bundle');
    touch(join(bundle, 'nsis/AT Tool_0.2.0_x64-setup.exe'));
    touch(join(bundle, 'nsis/AT Tool_0.2.0_x64-setup.exe.sig'), 'sig');

    const files = runStage(cwd, 'x86_64-pc-windows-msvc', 'windows-x64', stage);

    expect(files).toEqual([
      'AT.Tool_0.2.0_amd64.exe',
      'AT.Tool_0.2.0_amd64.exe.sig',
    ]);
    expect(readFileSync(join(stage, 'AT.Tool_0.2.0_amd64.exe.sig'), 'utf8')).toBe('sig');
  });
});
