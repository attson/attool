<script setup lang="ts">
import { ref } from 'vue';
import { NAlert, NButton, NInput, NInputNumber } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import Panel from '../ui/Panel.vue';

interface PingResponse {
  rawOutput: string;
  transmitted?: number;
  received?: number;
  lossPercent?: number;
  minMs?: number;
  avgMs?: number;
  maxMs?: number;
}

const host = ref('example.com');
const count = ref(4);
const timeoutSeconds = ref(2);
const running = ref(false);
const error = ref('');
const result = ref<PingResponse | null>(null);

async function run() {
  error.value = '';
  result.value = null;
  running.value = true;
  try {
    result.value = await invoke<PingResponse>('ping_host', {
      request: {
        host: host.value.trim(),
        count: count.value,
        timeoutSeconds: timeoutSeconds.value
      }
    });
  } catch (err) {
    error.value = String(err);
  } finally {
    running.value = false;
  }
}
</script>

<template>
  <div class="pane">
    <Panel title="Ping">
      <div class="form">
        <label class="field">
          <span class="lbl">主机名或 IP</span>
          <n-input v-model:value="host" placeholder="example.com / 8.8.8.8" @keyup.enter="run" />
        </label>

        <div class="row">
          <label class="field">
            <span class="lbl">次数</span>
            <n-input-number v-model:value="count" :min="1" :max="20" style="width: 100%" />
          </label>
          <label class="field">
            <span class="lbl">单次超时（秒）</span>
            <n-input-number v-model:value="timeoutSeconds" :min="1" :max="30" style="width: 100%" />
          </label>
        </div>

        <n-alert v-if="error" type="error" :bordered="false">{{ error }}</n-alert>

        <n-button type="primary" block :loading="running" :disabled="!host.trim()" @click="run">
          {{ running ? '进行中...' : '开始 Ping' }}
        </n-button>
      </div>
    </Panel>

    <template v-if="result">
      <Panel title="摘要">
        <div v-if="result.transmitted !== undefined" class="summary">
          <div class="cell">
            <div class="cell-k">发/收/丢</div>
            <div class="cell-v mono">
              {{ result.transmitted }} / {{ result.received }} / {{ result.lossPercent }}%
            </div>
          </div>
          <div class="cell">
            <div class="cell-k">最小</div>
            <div class="cell-v mono">{{ result.minMs !== undefined ? result.minMs + ' ms' : '—' }}</div>
          </div>
          <div class="cell">
            <div class="cell-k">平均</div>
            <div class="cell-v mono">{{ result.avgMs !== undefined ? result.avgMs + ' ms' : '—' }}</div>
          </div>
          <div class="cell">
            <div class="cell-k">最大</div>
            <div class="cell-v mono">{{ result.maxMs !== undefined ? result.maxMs + ' ms' : '—' }}</div>
          </div>
        </div>
        <p v-else class="muted">未能解析统计行，直接看下面的原始输出。</p>
      </Panel>

      <Panel title="原始输出">
        <pre class="raw">{{ result.rawOutput }}</pre>
      </Panel>
    </template>
  </div>
</template>

<style scoped>
.pane { display: grid; gap: 12px; padding: 16px; height: 100%; overflow: auto; }
.form { display: grid; gap: 12px; }
.field { display: grid; gap: 6px; }
.lbl { font-size: var(--fs-xxs); color: var(--text-muted); }
.row { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
.summary {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 12px;
}
@media (max-width: 700px) { .summary { grid-template-columns: 1fr 1fr; } }
.cell { display: grid; gap: 4px; }
.cell-k { font-size: var(--fs-xxs); color: var(--text-muted); }
.cell-v { font-size: var(--fs-sm); }
.mono { font-family: var(--font-mono, ui-monospace, monospace); font-variant-numeric: tabular-nums; }
.raw {
  margin: 0;
  padding: 10px 12px;
  background: var(--bg-elev);
  border-radius: var(--radius-sm);
  font-family: var(--font-mono, monospace);
  font-size: var(--fs-xxs);
  color: var(--text);
  white-space: pre-wrap;
  word-break: break-word;
}
.muted { margin: 0; color: var(--text-muted); font-size: var(--fs-xs); }
</style>
