<script setup lang="ts">
import { ref } from 'vue';
import { NTabs, NTabPane } from 'naive-ui';
import Base64Pane from './Base64Pane.vue';
import UrlPane from './UrlPane.vue';
import UnicodePane from './UnicodePane.vue';
import HexPane from './HexPane.vue';
import HashPane from './HashPane.vue';
import JwtPane from './JwtPane.vue';

const tab = ref('base64');
</script>

<template>
  <div class="codec-tool">
    <n-tabs v-model:value="tab" type="line" animated>
      <n-tab-pane name="base64" tab="Base64">
        <div class="pane-wrap"><Base64Pane /></div>
      </n-tab-pane>
      <n-tab-pane name="url" tab="URL">
        <div class="pane-wrap"><UrlPane /></div>
      </n-tab-pane>
      <n-tab-pane name="unicode" tab="Unicode">
        <div class="pane-wrap"><UnicodePane /></div>
      </n-tab-pane>
      <n-tab-pane name="hex" tab="Hex">
        <div class="pane-wrap"><HexPane /></div>
      </n-tab-pane>
      <n-tab-pane name="hash" tab="Hash">
        <div class="pane-wrap"><HashPane /></div>
      </n-tab-pane>
      <n-tab-pane name="jwt" tab="JWT">
        <div class="pane-wrap"><JwtPane /></div>
      </n-tab-pane>
    </n-tabs>
  </div>
</template>

<style scoped>
.codec-tool { height: 100%; display: flex; flex-direction: column; }
.codec-tool :deep(.n-tabs) { height: 100%; display: flex; flex-direction: column; }
.codec-tool :deep(.n-tabs-pane-wrapper) { flex: 1; min-height: 0; }
.codec-tool :deep(.n-tab-pane) { height: 100%; }
.pane-wrap { height: 100%; min-height: 480px; }
</style>
