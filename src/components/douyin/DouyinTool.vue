<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { NButton, NInput } from 'naive-ui';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { invoke } from '@tauri-apps/api/core';
import Panel from '../ui/Panel.vue';
import { extractLinks, platformName, type VideoPlatform } from '../../utils/platformDetect';
import { useAria2Handoff } from '../../composables/useAria2Handoff';
import type { DouyinVideoInfo } from '../../types/douyin';

interface SubtitleTrack {
  language: string;
  name: string;
  url: string;
}

interface MediaInfo {
  title: string;
  cover?: string;
  videoUrl?: string;
  audioUrl?: string;
  imageUrls?: string[];
  subtitles?: SubtitleTrack[];
  hasWatermark?: boolean;
  uploader?: string;
  notes?: string;
}

interface VideoState {
  status: 'pending' | 'ok' | 'fail';
  info: MediaInfo | null;
  error: string | null;
}

interface Entry {
  short: string;
  platform: VideoPlatform;
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
const youtubeProxy = ref('');
const handoff = useAria2Handoff();

let debounceTimer: ReturnType<typeof setTimeout> | null = null;
let resolveRun = 0;

watch(raw, (value) => {
  if (debounceTimer) clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => {
    rebuildEntries(extractLinks(value));
  }, 300);
});

function extractNow() {
  if (debounceTimer) clearTimeout(debounceTimer);
  rebuildEntries(extractLinks(raw.value));
}

function rebuildEntries(links: { url: string; platform: VideoPlatform }[]) {
  const runId = ++resolveRun;
  entries.value = links.map((link) => ({
    short: link.url,
    platform: link.platform,
    status: 'pending',
    resolved: null,
    error: null,
    video: { status: 'pending', info: null, error: null }
  }));
  copyState.value = {};
  videoCopyState.value = {};

  links.forEach((link) => {
    const short = link.url;
    if (link.platform === 'douyin') {
      resolveDouyin(runId, short);
    } else if (link.platform === 'xhs') {
      resolveXhs(runId, short);
    } else if (link.platform === 'bilibili') {
      resolveBilibili(runId, short);
    } else if (link.platform === 'youtube') {
      resolveYoutube(runId, short);
    }
  });
}

function updateEntry(runId: number, short: string, patch: Partial<Entry>) {
  if (runId !== resolveRun) return;
  const idx = entries.value.findIndex((e) => e.short === short);
  if (idx === -1) return;
  entries.value[idx] = { ...entries.value[idx], ...patch };
}

function resolveDouyin(runId: number, short: string) {
  invoke<string>('resolve_douyin_url', { url: short })
    .then((resolved) => {
      updateEntry(runId, short, { status: 'ok', resolved, error: null });
      invoke<DouyinVideoInfo>('extract_douyin_video', { url: resolved })
        .then((info) => {
          updateEntry(runId, short, {
            video: {
              status: 'ok',
              info: {
                title: info.title,
                videoUrl: info.mp4Url,
                hasWatermark: info.hasWatermark
              },
              error: null
            }
          });
        })
        .catch((err) => {
          updateEntry(runId, short, {
            video: { status: 'fail', info: null, error: String(err) }
          });
        });
    })
    .catch((err) => {
      updateEntry(runId, short, {
        status: 'fail',
        resolved: null,
        error: String(err),
        video: { status: 'fail', info: null, error: '短链未能解析' }
      });
    });
}

interface XhsResult {
  title: string;
  noteType: string;
  cover?: string;
  videoUrl?: string;
  imageUrls: string[];
}

function resolveXhs(runId: number, short: string) {
  invoke<XhsResult>('extract_xhs_note', { url: short })
    .then((info) => {
      updateEntry(runId, short, {
        status: 'ok',
        resolved: short,
        error: null,
        video: {
          status: 'ok',
          info: {
            title: info.title,
            cover: info.cover,
            videoUrl: info.videoUrl,
            imageUrls: info.imageUrls,
            notes: info.noteType === 'video' ? undefined : '图文帖：下载图片列表'
          },
          error: null
        }
      });
    })
    .catch((err) => {
      updateEntry(runId, short, {
        status: 'fail',
        error: String(err),
        video: { status: 'fail', info: null, error: String(err) }
      });
    });
}

interface BiliResult {
  title: string;
  cover?: string;
  uploader?: string;
  bvid: string;
  duration?: number;
  videoUrl?: string;
  audioUrl?: string;
  qualityNote?: string;
}

function resolveBilibili(runId: number, short: string) {
  invoke<BiliResult>('extract_bilibili_video', { url: short })
    .then((info) => {
      updateEntry(runId, short, {
        status: 'ok',
        resolved: `https://www.bilibili.com/video/${info.bvid}`,
        error: null,
        video: {
          status: 'ok',
          info: {
            title: info.title,
            cover: info.cover,
            uploader: info.uploader,
            videoUrl: info.videoUrl,
            audioUrl: info.audioUrl,
            notes: info.qualityNote
          },
          error: null
        }
      });
    })
    .catch((err) => {
      updateEntry(runId, short, {
        status: 'fail',
        error: String(err),
        video: { status: 'fail', info: null, error: String(err) }
      });
    });
}

interface YoutubeResult {
  title: string;
  uploader?: string;
  cover?: string;
  duration?: number;
  videoUrl?: string;
  subtitleUrls: SubtitleTrack[];
  notes?: string;
}

function resolveYoutube(runId: number, short: string) {
  const proxy = youtubeProxy.value.trim() || null;
  invoke<YoutubeResult>('extract_youtube_video', { url: short, proxy })
    .then((info) => {
      updateEntry(runId, short, {
        status: 'ok',
        resolved: short,
        error: null,
        video: {
          status: 'ok',
          info: {
            title: info.title,
            uploader: info.uploader,
            cover: info.cover,
            videoUrl: info.videoUrl,
            subtitles: info.subtitleUrls,
            notes: info.notes
          },
          error: null
        }
      });
    })
    .catch((err) => {
      updateEntry(runId, short, {
        status: 'fail',
        error: String(err),
        video: { status: 'fail', info: null, error: String(err) }
      });
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
  if (!entry.video.info?.videoUrl) return;
  const key = entry.short;
  try {
    await writeText(entry.video.info.videoUrl);
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
  setTimeout(() => {
    allCopyState.value = 'idle';
  }, 1500);
}

async function openLink(entry: Entry) {
  const url = effectiveUrl(entry);
  try {
    await invoke('open_external_url', { url });
  } catch {
    try {
      await writeText(url);
    } catch {
      /* noop */
    }
  }
}

function downloadUrl(url: string) {
  handoff.push(url);
  emit('requestNavigate', 'aria2');
}

function downloadVideo(entry: Entry) {
  if (!entry.video.info?.videoUrl) return;
  downloadUrl(entry.video.info.videoUrl);
}

function downloadAudio(entry: Entry) {
  if (!entry.video.info?.audioUrl) return;
  downloadUrl(entry.video.info.audioUrl);
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
const hasYoutubeEntry = computed(() => entries.value.some((e) => e.platform === 'youtube'));
</script>

<template>
  <div class="page">
    <header class="page-header">
      <h2>视频链接抽取</h2>
      <p>
        识别抖音 / 小红书 / B 站 / YouTube 分享链接，抓取视频直链、封面、字幕，一键送入 Aria2 下载。
      </p>
    </header>

    <Panel title="分享文案">
      <div class="form">
        <n-input
          v-model:value="raw"
          type="textarea"
          placeholder="粘贴多平台分享文案。抖音 v.douyin.com、小红书 xhslink.com、B 站 b23.tv、YouTube youtu.be/watch 都能识别。"
          :autosize="{ minRows: 6, maxRows: 12 }"
        />
        <div v-if="hasYoutubeEntry" class="proxy-row">
          <span class="proxy-lbl">YouTube 代理（可选）：</span>
          <n-input
            v-model:value="youtubeProxy"
            placeholder="socks5://127.0.0.1:1080 或 http://..."
            size="small"
            style="max-width: 320px"
          />
        </div>
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
                <span class="platform-badge">{{ platformName(entry.platform) }}</span>
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
            <span v-if="entry.status === 'ok' && entry.short !== entry.resolved" class="secondary">
              短链：{{ entry.short }}
            </span>
            <span v-else-if="entry.status === 'fail'" class="secondary fail">
              解析失败：{{ entry.error }}
            </span>

            <template v-if="entry.status === 'ok'">
              <div v-if="entry.video.status === 'pending'" class="line-video secondary">
                解析视频中...
              </div>
              <div v-else-if="entry.video.status === 'fail'" class="line-video secondary fail">
                视频解析失败：{{ entry.video.error }}
              </div>
              <div v-else class="line-video">
                <div v-if="entry.video.info?.title" class="video-meta">
                  <span class="video-title" :title="entry.video.info.title">
                    {{ entry.video.info.title }}
                  </span>
                  <span v-if="entry.video.info.uploader" class="uploader">
                    @{{ entry.video.info.uploader }}
                  </span>
                  <span
                    v-if="entry.platform === 'douyin' && typeof entry.video.info.hasWatermark === 'boolean'"
                    class="badge"
                    :class="{ wm: entry.video.info.hasWatermark }"
                  >
                    {{ entry.video.info.hasWatermark ? '含水印' : '无水印' }}
                  </span>
                </div>

                <div v-if="entry.video.info?.videoUrl" class="media-row">
                  <span class="video-url">{{ entry.video.info.videoUrl }}</span>
                  <div class="line-actions">
                    <n-button size="tiny" secondary @click="copyVideo(entry)">
                      {{ videoCopyLabel(entry) }}
                    </n-button>
                    <n-button size="tiny" type="primary" @click="downloadVideo(entry)">
                      下载视频
                    </n-button>
                  </div>
                </div>

                <div v-if="entry.video.info?.audioUrl" class="media-row secondary">
                  <span class="video-url">🎵 {{ entry.video.info.audioUrl }}</span>
                  <n-button size="tiny" secondary @click="downloadAudio(entry)">
                    下载音频
                  </n-button>
                </div>

                <div v-if="entry.video.info?.imageUrls?.length" class="media-row secondary">
                  <span>图片 {{ entry.video.info.imageUrls.length }} 张</span>
                  <n-button
                    size="tiny"
                    secondary
                    @click="entry.video.info?.imageUrls?.forEach((u) => downloadUrl(u))"
                  >
                    下载全部
                  </n-button>
                </div>

                <div v-if="entry.video.info?.subtitles?.length" class="media-row secondary">
                  <span>字幕：</span>
                  <n-button
                    v-for="sub in entry.video.info.subtitles"
                    :key="sub.url"
                    size="tiny"
                    secondary
                    @click="downloadUrl(sub.url)"
                  >
                    {{ sub.name }}
                  </n-button>
                </div>

                <div v-if="entry.video.info?.notes" class="notes">
                  {{ entry.video.info.notes }}
                </div>
              </div>
            </template>
          </div>
        </div>
      </div>
      <div v-else-if="hasInput" class="empty">未检测到已知平台的分享链接</div>
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
.platform-badge {
  display: inline-block;
  padding: 1px 6px;
  margin-right: 6px;
  border: 1px solid var(--line-strong);
  border-radius: var(--radius-sm);
  color: var(--text-muted);
  font-size: var(--fs-xxs);
  font-family: -apple-system, "PingFang SC", "Segoe UI", sans-serif;
  vertical-align: middle;
}

.form { display: grid; gap: 12px; }
.proxy-row {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: var(--fs-xs);
}
.proxy-lbl { color: var(--text-muted); }
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
.secondary.fail { color: var(--danger, #f87171); }

.line-video {
  display: grid;
  gap: 6px;
  padding: 8px 10px;
  border: 1px dashed var(--line);
  border-radius: var(--radius-sm);
}
.video-meta {
  display: flex;
  gap: 8px;
  align-items: center;
  font-size: var(--fs-xxs);
  color: var(--text-muted);
  flex-wrap: wrap;
}
.uploader { color: var(--text-muted); font-size: var(--fs-xxs); }
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
  color: var(--text);
  font-size: var(--fs-xs);
}
.media-row {
  display: flex;
  gap: 8px;
  align-items: center;
  justify-content: space-between;
}
.media-row.secondary { color: var(--text-muted); font-size: var(--fs-xxs); }
.video-url {
  font-size: var(--fs-xs);
  font-family: var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace);
  word-break: break-all;
  color: var(--text);
  flex: 1;
  min-width: 0;
}
.line-actions {
  display: flex;
  gap: 6px;
  align-self: start;
  justify-content: flex-end;
}
.notes {
  font-size: var(--fs-xxs);
  color: var(--text-muted);
  font-style: italic;
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
