<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue';
import { loadMonaco } from '../../composables/useMonacoLoader';
import type { editor as MonacoEditor } from 'monaco-editor';

const props = defineProps<{
  original: string;
  modified: string;
  height?: number | string;
}>();

const container = ref<HTMLDivElement | null>(null);
const instance = shallowRef<MonacoEditor.IStandaloneDiffEditor | null>(null);
const originalModel = shallowRef<MonacoEditor.ITextModel | null>(null);
const modifiedModel = shallowRef<MonacoEditor.ITextModel | null>(null);

function viewHeight(height: number | string | undefined): string {
  if (typeof height === 'number') return `${height}px`;
  return height ?? '100%';
}

onMounted(async () => {
  if (!container.value) return;
  const monaco = await loadMonaco();
  originalModel.value = monaco.editor.createModel(props.original, 'json');
  modifiedModel.value = monaco.editor.createModel(props.modified, 'json');

  const editor = monaco.editor.createDiffEditor(container.value, {
    automaticLayout: true,
    readOnly: true,
    originalEditable: false,
    renderSideBySide: true,
    minimap: { enabled: false },
    fontSize: 13,
    scrollBeyondLastLine: false,
    tabSize: 2,
    wordWrap: 'on',
    folding: true,
    largeFileOptimizations: true,
    renderOverviewRuler: true,
    ignoreTrimWhitespace: false,
    renderIndicators: true,
    theme: matchMedia('(prefers-color-scheme: dark)').matches ? 'vs-dark' : 'vs',
  });
  editor.setModel({
    original: originalModel.value,
    modified: modifiedModel.value,
  });
  instance.value = editor;
});

watch(
  () => [props.original, props.modified] as const,
  ([original, modified]) => {
    originalModel.value?.setValue(original);
    modifiedModel.value?.setValue(modified);
  },
);

onBeforeUnmount(() => {
  instance.value?.dispose();
  originalModel.value?.dispose();
  modifiedModel.value?.dispose();
  instance.value = null;
  originalModel.value = null;
  modifiedModel.value = null;
});
</script>

<template>
  <div
    ref="container"
    class="json-diff-viewer"
    :style="{ height: viewHeight(height) }"
  />
</template>

<style scoped>
.json-diff-viewer {
  width: 100%;
  min-height: 0;
  border: 1px solid var(--line);
  border-radius: var(--radius);
  overflow: hidden;
}
</style>
