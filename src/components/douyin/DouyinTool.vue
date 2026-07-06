<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { NButton, NInput } from 'naive-ui';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { invoke } from '@tauri-apps/api/core';
import Panel from '../ui/Panel.vue';
import { extractDouyinLinks } from '../../utils/douyinLink';

const raw = ref('');
const results = ref<string[]>([]);
const copyState = ref<Record<string, 'idle' | 'ok' | 'fail'>>({});
const allCopyState = ref<'idle' | 'ok' | 'fail'>('idle');

let debounceTimer: ReturnType<typeof setTimeout> | null = null;

watch(raw, (value) => {
  if (debounceTimer) clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => {
    results.value = extractDouyinLinks(value);
  }, 300);
});

function extractNow() {
  if (debounceTimer) clearTimeout(debounceTimer);
  results.value = extractDouyinLinks(raw.value);
}

function clearAll() {
  raw.value = '';
  results.value = [];
  copyState.value = {};
  allCopyState.value = 'idle';
}

async function copyOne(link: string) {
  try {
    await writeText(link);
    copyState.value = { ...copyState.value, [link]: 'ok' };
  } catch {
    copyState.value = { ...copyState.value, [link]: 'fail' };
  }
  setTimeout(() => {
    copyState.value = { ...copyState.value, [link]: 'idle' };
  }, 1500);
}

async function copyAll() {
  try {
    await writeText(results.value.join('\n'));
    allCopyState.value = 'ok';
  } catch {
    allCopyState.value = 'fail';
  }
  setTimeout(() => { allCopyState.value = 'idle'; }, 1500);
}

async function openLink(link: string) {
  try {
    await invoke('open_external_url', { url: link });
    return;
  } catch {
    // 打开失败降级为复制到剪贴板
  }
  try {
    await writeText(link);
    copyState.value = { ...copyState.value, [link]: 'ok' };
    setTimeout(() => {
      copyState.value = { ...copyState.value, [link]: 'idle' };
    }, 1500);
  } catch {
    /* noop */
  }
}

function copyLabel(link: string): string {
  const state = copyState.value[link] ?? 'idle';
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
</script>

<template>
  <div class="page">
    <header class="page-header">
      <h2>抖音链接提取</h2>
      <p>从抖音 App 分享文案中提取所有 v.douyin.com 短链。</p>
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

    <Panel :title="`提取结果${results.length ? ' · 共 ' + results.length + ' 条' : ''}`">
      <template v-if="results.length" #right>
        <n-button size="small" secondary @click="copyAll">{{ allCopyLabel }}</n-button>
      </template>
      <div v-if="results.length" class="list">
        <div v-for="(link, index) in results" :key="link" class="row">
          <span class="idx">{{ index + 1 }}</span>
          <span class="link">{{ link }}</span>
          <div class="row-actions">
            <n-button size="small" secondary @click="copyOne(link)">{{ copyLabel(link) }}</n-button>
            <n-button size="small" secondary @click="openLink(link)">打开</n-button>
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
.link {
  color: var(--text);
  font-size: var(--fs-sm);
  font-family: var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace);
  word-break: break-all;
}
.row-actions { display: flex; gap: 6px; }

.empty {
  padding: 40px 20px;
  text-align: center;
  color: var(--text-muted);
  border: 1px dashed var(--line-strong);
  border-radius: var(--radius-md);
  font-size: var(--fs-sm);
}
</style>
