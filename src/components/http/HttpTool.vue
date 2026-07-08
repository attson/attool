<script setup lang="ts">
import { computed, ref } from 'vue';
import { NAlert, NButton, NInput, NInputNumber, NSelect, NTabs, NTabPane } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import Panel from '../ui/Panel.vue';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';

interface KV {
  key: string;
  value: string;
  enabled: boolean;
}

interface Resp {
  status: number;
  statusText: string;
  headers: [string, string][];
  body: string;
  bodyBytes: number;
  elapsedMs: number;
  finalUrl: string;
}

const method = ref('GET');
const url = ref('https://httpbin.org/get');
const headers = ref<KV[]>([{ key: 'User-Agent', value: 'AT Tool/0.7', enabled: true }]);
const queryParams = ref<KV[]>([]);
const bodyType = ref('none');
const body = ref('');
const timeoutSeconds = ref(30);
const followRedirects = ref(true);
const requestTab = ref('headers');
const respTab = ref('body');

const sending = ref(false);
const response = ref<Resp | null>(null);
const error = ref('');

const methodOptions = [
  { label: 'GET', value: 'GET' },
  { label: 'POST', value: 'POST' },
  { label: 'PUT', value: 'PUT' },
  { label: 'PATCH', value: 'PATCH' },
  { label: 'DELETE', value: 'DELETE' },
  { label: 'HEAD', value: 'HEAD' },
  { label: 'OPTIONS', value: 'OPTIONS' }
];

const bodyTypeOptions = [
  { label: 'None', value: 'none' },
  { label: 'JSON', value: 'json' },
  { label: 'Form URL-encoded', value: 'form' },
  { label: 'Raw text', value: 'text' }
];

const bodyDisabled = computed(() => method.value === 'GET' || method.value === 'HEAD');

async function send() {
  error.value = '';
  response.value = null;
  sending.value = true;
  try {
    response.value = await invoke<Resp>('send_http', {
      request: {
        method: method.value,
        url: url.value.trim(),
        headers: headers.value,
        queryParams: queryParams.value,
        bodyType: bodyDisabled.value ? 'none' : bodyType.value,
        body: bodyDisabled.value ? '' : body.value,
        timeoutSeconds: timeoutSeconds.value,
        followRedirects: followRedirects.value
      }
    });
  } catch (err) {
    error.value = String(err);
  } finally {
    sending.value = false;
  }
}

function addRow(list: KV[]) {
  list.push({ key: '', value: '', enabled: true });
}
function removeRow(list: KV[], idx: number) {
  list.splice(idx, 1);
}

const prettyBody = computed(() => {
  if (!response.value) return '';
  const text = response.value.body;
  const ct = response.value.headers.find(([k]) => k.toLowerCase() === 'content-type')?.[1] ?? '';
  if (ct.includes('json')) {
    try {
      return JSON.stringify(JSON.parse(text), null, 2);
    } catch {
      return text;
    }
  }
  return text;
});

const statusClass = computed(() => {
  const s = response.value?.status ?? 0;
  if (s === 0) return 'status-error';
  if (s < 300) return 'status-2xx';
  if (s < 400) return 'status-3xx';
  if (s < 500) return 'status-4xx';
  return 'status-5xx';
});

async function copyBody() {
  if (!response.value) return;
  try { await writeText(prettyBody.value); } catch {}
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}
</script>

<template>
  <div class="http-tool">
    <div class="request-bar">
      <n-select v-model:value="method" :options="methodOptions" size="small" style="width: 110px" />
      <n-input v-model:value="url" placeholder="https://example.com/api" @keyup.enter="send" />
      <n-button type="primary" :loading="sending" @click="send">发送</n-button>
    </div>

    <n-tabs v-model:value="requestTab" type="line" size="small">
      <n-tab-pane name="headers" :tab="`Headers (${headers.filter((h) => h.enabled && h.key).length})`">
        <div class="kv-table">
          <div v-for="(row, i) in headers" :key="i" class="kv-row">
            <input type="checkbox" v-model="row.enabled" />
            <input class="kv-input" v-model="row.key" placeholder="Header 名" />
            <input class="kv-input" v-model="row.value" placeholder="值" />
            <button class="kv-del" @click="removeRow(headers, i)">✕</button>
          </div>
          <n-button size="tiny" secondary @click="addRow(headers)">+ 添加 header</n-button>
        </div>
      </n-tab-pane>

      <n-tab-pane name="query" :tab="`Query (${queryParams.filter((h) => h.enabled && h.key).length})`">
        <div class="kv-table">
          <div v-for="(row, i) in queryParams" :key="i" class="kv-row">
            <input type="checkbox" v-model="row.enabled" />
            <input class="kv-input" v-model="row.key" placeholder="参数名" />
            <input class="kv-input" v-model="row.value" placeholder="值" />
            <button class="kv-del" @click="removeRow(queryParams, i)">✕</button>
          </div>
          <n-button size="tiny" secondary @click="addRow(queryParams)">+ 添加 query</n-button>
        </div>
      </n-tab-pane>

      <n-tab-pane name="body" tab="Body">
        <div class="body-pane">
          <div class="body-opts">
            <n-select v-model:value="bodyType" :options="bodyTypeOptions" size="small" style="width: 180px" :disabled="bodyDisabled" />
            <span v-if="bodyDisabled" class="muted">{{ method }} 请求不带 body</span>
          </div>
          <n-input
            v-if="!bodyDisabled && bodyType !== 'none'"
            v-model:value="body"
            type="textarea"
            :placeholder="bodyType === 'json' ? '{\n  &quot;key&quot;: &quot;value&quot;\n}' : bodyType === 'form' ? 'k1=v1&k2=v2' : '任意文本'"
            :autosize="{ minRows: 8, maxRows: 20 }"
          />
        </div>
      </n-tab-pane>

      <n-tab-pane name="settings" tab="设置">
        <div class="settings">
          <label class="opt">
            <span>超时（秒）</span>
            <n-input-number v-model:value="timeoutSeconds" :min="1" :max="300" size="small" style="width: 100px" />
          </label>
          <label class="opt">
            <input type="checkbox" v-model="followRedirects" />
            <span>跟随 302 / 301（最多 10 次）</span>
          </label>
        </div>
      </n-tab-pane>
    </n-tabs>

    <n-alert v-if="error" type="error" :bordered="false">{{ error }}</n-alert>

    <Panel v-if="response" title="响应">
      <template #right>
        <span class="resp-summary mono">
          <span :class="statusClass">{{ response.status }} {{ response.statusText }}</span>
          <span class="muted"> · {{ response.elapsedMs }} ms · {{ formatSize(response.bodyBytes) }}</span>
        </span>
      </template>

      <n-tabs v-model:value="respTab" type="line" size="small">
        <n-tab-pane name="body" tab="Body">
          <template #default>
            <div class="resp-body-actions">
              <n-button size="tiny" secondary :disabled="!prettyBody" @click="copyBody">复制</n-button>
            </div>
            <pre class="resp-body mono">{{ prettyBody }}</pre>
          </template>
        </n-tab-pane>
        <n-tab-pane name="headers" :tab="`Headers (${response.headers.length})`">
          <table class="resp-headers">
            <tr v-for="([k, v]) in response.headers" :key="k">
              <td class="k mono">{{ k }}</td>
              <td class="v mono">{{ v }}</td>
            </tr>
          </table>
        </n-tab-pane>
      </n-tabs>

      <p v-if="response.finalUrl !== url.trim()" class="muted final-url">
        最终 URL：<span class="mono">{{ response.finalUrl }}</span>
      </p>
    </Panel>
  </div>
</template>

<style scoped>
.http-tool { display: grid; gap: 12px; padding: 16px; height: 100%; overflow: auto; }
.request-bar { display: flex; gap: 8px; align-items: center; }
.request-bar > :nth-child(2) { flex: 1; }

.kv-table { display: grid; gap: 4px; padding: 4px 0; }
.kv-row {
  display: grid;
  grid-template-columns: 24px 1fr 2fr 32px;
  gap: 6px;
  align-items: center;
}
.kv-input {
  padding: 4px 8px;
  background: var(--bg-elev);
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  color: var(--text);
  font-size: var(--fs-xs);
  font-family: var(--font-mono, monospace);
  outline: none;
}
.kv-input:focus { border-color: var(--accent, #10b981); }
.kv-del {
  background: none; border: none; color: var(--text-muted); cursor: pointer;
}
.kv-del:hover { color: #ef4444; }

.body-pane { display: grid; gap: 10px; padding: 4px 0; }
.body-opts { display: flex; align-items: center; gap: 12px; }

.settings { display: grid; gap: 10px; padding: 4px 0; }
.opt { display: flex; align-items: center; gap: 8px; font-size: var(--fs-xs); }

.resp-summary { font-size: var(--fs-xs); }
.status-2xx { color: #10b981; font-weight: 600; }
.status-3xx { color: #3b82f6; font-weight: 600; }
.status-4xx { color: #f59e0b; font-weight: 600; }
.status-5xx, .status-error { color: #ef4444; font-weight: 600; }
.muted { color: var(--text-muted); }
.mono { font-family: var(--font-mono, ui-monospace, monospace); font-variant-numeric: tabular-nums; }

.resp-body-actions { display: flex; justify-content: flex-end; padding: 4px 0; }
.resp-body {
  margin: 0;
  padding: 10px 12px;
  background: var(--bg-elev);
  border-radius: var(--radius-sm);
  font-size: var(--fs-xs);
  color: var(--text);
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 500px;
  overflow: auto;
}
.resp-headers { width: 100%; border-collapse: collapse; font-size: var(--fs-xs); }
.resp-headers td { padding: 4px 10px; border-bottom: 1px solid var(--line-weak, var(--line)); vertical-align: top; }
.resp-headers .k { color: var(--text-muted); width: 30%; }
.resp-headers .v { word-break: break-all; }

.final-url { margin: 8px 0 0; font-size: var(--fs-xxs); }
</style>
