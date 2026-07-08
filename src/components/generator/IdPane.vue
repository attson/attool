<script setup lang="ts">
import { ref } from 'vue';
import { NButton, NInputNumber } from 'naive-ui';
import { nanoId, ulid, uuidV4 } from '../../utils/generators';
import Panel from '../ui/Panel.vue';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';

type IdKind = 'uuid' | 'nanoid' | 'ulid';

interface Batch {
  kind: IdKind;
  values: string[];
}

const count = ref(5);
const nanoSize = ref(21);
const results = ref<Batch | null>(null);

function make(kind: IdKind) {
  const n = Math.max(1, Math.min(100, count.value));
  const values: string[] = [];
  for (let i = 0; i < n; i++) {
    if (kind === 'uuid') values.push(uuidV4());
    else if (kind === 'nanoid') values.push(nanoId(Math.max(4, Math.min(50, nanoSize.value))));
    else values.push(ulid());
  }
  results.value = { kind, values };
}

async function copyLine(v: string) { try { await writeText(v); } catch {} }
async function copyAll() {
  if (!results.value) return;
  try { await writeText(results.value.values.join('\n')); } catch {}
}
</script>

<template>
  <div class="pane">
    <Panel title="参数">
      <div class="form">
        <div class="row">
          <label class="field">
            <span class="lbl">数量</span>
            <n-input-number v-model:value="count" :min="1" :max="100" size="small" style="width: 100px" />
          </label>
          <label class="field">
            <span class="lbl">NanoID 长度</span>
            <n-input-number v-model:value="nanoSize" :min="4" :max="50" size="small" style="width: 100px" />
          </label>
        </div>
        <div class="btn-row">
          <n-button type="primary" @click="make('uuid')">UUID v4</n-button>
          <n-button secondary @click="make('nanoid')">NanoID</n-button>
          <n-button secondary @click="make('ulid')">ULID</n-button>
        </div>
      </div>
    </Panel>

    <Panel v-if="results" :title="'结果（' + results.values.length + '）'">
      <template #right>
        <n-button size="tiny" secondary @click="copyAll">全部复制</n-button>
      </template>
      <ul class="results">
        <li v-for="v in results.values" :key="v" class="line">
          <span class="mono">{{ v }}</span>
          <n-button size="tiny" secondary @click="copyLine(v)">复制</n-button>
        </li>
      </ul>
    </Panel>
  </div>
</template>

<style scoped>
.pane { display: grid; gap: 12px; padding: 16px; height: 100%; overflow: auto; }
.form { display: grid; gap: 12px; }
.row { display: flex; gap: 20px; }
.field { display: grid; gap: 6px; }
.lbl { font-size: var(--fs-xxs); color: var(--text-muted); }
.btn-row { display: flex; gap: 8px; }
.results { list-style: none; margin: 0; padding: 0; display: grid; gap: 4px; }
.line { display: flex; justify-content: space-between; gap: 10px; padding: 6px 10px; background: var(--bg-elev); border-radius: var(--radius-sm); font-size: var(--fs-sm); }
.mono { font-family: var(--font-mono, ui-monospace, monospace); word-break: break-all; }
</style>
