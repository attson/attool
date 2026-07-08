<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { NAlert, NInput } from 'naive-ui';
import { decodeJwt, type JwtParts } from '../../utils/codec';
import Panel from '../ui/Panel.vue';
import PaneActions from './PaneActions.vue';

const token = ref('');
const parts = ref<JwtParts | null>(null);
const error = ref('');

let debounce: ReturnType<typeof setTimeout> | null = null;

watch(token, () => {
  if (debounce) clearTimeout(debounce);
  debounce = setTimeout(run, 150);
});

function run() {
  error.value = '';
  const s = token.value.trim();
  if (!s) {
    parts.value = null;
    return;
  }
  try {
    parts.value = decodeJwt(s);
  } catch (err) {
    parts.value = null;
    error.value = String(err);
  }
}

const expiryNote = computed(() => {
  if (!parts.value?.payload) return '';
  try {
    const payload = JSON.parse(parts.value.payload);
    if (typeof payload.exp === 'number') {
      const d = new Date(payload.exp * 1000);
      const expired = payload.exp * 1000 < Date.now();
      return `exp: ${d.toLocaleString()}${expired ? ' · 已过期' : ''}`;
    }
  } catch {}
  return '';
});
</script>

<template>
  <div class="pane">
    <Panel title="JWT">
      <n-input
        v-model:value="token"
        type="textarea"
        placeholder="粘贴 JWT（xxxxx.yyyyy.zzzzz）"
        :autosize="{ minRows: 4, maxRows: 8 }"
      />
      <p class="note">仅解码显示 payload，不验证签名（需要 secret / public key）。</p>
    </Panel>

    <n-alert v-if="error" type="error" :bordered="false">{{ error }}</n-alert>

    <template v-if="parts">
      <Panel title="Header">
        <template #right>
          <PaneActions :value="parts.header" />
        </template>
        <pre class="json">{{ parts.header }}</pre>
      </Panel>

      <Panel title="Payload">
        <template #right>
          <div class="header-right">
            <span v-if="expiryNote" class="muted">{{ expiryNote }}</span>
            <PaneActions :value="parts.payload" />
          </div>
        </template>
        <pre class="json">{{ parts.payload }}</pre>
      </Panel>

      <Panel title="Signature">
        <template #right>
          <PaneActions :value="parts.signatureRaw" />
        </template>
        <div class="sig mono">{{ parts.signatureRaw }}</div>
      </Panel>
    </template>
  </div>
</template>

<style scoped>
.pane { display: grid; gap: 12px; padding: 16px; height: 100%; overflow: auto; }
.note { margin: 8px 0 0; color: var(--text-muted); font-size: var(--fs-xxs); }
.json {
  margin: 0;
  padding: 8px 12px;
  background: var(--bg-elev);
  border-radius: var(--radius-sm);
  font-family: var(--font-mono, monospace);
  font-size: var(--fs-xs);
  color: var(--text);
  white-space: pre-wrap;
  word-break: break-word;
}
.sig {
  font-family: var(--font-mono, monospace);
  font-size: var(--fs-xs);
  word-break: break-all;
  color: var(--text-muted);
  padding: 6px 0;
}
.mono { font-variant-numeric: tabular-nums; }
.header-right { display: flex; align-items: center; gap: 10px; }
.muted { color: var(--text-muted); font-size: var(--fs-xxs); }
</style>
