import { existsSync, readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const manifestPath = 'src-tauri/runtime-manifest.json';

describe('bundled runtime manifest', () => {
  it('pins every application-owned engine and OCR language source', () => {
    expect(existsSync(manifestPath)).toBe(true);
    const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));

    expect(manifest.tesseract.version).toBe('5.5.3');
    expect(manifest.tesseract.vcpkgCommit).toMatch(/^[0-9a-f]{40}$/);
    expect(manifest.tessdata.commit).toMatch(/^[0-9a-f]{40}$/);
    expect(manifest.tessdata.languages).toEqual(['eng', 'chi_sim', 'chi_tra', 'jpn']);
    expect(manifest.aria2.version).toBe('1.37.0');
    expect(manifest.aria2.commit).toMatch(/^[0-9a-f]{40}$/);
    expect(manifest.ytDlp.version).toMatch(/^\d{4}\.\d{2}\.\d{2}$/);
    expect(manifest.ytDlp.ejs.sha256).toMatch(/^[0-9a-f]{64}$/);
    expect(manifest.ytDlp.wheelSha256).toMatch(/^[0-9a-f]{64}$/);
    expect(manifest.ytDlp.ejs.wheelSha256).toMatch(/^[0-9a-f]{64}$/);
    expect(Object.keys(manifest.quickJs.assets)).toHaveLength(5);
    for (const asset of Object.values(manifest.quickJs.assets)) expect(asset.sha256).toMatch(/^[0-9a-f]{64}$/);
  });

  it('runs runtime preparation before every release build', () => {
    const packageJson = JSON.parse(readFileSync('package.json', 'utf8'));
    const tauriConfig = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8'));

    expect(packageJson.scripts['build:runtime']).toBeTruthy();
    expect(tauriConfig.build.beforeBuildCommand).toContain('pnpm build:runtime');
  });

  it('embeds prepared runtime files into the release executable', () => {
    const rustBuild = readFileSync('src-tauri/build.rs', 'utf8');
    const runtimeModule = readFileSync('src-tauri/src/bundled_tools.rs', 'utf8');

    expect(rustBuild).toContain('bundled_tools.rs');
    expect(rustBuild).toContain('EmbeddedFile');
    expect(runtimeModule).toContain('include!(concat!(env!("OUT_DIR"), "/bundled_tools.rs"))');
  });

  it('does not tell users to install bundled engines', () => {
    const ocrPane = readFileSync('src/components/image/OcrPane.vue', 'utf8');
    const ocrBackend = readFileSync('src-tauri/src/imaging/ocr.rs', 'utf8');
    const downloadBackend = readFileSync('src-tauri/src/lib.rs', 'utf8');

    expect(ocrPane).not.toContain('brew install tesseract');
    expect(ocrBackend).not.toContain('apt install tesseract');
    expect(downloadBackend).not.toContain('brew install aria2');
  });

  it('declares the Linux GStreamer ABI and stages every recording element', () => {
    const tauriConfig = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8'));
    const prepareRuntime = readFileSync('.github/scripts/prepare-runtime.mjs', 'utf8');
    const dependencies = tauriConfig.bundle.linux.deb.depends;

    expect(dependencies).toContain('libgstreamer1.0-0');
    expect(dependencies).toContain('gstreamer1.0-plugins-base');
    expect(dependencies).toContain('gstreamer1.0-plugins-good');
    for (const element of ['appsrc', 'videoconvert', 'videocrop', 'videoscale', 'queue', 'vp8enc', 'webmmux', 'filesink']) {
      expect(prepareRuntime).toContain(`'${element}'`);
    }
  });

  it('builds native engines from pinned sources on every target', () => {
    const vcpkg = JSON.parse(readFileSync('src-tauri/vcpkg.json', 'utf8'));
    const workflow = readFileSync('.github/workflows/build.yml', 'utf8');
    const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));

    expect(vcpkg['builtin-baseline']).toBe(manifest.tesseract.vcpkgCommit);
    expect(vcpkg.dependencies).toContain('tesseract');
    expect(vcpkg.dependencies).not.toContain('aria2');
    expect(workflow).toContain(`vcpkgGitCommitId: ${manifest.tesseract.vcpkgCommit}`);
    expect(workflow).toContain('build-aria2.mjs');
    expect(manifest.aria2.sourceSha256).toMatch(/^[0-9a-f]{64}$/);
    expect(manifest.aria2.windowsAssetSha256).toMatch(/^[0-9a-f]{64}$/);
    expect(workflow.match(/vcpkg: /g)).toHaveLength(5);
  });

  it('verifies the pinned yt-dlp source before installing the shared runtime', () => {
    const workflow = readFileSync('.github/workflows/build.yml', 'utf8');
    const installer = readFileSync('.github/scripts/install-yt-dlp.mjs', 'utf8');

    expect(workflow).toContain('install-yt-dlp.mjs');
    expect(installer).toContain('manifest.ytDlp.sha256');
    expect(installer).toContain('manifest.ytDlp.ejs.sha256');
    expect(installer).toContain("createHash('sha256')");
  });

  it('ships third-party notices, corresponding aria2 source, and size budgets', () => {
    const workflow = readFileSync('.github/workflows/build.yml', 'utf8');
    const prepareRuntime = readFileSync('.github/scripts/prepare-runtime.mjs', 'utf8');
    const sizeCheck = readFileSync('.github/scripts/check-package-size.mjs', 'utf8');

    expect(prepareRuntime).toContain('THIRD_PARTY_NOTICES.md');
    expect(workflow).toContain('aria2-1.37.0-source.tar.gz');
    expect(workflow).toContain('aria2.sourceSha256');
    expect(sizeCheck).toContain("macos: 55");
    expect(sizeCheck).toContain("windows: 55");
    expect(sizeCheck).toContain("linux: 75");
  });
});
