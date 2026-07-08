<script setup lang="ts">
import { computed, ref } from 'vue';
import { NAlert, NInput } from 'naive-ui';
import { formatDurationHuman, parseDuration } from '../../utils/timeTools';
import Panel from '../ui/Panel.vue';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';

const raw = ref('1h30m');
const parsedMs = computed(() => parseDuration(raw.value));

const parsedError = computed(() => (raw.value.trim() && parsedMs.value === null ? '无法解析：支持 ms/s/m/h/d/w 或 毫秒/秒/分钟/小时/天/周' : ''));

const values = computed(() => {
  if (parsedMs.value === null) return null;
  const ms = parsedMs.value;
  return {
    ms,
    s: ms / 1000,
    m: ms / 60_000,
    h: ms / 3_600_000,
    d: ms / 86_400_000,
    human: formatDurationHuman(ms)
  };
});

async function copy(v: string | number) {
  try { await writeText(String(v)); } catch {}
}
</script>

<template>
  <div class="pane">
    <Panel title="输入">
      <n-input v-model:value="raw" placeholder="例如 1h30m / 2d3h / 90m / 90000（毫秒） / 1天2小时" />
    </Panel>

    <n-alert v-if="parsedError" type="warning" :bordered="false">{{ parsedError }}</n-alert>

    <Panel v-if="values" title="换算">
      <table class="conv">
        <tr>
          <td class="k">毫秒</td>
          <td class="v mono" @click="copy(values.ms)">{{ values.ms }}</td>
        </tr>
        <tr>
          <td class="k">秒</td>
          <td class="v mono" @click="copy(values.s)">{{ values.s }}</td>
        </tr>
        <tr>
          <td class="k">分</td>
          <td class="v mono" @click="copy(values.m)">{{ values.m }}</td>
        </tr>
        <tr>
          <td class="k">小时</td>
          <td class="v mono" @click="copy(values.h)">{{ values.h }}</td>
        </tr>
        <tr>
          <td class="k">天</td>
          <td class="v mono" @click="copy(values.d)">{{ values.d }}</td>
        </tr>
        <tr>
          <td class="k">人类可读</td>
          <td class="v" @click="copy(values.human)">{{ values.human }}</td>
        </tr>
      </table>
      <p class="hint muted">点数值复制。</p>
    </Panel>
  </div>
</template>

<style scoped>
.pane { display: grid; gap: 12px; padding: 16px; height: 100%; overflow: auto; }
.conv { width: 100%; border-collapse: collapse; }
.conv td { padding: 6px 10px; border-bottom: 1px solid var(--line-weak, var(--line)); font-size: var(--fs-sm); }
.conv .k { color: var(--text-muted); width: 30%; }
.conv .v { cursor: pointer; }
.conv .v:hover { color: var(--accent, #10b981); }
.mono { font-family: var(--font-mono, ui-monospace, monospace); font-variant-numeric: tabular-nums; }
.hint { margin: 8px 0 0; font-size: var(--fs-xxs); }
.muted { color: var(--text-muted); }
</style>
