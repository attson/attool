<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { NButton, NCheckbox, NInputNumber, NSlider } from 'naive-ui';
import { generatePassword, scorePassword, type PasswordOptions } from '../../utils/generators';
import Panel from '../ui/Panel.vue';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';

const opts = ref<PasswordOptions>({
  length: 16,
  lowercase: true,
  uppercase: true,
  digits: true,
  symbols: true,
  excludeAmbiguous: true
});
const batch = ref(5);
const results = ref<string[]>([]);

function generate() {
  const out: string[] = [];
  const n = Math.max(1, Math.min(50, batch.value));
  for (let i = 0; i < n; i++) out.push(generatePassword(opts.value));
  results.value = out;
}

const strength = computed(() => (results.value[0] ? scorePassword(results.value[0]) : null));

async function copyLine(v: string) {
  try { await writeText(v); } catch {}
}
async function copyAll() {
  try { await writeText(results.value.join('\n')); } catch {}
}

onMounted(generate);
</script>

<template>
  <div class="pane">
    <Panel title="参数">
      <div class="form">
        <label class="field">
          <span class="lbl">长度 {{ opts.length }}</span>
          <div class="len-row">
            <n-slider v-model:value="opts.length" :min="4" :max="64" style="flex: 1" />
            <n-input-number v-model:value="opts.length" :min="4" :max="128" size="small" style="width: 90px" />
          </div>
        </label>
        <div class="charset-row">
          <n-checkbox v-model:checked="opts.lowercase">小写 a-z</n-checkbox>
          <n-checkbox v-model:checked="opts.uppercase">大写 A-Z</n-checkbox>
          <n-checkbox v-model:checked="opts.digits">数字 0-9</n-checkbox>
          <n-checkbox v-model:checked="opts.symbols">符号 !@#$…</n-checkbox>
          <n-checkbox v-model:checked="opts.excludeAmbiguous">排除易混（Il1O0oB8Z2S5）</n-checkbox>
        </div>
        <div class="batch-row">
          <span class="lbl">批量生成</span>
          <n-input-number v-model:value="batch" :min="1" :max="50" size="small" style="width: 100px" />
          <n-button type="primary" @click="generate">生成</n-button>
          <n-button v-if="results.length > 1" secondary @click="copyAll">全部复制</n-button>
        </div>
      </div>
    </Panel>

    <Panel :title="results.length > 1 ? `结果（${results.length}）` : '结果'">
      <template #right>
        <span v-if="strength" class="strength">
          强度：
          <span :class="'strength-' + strength.score">{{ strength.label }}</span>
        </span>
      </template>
      <ul class="results">
        <li v-for="pw in results" :key="pw" class="row">
          <span class="pw mono">{{ pw }}</span>
          <n-button size="tiny" secondary @click="copyLine(pw)">复制</n-button>
        </li>
      </ul>
    </Panel>
  </div>
</template>

<style scoped>
.pane { display: grid; gap: 12px; padding: 16px; height: 100%; overflow: auto; }
.form { display: grid; gap: 12px; }
.field { display: grid; gap: 6px; }
.lbl { font-size: var(--fs-xxs); color: var(--text-muted); }
.len-row { display: flex; align-items: center; gap: 12px; }
.charset-row { display: flex; flex-wrap: wrap; gap: 12px 20px; }
.batch-row { display: flex; align-items: center; gap: 10px; }
.results { list-style: none; margin: 0; padding: 0; display: grid; gap: 4px; }
.row {
  display: flex; align-items: center; justify-content: space-between;
  gap: 10px; padding: 6px 10px;
  background: var(--bg-elev); border-radius: var(--radius-sm);
}
.pw { flex: 1; min-width: 0; word-break: break-all; font-size: var(--fs-sm); }
.mono { font-family: var(--font-mono, ui-monospace, monospace); }
.strength { font-size: var(--fs-xs); color: var(--text-muted); }
.strength-0, .strength-1 { color: #ef4444; }
.strength-2 { color: #f59e0b; }
.strength-3 { color: #10b981; }
.strength-4 { color: #059669; font-weight: 600; }
</style>
