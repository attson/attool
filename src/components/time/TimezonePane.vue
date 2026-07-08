<script setup lang="ts">
import { computed, ref } from 'vue';
import { NButton, NInput, NSelect } from 'naive-ui';
import { formatInZones } from '../../utils/timeTools';
import Panel from '../ui/Panel.vue';

const dateStr = ref(formatLocalNow());
const sourceZone = ref(Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC');
const zoneList = ref<string[]>([
  'Asia/Shanghai',
  'Asia/Tokyo',
  'Asia/Singapore',
  'Asia/Kolkata',
  'Europe/London',
  'Europe/Berlin',
  'Europe/Paris',
  'America/New_York',
  'America/Los_Angeles',
  'America/Chicago',
  'Australia/Sydney',
  'UTC'
]);

function formatLocalNow(): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

// A "wall-clock" datetime in `sourceZone` — convert to UTC by first parsing as
// a naive local time, then offsetting by the difference between source zone and local.
function wallInZoneToMs(wall: string, zone: string): number | null {
  const m = wall.match(/^(\d{4})-(\d{2})-(\d{2})[T ](\d{2}):(\d{2})(?::(\d{2}))?$/);
  if (!m) return null;
  const [, y, mo, d, h, mi, s] = m;
  // Assume the wall time is in `zone`. Get the actual UTC ms for that wall time
  // by binary search: since offsets don't change more than ~1 day, we can do it
  // by finding the ms such that formatting it in zone gives the same wall time.
  // Simpler: use Date.UTC and adjust by measured zone offset.
  const asUtc = Date.UTC(+y, +mo - 1, +d, +h, +mi, s ? +s : 0);
  // Now compute what UTC ms would produce this wall time when displayed in zone
  const offsetMs = zoneOffsetMs(asUtc, zone);
  return asUtc - offsetMs;
}

/** Milliseconds offset of `zone` at `atMs` (e.g. +8h → 8*3_600_000). */
function zoneOffsetMs(atMs: number, zone: string): number {
  try {
    const fmt = new Intl.DateTimeFormat('en-US', {
      timeZone: zone,
      hourCycle: 'h23',
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit'
    });
    const parts = fmt.formatToParts(new Date(atMs));
    const p = (t: string) => Number(parts.find((x) => x.type === t)?.value);
    const asLocalUtc = Date.UTC(p('year'), p('month') - 1, p('day'), p('hour'), p('minute'), p('second'));
    return asLocalUtc - atMs;
  } catch {
    return 0;
  }
}

const parsedMs = computed(() => wallInZoneToMs(dateStr.value, sourceZone.value));
const zonedDisplays = computed(() => (parsedMs.value === null ? [] : formatInZones(parsedMs.value, zoneList.value)));

const zoneSelectOptions = Intl.supportedValuesOf?.('timeZone').map((z) => ({ label: z, value: z })) ?? [
  { label: sourceZone.value, value: sourceZone.value }
];

function useNow() {
  dateStr.value = formatLocalNow();
  sourceZone.value = Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC';
}

const newZone = ref('');
function addZone() {
  const z = newZone.value.trim();
  if (z && !zoneList.value.includes(z)) zoneList.value.push(z);
  newZone.value = '';
}
function removeZone(z: string) {
  zoneList.value = zoneList.value.filter((v) => v !== z);
}
</script>

<template>
  <div class="pane">
    <Panel title="源时间">
      <div class="form">
        <div class="row">
          <label class="field">
            <span class="lbl">时间（YYYY-MM-DD HH:mm:ss）</span>
            <n-input v-model:value="dateStr" placeholder="2026-07-08 14:30:00" />
          </label>
          <label class="field grow">
            <span class="lbl">源时区</span>
            <n-select v-model:value="sourceZone" :options="zoneSelectOptions" filterable />
          </label>
          <n-button type="primary" @click="useNow" style="align-self: end">当前时间</n-button>
        </div>
      </div>
    </Panel>

    <Panel title="目标时区">
      <div class="zone-list">
        <span v-for="z in zoneList" :key="z" class="chip">
          {{ z }}
          <button class="chip-x" @click="removeZone(z)">×</button>
        </span>
      </div>
      <div class="row" style="margin-top: 8px">
        <n-select v-model:value="newZone" :options="zoneSelectOptions" filterable placeholder="添加时区" style="flex: 1" />
        <n-button secondary :disabled="!newZone" @click="addZone">添加</n-button>
      </div>
    </Panel>

    <Panel v-if="zonedDisplays.length > 0" title="转换结果">
      <table class="tz">
        <tbody>
          <tr v-for="d in zonedDisplays" :key="d.zone">
            <td class="tz-zone mono">{{ d.zone }}</td>
            <td class="tz-time mono">{{ d.formatted }}</td>
            <td class="tz-offset muted">{{ d.offset }}</td>
          </tr>
        </tbody>
      </table>
    </Panel>
  </div>
</template>

<style scoped>
.pane { display: grid; gap: 12px; padding: 16px; height: 100%; overflow: auto; }
.form { display: grid; gap: 12px; }
.row { display: flex; gap: 12px; align-items: end; }
.field { display: grid; gap: 6px; }
.field.grow { flex: 1; min-width: 200px; }
.lbl { font-size: var(--fs-xxs); color: var(--text-muted); }
.zone-list { display: flex; flex-wrap: wrap; gap: 6px; }
.chip {
  display: inline-flex; align-items: center; gap: 4px;
  padding: 3px 8px; background: var(--bg-elev); border-radius: var(--radius-sm);
  font-size: var(--fs-xs); color: var(--text);
}
.chip-x {
  background: none; border: none; color: var(--text-muted);
  cursor: pointer; font-size: 14px; padding: 0 2px;
}
.chip-x:hover { color: #ef4444; }
.tz { width: 100%; border-collapse: collapse; font-size: var(--fs-sm); }
.tz td { padding: 6px 10px; border-bottom: 1px solid var(--line-weak, var(--line)); }
.tz-zone { color: var(--text-muted); width: 30%; }
.tz-offset { width: 20%; }
.mono { font-family: var(--font-mono, ui-monospace, monospace); font-variant-numeric: tabular-nums; }
.muted { color: var(--text-muted); }
</style>
