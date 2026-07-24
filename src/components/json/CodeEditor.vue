<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue';
import { loadMonaco } from '../../composables/useMonacoLoader';
import type { editor as MonacoEditor } from 'monaco-editor';

const LARGE_INPUT = 1_000_000;
const LONG_LINE = 50_000;

function maxLineLength(text: string): number {
  let max = 0;
  let start = 0;
  for (let i = 0; i < text.length; i++) {
    if (text.charCodeAt(i) === 10) { // '\n'
      if (i - start > max) max = i - start;
      start = i + 1;
    }
  }
  if (text.length - start > max) max = text.length - start;
  return max;
}

function resolveLargeOptions(text: string) {
  const isLarge = text.length > LARGE_INPUT;
  const noWrap = maxLineLength(text) > LONG_LINE;
  return {
    wordWrap: noWrap ? 'off' as const : 'on' as const,
    folding: !isLarge,
    renderValidationDecorations: isLarge ? 'off' as const : 'editable' as const,
  };
}

const props = defineProps<{
  modelValue: string;
  language?: string;
  readonly?: boolean;
  height?: number | string;
}>();

const emit = defineEmits<{
  'update:modelValue': [value: string];
}>();

const container = ref<HTMLDivElement | null>(null);
const instance = shallowRef<MonacoEditor.IStandaloneCodeEditor | null>(null);
let suppress = false;

onMounted(async () => {
  if (!container.value) return;
  const monaco = await loadMonaco();
  const initialOpts = resolveLargeOptions(props.modelValue);
  const editor = monaco.editor.create(container.value, {
    value: props.modelValue,
    language: props.language ?? 'json',
    readOnly: !!props.readonly,
    automaticLayout: true,
    minimap: { enabled: false },
    fontSize: 13,
    scrollBeyondLastLine: false,
    tabSize: 2,
    largeFileOptimizations: true,
    wordWrap: initialOpts.wordWrap,
    folding: initialOpts.folding,
    renderValidationDecorations: initialOpts.renderValidationDecorations,
    theme: matchMedia('(prefers-color-scheme: dark)').matches ? 'vs-dark' : 'vs',
  });
  editor.onDidChangeModelContent(() => {
    if (suppress) return;
    emit('update:modelValue', editor.getValue());
  });
  instance.value = editor;
});

watch(
  () => props.modelValue,
  (next) => {
    const editor = instance.value;
    if (!editor) return;
    if (editor.getValue() === next) return;
    editor.updateOptions(resolveLargeOptions(next));
    suppress = true;
    editor.setValue(next);
    suppress = false;
  },
);

watch(
  () => props.language,
  async (lang) => {
    const editor = instance.value;
    if (!editor) return;
    const monaco = await loadMonaco();
    const model = editor.getModel();
    if (model) monaco.editor.setModelLanguage(model, lang ?? 'json');
  },
);

watch(
  () => props.readonly,
  (ro) => {
    instance.value?.updateOptions({ readOnly: !!ro });
  },
);

onBeforeUnmount(() => {
  instance.value?.dispose();
  instance.value = null;
});
</script>

<template>
  <div
    ref="container"
    class="code-editor"
    :style="{ height: typeof height === 'number' ? `${height}px` : (height ?? '320px') }"
  />
</template>

<style scoped>
.code-editor {
  width: 100%;
  border: 1px solid var(--line);
  border-radius: var(--radius);
  overflow: hidden;
}
</style>
