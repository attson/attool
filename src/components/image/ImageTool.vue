<script setup lang="ts">
import { onMounted, provide, ref, watch } from 'vue';
import { NTabs, NTabPane } from 'naive-ui';
import CapturePane from './CapturePane.vue';
import CompressPane from './CompressPane.vue';
import ConvertPane from './ConvertPane.vue';
import ExifPane from './ExifPane.vue';
import AnnotatePane from './AnnotatePane.vue';
import OcrPane from './OcrPane.vue';
import { useRequestedTab } from './imageBus';

const tab = ref('capture');

function switchTab(name: string) {
  tab.value = name;
}
provide('image-tool:switchTab', switchTab);

const { requested, consume } = useRequestedTab();

function drainRequestedTab() {
  if (requested.value) {
    const target = consume();
    if (target) tab.value = target;
  }
}

onMounted(drainRequestedTab);
watch(requested, (val) => { if (val) drainRequestedTab(); });
</script>

<template>
  <div class="image-tool">
    <n-tabs v-model:value="tab" type="line" animated>
      <n-tab-pane name="capture" tab="截图">
        <div class="pane-wrap"><CapturePane /></div>
      </n-tab-pane>
      <n-tab-pane name="annotate" tab="标注">
        <div class="pane-wrap"><AnnotatePane /></div>
      </n-tab-pane>
      <n-tab-pane name="compress" tab="压缩">
        <div class="pane-wrap"><CompressPane /></div>
      </n-tab-pane>
      <n-tab-pane name="convert" tab="格式转换">
        <div class="pane-wrap"><ConvertPane /></div>
      </n-tab-pane>
      <n-tab-pane name="exif" tab="EXIF">
        <div class="pane-wrap"><ExifPane /></div>
      </n-tab-pane>
      <n-tab-pane name="ocr" tab="OCR">
        <div class="pane-wrap"><OcrPane /></div>
      </n-tab-pane>
    </n-tabs>
  </div>
</template>

<style scoped>
.image-tool { height: 100%; display: flex; flex-direction: column; }
.image-tool :deep(.n-tabs) { height: 100%; display: flex; flex-direction: column; }
.image-tool :deep(.n-tabs-pane-wrapper) { flex: 1; min-height: 0; }
.image-tool :deep(.n-tab-pane) { height: 100%; }
.pane-wrap { height: 100%; min-height: 480px; }
</style>
