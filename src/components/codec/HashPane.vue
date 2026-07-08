<script setup lang="ts">
import { ref, watch } from 'vue';
import { NInput } from 'naive-ui';
import { md5, sha1, sha256, sha512 } from '../../utils/codec';
import Panel from '../ui/Panel.vue';
import PaneActions from './PaneActions.vue';

interface HashOut {
  name: string;
  value: string;
}

const plain = ref('');
const outputs = ref<HashOut[]>([
  { name: 'MD5', value: '' },
  { name: 'SHA-1', value: '' },
  { name: 'SHA-256', value: '' },
  { name: 'SHA-512', value: '' }
]);

let debounce: ReturnType<typeof setTimeout> | null = null;

watch(plain, () => {
  if (debounce) clearTimeout(debounce);
  debounce = setTimeout(recompute, 150);
}, { immediate: true });

async function recompute() {
  const text = plain.value;
  const [s1, s256, s512] = await Promise.all([sha1(text), sha256(text), sha512(text)]);
  outputs.value = [
    { name: 'MD5', value: md5(text) },
    { name: 'SHA-1', value: s1 },
    { name: 'SHA-256', value: s256 },
    { name: 'SHA-512', value: s512 }
  ];
}
</script>

<template>
  <div class="pane">
    <Panel title="输入">
      <n-input
        v-model:value="plain"
        type="textarea"
        placeholder="任意文本；下面的四种 hash 会自动实时计算"
        :autosize="{ minRows: 6, maxRows: 12 }"
      />
    </Panel>

    <div class="results">
      <Panel v-for="out in outputs" :key="out.name" :title="out.name">
        <template #right>
          <PaneActions :value="out.value" />
        </template>
        <div class="hash mono">{{ out.value || '—' }}</div>
      </Panel>
    </div>
  </div>
</template>

<style scoped>
.pane { display: grid; gap: 12px; padding: 16px; height: 100%; overflow: auto; }
.results { display: grid; gap: 10px; }
.hash {
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: var(--fs-sm);
  word-break: break-all;
  color: var(--text);
  padding: 6px 0;
}
.mono { font-variant-numeric: tabular-nums; }
</style>
