<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { NButton, NInput } from 'naive-ui';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { invoke } from '@tauri-apps/api/core';
import Panel from '../ui/Panel.vue';
import { extractDouyinLinks } from '../../utils/douyinLink';

interface Entry {
  short: string;
  status: 'pending' | 'ok' | 'fail';
  resolved: string | null;
  error: string | null;
}

const raw = ref('');
const entries = ref<Entry[]>([]);
const copyState = ref<Record<string, 'idle' | 'ok' | 'fail'>>({});
const allCopyState = ref<'idle' | 'ok' | 'fail'>('idle');

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
  }));
  copyState.value = {};

  links.forEach((short) => {
    invoke<string>('resolve_douyin_url', { url: short })
      .then((resolved) => {
        if (runId !== resolveRun) return;
        const idx = entries.value.findIndex((e) => e.short === short);
        if (idx === -1) return;
        entries.value[idx] = { short, status: 'ok', resolved, error: null };
      })
      .catch((err) => {
        if (runId !== resolveRun) return;
        const idx = entries.value.findIndex((e) => e.short === short);
        if (idx === -1) return;
        entries.value[idx] = { short, status: 'fail', resolved: null, error: String(err) };
      });
  });
}

function clearAll() {
  raw.value = '';
  entries.value = [];
  copyState.value = {};
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
    // fallback: 写剪贴板
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

function copyLabel(entry: Entry): string {
  const state = copyState.value[entry.short] ?? 'idle';
  if (state === 'ok') return '已复制';
  if (state === 'fail') return '复制失败';
  return '复制';
}

const allCopyLabel = computed(() => {
  if (allCopyState.value === 'ok') return '已全部复制';
  if (allCopyState.value === 'fail') return '复制失败';
  return '全部复制';
});

const hasInput = computed(() => raw.value.trim().length > 0);
const hasPending = computed(() => entries.value.some((e) => e.status === 'pending'));
const allCopyDisabled = computed(() => hasPending.value);
</script>

<template>
  <div class="page">
    <header class="page-header">
      <h2>抖音链接提取</h2>
      <p>从抖音 App 分享文案中提取 v.douyin.com 短链，并自动跟踪 302 转真实视频页地址。</p>
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
            <span class="primary" :class="{ pending: entry.status === 'pending' }">
              <template v-if="entry.status === 'ok'">{{ entry.resolved }}</template>
              <template v-else-if="entry.status === 'pending'">解析中... {{ entry.short }}</template>
              <template v-else>{{ entry.short }}</template>
            </span>
            <span v-if="entry.status === 'ok'" class="secondary">短链：{{ entry.short }}</span>
            <span v-else-if="entry.status === 'fail'" class="secondary fail">解析失败：{{ entry.error }}（已保留短链）</span>
          </div>
          <div class="row-actions">
            <n-button size="small" secondary :disabled="entry.status === 'pending'" @click="copyOne(entry)">
              {{ copyLabel(entry) }}
            </n-button>
            <n-button size="small" secondary :disabled="entry.status === 'pending'" @click="openLink(entry)">
              打开
            </n-button>
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

.form { display: grid; gap: 12px; }
.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

.list { display: grid; gap: 6px; }
.row {
  display: grid;
  grid-template-columns: 28px 1fr auto;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  background: var(--bg-base);
}
.idx {
  color: var(--text-muted);
  font-size: var(--fs-xs);
  text-align: right;
  font-variant-numeric: tabular-nums;
}
.link-col { display: grid; gap: 2px; min-width: 0; }
.primary {
  color: var(--text);
  font-size: var(--fs-sm);
  font-family: var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace);
  word-break: break-all;
}
.primary.pending { color: var(--text-muted); font-style: italic; }
.secondary {
  color: var(--text-muted);
  font-size: var(--fs-xxs);
  word-break: break-all;
}
.secondary.fail { color: var(--text-muted); }
.row-actions { display: flex; gap: 6px; align-self: start; }

.empty {
  padding: 40px 20px;
  text-align: center;
  color: var(--text-muted);
  border: 1px dashed var(--line-strong);
  border-radius: var(--radius-md);
  font-size: var(--fs-sm);
}
</style>
