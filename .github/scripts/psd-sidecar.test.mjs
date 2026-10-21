import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const packageJson = JSON.parse(readFileSync('package.json', 'utf8'));
const releaseConfig = JSON.parse(readFileSync('src-tauri/tauri.release.conf.json', 'utf8'));
const buildScript = readFileSync('.github/scripts/build-psd-sidecar.mjs', 'utf8');
const rustBuildScript = readFileSync('src-tauri/build.rs', 'utf8');
const bridgeSource = readFileSync('src-tauri/src/ecommerce/psd_bridge.rs', 'utf8');
const workflow = readFileSync('.github/workflows/build.yml', 'utf8');
const stageScript = readFileSync('.github/scripts/stage-bundles.sh', 'utf8');
const linuxInstaller = readFileSync('src-tauri/src/updater/scripts/install-linux.sh', 'utf8');
const windowsInstaller = readFileSync('src-tauri/src/updater/scripts/update-windows.bat', 'utf8');
const windowsApply = readFileSync('src-tauri/src/updater/apply/windows.rs', 'utf8');

describe('PSD parser sidecar packaging', () => {
  it('embeds the parser in the release binary', () => {
    expect(packageJson.scripts['build:psd-sidecar']).toBeTruthy();
    expect(packageJson.scripts['tauri:build']).toContain('tauri.release.conf.json');
    expect(releaseConfig.bundle?.externalBin).toBeUndefined();
    expect(buildScript).toContain("spawnSync(output, ['--help']");
    expect(rustBuildScript).toContain('ATTOOL_PSD_BRIDGE_PATH');
    expect(bridgeSource).toContain('include_bytes!(env!("ATTOOL_PSD_BRIDGE_PATH"))');
    expect(workflow).toContain('python/requirements-build.txt');
    expect(workflow).toContain('pnpm tauri:build --target ${{ matrix.target }}');
  });

  it('keeps transition archives compatible while making the sidecar optional', () => {
    expect(stageScript).toContain('touch "$PACKAGE_DIR/sidecar/psd-bridge.exe"');
    expect(stageScript).not.toContain('tar -czf "$STAGE/${STEM}_${ARCH}.tar.gz" attool psd-bridge');
    expect(stageScript).toContain('sidecar/psd-bridge.exe');
    expect(windowsApply).not.toContain('return Err("解压后未找到 PSD 解析器"');
    expect(windowsInstaller).toContain('if exist "%~3"');
    expect(linuxInstaller).toContain('new_bin="$tmp/attool"');
  });
});
