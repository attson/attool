<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { NButton, NInput } from 'naive-ui';
import { formatInZones, parseUnixInput } from '../../utils/timeTools';
import Panel from '../ui/Panel.vue';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';

const raw = ref('');
const now = ref(Date.now());
let timer: ReturnType<typeof setInterval> | null = null;

onMounted(() => {
  timer = setInterval(() => (now.value = Date.now()), 1000);
});
onUnmounted(() => {
  if (timer) clearInterval(timer);
});

const parsedMs = computed(() => {
  const p = parseUnixInput(raw.value);
  return p ?? null;
});

const useMs = computed(() => parsedMs.value ?? now.value);
const displays = computed(() => formatInZones(useMs.value));

const sec = computed(() => Math.floor(useMs.value / 1000));
const ms = computed(() => useMs.value);

async function copy(v: string) { try { await writeText(v); } catch {} }
function useNow() { raw.value = String(now.value); }
</script>

<template>
  <div class="pane">
    <Panel title="时间戳">
      <div class="form">
        <n-input v-model:value="raw" placeholder="秒 / 毫秒 / 微秒 / 纳秒（留空则用当前时间）" clearable />
        <div class="row">
          <n-button type="primary" @click="useNow">使用当前时间</n-button>
        </div>

        <div class="stat-row">
          <div class="stat">
            <div class="stat-k">Unix 秒</div>
            <div class="stat-v mono" @click="copy(String(sec))">{{ sec }}</div>
          </div>
          <div class="stat">
            <div class="stat-k">Unix 毫秒</div>
            <div class="stat-v mono" @click="copy(String(ms))">{{ ms }}</div>
          </div>
        </div>
      </div>
    </Panel>

    <Panel title="多时区显示">
      <table class="tz">
        <tbody>
          <tr v-for="d in displays" :key="d.zone">
            <td class="tz-zone mono">{{ d.zone }}</td>
            <td class="tz-time mono" @click="copy(d.formatted)">{{ d.formatted }}</td>
            <td class="tz-offset muted">{{ d.offset }}</td>
          </tr>
        </tbody>
      </table>
      <p class="hint muted">点单元格复制。</p>
    </Panel>
  </div>
</template>

<style scoped>
.pane { display: grid; gap: 12px; padding: 16px; height: 100%; overflow: auto; }
.form { display: grid; gap: 12px; }
.row { display: flex; gap: 8px; }
.stat-row { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
.stat { display: grid; gap: 4px; padding: 10px 12px; background: var(--bg-elev); border-radius: var(--radius-sm); }
.stat-k { font-size: var(--fs-xxs); color: var(--text-muted); }
.stat-v { font-size: var(--fs-md); cursor: pointer; }
.stat-v:hover { color: var(--accent, #10b981); }
.tz { width: 100%; border-collapse: collapse; font-size: var(--fs-sm); }
.tz td { padding: 6px 10px; border-bottom: 1px solid var(--line-weak, var(--line)); }
.tz-zone { color: var(--text-muted); width: 30%; }
.tz-time { cursor: pointer; }
.tz-time:hover { color: var(--accent, #10b981); }
.tz-offset { width: 20%; }
.hint { font-size: var(--fs-xxs); }
.muted { color: var(--text-muted); }
.mono { font-family: var(--font-mono, ui-monospace, monospace); font-variant-numeric: tabular-nums; }
</style>
