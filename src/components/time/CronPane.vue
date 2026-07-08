<script setup lang="ts">
import { computed, ref } from 'vue';
import { NAlert, NInput } from 'naive-ui';
import { cronNextRuns, describeCron, parseCron, type CronParts } from '../../utils/timeTools';
import Panel from '../ui/Panel.vue';

const expr = ref('*/5 * * * *');
const error = ref('');
const parts = ref<CronParts | null>(null);

const description = computed(() => (parts.value ? describeCron(parts.value) : ''));
const nextRuns = computed(() => {
  if (!parts.value) return [];
  return cronNextRuns(parts.value, Date.now(), 8);
});

function tryParse() {
  error.value = '';
  try {
    parts.value = parseCron(expr.value);
  } catch (err) {
    parts.value = null;
    error.value = String(err);
  }
}

let debounce: ReturnType<typeof setTimeout> | null = null;
function onChange() {
  if (debounce) clearTimeout(debounce);
  debounce = setTimeout(tryParse, 200);
}
tryParse();

function formatMs(ms: number): string {
  const d = new Date(ms);
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())} · ${['周日', '周一', '周二', '周三', '周四', '周五', '周六'][d.getDay()]}`;
}
</script>

<template>
  <div class="pane">
    <Panel title="Cron 表达式（5 段：分 时 日 月 周）">
      <n-input v-model:value="expr" placeholder="*/5 * * * *" @update:value="onChange" />
      <p class="hint muted">支持 `*`, `,`, `-`, `/`。周里 0 与 7 都是周日。</p>
    </Panel>

    <n-alert v-if="error" type="error" :bordered="false">{{ error }}</n-alert>

    <Panel v-if="parts" title="解读">
      <p class="describe">{{ description }}</p>
    </Panel>

    <Panel v-if="parts" title="未来 8 次触发（本地时区）">
      <ol class="runs">
        <li v-for="ms in nextRuns" :key="ms" class="mono">
          {{ formatMs(ms) }}
        </li>
      </ol>
    </Panel>
  </div>
</template>

<style scoped>
.pane { display: grid; gap: 12px; padding: 16px; height: 100%; overflow: auto; }
.hint { margin: 8px 0 0; font-size: var(--fs-xxs); }
.muted { color: var(--text-muted); }
.describe { margin: 0; font-size: var(--fs-sm); color: var(--text); line-height: 1.6; }
.runs { list-style: none; margin: 0; padding: 0; display: grid; gap: 4px; counter-reset: run; }
.runs li {
  padding: 6px 10px; background: var(--bg-elev); border-radius: var(--radius-sm);
  font-size: var(--fs-sm); display: flex; gap: 10px;
  counter-increment: run;
}
.runs li::before {
  content: counter(run);
  color: var(--text-muted); min-width: 20px; font-family: var(--font-mono, monospace);
}
.mono { font-family: var(--font-mono, ui-monospace, monospace); font-variant-numeric: tabular-nums; }
</style>
