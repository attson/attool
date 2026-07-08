<script setup lang="ts">
import { ref } from 'vue';
import { NTabs, NTabPane } from 'naive-ui';
import TimestampPane from './TimestampPane.vue';
import TimezonePane from './TimezonePane.vue';
import CronPane from './CronPane.vue';
import DurationPane from './DurationPane.vue';

const tab = ref('timestamp');
</script>

<template>
  <div class="time-tool">
    <n-tabs v-model:value="tab" type="line" animated>
      <n-tab-pane name="timestamp" tab="时间戳">
        <div class="pane-wrap"><TimestampPane /></div>
      </n-tab-pane>
      <n-tab-pane name="timezone" tab="时区">
        <div class="pane-wrap"><TimezonePane /></div>
      </n-tab-pane>
      <n-tab-pane name="cron" tab="Cron">
        <div class="pane-wrap"><CronPane /></div>
      </n-tab-pane>
      <n-tab-pane name="duration" tab="Duration">
        <div class="pane-wrap"><DurationPane /></div>
      </n-tab-pane>
    </n-tabs>
  </div>
</template>

<style scoped>
.time-tool { height: 100%; display: flex; flex-direction: column; }
.time-tool :deep(.n-tabs) { height: 100%; display: flex; flex-direction: column; }
.time-tool :deep(.n-tabs-pane-wrapper) { flex: 1; min-height: 0; }
.time-tool :deep(.n-tab-pane) { height: 100%; }
.pane-wrap { height: 100%; min-height: 480px; }
</style>
