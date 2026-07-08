<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { NButton, NInputNumber } from 'naive-ui';
import {
  fakeChineseIdCard,
  fakeChineseName,
  fakeChinesePhone,
  fakeCreditCard,
  fakeEmail,
  fakeEnglishName
} from '../../utils/generators';
import Panel from '../ui/Panel.vue';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';

interface Row {
  cnName: string;
  enName: string;
  email: string;
  phone: string;
  idCard: string;
  visa: string;
}

const count = ref(5);
const rows = ref<Row[]>([]);

function make() {
  const n = Math.max(1, Math.min(50, count.value));
  const out: Row[] = [];
  for (let i = 0; i < n; i++) {
    out.push({
      cnName: fakeChineseName(),
      enName: fakeEnglishName(),
      email: fakeEmail(),
      phone: fakeChinesePhone(),
      idCard: fakeChineseIdCard(),
      visa: fakeCreditCard('visa')
    });
  }
  rows.value = out;
}

async function copy(v: string) { try { await writeText(v); } catch {} }
async function copyRow(r: Row) {
  const line = `${r.cnName} | ${r.enName} | ${r.email} | ${r.phone} | ${r.idCard} | ${r.visa}`;
  try { await writeText(line); } catch {}
}

onMounted(make);
</script>

<template>
  <div class="pane">
    <Panel title="参数">
      <div class="row">
        <label class="field">
          <span class="lbl">条数</span>
          <n-input-number v-model:value="count" :min="1" :max="50" size="small" style="width: 100px" />
        </label>
        <n-button type="primary" @click="make">重新生成</n-button>
      </div>
      <p class="note">全部随机拼装，用于填测试表单。不是真实身份，勿用于任何认证/欺诈场景。</p>
    </Panel>

    <Panel v-if="rows.length > 0" :title="`结果（${rows.length}）`">
      <div class="table">
        <div class="th">
          <span>中文名</span><span>英文名</span><span>邮箱</span><span>手机</span><span>身份证</span><span>Visa</span><span></span>
        </div>
        <div v-for="(r, idx) in rows" :key="idx" class="tr">
          <span class="mono cell" @click="copy(r.cnName)">{{ r.cnName }}</span>
          <span class="mono cell" @click="copy(r.enName)">{{ r.enName }}</span>
          <span class="mono cell" @click="copy(r.email)">{{ r.email }}</span>
          <span class="mono cell" @click="copy(r.phone)">{{ r.phone }}</span>
          <span class="mono cell" @click="copy(r.idCard)">{{ r.idCard }}</span>
          <span class="mono cell" @click="copy(r.visa)">{{ r.visa }}</span>
          <n-button size="tiny" secondary @click="copyRow(r)">整行</n-button>
        </div>
      </div>
      <p class="hint muted">点单元格复制该字段。</p>
    </Panel>
  </div>
</template>

<style scoped>
.pane { display: grid; gap: 12px; padding: 16px; height: 100%; overflow: auto; }
.row { display: flex; align-items: end; gap: 12px; }
.field { display: grid; gap: 6px; }
.lbl { font-size: var(--fs-xxs); color: var(--text-muted); }
.note { margin: 8px 0 0; color: var(--text-muted); font-size: var(--fs-xxs); }
.table { display: grid; gap: 3px; }
.th, .tr {
  display: grid;
  grid-template-columns: 1fr 1.2fr 1.6fr 1.1fr 1.5fr 1.3fr 60px;
  gap: 6px;
  align-items: center;
}
.th {
  padding: 4px 6px;
  color: var(--text-muted);
  font-size: var(--fs-xxs);
  border-bottom: 1px solid var(--line);
}
.tr { padding: 4px 6px; background: var(--bg-elev); border-radius: var(--radius-sm); font-size: var(--fs-xs); }
.mono { font-family: var(--font-mono, monospace); }
.cell {
  cursor: pointer;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.cell:hover { color: var(--accent, #10b981); }
.hint { font-size: var(--fs-xxs); }
.muted { color: var(--text-muted); margin: 8px 0 0; }
</style>
