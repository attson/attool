<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { NAlert, NInput } from 'naive-ui';
import Panel from '../ui/Panel.vue';

interface QueryPair {
  key: string;
  value: string;
}

interface Parsed {
  protocol: string;
  username: string;
  password: string;
  hostname: string;
  port: string;
  pathname: string;
  search: string;
  hash: string;
  queryPairs: QueryPair[];
  origin: string;
}

const raw = ref('');
const error = ref('');
const parsed = ref<Parsed | null>(null);

let debounce: ReturnType<typeof setTimeout> | null = null;

watch(raw, () => {
  if (debounce) clearTimeout(debounce);
  debounce = setTimeout(parse, 150);
});

function parse() {
  error.value = '';
  const s = raw.value.trim();
  if (!s) {
    parsed.value = null;
    return;
  }
  try {
    const u = new URL(s);
    const pairs: QueryPair[] = [];
    u.searchParams.forEach((v, k) => pairs.push({ key: k, value: v }));
    parsed.value = {
      protocol: u.protocol.replace(/:$/, ''),
      username: u.username,
      password: u.password,
      hostname: u.hostname,
      port: u.port,
      pathname: u.pathname,
      search: u.search,
      hash: u.hash,
      queryPairs: pairs,
      origin: u.origin
    };
  } catch (err) {
    parsed.value = null;
    error.value = `无法解析：${err}`;
  }
}

const rows = computed(() => {
  if (!parsed.value) return [];
  return [
    ['协议', parsed.value.protocol],
    ['用户名', parsed.value.username],
    ['密码', parsed.value.password ? '••••••' : ''],
    ['主机', parsed.value.hostname],
    ['端口', parsed.value.port],
    ['路径', parsed.value.pathname],
    ['查询串', parsed.value.search],
    ['锚点', parsed.value.hash],
    ['Origin', parsed.value.origin]
  ].filter(([, v]) => v !== '');
});
</script>

<template>
  <div class="pane">
    <Panel title="URL">
      <n-input
        v-model:value="raw"
        type="textarea"
        placeholder="粘贴 URL，例如 https://user:pass@example.com:8443/path/to?x=1&y=hello#top"
        :autosize="{ minRows: 3, maxRows: 6 }"
      />
    </Panel>

    <n-alert v-if="error" type="error" :bordered="false">{{ error }}</n-alert>

    <template v-if="parsed">
      <Panel title="组件">
        <table class="kv">
          <tr v-for="[k, v] in rows" :key="k">
            <td class="k">{{ k }}</td>
            <td class="v mono">{{ v }}</td>
          </tr>
        </table>
      </Panel>

      <Panel v-if="parsed.queryPairs.length > 0" title="查询参数">
        <table class="kv">
          <tr v-for="pair in parsed.queryPairs" :key="pair.key + '/' + pair.value">
            <td class="k">{{ pair.key }}</td>
            <td class="v mono">{{ pair.value }}</td>
          </tr>
        </table>
      </Panel>
    </template>
  </div>
</template>

<style scoped>
.pane { display: grid; gap: 12px; padding: 16px; height: 100%; overflow: auto; }
.kv { width: 100%; border-collapse: collapse; }
.kv td {
  padding: 6px 10px;
  border-bottom: 1px solid var(--line-weak, var(--line));
  vertical-align: top;
  font-size: var(--fs-sm);
}
.kv .k { color: var(--text-muted); width: 30%; }
.kv .v { word-break: break-all; }
.mono { font-family: var(--font-mono, ui-monospace, monospace); font-size: var(--fs-xs); }
</style>
