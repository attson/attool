import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { chmodSync, copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { basename, resolve } from 'node:path';

const GSTREAMER_ELEMENTS = ['appsrc', 'videoconvert', 'videocrop', 'videoscale', 'queue', 'vp8enc', 'webmmux', 'filesink'];

const target = process.env.TAURI_ENV_TARGET_TRIPLE || process.argv.slice(2).find((argument) => argument !== '--');
if (!target) {
  console.error('TAURI_ENV_TARGET_TRIPLE or an explicit target triple is required');
  process.exit(1);
}

const manifest = JSON.parse(readFileSync('src-tauri/runtime-manifest.json', 'utf8'));
const windows = target.includes('windows');
const output = resolve('src-tauri/binaries/runtime', target);
const tesseract = resolveProgram('ATTOOL_TESSERACT_PATH', windows ? 'tesseract.exe' : 'tesseract');
const aria2 = resolveProgram('ATTOOL_ARIA2_PATH', windows ? 'aria2c.exe' : 'aria2c');

assertVersion(tesseract, manifest.tesseract.version, 'Tesseract');
assertVersion(aria2, manifest.aria2.version, 'aria2');

rmSync(output, { recursive: true, force: true });
const tesseractBin = resolve(output, 'tesseract/bin', windows ? 'tesseract.exe' : 'tesseract');
const aria2Bin = resolve(output, 'aria2/bin', windows ? 'aria2c.exe' : 'aria2c');
mkdirSync(resolve(output, 'tesseract/bin'), { recursive: true });
mkdirSync(resolve(output, 'tesseract/tessdata'), { recursive: true });
mkdirSync(resolve(output, 'aria2/bin'), { recursive: true });
mkdirSync(resolve(output, 'quickjs/bin'), { recursive: true });
copyFileSync(tesseract, tesseractBin);
copyFileSync(aria2, aria2Bin);
stageNativeDependencies(tesseract, resolve(output, 'tesseract/bin'));
stageNativeDependencies(aria2, resolve(output, 'aria2/bin'));
writeFileSync(resolve(output, 'tesseract/VERSION'), `${manifest.tesseract.version}\n`);
writeFileSync(resolve(output, 'aria2/VERSION'), `${manifest.aria2.version}\n`);
if (!windows) {
  chmodSync(tesseractBin, 0o755);
  chmodSync(aria2Bin, 0o755);
}

const quickJsAsset = manifest.quickJs.assets[target];
if (!quickJsAsset) throw new Error(`QuickJS does not support target ${target}`);
const quickJsBin = resolve(output, 'quickjs/bin', windows ? 'qjs.exe' : 'qjs');
const localQuickJs = process.env.ATTOOL_QUICKJS_PATH;
if (localQuickJs) {
  copyFileSync(resolve(localQuickJs), quickJsBin);
} else {
  const url = `https://github.com/quickjs-ng/quickjs/releases/download/v${manifest.quickJs.version}/${quickJsAsset.name}`;
  const response = await fetch(url);
  if (!response.ok) throw new Error(`download ${url}: HTTP ${response.status}`);
  const bytes = Buffer.from(await response.arrayBuffer());
  const actual = createHash('sha256').update(bytes).digest('hex');
  if (actual !== quickJsAsset.sha256) throw new Error(`QuickJS checksum mismatch: expected ${quickJsAsset.sha256}, got ${actual}`);
  writeFileSync(quickJsBin, bytes);
}
if (!windows) chmodSync(quickJsBin, 0o755);
const quickJsVersion = spawnSync(quickJsBin, ['--help'], { encoding: 'utf8' });
if (!`${quickJsVersion.stdout || ''}\n${quickJsVersion.stderr || ''}`.includes(`QuickJS-ng version ${manifest.quickJs.version}`)) {
  throw new Error(`QuickJS-ng ${manifest.quickJs.version} required`);
}
const quickJsSmoke = spawnSync(quickJsBin, ['-e', "console.log('ATTOOL_QJS_OK')"], { encoding: 'utf8' });
if (quickJsSmoke.status !== 0 || !quickJsSmoke.stdout.includes('ATTOOL_QJS_OK')) {
  throw new Error(`QuickJS-ng execution smoke test failed: ${quickJsSmoke.stderr || quickJsSmoke.stdout}`);
}
writeFileSync(resolve(output, 'quickjs/VERSION'), `${manifest.quickJs.version}\n`);

for (const language of manifest.tessdata.languages) {
  const destination = resolve(output, 'tesseract/tessdata', `${language}.traineddata`);
  const localDir = process.env.ATTOOL_TESSDATA_DIR;
  if (localDir) {
    copyFileSync(resolve(localDir, `${language}.traineddata`), destination);
    continue;
  }
  const url = `https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/${manifest.tessdata.commit}/${language}.traineddata`;
  const response = await fetch(url);
  if (!response.ok) throw new Error(`download ${url}: HTTP ${response.status}`);
  writeFileSync(destination, Buffer.from(await response.arrayBuffer()));
}

if (target.includes('linux')) {
  stageGStreamer(output, manifest.gstreamer);
}
stageLicenses(output);

writeFileSync(
  resolve(output, 'manifest.json'),
  `${JSON.stringify({ target, tesseract: manifest.tesseract.version, aria2: manifest.aria2.version, quickJs: manifest.quickJs.version, tessdata: manifest.tessdata, gstreamer: target.includes('linux') ? manifest.gstreamer : undefined }, null, 2)}\n`,
);
console.log(`Bundled runtime ready: ${output}`);

function resolveProgram(envName, name) {
  if (process.env[envName]) return resolve(process.env[envName]);
  const lookup = spawnSync(windows ? 'where' : 'sh', windows ? [name] : ['-c', `command -v "$1"`, 'sh', name], {
    encoding: 'utf8',
  });
  const path = lookup.stdout?.split(/\r?\n/).find(Boolean);
  if (lookup.status !== 0 || !path) {
    throw new Error(`${name} is required; set ${envName} to the pinned build`);
  }
  return resolve(path.trim());
}

function assertVersion(program, expected, label) {
  const result = spawnSync(program, ['--version'], { encoding: 'utf8' });
  const text = `${result.stdout || ''}\n${result.stderr || ''}`;
  if (result.status !== 0 || !text.includes(expected)) {
    throw new Error(`${label} ${expected} required, got ${basename(program)}: ${text.trim()}`);
  }
}

function stageGStreamer(runtimeOutput, config) {
  const inspect = resolveProgram('ATTOOL_GST_INSPECT_PATH', 'gst-inspect-1.0');
  const version = spawnSync(inspect, ['--version'], { encoding: 'utf8' });
  const versionText = `${version.stdout || ''}\n${version.stderr || ''}`;
  if (version.status !== 0 || !versionText.includes(`GStreamer ${config.minimumVersion}`)) {
    throw new Error(`GStreamer ${config.minimumVersion}.x required: ${versionText.trim()}`);
  }

  const root = resolve(runtimeOutput, 'gstreamer');
  const pluginsDirectory = resolve(root, 'plugins');
  const librariesDirectory = resolve(root, 'lib');
  mkdirSync(pluginsDirectory, { recursive: true });
  mkdirSync(librariesDirectory, { recursive: true });

  const plugins = new Set();
  for (const element of GSTREAMER_ELEMENTS) {
    const inspected = spawnSync(inspect, [element], { encoding: 'utf8' });
    const text = `${inspected.stdout || ''}\n${inspected.stderr || ''}`;
    if (inspected.status !== 0) throw new Error(`GStreamer element ${element} is unavailable: ${text.trim()}`);
    const filename = text.match(/^\s*Filename\s+(.+)$/m)?.[1]?.trim();
    if (!filename) throw new Error(`Cannot resolve GStreamer plugin for ${element}`);
    plugins.add(resolve(filename));
  }

  const copiedLibraries = new Set();
  for (const plugin of plugins) {
    const destination = resolve(pluginsDirectory, basename(plugin));
    copyFileSync(plugin, destination);
    const dependencies = spawnSync('ldd', [plugin], { encoding: 'utf8' });
    if (dependencies.status !== 0) throw new Error(`Cannot inspect ${plugin}: ${dependencies.stderr?.trim()}`);
    for (const line of dependencies.stdout.split(/\r?\n/)) {
      const match = line.match(/^\s*(libvpx\.so\.[^\s]+)\s+=>\s+(\/[^\s]+)/);
      if (!match || copiedLibraries.has(match[1])) continue;
      copyFileSync(match[2], resolve(librariesDirectory, match[1]));
      copiedLibraries.add(match[1]);
    }
  }

  if (copiedLibraries.size === 0) throw new Error('GStreamer vp8enc does not expose a libvpx runtime dependency');
  for (const plugin of plugins) {
    const patched = resolve(pluginsDirectory, basename(plugin));
    const result = spawnSync('patchelf', ['--set-rpath', '$ORIGIN/../lib', patched], { encoding: 'utf8' });
    if (result.status !== 0) throw new Error(`Cannot patch ${patched}: ${result.stderr?.trim()}`);
  }
  writeFileSync(resolve(root, 'VERSION'), `${config.minimumVersion}\n`);
}

function stageNativeDependencies(program, destinationDirectory) {
  const prefixValue = process.env.ATTOOL_NATIVE_RUNTIME_PREFIX;
  if (!prefixValue) return;
  const prefix = resolve(prefixValue);
  const stagedProgram = resolve(destinationDirectory, basename(program));

  if (windows) {
    const binaryDirectory = resolve(prefix, 'bin');
    if (!existsSync(binaryDirectory)) return;
    for (const name of readdirSync(binaryDirectory)) {
      if (name.toLowerCase().endsWith('.dll')) {
        copyFileSync(resolve(binaryDirectory, name), resolve(destinationDirectory, name));
      }
    }
    return;
  }

  const pending = [program];
  const sources = new Map();
  while (pending.length > 0) {
    const current = pending.pop();
    for (const dependency of nativeDependencies(current, prefix)) {
      const name = basename(dependency);
      if (sources.has(name)) continue;
      sources.set(name, dependency);
      pending.push(dependency);
    }
  }
  for (const [name, source] of sources) {
    copyFileSync(source, resolve(destinationDirectory, name));
  }

  const stagedFiles = [stagedProgram, ...sources.keys()].map((name) =>
    typeof name === 'string' && name.startsWith(destinationDirectory) ? name : resolve(destinationDirectory, name),
  );
  if (target.includes('linux')) {
    for (const file of stagedFiles) {
      const needed = spawnSync('patchelf', ['--print-needed', file], { encoding: 'utf8' });
      if (needed.status !== 0 || !needed.stdout.trim()) continue;
      const result = spawnSync('patchelf', ['--set-rpath', '$ORIGIN', file], { encoding: 'utf8' });
      if (result.status !== 0) throw new Error(`Cannot patch ${file}: ${result.stderr?.trim()}`);
    }
    return;
  }

  for (const file of stagedFiles) {
    const dependencies = spawnSync('otool', ['-L', file], { encoding: 'utf8' });
    if (dependencies.status !== 0) throw new Error(`Cannot inspect ${file}: ${dependencies.stderr?.trim()}`);
    for (const line of dependencies.stdout.split(/\r?\n/).slice(1)) {
      const reference = line.trim().split(/\s+\(/)[0];
      const name = basename(reference || '');
      if (!sources.has(name)) continue;
      const changed = spawnSync('install_name_tool', ['-change', reference, `@loader_path/${name}`, file], { encoding: 'utf8' });
      if (changed.status !== 0) throw new Error(`Cannot patch ${file}: ${changed.stderr?.trim()}`);
    }
    if (file.endsWith('.dylib')) {
      const identified = spawnSync('install_name_tool', ['-id', `@loader_path/${basename(file)}`, file], { encoding: 'utf8' });
      if (identified.status !== 0) throw new Error(`Cannot identify ${file}: ${identified.stderr?.trim()}`);
    }
  }
}

function nativeDependencies(program, prefix) {
  if (target.includes('linux')) {
    const result = spawnSync('ldd', [program], { encoding: 'utf8' });
    const output = `${result.stdout || ''}\n${result.stderr || ''}`;
    if (result.status !== 0 && /not a dynamic executable|statically linked/i.test(output)) return [];
    if (result.status !== 0) throw new Error(`Cannot inspect ${program}: ${output.trim()}`);
    return result.stdout
      .split(/\r?\n/)
      .map((line) => line.match(/=>\s+(\/[^\s]+)/)?.[1])
      .filter((path) => path?.startsWith(`${prefix}/`));
  }

  const result = spawnSync('otool', ['-L', program], { encoding: 'utf8' });
  if (result.status !== 0) throw new Error(`Cannot inspect ${program}: ${result.stderr?.trim()}`);
  const libraries = resolve(prefix, 'lib');
  return result.stdout
    .split(/\r?\n/)
    .slice(1)
    .map((line) => line.trim().split(/\s+\(/)[0])
    .map((reference) => {
      if (reference?.startsWith(`${prefix}/`)) return reference;
      if (reference?.startsWith('@rpath/')) return resolve(libraries, basename(reference));
      return null;
    })
    .filter(Boolean);
}

function stageLicenses(runtimeOutput) {
  const root = resolve(runtimeOutput, 'notices');
  const licenses = resolve(root, 'licenses');
  mkdirSync(licenses, { recursive: true });
  copyFileSync('src-tauri/THIRD_PARTY_NOTICES.md', resolve(root, 'THIRD_PARTY_NOTICES.md'));
  copyFileSync('src-tauri/licenses/yt-dlp-UNLICENSE.txt', resolve(licenses, 'yt-dlp-UNLICENSE.txt'));
  copyFileSync('src-tauri/licenses/QuickJS-ng-MIT.txt', resolve(licenses, 'QuickJS-ng-MIT.txt'));
  copyFileSync('src-tauri/licenses/libvpx-BSD-3-Clause.txt', resolve(licenses, 'libvpx-BSD-3-Clause.txt'));
  const ariaLicenseDirectory = resolve(aria2, '..');
  copyFileSync(resolve(ariaLicenseDirectory, 'aria2-COPYING'), resolve(licenses, 'aria2-GPL-2.0-or-later.txt'));
  const ariaOpenSslLicense = resolve(ariaLicenseDirectory, 'aria2-LICENSE.OpenSSL');
  if (existsSync(ariaOpenSslLicense)) copyFileSync(ariaOpenSslLicense, resolve(licenses, 'aria2-OpenSSL-license.txt'));

  const prefixValue = process.env.ATTOOL_NATIVE_RUNTIME_PREFIX;
  if (prefixValue) {
    const prefix = resolve(prefixValue);
    for (const entry of readdirSync(resolve(prefix, 'share'), { withFileTypes: true })) {
      if (!entry.isDirectory()) continue;
      const copyright = resolve(prefix, 'share', entry.name, 'copyright');
      if (existsSync(copyright)) copyFileSync(copyright, resolve(licenses, `${entry.name}-copyright`));
    }
  }
  if (target.includes('linux')) {
    copyFileSync('/usr/share/common-licenses/LGPL-2.1', resolve(licenses, 'GStreamer-LGPL-2.1.txt'));
    copyFileSync('/usr/share/doc/gstreamer1.0-plugins-base/copyright', resolve(licenses, 'gstreamer-plugins-base-copyright'));
    copyFileSync('/usr/share/doc/gstreamer1.0-plugins-good/copyright', resolve(licenses, 'gstreamer-plugins-good-copyright'));
  }
  writeFileSync(resolve(root, 'VERSION'), '2026-10-22\n');
}
