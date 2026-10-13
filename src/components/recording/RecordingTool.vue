<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
import {
  NAlert,
  NButton,
  NRadioButton,
  NRadioGroup,
  NSelect,
  NSwitch,
  useMessage
} from 'naive-ui';
import {
  buildRecordingFileName,
  formatRecordingDuration,
  formatRecordingSize,
  selectRecordingProfile,
  type RecordingProfile
} from './recording';

type RecorderStatus = 'idle' | 'selecting' | 'recording' | 'paused' | 'stopping' | 'completed' | 'error';

interface RecordingSession {
  sessionId: string;
}

interface RecordingSaved {
  outputPath: string;
  size: number;
}

const message = useMessage();
const previewEl = ref<HTMLVideoElement | null>(null);
const status = ref<RecorderStatus>('idle');
const profile = ref<RecordingProfile | null>(null);
const frameRate = ref(30);
const resolution = ref('original');
const systemAudio = ref(true);
const microphone = ref(false);
const elapsedMs = ref(0);
const bytesWritten = ref(0);
const sourceLabel = ref('');
const savedPath = ref('');
const cachePath = ref('');
const previewUrl = ref('');
const errorMessage = ref('');
const saving = ref(false);

let recorder: MediaRecorder | null = null;
let displayStream: MediaStream | null = null;
let microphoneStream: MediaStream | null = null;
let recordingStream: MediaStream | null = null;
let audioContext: AudioContext | null = null;
let sessionId = '';
let writeQueue: Promise<void> = Promise.resolve();
let writeError: unknown = null;
let timer: ReturnType<typeof setInterval> | null = null;
let startedAt = 0;
let pausedAt = 0;
let pausedTotal = 0;
let disposing = false;

const supported = computed(() => profile.value !== null && !!navigator.mediaDevices?.getDisplayMedia);
const busy = computed(() => ['selecting', 'recording', 'paused', 'stopping'].includes(status.value));
const elapsedLabel = computed(() => formatRecordingDuration(elapsedMs.value));
const sizeLabel = computed(() => formatRecordingSize(bytesWritten.value));
const statusLabel = computed(() => {
  switch (status.value) {
    case 'selecting': return '等待选择屏幕';
    case 'recording': return '录制中';
    case 'paused': return '已暂停';
    case 'stopping': return '正在处理';
    case 'completed': return savedPath.value ? '已保存' : '录制完成';
    case 'error': return '录制失败';
    default: return '准备就绪';
  }
});

const resolutionOptions = [
  { label: '原始尺寸', value: 'original' },
  { label: '1080p', value: '1080' },
  { label: '720p', value: '720' }
];

function videoConstraints(): MediaTrackConstraints {
  const constraints: MediaTrackConstraints = {
    frameRate: { ideal: frameRate.value, max: frameRate.value }
  };
  if (resolution.value === '1080') {
    constraints.width = { ideal: 1920 };
    constraints.height = { ideal: 1080 };
  } else if (resolution.value === '720') {
    constraints.width = { ideal: 1280 };
    constraints.height = { ideal: 720 };
  }
  return constraints;
}

async function blobToBase64(blob: Blob): Promise<string> {
  return await new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result).split(',')[1] ?? '');
    reader.onerror = () => reject(reader.error ?? new Error('读取录制数据失败'));
    reader.readAsDataURL(blob);
  });
}

async function uploadBlob(blob: Blob) {
  const chunkSize = 512 * 1024;
  for (let offset = 0; offset < blob.size; offset += chunkSize) {
    const base64 = await blobToBase64(blob.slice(offset, offset + chunkSize));
    bytesWritten.value = await invoke<number>('append_recording_chunk', {
      sessionId,
      base64
    });
  }
}

async function composeRecordingStream(screen: MediaStream): Promise<MediaStream> {
  const audioTracks = [...screen.getAudioTracks()];
  if (microphone.value) {
    microphoneStream = await navigator.mediaDevices.getUserMedia({ audio: true });
    audioTracks.push(...microphoneStream.getAudioTracks());
  }
  if (audioTracks.length <= 1) {
    return new MediaStream([...screen.getVideoTracks(), ...audioTracks]);
  }

  audioContext = new AudioContext();
  const destination = audioContext.createMediaStreamDestination();
  for (const track of audioTracks) {
    const source = audioContext.createMediaStreamSource(new MediaStream([track]));
    source.connect(destination);
  }
  return new MediaStream([...screen.getVideoTracks(), ...destination.stream.getAudioTracks()]);
}

function startClock() {
  startedAt = performance.now();
  pausedAt = 0;
  pausedTotal = 0;
  elapsedMs.value = 0;
  timer = setInterval(() => {
    const end = pausedAt || performance.now();
    elapsedMs.value = end - startedAt - pausedTotal;
  }, 250);
}

function stopClock() {
  if (timer) clearInterval(timer);
  timer = null;
}

function stopStreams() {
  displayStream?.getTracks().forEach((track) => track.stop());
  microphoneStream?.getTracks().forEach((track) => track.stop());
  recordingStream?.getTracks().forEach((track) => track.stop());
  displayStream = null;
  microphoneStream = null;
  recordingStream = null;
  if (audioContext) void audioContext.close();
  audioContext = null;
  if (previewEl.value) previewEl.value.srcObject = null;
}

async function cancelFileSession() {
  const currentSession = sessionId;
  sessionId = '';
  if (currentSession) {
    await invoke('cancel_recording_file', { sessionId: currentSession }).catch(() => {});
  }
}

function failRecording(error: unknown) {
  errorMessage.value = String(error);
  status.value = 'error';
  stopClock();
  stopStreams();
}

async function finishRecording() {
  stopClock();
  stopStreams();
  await writeQueue;
  if (disposing) return;
  if (writeError) {
    await cancelFileSession();
    failRecording(writeError);
    return;
  }

  const currentSession = sessionId;
  sessionId = '';
  try {
    const saved = await invoke<RecordingSaved>('finish_recording_file', {
      sessionId: currentSession,
      extension: profile.value?.extension ?? 'webm'
    });
    cachePath.value = saved.outputPath;
    bytesWritten.value = saved.size;
    previewUrl.value = convertFileSrc(saved.outputPath);
    status.value = 'completed';
    await nextTick();
    previewEl.value?.load();
    await saveCompletedRecording();
  } catch (error) {
    failRecording(error);
  }
}

async function startRecording() {
  if (!profile.value || !supported.value) return;
  errorMessage.value = '';
  status.value = 'selecting';
  savedPath.value = '';
  cachePath.value = '';
  previewUrl.value = '';
  bytesWritten.value = 0;
  sourceLabel.value = '';
  writeQueue = Promise.resolve();
  writeError = null;

  try {
    displayStream = await navigator.mediaDevices.getDisplayMedia({
      video: videoConstraints(),
      audio: systemAudio.value
    });
    recordingStream = await composeRecordingStream(displayStream);
    sourceLabel.value = displayStream.getVideoTracks()[0]?.label || '共享屏幕';
    const session = await invoke<RecordingSession>('begin_recording_file');
    sessionId = session.sessionId;

    recorder = new MediaRecorder(recordingStream, {
      mimeType: profile.value.mimeType,
      videoBitsPerSecond: resolution.value === '720' ? 4_000_000 : 8_000_000,
      audioBitsPerSecond: 128_000
    });
    recorder.addEventListener('dataavailable', (event) => {
      if (!event.data.size || !sessionId) return;
      writeQueue = writeQueue
        .then(() => uploadBlob(event.data))
        .catch((error) => {
          writeError = error;
          if (recorder?.state !== 'inactive') recorder?.stop();
        });
    });
    recorder.addEventListener('stop', () => void finishRecording(), { once: true });
    recorder.addEventListener('error', (event) => {
      writeError = event.error;
      if (recorder?.state !== 'inactive') recorder?.stop();
    });
    displayStream.getVideoTracks()[0]?.addEventListener('ended', () => stopRecording(), { once: true });

    await nextTick();
    if (previewEl.value) {
      previewEl.value.srcObject = recordingStream;
      await previewEl.value.play().catch(() => {});
    }
    recorder.start(1000);
    startClock();
    status.value = 'recording';
  } catch (error) {
    await cancelFileSession();
    stopStreams();
    if (error instanceof DOMException && error.name === 'NotAllowedError') {
      status.value = 'idle';
      return;
    }
    failRecording(error);
  }
}

async function saveCompletedRecording() {
  if (!profile.value || !cachePath.value || saving.value) return;
  saving.value = true;
  try {
    const fileName = buildRecordingFileName(new Date(), profile.value.extension);
    const defaultPath = await invoke<string>('default_recording_path', { fileName })
      .catch(() => fileName);
    const target = await save({
      title: '保存录屏',
      defaultPath,
      filters: [{ name: profile.value.label, extensions: [profile.value.extension] }]
    }).catch(() => null);
    if (!target) return;

    const saved = await invoke<RecordingSaved>('save_recording_file', {
      source: cachePath.value,
      target
    });
    savedPath.value = saved.outputPath;
    bytesWritten.value = saved.size;
    message.success('录屏已保存');
  } catch (error) {
    message.error(String(error));
  } finally {
    saving.value = false;
  }
}

function discardCachedRecording() {
  const path = cachePath.value;
  cachePath.value = '';
  if (previewEl.value) {
    previewEl.value.pause();
    previewEl.value.removeAttribute('src');
    previewEl.value.load();
  }
  previewUrl.value = '';
  if (path) void invoke('discard_recording_file', { path }).catch(() => {});
}

function pauseRecording() {
  if (recorder?.state !== 'recording') return;
  recorder.pause();
  pausedAt = performance.now();
  status.value = 'paused';
}

function resumeRecording() {
  if (recorder?.state !== 'paused') return;
  recorder.resume();
  pausedTotal += performance.now() - pausedAt;
  pausedAt = 0;
  status.value = 'recording';
}

function stopRecording() {
  if (!recorder || recorder.state === 'inactive') return;
  status.value = 'stopping';
  recorder.stop();
}

async function openFolder() {
  if (!savedPath.value) return;
  try {
    await invoke('open_recording_folder', { path: savedPath.value });
  } catch (error) {
    message.error(String(error));
  }
}

function resetRecording() {
  discardCachedRecording();
  status.value = 'idle';
  savedPath.value = '';
  previewUrl.value = '';
  sourceLabel.value = '';
  elapsedMs.value = 0;
  bytesWritten.value = 0;
  errorMessage.value = '';
  recorder = null;
}

onMounted(() => {
  if (typeof MediaRecorder === 'undefined') return;
  profile.value = selectRecordingProfile((mimeType) => MediaRecorder.isTypeSupported(mimeType));
});

onBeforeUnmount(() => {
  disposing = true;
  stopClock();
  if (recorder?.state !== 'inactive') recorder?.stop();
  stopStreams();
  void cancelFileSession();
  discardCachedRecording();
});
</script>

<template>
  <div class="recording-tool">
    <header class="recording-toolbar">
      <div class="recording-state">
        <span v-if="status === 'recording'" class="record-dot" aria-hidden="true"></span>
        <span class="state-label">{{ statusLabel }}</span>
        <span v-if="busy || status === 'completed'" class="mono metric">{{ elapsedLabel }}</span>
        <span v-if="bytesWritten" class="mono metric">{{ sizeLabel }}</span>
      </div>

      <div class="recording-actions">
        <n-button v-if="status === 'recording'" size="small" secondary @click="pauseRecording">
          <template #icon>
            <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/></svg>
          </template>
          暂停
        </n-button>
        <n-button v-if="status === 'paused'" size="small" secondary @click="resumeRecording">
          <template #icon>
            <svg viewBox="0 0 24 24" aria-hidden="true"><polygon points="7 4 20 12 7 20"/></svg>
          </template>
          继续
        </n-button>
        <n-button v-if="status === 'recording' || status === 'paused'" size="small" type="error" @click="stopRecording">
          <template #icon>
            <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="5" y="5" width="14" height="14" rx="1"/></svg>
          </template>
          停止
        </n-button>
        <n-button v-if="status === 'completed'" size="small" type="primary" :loading="saving" @click="saveCompletedRecording">
          <template #icon>
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 3v12M7 10l5 5 5-5M4 20h16" fill="none" /></svg>
          </template>
          {{ savedPath ? '另存为' : '保存' }}
        </n-button>
        <n-button v-if="status === 'completed' && savedPath" size="small" secondary @click="openFolder">
          <template #icon>
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 6h7l2 2h9v10H3z"/></svg>
          </template>
          打开目录
        </n-button>
        <n-button v-if="status === 'completed' || status === 'error'" size="small" secondary @click="resetRecording">
          新建录制
        </n-button>
      </div>
    </header>

    <div class="recording-body">
      <main class="preview-area">
        <div class="preview-stage">
          <video
            v-show="status === 'recording' || status === 'paused' || status === 'completed'"
            ref="previewEl"
            :src="previewUrl || undefined"
            :controls="status === 'completed'"
            :muted="status !== 'completed'"
            playsinline
          ></video>

          <div v-if="status === 'idle'" class="stage-empty">
            <svg class="stage-icon" viewBox="0 0 24 24" aria-hidden="true">
              <rect x="3" y="4" width="18" height="14" rx="2" />
              <path d="M8 22h8M12 18v4" />
              <circle cx="12" cy="11" r="3" />
            </svg>
            <n-button type="primary" :disabled="!supported" @click="startRecording">
              选择屏幕并录制
            </n-button>
          </div>

          <div v-else-if="status === 'selecting' || status === 'stopping'" class="stage-empty">
            <span class="stage-status">{{ statusLabel }}</span>
          </div>

          <div v-else-if="status === 'error'" class="stage-empty error-state">
            <span>录制未完成</span>
          </div>

          <div v-if="status === 'recording' || status === 'paused'" class="live-badge">
            {{ status === 'paused' ? 'PAUSED' : 'REC' }}
          </div>
        </div>

        <n-alert v-if="!supported" type="warning" :bordered="false">
          当前系统 WebView 不支持屏幕录制。请升级系统 WebView 后重试。
        </n-alert>
        <n-alert v-else-if="errorMessage" type="error" :bordered="false">
          {{ errorMessage }}
        </n-alert>
      </main>

      <aside class="recording-settings">
        <section class="setting-section">
          <h3>画面</h3>
          <label class="setting-field">
            <span>分辨率</span>
            <n-select v-model:value="resolution" size="small" :options="resolutionOptions" :disabled="busy" />
          </label>
          <label class="setting-field">
            <span>帧率</span>
            <n-radio-group v-model:value="frameRate" size="small" :disabled="busy">
              <n-radio-button :value="30">30 FPS</n-radio-button>
              <n-radio-button :value="60">60 FPS</n-radio-button>
            </n-radio-group>
          </label>
        </section>

        <section class="setting-section">
          <h3>音频</h3>
          <label class="switch-row">
            <span>系统音频</span>
            <n-switch v-model:value="systemAudio" size="small" :disabled="busy" />
          </label>
          <label class="switch-row">
            <span>麦克风</span>
            <n-switch v-model:value="microphone" size="small" :disabled="busy" />
          </label>
        </section>

        <section class="setting-section output-section">
          <h3>输出</h3>
          <div class="output-row">
            <span>格式</span>
            <span class="mono">{{ profile?.label ?? '不可用' }}</span>
          </div>
          <div v-if="sourceLabel" class="output-row">
            <span>来源</span>
            <span class="output-value">{{ sourceLabel }}</span>
          </div>
          <div v-if="savedPath" class="saved-path mono" :title="savedPath">{{ savedPath }}</div>
          <div v-else-if="status === 'completed'" class="output-row">
            <span>文件</span>
            <span>尚未保存</span>
          </div>
        </section>
      </aside>
    </div>
  </div>
</template>

<style scoped>
.recording-tool {
  height: 100%;
  min-height: 520px;
  display: flex;
  flex-direction: column;
  background: var(--bg-base);
}

.recording-toolbar {
  min-height: 52px;
  padding: 8px 16px;
  border-bottom: 1px solid var(--line);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.recording-state,
.recording-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.record-dot {
  width: 8px;
  height: 8px;
  border-radius: var(--radius-pill);
  background: var(--error);
  animation: record-pulse 1.2s ease-in-out infinite;
}

.state-label { font-size: var(--fs-sm); color: var(--text); }
.metric { font-size: var(--fs-xs); color: var(--text-muted); }

.recording-actions svg {
  width: 14px;
  height: 14px;
  fill: currentColor;
  stroke: currentColor;
  stroke-width: 1.5;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.recording-body {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 280px;
}

.preview-area {
  min-width: 0;
  padding: 16px;
  overflow: auto;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.preview-stage {
  position: relative;
  flex: 1;
  min-height: 360px;
  display: grid;
  place-items: center;
  overflow: hidden;
  background: var(--bg-elevated);
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
}

.preview-stage video {
  width: 100%;
  height: 100%;
  max-height: calc(100vh - 190px);
  object-fit: contain;
  background: var(--bg-base);
}

.stage-empty {
  display: grid;
  justify-items: center;
  gap: 18px;
  color: var(--text-muted);
}

.stage-icon {
  width: 58px;
  height: 58px;
  fill: none;
  stroke: var(--text-faint);
  stroke-width: 1.4;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.stage-status { font-size: var(--fs-sm); }
.error-state { color: var(--error); }

.live-badge {
  position: absolute;
  top: 12px;
  left: 12px;
  padding: 3px 7px;
  border-radius: var(--radius-sm);
  background: var(--error-soft);
  color: var(--error);
  font-family: var(--font-mono);
  font-size: var(--fs-xxs);
  font-weight: 700;
}

.recording-settings {
  border-left: 1px solid var(--line);
  background: var(--bg-elevated);
  overflow: auto;
}

.setting-section {
  padding: 16px;
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 14px;
}

.setting-section h3 {
  margin: 0;
  color: var(--text);
  font-size: var(--fs-sm);
  font-weight: 600;
}

.setting-field {
  display: grid;
  gap: 7px;
  color: var(--text-muted);
  font-size: var(--fs-xs);
}

.setting-field :deep(.n-radio-group) { display: flex; }
.setting-field :deep(.n-radio-button) { flex: 1; text-align: center; }

.switch-row,
.output-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  color: var(--text-muted);
  font-size: var(--fs-xs);
}

.output-row .mono { color: var(--text); }
.output-value {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text);
}

.saved-path {
  padding: 8px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  background: var(--bg-base);
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  color: var(--text-muted);
  font-size: var(--fs-xxs);
}

@keyframes record-pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.35; }
}

@media (max-width: 820px) {
  .recording-body { grid-template-columns: 1fr; }
  .recording-settings { border-left: 0; border-top: 1px solid var(--line); }
  .preview-stage { min-height: 300px; }
  .recording-toolbar { align-items: flex-start; flex-direction: column; }
}
</style>
