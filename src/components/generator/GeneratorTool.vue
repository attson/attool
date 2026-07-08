<script setup lang="ts">
import { ref } from 'vue';
import { NTabs, NTabPane } from 'naive-ui';
import PasswordPane from './PasswordPane.vue';
import IdPane from './IdPane.vue';
import QrPane from './QrPane.vue';
import LoremPane from './LoremPane.vue';
import FakeDataPane from './FakeDataPane.vue';
import DicePane from './DicePane.vue';

const tab = ref('password');
</script>

<template>
  <div class="generator-tool">
    <n-tabs v-model:value="tab" type="line" animated>
      <n-tab-pane name="password" tab="密码">
        <div class="pane-wrap"><PasswordPane /></div>
      </n-tab-pane>
      <n-tab-pane name="id" tab="ID">
        <div class="pane-wrap"><IdPane /></div>
      </n-tab-pane>
      <n-tab-pane name="qr" tab="二维码">
        <div class="pane-wrap"><QrPane /></div>
      </n-tab-pane>
      <n-tab-pane name="lorem" tab="Lorem">
        <div class="pane-wrap"><LoremPane /></div>
      </n-tab-pane>
      <n-tab-pane name="fake" tab="假数据">
        <div class="pane-wrap"><FakeDataPane /></div>
      </n-tab-pane>
      <n-tab-pane name="dice" tab="骰子 / 抽奖">
        <div class="pane-wrap"><DicePane /></div>
      </n-tab-pane>
    </n-tabs>
  </div>
</template>

<style scoped>
.generator-tool { height: 100%; display: flex; flex-direction: column; }
.generator-tool :deep(.n-tabs) { height: 100%; display: flex; flex-direction: column; }
.generator-tool :deep(.n-tabs-pane-wrapper) { flex: 1; min-height: 0; }
.generator-tool :deep(.n-tab-pane) { height: 100%; }
.pane-wrap { height: 100%; min-height: 480px; }
</style>
