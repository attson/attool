import { chmodSync, existsSync, mkdirSync, readFileSync, rmSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';

const target = process.env.TAURI_ENV_TARGET_TRIPLE || process.argv[2];
if (!target) {
  console.error('TAURI_ENV_TARGET_TRIPLE or an explicit target triple is required');
  process.exit(1);
}

const targetArch = target.startsWith('aarch64-') ? 'arm64' : target.startsWith('x86_64-') ? 'x64' : null;
if (!targetArch) {
  console.error(`Unsupported PSD sidecar target: ${target}`);
  process.exit(1);
}
if (process.arch !== targetArch) {
  console.error(`PyInstaller cannot cross-compile from ${process.arch} to ${targetArch}; use a native runner`);
  process.exit(1);
}

const isWindows = target.includes('windows');
const python = process.env.PYTHON || (process.platform === 'win32' ? 'python' : 'python3');
const name = `psd-bridge-${target}`;
const binariesDir = resolve('src-tauri/binaries');
const workDir = resolve('src-tauri/target/pyinstaller', target);
const script = resolve('src-tauri/python/psd_template_bridge.py');
const output = resolve(binariesDir, `${name}${isWindows ? '.exe' : ''}`);

mkdirSync(binariesDir, { recursive: true });
mkdirSync(workDir, { recursive: true });
rmSync(output, { force: true });

const result = spawnSync(
  python,
  [
    '-m',
    'PyInstaller',
    '--noconfirm',
    '--clean',
    '--onefile',
    '--noupx',
    '--collect-all',
    'psd_tools',
    '--collect-all',
    'yt_dlp',
    '--collect-all',
    'yt_dlp_ejs',
    '--name',
    name,
    '--distpath',
    binariesDir,
    '--workpath',
    workDir,
    '--specpath',
    workDir,
    script,
  ],
  { stdio: 'inherit' },
);

if (result.error) {
  console.error(`Failed to start ${python}: ${result.error.message}`);
  process.exit(1);
}
if (result.status !== 0 || !existsSync(output)) {
  console.error(`PSD sidecar build failed. Install build dependencies with: ${python} -m pip install -r src-tauri/python/requirements-build.txt`);
  process.exit(result.status || 1);
}
if (!isWindows) chmodSync(output, 0o755);

const smoke = spawnSync(output, ['--help'], { encoding: 'utf8' });
if (smoke.status !== 0) {
  console.error(smoke.stderr || smoke.stdout || 'PSD sidecar smoke test failed');
  process.exit(smoke.status || 1);
}
const manifest = JSON.parse(readFileSync('src-tauri/runtime-manifest.json', 'utf8'));
const versionSmoke = spawnSync(output, ['--runtime-version'], { encoding: 'utf8' });
if (
  versionSmoke.status !== 0 ||
  !versionSmoke.stdout.includes(`"ytDlp": "${manifest.ytDlp.version}"`) ||
  !versionSmoke.stdout.includes(`"ytDlpEjs": "${manifest.ytDlp.ejs.version}"`)
) {
  console.error(versionSmoke.stderr || versionSmoke.stdout || 'Shared Python runtime version smoke test failed');
  process.exit(versionSmoke.status || 1);
}
console.log(`PSD sidecar ready: ${output}`);
