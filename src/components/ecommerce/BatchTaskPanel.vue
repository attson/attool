<script setup lang="ts">
import { computed, ref } from 'vue';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { NAlert, NButton, NEmpty, NInput, NRadioButton, NRadioGroup, NTag } from 'naive-ui';
import type { BatchOutputItem, BatchRunMode, BatchTaskInput, TemplateLayer } from '../../types/ecommerceTemplate';

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
const mode = ref<BatchRunMode>('product');

type VariantDragState = { taskId: string; index: number };
const draggingVariant = ref<VariantDragState | null>(null);
const dragOverVariant = ref<VariantDragState | null>(null);
const variantDropPlacement = ref<'before' | 'after'>('before');

const allEqualLength = computed(() => {
  if (tasks.value.length === 0) return false;
  const first = tasks.value[0].variants.length;
  return tasks.value.every((task) => task.variants.length === first);
});

const totalCombinations = computed(() => {
  if (tasks.value.length === 0) return 0;
  if (tasks.value.some((task) => task.variants.length === 0)) return 0;
  if (mode.value === 'product') {
    return tasks.value.reduce((acc, task) => acc * task.variants.length, 1);
  }
  return allEqualLength.value ? tasks.value[0].variants.length : 0;
});

const canRun = computed(() => {
  if (tasks.value.length === 0) return false;
  if (tasks.value.some((task) => task.variants.length === 0)) return false;
  if (mode.value === 'parallel' && !allEqualLength.value) return false;
  return true;
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

function onVariantDragStart(event: DragEvent, taskId: string, index: number) {
  draggingVariant.value = { taskId, index };
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = 'move';
    event.dataTransfer.setData('text/plain', `${taskId}:${index}`);
  }
}

function onVariantDragOver(event: DragEvent, taskId: string, index: number, axis: 'x' | 'y') {
  const dragging = draggingVariant.value;
  if (!dragging || dragging.taskId !== taskId || dragging.index === index) return;
  event.preventDefault();
  const target = event.currentTarget as HTMLElement;
  const bounds = target.getBoundingClientRect();
  variantDropPlacement.value = axis === 'x'
    ? (event.clientX > bounds.left + bounds.width / 2 ? 'after' : 'before')
    : (event.clientY > bounds.top + bounds.height / 2 ? 'after' : 'before');
  dragOverVariant.value = { taskId, index };
  if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';
}

function onVariantDragLeave(taskId: string, index: number) {
  if (dragOverVariant.value?.taskId === taskId && dragOverVariant.value.index === index) {
    dragOverVariant.value = null;
  }
}

function onVariantDrop(event: DragEvent, taskId: string, dropIndex: number) {
  event.preventDefault();
  const dragging = draggingVariant.value;
  const placement = variantDropPlacement.value;
  endVariantDrag();
  if (!dragging || dragging.taskId !== taskId) return;
  const task = tasks.value.find((item) => item.id === taskId);
  if (!task) return;
  const fromIndex = dragging.index;
  const insertIndex = placement === 'after' ? dropIndex + 1 : dropIndex;
  const adjusted = fromIndex < insertIndex ? insertIndex - 1 : insertIndex;
  if (adjusted === fromIndex) return;
  const [moved] = task.variants.splice(fromIndex, 1);
  task.variants.splice(adjusted, 0, moved);
}

function endVariantDrag() {
  draggingVariant.value = null;
  dragOverVariant.value = null;
  variantDropPlacement.value = 'before';
}

function variantDragClasses(taskId: string, index: number) {
  return {
    dragging: draggingVariant.value?.taskId === taskId && draggingVariant.value.index === index,
    'drag-over-before': dragOverVariant.value?.taskId === taskId && dragOverVariant.value.index === index && variantDropPlacement.value === 'before',
    'drag-over-after': dragOverVariant.value?.taskId === taskId && dragOverVariant.value.index === index && variantDropPlacement.value === 'after'
  };
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
      tasks: payload,
      mode: mode.value
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
        <div
          v-for="(variant, index) in task.variants"
          :key="`${task.id}-${index}`"
          :class="['batch-variant-image', variantDragClasses(task.id, index)]"
          draggable="true"
          @dragstart.stop="onVariantDragStart($event, task.id, index)"
          @dragover.stop="onVariantDragOver($event, task.id, index, 'x')"
          @dragleave.stop="onVariantDragLeave(task.id, index)"
          @drop.stop="onVariantDrop($event, task.id, index)"
          @dragend.stop="endVariantDrag"
        >
          <img :src="imageVariantSrc(variant as ImageVariant)" :alt="imageVariantLabel(variant as ImageVariant)" draggable="false" />
          <span>{{ imageVariantLabel(variant as ImageVariant) }}</span>
          <button type="button" class="batch-variant-remove" title="移除" @click.stop="removeVariant(task.id, index)">×</button>
        </div>
        <button type="button" class="batch-variant-add" @click="pickImageVariants(task.id)">+ 添加图片</button>
      </div>

      <div v-else class="batch-variant-text-list">
        <div
          v-for="(variant, index) in task.variants"
          :key="`${task.id}-${index}`"
          :class="['batch-variant-text', variantDragClasses(task.id, index)]"
          draggable="true"
          @dragstart.stop="onVariantDragStart($event, task.id, index)"
          @dragover.stop="onVariantDragOver($event, task.id, index, 'y')"
          @dragleave.stop="onVariantDragLeave(task.id, index)"
          @drop.stop="onVariantDrop($event, task.id, index)"
          @dragend.stop="endVariantDrag"
        >
          <span>{{ (variant as TextVariant).value }}</span>
          <button type="button" class="batch-variant-remove" title="移除" @click.stop="removeVariant(task.id, index)">×</button>
        </div>
        <div class="batch-variant-text-add">
          <n-input v-model:value="task.textDraft" placeholder="输入文字变体" size="small" @keyup.enter="addTextVariant(task.id)" />
          <n-button size="small" secondary :disabled="!task.textDraft.trim()" @click="addTextVariant(task.id)">添加</n-button>
        </div>
      </div>
    </div>

    <footer class="batch-task-footer">
      <div class="batch-task-footer-info">
        <n-radio-group v-model:value="mode" size="small">
          <n-radio-button value="product">叉乘</n-radio-button>
          <n-radio-button value="parallel">1:1</n-radio-button>
        </n-radio-group>
        <span class="batch-task-summary">
          <template v-if="mode === 'parallel' && !allEqualLength">1:1 模式要求各任务变体数相等</template>
          <template v-else>将生成 {{ totalCombinations }} 张组合</template>
        </span>
      </div>
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
