import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const target = process.argv[2];
if (!target) throw new Error('target triple is required');

const manifest = JSON.parse(readFileSync('src-tauri/runtime-manifest.json', 'utf8'));
const root = resolve('src-tauri/binaries/runtime', target);
const windows = target.includes('windows');
const tesseract = resolve(root, 'tesseract/bin', windows ? 'tesseract.exe' : 'tesseract');
const tessdata = resolve(root, 'tesseract/tessdata');
const aria2 = resolve(root, 'aria2/bin', windows ? 'aria2c.exe' : 'aria2c');
const quickjs = resolve(root, 'quickjs/bin', windows ? 'qjs.exe' : 'qjs');

assertCommand(tesseract, ['--version'], manifest.tesseract.version);
const languages = assertCommand(tesseract, ['--tessdata-dir', tessdata, '--list-langs']);
for (const language of manifest.tessdata.languages) {
  if (!languages.split(/\r?\n/).includes(language)) throw new Error(`Tesseract does not list ${language}`);
}
assertCommand(aria2, ['--version'], manifest.aria2.version);
const quickJsResult = spawnSync(quickjs, ['--help'], { encoding: 'utf8' });
const quickJsOutput = `${quickJsResult.stdout || ''}\n${quickJsResult.stderr || ''}`;
if (!quickJsOutput.includes(`QuickJS-ng version ${manifest.quickJs.version}`)) {
  throw new Error(`QuickJS-ng ${manifest.quickJs.version} smoke test failed: ${quickJsOutput.trim()}`);
}
assertCommand(quickjs, ['-e', "console.log('ATTOOL_QJS_OK')"], 'ATTOOL_QJS_OK');

if (target.includes('linux')) {
  const plugins = resolve(root, 'gstreamer/plugins');
  const environment = {
    ...process.env,
    GST_PLUGIN_PATH_1_0: plugins,
    GST_PLUGIN_SYSTEM_PATH_1_0: '',
    GST_REGISTRY_1_0: resolve(root, 'gstreamer/smoke-registry.bin'),
  };
  for (const element of ['appsrc', 'videoconvert', 'videocrop', 'videoscale', 'queue', 'vp8enc', 'webmmux', 'filesink']) {
    assertCommand('gst-inspect-1.0', [element], element, environment);
  }
}

console.log(`Bundled runtime smoke test passed: ${target}`);

function assertCommand(program, args, expected, env = process.env) {
  const result = spawnSync(program, args, { encoding: 'utf8', env });
  const output = `${result.stdout || ''}\n${result.stderr || ''}`;
  if (result.status !== 0 || (expected !== undefined && !output.includes(expected))) {
    throw new Error(`${program} ${args.join(' ')} failed or omitted ${expected}: ${output.trim()}`);
  }
  return output;
}
