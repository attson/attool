<script setup lang="ts">
import { computed, ref } from 'vue';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { NAlert, NButton, NEmpty, NInput, NTag } from 'naive-ui';
import type { BatchOutputItem, BatchTaskInput, TemplateLayer } from '../../types/ecommerceTemplate';

type ImageVariant = { kind: 'image'; sourcePath: string };
type TextVariant = { kind: 'text'; value: string };
type Variant = ImageVariant | TextVariant;

type BatchTaskRecord = {
  id: string;
  layerId: string;
  layerName: string;
  layerKind: 'image' | 'text';
  variants: Variant[];
  textDraft: string;
};

const props = defineProps<{
  templateId: string;
  saveTemplate: () => Promise<void>;
}>();

const tasks = ref<BatchTaskRecord[]>([]);
const outputs = ref<BatchOutputItem[]>([]);
const selectedIds = ref<Set<string>>(new Set());
const running = ref(false);
const saving = ref(false);
const notice = ref('');
const success = ref('');

const totalCombinations = computed(() => {
  if (tasks.value.length === 0) return 0;
  return tasks.value.reduce((acc, task) => acc * Math.max(task.variants.length, 0), 1);
});

const canRun = computed(() => {
  if (tasks.value.length === 0) return false;
  return tasks.value.every((task) => task.variants.length > 0);
});

const allSelected = computed(() => outputs.value.length > 0 && selectedIds.value.size === outputs.value.length);

function addTaskForLayer(layer: TemplateLayer) {
  if (layer.type !== 'image' && layer.type !== 'text') {
    notice.value = '只能为图片或文字图层创建批量替换任务';
    return;
  }
  if (tasks.value.some((task) => task.layerId === layer.id)) {
    notice.value = `图层「${layer.name}」已有批量替换任务`;
    return;
  }
  tasks.value.push({
    id: `task-${crypto.randomUUID()}`,
    layerId: layer.id,
    layerName: layer.name,
    layerKind: layer.type,
    variants: [],
    textDraft: ''
  });
  notice.value = '';
}

defineExpose({ addTaskForLayer });

function removeTask(taskId: string) {
  tasks.value = tasks.value.filter((task) => task.id !== taskId);
}

function removeVariant(taskId: string, index: number) {
  const task = tasks.value.find((item) => item.id === taskId);
  if (!task) return;
  task.variants.splice(index, 1);
}

async function pickImageVariants(taskId: string) {
  notice.value = '';
  try {
    const selected = await open({ multiple: true, filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp'] }] });
    if (!Array.isArray(selected) || selected.length === 0) return;
    const task = tasks.value.find((item) => item.id === taskId);
    if (!task || task.layerKind !== 'image') return;
    for (const path of selected) {
      task.variants.push({ kind: 'image', sourcePath: path });
    }
  } catch (error) {
    notice.value = String(error);
  }
}

function addTextVariant(taskId: string) {
  const task = tasks.value.find((item) => item.id === taskId);
  if (!task || task.layerKind !== 'text') return;
  const value = task.textDraft.trim();
  if (!value) return;
  task.variants.push({ kind: 'text', value });
  task.textDraft = '';
}

function imageVariantSrc(variant: ImageVariant) {
  return convertFileSrc(variant.sourcePath);
}

function imageVariantLabel(variant: ImageVariant) {
  const segments = variant.sourcePath.split(/[/\\]/);
  return segments[segments.length - 1] || variant.sourcePath;
}

async function runBatch() {
  if (running.value || !canRun.value) return;
  if (totalCombinations.value > 100) {
    if (!window.confirm(`本次将生成 ${totalCombinations.value} 张图片，可能耗时较久，继续？`)) return;
  }
  notice.value = '';
  success.value = '';
  running.value = true;
  try {
    await props.saveTemplate();
    const payload: BatchTaskInput[] = tasks.value.map((task) => ({
      layerId: task.layerId,
      variants: task.variants.map((variant) =>
        variant.kind === 'image'
          ? { kind: 'image', sourcePath: variant.sourcePath }
          : { kind: 'text', value: variant.value }
      )
    }));
    const result = await invoke<BatchOutputItem[]>('run_batch_replace_tasks', {
      templateId: props.templateId,
      tasks: payload
    });
    outputs.value = result;
    selectedIds.value = new Set(result.map((item) => item.id));
  } catch (error) {
    notice.value = String(error);
  } finally {
    running.value = false;
  }
}

function toggleOutput(id: string) {
  const next = new Set(selectedIds.value);
  if (next.has(id)) {
    next.delete(id);
  } else {
    next.add(id);
  }
  selectedIds.value = next;
}

function selectAll() {
  selectedIds.value = new Set(outputs.value.map((item) => item.id));
}

function clearSelection() {
  selectedIds.value = new Set();
}

function outputSrc(output: BatchOutputItem) {
  return convertFileSrc(output.filePath);
}

async function downloadSelected() {
  if (saving.value || selectedIds.value.size === 0) return;
  notice.value = '';
  success.value = '';
  try {
    const targetDir = await open({ directory: true, multiple: false });
    if (typeof targetDir !== 'string') return;
    saving.value = true;
    const filePaths = outputs.value
      .filter((item) => selectedIds.value.has(item.id))
      .map((item) => item.filePath);
    const saved = await invoke<number>('save_batch_replace_outputs', { filePaths, targetDir });
    success.value = `已保存 ${saved} 张到：${targetDir}`;
  } catch (error) {
    notice.value = String(error);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <n-empty v-if="tasks.length === 0" description="选中图层后点「添加批量替换」开始一个任务" />

  <div v-else class="batch-task-list">
    <div v-for="task in tasks" :key="task.id" class="batch-task-card">
      <header class="batch-task-header">
        <div class="batch-task-title">
          <strong>{{ task.layerName }}</strong>
          <n-tag size="small" round>{{ task.layerKind === 'image' ? '图片' : '文字' }}</n-tag>
          <span class="batch-task-count">{{ task.variants.length }} 个变体</span>
        </div>
        <button type="button" class="batch-task-remove" title="删除任务" @click="removeTask(task.id)">×</button>
      </header>

      <div v-if="task.layerKind === 'image'" class="batch-variant-grid">
        <div v-for="(variant, index) in task.variants" :key="`${task.id}-${index}`" class="batch-variant-image">
          <img :src="imageVariantSrc(variant as ImageVariant)" :alt="imageVariantLabel(variant as ImageVariant)" />
          <span>{{ imageVariantLabel(variant as ImageVariant) }}</span>
          <button type="button" class="batch-variant-remove" title="移除" @click="removeVariant(task.id, index)">×</button>
        </div>
        <button type="button" class="batch-variant-add" @click="pickImageVariants(task.id)">+ 添加图片</button>
      </div>

      <div v-else class="batch-variant-text-list">
        <div v-for="(variant, index) in task.variants" :key="`${task.id}-${index}`" class="batch-variant-text">
          <span>{{ (variant as TextVariant).value }}</span>
          <button type="button" class="batch-variant-remove" title="移除" @click="removeVariant(task.id, index)">×</button>
        </div>
        <div class="batch-variant-text-add">
          <n-input v-model:value="task.textDraft" placeholder="输入文字变体" size="small" @keyup.enter="addTextVariant(task.id)" />
          <n-button size="small" secondary :disabled="!task.textDraft.trim()" @click="addTextVariant(task.id)">添加</n-button>
        </div>
      </div>
    </div>

    <footer class="batch-task-footer">
      <span class="batch-task-summary">将生成 {{ totalCombinations }} 张组合</span>
      <n-button type="primary" :loading="running" :disabled="!canRun" @click="runBatch">执行生成</n-button>
    </footer>
  </div>

  <n-alert v-if="notice" type="error" :bordered="false" closable @close="notice = ''">{{ notice }}</n-alert>
  <n-alert v-if="success" type="success" :bordered="false" closable @close="success = ''">{{ success }}</n-alert>

  <section v-if="outputs.length" class="batch-output-section">
    <header class="batch-output-header">
      <strong>生成结果（{{ outputs.length }}）</strong>
      <div class="batch-output-actions">
        <n-button size="small" :disabled="allSelected" @click="selectAll">全选</n-button>
        <n-button size="small" :disabled="selectedIds.size === 0" @click="clearSelection">清除</n-button>
        <n-button type="primary" size="small" :loading="saving" :disabled="selectedIds.size === 0" @click="downloadSelected">下载选中（{{ selectedIds.size }}）</n-button>
      </div>
    </header>
    <div class="batch-output-grid">
      <button
        v-for="output in outputs"
        :key="output.id"
        type="button"
        :class="['batch-output-card', { selected: selectedIds.has(output.id) }]"
        @click="toggleOutput(output.id)"
      >
        <img :src="outputSrc(output)" :alt="output.fileName" />
        <span>{{ output.fileName }}</span>
        <span class="batch-output-check">{{ selectedIds.has(output.id) ? '✓' : '' }}</span>
      </button>
    </div>
  </section>
</template>
