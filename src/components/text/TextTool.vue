<script setup lang="ts">
import { ref } from 'vue';
import { NTabs, NTabPane } from 'naive-ui';
import CleanPane from './CleanPane.vue';
import SortPane from './SortPane.vue';
import CasePane from './CasePane.vue';
import SplitJoinPane from './SplitJoinPane.vue';
import ExtractPane from './ExtractPane.vue';
import DiffPane from './DiffPane.vue';

const tab = ref('clean');
</script>

<template>
  <div class="text-tool">
    <n-tabs v-model:value="tab" type="line" animated>
      <n-tab-pane name="clean" tab="整理">
        <div class="pane-wrap"><CleanPane /></div>
      </n-tab-pane>
      <n-tab-pane name="sort" tab="排序">
        <div class="pane-wrap"><SortPane /></div>
      </n-tab-pane>
      <n-tab-pane name="case" tab="大小写">
        <div class="pane-wrap"><CasePane /></div>
      </n-tab-pane>
      <n-tab-pane name="splitjoin" tab="拆合">
        <div class="pane-wrap"><SplitJoinPane /></div>
      </n-tab-pane>
      <n-tab-pane name="extract" tab="抽取">
        <div class="pane-wrap"><ExtractPane /></div>
      </n-tab-pane>
      <n-tab-pane name="diff" tab="对比">
        <div class="pane-wrap"><DiffPane /></div>
      </n-tab-pane>
    </n-tabs>
  </div>
</template>

<style scoped>
.text-tool { height: 100%; display: flex; flex-direction: column; }
.text-tool :deep(.n-tabs) { height: 100%; display: flex; flex-direction: column; }
.text-tool :deep(.n-tabs-pane-wrapper) { flex: 1; min-height: 0; }
.text-tool :deep(.n-tab-pane) { height: 100%; }
.pane-wrap { height: 100%; min-height: 480px; }
</style>
