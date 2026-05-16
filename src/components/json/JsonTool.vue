<script setup lang="ts">
import { ref } from 'vue';
import { NTabs, NTabPane } from 'naive-ui';
import JsonFormatPane from './JsonFormatPane.vue';

const tab = ref('format');
</script>

<template>
  <div class="json-tool">
    <n-tabs v-model:value="tab" type="line" animated>
      <n-tab-pane name="format" tab="格式化">
        <div class="pane-wrap"><JsonFormatPane /></div>
      </n-tab-pane>
    </n-tabs>
  </div>
</template>

<style scoped>
.json-tool { height: 100%; display: flex; flex-direction: column; }
.json-tool :deep(.n-tabs) { height: 100%; display: flex; flex-direction: column; }
.json-tool :deep(.n-tabs-pane-wrapper) { flex: 1; min-height: 0; }
.json-tool :deep(.n-tab-pane) { height: 100%; }
.pane-wrap { height: 100%; min-height: 480px; }
</style>
