<script setup lang="ts">
import { ref } from 'vue';
import { NAlert, NButton, NInput } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import Panel from '../ui/Panel.vue';

interface DnsResponse {
  v4: string[];
  v6: string[];
}

const host = ref('example.com');
const running = ref(false);
const error = ref('');
const result = ref<DnsResponse | null>(null);

async function run() {
  error.value = '';
  result.value = null;
  running.value = true;
  try {
    result.value = await invoke<DnsResponse>('resolve_dns', { host: host.value.trim() });
  } catch (err) {
    error.value = String(err);
  } finally {
    running.value = false;
  }
}
</script>

<template>
  <div class="pane">
    <Panel title="DNS 解析">
      <div class="form">
        <label class="field">
          <span class="lbl">主机名</span>
          <n-input v-model:value="host" placeholder="example.com" @keyup.enter="run" />
        </label>

        <n-alert v-if="error" type="error" :bordered="false">{{ error }}</n-alert>

        <n-button type="primary" block :loading="running" :disabled="!host.trim()" @click="run">
          {{ running ? '查询中...' : '解析' }}
        </n-button>

        <p class="note">
          走系统 <code>getaddrinfo</code>，仅返回 A / AAAA 记录，不支持 MX / TXT / CNAME 等
          （那些需要额外 DNS 客户端库）。
        </p>
      </div>
    </Panel>

    <template v-if="result">
      <Panel :title="'IPv4（' + result.v4.length + '）'">
        <ul v-if="result.v4.length > 0" class="ip-list">
          <li v-for="ip in result.v4" :key="ip" class="mono">{{ ip }}</li>
        </ul>
        <p v-else class="muted">无</p>
      </Panel>

      <Panel :title="'IPv6（' + result.v6.length + '）'">
        <ul v-if="result.v6.length > 0" class="ip-list">
          <li v-for="ip in result.v6" :key="ip" class="mono">{{ ip }}</li>
        </ul>
        <p v-else class="muted">无</p>
      </Panel>
    </template>
  </div>
</template>

<style scoped>
.pane { display: grid; gap: 12px; padding: 16px; height: 100%; overflow: auto; }
.form { display: grid; gap: 12px; }
.field { display: grid; gap: 6px; }
.lbl { font-size: var(--fs-xxs); color: var(--text-muted); }
.note { margin: 0; color: var(--text-muted); font-size: var(--fs-xxs); }
.note code {
  padding: 1px 6px;
  background: var(--bg-elev);
  border-radius: 3px;
  font-family: var(--font-mono, monospace);
}
.ip-list { list-style: none; margin: 0; padding: 0; display: grid; gap: 4px; }
.ip-list li {
  padding: 6px 10px;
  background: var(--bg-elev);
  border-radius: var(--radius-sm);
  font-size: var(--fs-sm);
}
.mono { font-family: var(--font-mono, ui-monospace, monospace); }
.muted { color: var(--text-muted); font-size: var(--fs-xs); margin: 0; }
</style>
