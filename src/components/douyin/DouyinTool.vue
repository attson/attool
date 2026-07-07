<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { NButton, NInput } from 'naive-ui';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { invoke } from '@tauri-apps/api/core';
import Panel from '../ui/Panel.vue';
import { extractDouyinLinks } from '../../utils/douyinLink';
import { useAria2Handoff } from '../../composables/useAria2Handoff';
import type { DouyinVideoInfo } from '../../types/douyin';

interface VideoState {
  status: 'pending' | 'ok' | 'fail';
  info: DouyinVideoInfo | null;
  error: string | null;
}

interface Entry {
  short: string;
  status: 'pending' | 'ok' | 'fail';
  resolved: string | null;
  error: string | null;
  video: VideoState;
}

const emit = defineEmits<{ (e: 'requestNavigate', tool: string): void }>();

const raw = ref('');
const entries = ref<Entry[]>([]);
const copyState = ref<Record<string, 'idle' | 'ok' | 'fail'>>({});
const videoCopyState = ref<Record<string, 'idle' | 'ok' | 'fail'>>({});
const allCopyState = ref<'idle' | 'ok' | 'fail'>('idle');
const handoff = useAria2Handoff();

let debounceTimer: ReturnType<typeof setTimeout> | null = null;
let resolveRun = 0;

watch(raw, (value) => {
  if (debounceTimer) clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => {
    rebuildEntries(extractDouyinLinks(value));
  }, 300);
});

function extractNow() {
  if (debounceTimer) clearTimeout(debounceTimer);
  rebuildEntries(extractDouyinLinks(raw.value));
}

function rebuildEntries(links: string[]) {
  const runId = ++resolveRun;
  entries.value = links.map((short) => ({
    short,
    status: 'pending',
    resolved: null,
    error: null,
    video: { status: 'pending', info: null, error: null },
  }));
  copyState.value = {};
  videoCopyState.value = {};

  links.forEach((short) => {
    invoke<string>('resolve_douyin_url', { url: short })
      .then((resolved) => {
        if (runId !== resolveRun) return;
        const idx = entries.value.findIndex((e) => e.short === short);
        if (idx === -1) return;
        entries.value[idx] = {
          ...entries.value[idx],
          status: 'ok',
          resolved,
          error: null,
        };
        kickOffVideo(runId, short, resolved);
      })
      .catch((err) => {
        if (runId !== resolveRun) return;
        const idx = entries.value.findIndex((e) => e.short === short);
        if (idx === -1) return;
        entries.value[idx] = {
          ...entries.value[idx],
          status: 'fail',
          resolved: null,
          error: String(err),
          video: { status: 'fail', info: null, error: '短链未能解析' },
        };
      });
  });
}

function kickOffVideo(runId: number, short: string, canonical: string) {
  invoke<DouyinVideoInfo>('extract_douyin_video', { url: canonical })
    .then((info) => {
      if (runId !== resolveRun) return;
      const idx = entries.value.findIndex((e) => e.short === short);
      if (idx === -1) return;
      entries.value[idx] = {
        ...entries.value[idx],
        video: { status: 'ok', info, error: null },
      };
    })
    .catch((err) => {
      if (runId !== resolveRun) return;
      const idx = entries.value.findIndex((e) => e.short === short);
      if (idx === -1) return;
      entries.value[idx] = {
        ...entries.value[idx],
        video: { status: 'fail', info: null, error: String(err) },
      };
    });
}

function clearAll() {
  raw.value = '';
  entries.value = [];
  copyState.value = {};
  videoCopyState.value = {};
  allCopyState.value = 'idle';
  resolveRun++;
}

function effectiveUrl(entry: Entry): string {
  return entry.resolved ?? entry.short;
}

async function copyOne(entry: Entry) {
  const key = entry.short;
  try {
    await writeText(effectiveUrl(entry));
    copyState.value = { ...copyState.value, [key]: 'ok' };
  } catch {
    copyState.value = { ...copyState.value, [key]: 'fail' };
  }
  setTimeout(() => {
    copyState.value = { ...copyState.value, [key]: 'idle' };
  }, 1500);
}

async function copyVideo(entry: Entry) {
  if (!entry.video.info) return;
  const key = entry.short;
  try {
    await writeText(entry.video.info.mp4Url);
    videoCopyState.value = { ...videoCopyState.value, [key]: 'ok' };
  } catch {
    videoCopyState.value = { ...videoCopyState.value, [key]: 'fail' };
  }
  setTimeout(() => {
    videoCopyState.value = { ...videoCopyState.value, [key]: 'idle' };
  }, 1500);
}

async function copyAll() {
  const urls = entries.value.map((e) => effectiveUrl(e));
  try {
    await writeText(urls.join('\n'));
    allCopyState.value = 'ok';
  } catch {
    allCopyState.value = 'fail';
  }
  setTimeout(() => { allCopyState.value = 'idle'; }, 1500);
}

async function openLink(entry: Entry) {
  const url = effectiveUrl(entry);
  const key = entry.short;
  try {
    await invoke('open_external_url', { url });
    return;
  } catch {
    /* fallback below */
  }
  try {
    await writeText(url);
    copyState.value = { ...copyState.value, [key]: 'ok' };
    setTimeout(() => {
      copyState.value = { ...copyState.value, [key]: 'idle' };
    }, 1500);
  } catch {
    /* noop */
  }
}

function downloadVideo(entry: Entry) {
  if (!entry.video.info) return;
  handoff.push(entry.video.info.mp4Url);
  emit('requestNavigate', 'aria2');
}

function copyLabel(entry: Entry): string {
  const state = copyState.value[entry.short] ?? 'idle';
  if (state === 'ok') return '已复制';
  if (state === 'fail') return '复制失败';
  return '复制页面';
}

function videoCopyLabel(entry: Entry): string {
  const state = videoCopyState.value[entry.short] ?? 'idle';
  if (state === 'ok') return '已复制';
  if (state === 'fail') return '复制失败';
  return '复制视频';
}

const allCopyLabel = computed(() => {
  if (allCopyState.value === 'ok') return '已全部复制';
  if (allCopyState.value === 'fail') return '复制失败';
  return '全部复制页面';
});

const hasInput = computed(() => raw.value.trim().length > 0);
const hasPending = computed(() => entries.value.some((e) => e.status === 'pending'));
const allCopyDisabled = computed(() => hasPending.value);
</script>

<template>
  <div class="page">
    <header class="page-header">
      <h2>视频链接抽取</h2>
      <p>
        从分享文案中识别多平台视频短链、跟踪 302 抓真实视频 mp4 直链，一键送入 Aria2 下载。
        <span class="platform-status">目前支持 抖音。小红书 / B 站 / YouTube 建设中。</span>
      </p>
    </header>

    <Panel title="分享文案">
      <div class="form">
        <n-input
          v-model:value="raw"
          type="textarea"
          placeholder="粘贴抖音 App 分享出来的整段文案，例如：9.99 复制打开抖音，看看【标题】... https://v.douyin.com/xxxxx/ 复制此链接..."
          :autosize="{ minRows: 8, maxRows: 14 }"
        />
        <div class="actions">
          <n-button secondary @click="clearAll">清空</n-button>
          <n-button type="primary" @click="extractNow">提取链接</n-button>
        </div>
      </div>
    </Panel>

    <Panel :title="`提取结果${entries.length ? ' · 共 ' + entries.length + ' 条' : ''}`">
      <template v-if="entries.length" #right>
        <n-button size="small" secondary :disabled="allCopyDisabled" @click="copyAll">
          {{ allCopyDisabled ? '解析中...' : allCopyLabel }}
        </n-button>
      </template>
      <div v-if="entries.length" class="list">
        <div v-for="(entry, index) in entries" :key="entry.short" class="row">
          <span class="idx">{{ index + 1 }}</span>
          <div class="link-col">
            <div class="line-primary">
              <span class="primary" :class="{ pending: entry.status === 'pending' }">
                <template v-if="entry.status === 'ok'">{{ entry.resolved }}</template>
                <template v-else-if="entry.status === 'pending'">解析中... {{ entry.short }}</template>
                <template v-else>{{ entry.short }}</template>
              </span>
              <div class="line-actions">
                <n-button size="tiny" secondary :disabled="entry.status === 'pending'" @click="copyOne(entry)">
                  {{ copyLabel(entry) }}
                </n-button>
                <n-button size="tiny" secondary :disabled="entry.status === 'pending'" @click="openLink(entry)">
                  打开
                </n-button>
              </div>
            </div>
            <span v-if="entry.status === 'ok'" class="secondary">短链：{{ entry.short }}</span>
            <span v-else-if="entry.status === 'fail'" class="secondary fail">短链解析失败：{{ entry.error }}</span>

            <template v-if="entry.status === 'ok'">
              <div v-if="entry.video.status === 'pending'" class="line-video secondary">解析视频中...</div>
              <div v-else-if="entry.video.status === 'fail'" class="line-video secondary fail">视频解析失败：{{ entry.video.error }}</div>
              <div v-else class="line-video">
                <span class="video-url" :class="{ 'has-wm': entry.video.info?.hasWatermark }">
                  {{ entry.video.info?.mp4Url }}
                </span>
                <div class="video-meta">
                  <span class="badge" :class="{ wm: entry.video.info?.hasWatermark }">
                    {{ entry.video.info?.hasWatermark ? '含水印' : '无水印' }}
                  </span>
                  <span class="video-title" :title="entry.video.info?.title">{{ entry.video.info?.title }}</span>
                </div>
                <div class="line-actions">
                  <n-button size="tiny" secondary @click="copyVideo(entry)">{{ videoCopyLabel(entry) }}</n-button>
                  <n-button size="tiny" type="primary" @click="downloadVideo(entry)">下载</n-button>
                </div>
              </div>
            </template>
          </div>
        </div>
      </div>
      <div v-else-if="hasInput" class="empty">未检测到 v.douyin.com 短链</div>
      <div v-else class="empty">粘贴分享文案后自动提取</div>
    </Panel>
  </div>
</template>

<style scoped>
.page { display: grid; gap: 16px; }
.page-header { display: grid; gap: 4px; }
.page-header h2 {
  margin: 0;
  font-size: var(--fs-xl);
  font-weight: 600;
  letter-spacing: -0.012em;
}
.page-header p {
  margin: 0;
  color: var(--text-muted);
  font-size: var(--fs-xs);
}
.platform-status {
  color: var(--text-muted);
  opacity: 0.7;
  margin-left: 4px;
}

.form { display: grid; gap: 12px; }
.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

.list { display: grid; gap: 6px; }
.row {
  display: grid;
  grid-template-columns: 28px 1fr;
  gap: 10px;
  padding: 10px 12px;
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  background: var(--bg-base);
}
.idx {
  color: var(--text-muted);
  font-size: var(--fs-xs);
  text-align: right;
  font-variant-numeric: tabular-nums;
  padding-top: 2px;
}
.link-col { display: grid; gap: 6px; min-width: 0; }

.line-primary {
  display: flex;
  gap: 10px;
  align-items: flex-start;
  justify-content: space-between;
}
.primary {
  color: var(--text);
  font-size: var(--fs-sm);
  font-family: var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace);
  word-break: break-all;
  flex: 1;
  min-width: 0;
}
.primary.pending { color: var(--text-muted); font-style: italic; }
.secondary {
  color: var(--text-muted);
  font-size: var(--fs-xxs);
  word-break: break-all;
}
.secondary.fail { color: var(--text-muted); }

.line-video {
  display: grid;
  gap: 4px;
  padding: 8px 10px;
  border: 1px dashed var(--line);
  border-radius: var(--radius-sm);
}
.video-url {
  font-size: var(--fs-xs);
  font-family: var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace);
  word-break: break-all;
  color: var(--text);
}
.video-meta {
  display: flex;
  gap: 8px;
  align-items: center;
  font-size: var(--fs-xxs);
  color: var(--text-muted);
}
.badge {
  padding: 1px 6px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--line-strong);
  color: var(--text-muted);
  font-size: var(--fs-xxs);
  line-height: 1.4;
}
.badge.wm { color: var(--text); border-color: var(--text-muted); }
.video-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
  flex: 1;
}
.line-actions {
  display: flex;
  gap: 6px;
  align-self: start;
  justify-content: flex-end;
}

.empty {
  padding: 40px 20px;
  text-align: center;
  color: var(--text-muted);
  border: 1px dashed var(--line-strong);
  border-radius: var(--radius-md);
  font-size: var(--fs-sm);
}
</style>
