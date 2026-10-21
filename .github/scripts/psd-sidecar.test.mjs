import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const packageJson = JSON.parse(readFileSync('package.json', 'utf8'));
const releaseConfig = JSON.parse(readFileSync('src-tauri/tauri.release.conf.json', 'utf8'));
const buildScript = readFileSync('.github/scripts/build-psd-sidecar.mjs', 'utf8');
const workflow = readFileSync('.github/workflows/build.yml', 'utf8');
const stageScript = readFileSync('.github/scripts/stage-bundles.sh', 'utf8');
const linuxInstaller = readFileSync('src-tauri/src/updater/scripts/install-linux.sh', 'utf8');
const windowsInstaller = readFileSync('src-tauri/src/updater/scripts/update-windows.bat', 'utf8');
const windowsApply = readFileSync('src-tauri/src/updater/apply/windows.rs', 'utf8');

describe('PSD parser sidecar packaging', () => {
  it('builds and bundles the sidecar in release builds', () => {
    expect(packageJson.scripts['build:psd-sidecar']).toBeTruthy();
    expect(packageJson.scripts['tauri:build']).toContain('tauri.release.conf.json');
    expect(releaseConfig.bundle?.externalBin).toEqual(['binaries/psd-bridge']);
    expect(buildScript).toContain("spawnSync(output, ['--help']");
    expect(workflow).toContain('python/requirements-build.txt');
    expect(workflow).toContain('pnpm tauri:build --target ${{ matrix.target }}');
  });

  it('ships the sidecar through Linux and Windows updater archives', () => {
    expect(stageScript).toContain('psd-bridge');
    expect(stageScript).toContain('sidecar/psd-bridge.exe');
    expect(linuxInstaller).toContain('psd-bridge');
    expect(windowsApply).toContain('sidecar").join("psd-bridge.exe');
    expect(windowsInstaller).toContain('copy /y "%~3" "%~4"');
  });
});
