<script setup lang="ts">
import { computed, ref } from 'vue';
import { NAlert, NButton, NInput, NInputNumber } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import Panel from '../ui/Panel.vue';

interface PortItem {
  port: number;
  status: string;
  message?: string;
  elapsedMs: number;
}

interface PortResponse {
  host: string;
  results: PortItem[];
}

const host = ref('example.com');
const ports = ref('80, 443, 22, 8080-8083');
const timeoutMs = ref(2000);
const running = ref(false);
const error = ref('');
const result = ref<PortResponse | null>(null);

async function run() {
  error.value = '';
  result.value = null;
  running.value = true;
  try {
    result.value = await invoke<PortResponse>('check_ports', {
      request: {
        host: host.value.trim(),
        ports: ports.value.trim(),
        timeoutMs: timeoutMs.value
      }
    });
  } catch (err) {
    error.value = String(err);
  } finally {
    running.value = false;
  }
}

const summary = computed(() => {
  if (!result.value) return { open: 0, closed: 0, timeout: 0, error: 0 };
  const s = { open: 0, closed: 0, timeout: 0, error: 0 };
  for (const r of result.value.results) {
    if (r.status === 'open') s.open++;
    else if (r.status === 'closed') s.closed++;
    else if (r.status === 'timeout') s.timeout++;
    else s.error++;
  }
  return s;
});

function tagClass(status: string) {
  return `tag tag-${status}`;
}
</script>

<template>
  <div class="pane">
    <Panel title="端口检查">
      <div class="form">
        <label class="field">
          <span class="lbl">主机名或 IP</span>
          <n-input v-model:value="host" placeholder="example.com / 192.168.1.1" @keyup.enter="run" />
        </label>

        <label class="field">
          <span class="lbl">端口（支持 逗号、区间；单次 &le; 1000 个）</span>
          <n-input v-model:value="ports" placeholder="80, 443, 8000-8010" @keyup.enter="run" />
        </label>

        <label class="field">
          <span class="lbl">单端口超时（毫秒，200-15000）</span>
          <n-input-number v-model:value="timeoutMs" :min="200" :max="15000" style="width: 100%" />
        </label>

        <n-alert v-if="error" type="error" :bordered="false">{{ error }}</n-alert>

        <n-button
          type="primary"
          block
          :loading="running"
          :disabled="!host.trim() || !ports.trim()"
          @click="run"
        >
          {{ running ? '扫描中...' : '开始扫描' }}
        </n-button>
      </div>
    </Panel>

    <Panel v-if="result" title="结果">
      <template #right>
        <span class="mono muted">
          open {{ summary.open }} · closed {{ summary.closed }} · timeout {{ summary.timeout }}
          <template v-if="summary.error > 0"> · error {{ summary.error }}</template>
        </span>
      </template>
      <table class="results">
        <thead>
          <tr>
            <th>端口</th>
            <th>状态</th>
            <th>耗时</th>
            <th>备注</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="r in result.results" :key="r.port">
            <td class="mono">{{ r.port }}</td>
            <td><span :class="tagClass(r.status)">{{ r.status }}</span></td>
            <td class="mono">{{ r.elapsedMs }} ms</td>
            <td class="muted">{{ r.message ?? '' }}</td>
          </tr>
        </tbody>
      </table>
    </Panel>
  </div>
</template>

<style scoped>
.pane { display: grid; gap: 12px; padding: 16px; height: 100%; overflow: auto; }
.form { display: grid; gap: 12px; }
.field { display: grid; gap: 6px; }
.lbl { font-size: var(--fs-xxs); color: var(--text-muted); }
.mono { font-family: var(--font-mono, ui-monospace, monospace); font-variant-numeric: tabular-nums; }
.muted { color: var(--text-muted); font-size: var(--fs-xxs); }

.results { width: 100%; border-collapse: collapse; font-size: var(--fs-sm); }
.results thead th {
  text-align: left;
  color: var(--text-muted);
  font-size: var(--fs-xxs);
  font-weight: 500;
  padding: 6px 10px;
  border-bottom: 1px solid var(--line);
}
.results tbody td {
  padding: 6px 10px;
  border-bottom: 1px solid var(--line-weak, var(--line));
}
.tag {
  display: inline-block;
  padding: 1px 8px;
  border: 1px solid var(--line-strong);
  border-radius: var(--radius-sm);
  font-size: var(--fs-xxs);
  color: var(--text-muted);
  font-family: var(--font-mono, monospace);
}
.tag-open { color: #10b981; border-color: #10b981; }
.tag-closed { color: var(--text-muted); }
.tag-timeout { color: #f59e0b; border-color: #f59e0b; }
.tag-error { color: #ef4444; border-color: #ef4444; }
</style>
