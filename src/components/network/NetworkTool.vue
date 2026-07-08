<script setup lang="ts">
import { ref } from 'vue';
import { NTabs, NTabPane } from 'naive-ui';
import UrlPane from './UrlPane.vue';
import PingPane from './PingPane.vue';
import PortPane from './PortPane.vue';
import DnsPane from './DnsPane.vue';

const tab = ref('url');
</script>

<template>
  <div class="network-tool">
    <n-tabs v-model:value="tab" type="line" animated>
      <n-tab-pane name="url" tab="URL 分析">
        <div class="pane-wrap"><UrlPane /></div>
      </n-tab-pane>
      <n-tab-pane name="ping" tab="Ping">
        <div class="pane-wrap"><PingPane /></div>
      </n-tab-pane>
      <n-tab-pane name="port" tab="端口检查">
        <div class="pane-wrap"><PortPane /></div>
      </n-tab-pane>
      <n-tab-pane name="dns" tab="DNS 解析">
        <div class="pane-wrap"><DnsPane /></div>
      </n-tab-pane>
    </n-tabs>
  </div>
</template>

<style scoped>
.network-tool { height: 100%; display: flex; flex-direction: column; }
.network-tool :deep(.n-tabs) { height: 100%; display: flex; flex-direction: column; }
.network-tool :deep(.n-tabs-pane-wrapper) { flex: 1; min-height: 0; }
.network-tool :deep(.n-tab-pane) { height: 100%; }
.pane-wrap { height: 100%; min-height: 480px; }
</style>
