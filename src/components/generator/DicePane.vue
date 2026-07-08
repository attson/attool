<script setup lang="ts">
import { computed, ref } from 'vue';
import { NButton, NInput, NInputNumber } from 'naive-ui';
import { pickN, randomInRange, rollDice } from '../../utils/generators';
import Panel from '../ui/Panel.vue';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';

const diceCount = ref(2);
const diceSides = ref(6);
const diceResult = ref<number[]>([]);
const diceSum = computed(() => diceResult.value.reduce((s, v) => s + v, 0));

function roll() {
  diceResult.value = rollDice(
    Math.max(1, Math.min(20, diceCount.value)),
    Math.max(2, Math.min(1000, diceSides.value))
  );
}

const rangeMin = ref(1);
const rangeMax = ref(100);
const rangeCount = ref(5);
const rangeResult = ref<number[]>([]);
function pickRange() {
  rangeResult.value = randomInRange(rangeMin.value, rangeMax.value, Math.max(1, Math.min(50, rangeCount.value)));
}

const listRaw = ref('北京\n上海\n广州\n深圳\n杭州');
const listPickN = ref(2);
const listResult = ref<string[]>([]);
function pickList() {
  const items = listRaw.value
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter(Boolean);
  if (items.length === 0) {
    listResult.value = [];
    return;
  }
  listResult.value = pickN(items, Math.max(1, Math.min(items.length, listPickN.value)));
}

async function copy(v: string) { try { await writeText(v); } catch {} }
</script>

<template>
  <div class="pane">
    <Panel title="骰子">
      <div class="row">
        <label class="field">
          <span class="lbl">数量</span>
          <n-input-number v-model:value="diceCount" :min="1" :max="20" size="small" style="width: 100px" />
        </label>
        <label class="field">
          <span class="lbl">面数</span>
          <n-input-number v-model:value="diceSides" :min="2" :max="1000" size="small" style="width: 100px" />
        </label>
        <n-button type="primary" @click="roll">掷</n-button>
      </div>
      <div v-if="diceResult.length > 0" class="dice-result">
        <span v-for="(v, i) in diceResult" :key="i" class="die mono">{{ v }}</span>
        <span class="sum">= {{ diceSum }}</span>
      </div>
    </Panel>

    <Panel title="区间随机">
      <div class="row">
        <label class="field">
          <span class="lbl">最小</span>
          <n-input-number v-model:value="rangeMin" size="small" style="width: 120px" />
        </label>
        <label class="field">
          <span class="lbl">最大</span>
          <n-input-number v-model:value="rangeMax" size="small" style="width: 120px" />
        </label>
        <label class="field">
          <span class="lbl">个数</span>
          <n-input-number v-model:value="rangeCount" :min="1" :max="50" size="small" style="width: 100px" />
        </label>
        <n-button type="primary" @click="pickRange">生成</n-button>
      </div>
      <div v-if="rangeResult.length > 0" class="range-result mono">
        {{ rangeResult.join(', ') }}
        <n-button size="tiny" secondary @click="copy(rangeResult.join(','))">复制</n-button>
      </div>
    </Panel>

    <Panel title="从列表抽奖">
      <div class="row list-row">
        <n-input
          v-model:value="listRaw"
          type="textarea"
          placeholder="每行一个候选项"
          :autosize="{ minRows: 5, maxRows: 10 }"
          style="flex: 1"
        />
        <div class="field">
          <span class="lbl">抽几个</span>
          <n-input-number v-model:value="listPickN" :min="1" :max="50" size="small" style="width: 100px" />
          <n-button type="primary" @click="pickList" style="margin-top: 8px">抽</n-button>
        </div>
      </div>
      <ul v-if="listResult.length > 0" class="winners">
        <li v-for="(w, i) in listResult" :key="i">
          <span class="idx">#{{ i + 1 }}</span>
          <span class="mono">{{ w }}</span>
        </li>
      </ul>
    </Panel>
  </div>
</template>

<style scoped>
.pane { display: grid; gap: 12px; padding: 16px; height: 100%; overflow: auto; }
.row { display: flex; align-items: end; gap: 12px; flex-wrap: wrap; }
.list-row { align-items: flex-start; }
.field { display: grid; gap: 6px; }
.lbl { font-size: var(--fs-xxs); color: var(--text-muted); }
.mono { font-family: var(--font-mono, ui-monospace, monospace); }

.dice-result { display: flex; align-items: center; gap: 8px; margin-top: 10px; }
.die {
  display: inline-grid;
  place-items: center;
  width: 44px; height: 44px;
  border: 1px solid var(--line-strong);
  border-radius: var(--radius-sm);
  font-size: var(--fs-lg);
  background: var(--bg-elev);
}
.sum { font-size: var(--fs-sm); color: var(--text-muted); margin-left: 8px; }

.range-result {
  display: flex; align-items: center; gap: 10px;
  margin-top: 10px; padding: 8px 10px;
  background: var(--bg-elev); border-radius: var(--radius-sm);
  font-size: var(--fs-sm);
}

.winners { list-style: none; margin: 12px 0 0; padding: 0; display: grid; gap: 4px; }
.winners li {
  display: flex; gap: 10px; padding: 6px 10px;
  background: var(--bg-elev); border-radius: var(--radius-sm); font-size: var(--fs-sm);
}
.idx { color: var(--text-muted); font-size: var(--fs-xs); min-width: 32px; }
</style>
