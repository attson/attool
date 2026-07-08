<script setup lang="ts">
import { computed, ref } from 'vue';
import { NButton, NInput } from 'naive-ui';
import { joinWith, splitBy } from '../../utils/textTools';
import Panel from '../ui/Panel.vue';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';

const input = ref('');
const splitDelim = ref(',');
const joinDelim = ref(', ');

const splitResult = computed(() => splitBy(input.value, splitDelim.value));
const joinResult = computed(() => joinWith(input.value, joinDelim.value));

async function copy(value: string) {
  try {
    await writeText(value);
  } catch {
    // noop
  }
}
</script>

<template>
  <div class="pane">
    <Panel title="输入">
      <n-input v-model:value="input" type="textarea" placeholder="要拆或要拼的文本" :autosize="{ minRows: 6, maxRows: 12 }" />
    </Panel>

    <div class="grid">
      <Panel title="按分隔符 → 拆成多行">
        <template #right>
          <n-button size="tiny" secondary :disabled="!splitResult" @click="copy(splitResult)">复制</n-button>
        </template>
        <div class="form">
          <label class="field">
            <span class="lbl">分隔符（支持多字符）</span>
            <n-input v-model:value="splitDelim" placeholder="," />
          </label>
          <n-input :value="splitResult" type="textarea" readonly :autosize="{ minRows: 8, maxRows: 16 }" />
        </div>
      </Panel>

      <Panel title="多行 → 按分隔符拼接">
        <template #right>
          <n-button size="tiny" secondary :disabled="!joinResult" @click="copy(joinResult)">复制</n-button>
        </template>
        <div class="form">
          <label class="field">
            <span class="lbl">拼接符</span>
            <n-input v-model:value="joinDelim" placeholder=", " />
          </label>
          <n-input :value="joinResult" type="textarea" readonly :autosize="{ minRows: 8, maxRows: 16 }" />
        </div>
      </Panel>
    </div>
  </div>
</template>

<style scoped>
.pane { display: grid; gap: 12px; padding: 16px; height: 100%; overflow: auto; }
.grid { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
@media (max-width: 900px) { .grid { grid-template-columns: 1fr; } }
.form { display: grid; gap: 10px; }
.field { display: grid; gap: 6px; }
.lbl { font-size: var(--fs-xxs); color: var(--text-muted); }
</style>
