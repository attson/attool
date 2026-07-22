<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { NAlert, NButton, NInput, NModal } from 'naive-ui';
import { parseOpenApiToCollection, type ImportedOpenApiCollection } from './openapiImport';

const props = defineProps<{
  show: boolean;
}>();

const emit = defineEmits<{
  (e: 'update:show', v: boolean): void;
  (e: 'import', v: ImportedOpenApiCollection): void;
}>();

const jsonText = ref('');
const baseUrl = ref('');
const collectionName = ref('');
const error = ref('');

const preview = computed(() => {
  if (!jsonText.value.trim()) return null;
  try {
    return parseOpenApiToCollection(jsonText.value, {
      baseUrl: baseUrl.value || undefined,
      collectionName: collectionName.value || undefined
    });
  } catch {
    return null;
  }
});

watch(
  () => props.show,
  (show) => {
    if (!show) return;
    jsonText.value = '';
    baseUrl.value = '';
    collectionName.value = '';
    error.value = '';
  }
);

async function onFile(e: Event) {
  const input = e.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  try {
    const text = await file.text();
    jsonText.value = text;
    const parsed = parseOpenApiToCollection(text);
    baseUrl.value = parsed.baseUrl;
    collectionName.value = parsed.collection.name;
    error.value = '';
  } catch (err) {
    error.value = String((err as Error).message ?? err);
  } finally {
    input.value = '';
  }
}

function close() {
  emit('update:show', false);
}

function doImport() {
  try {
    const parsed = parseOpenApiToCollection(jsonText.value, {
      baseUrl: baseUrl.value || undefined,
      collectionName: collectionName.value || undefined
    });
    emit('import', parsed);
    close();
  } catch (err) {
    error.value = String((err as Error).message ?? err);
  }
}
</script>

<template>
  <n-modal
    :show="show"
    preset="card"
    title="导入 OpenAPI JSON"
    :bordered="false"
    style="width: 680px; max-width: 94vw;"
    @update:show="(v: boolean) => emit('update:show', v)"
  >
    <div class="import-modal">
      <label class="file-pick">
        <input type="file" accept=".json,application/json" @change="onFile" />
        <span>选择 openapi.json</span>
      </label>

      <div class="grid">
        <label>
          <span>集合名称</span>
          <n-input v-model:value="collectionName" size="small" placeholder="默认读取 info.title" />
        </label>
        <label>
          <span>Base URL</span>
          <n-input v-model:value="baseUrl" size="small" placeholder="https://api.example.com 或 {{baseUrl}}" />
        </label>
      </div>

      <label class="json-box">
        <span>JSON 内容</span>
        <n-input
          v-model:value="jsonText"
          type="textarea"
          :autosize="{ minRows: 10, maxRows: 16 }"
          placeholder="{ &quot;openapi&quot;: &quot;3.0.0&quot;, ... }"
        />
      </label>

      <n-alert v-if="error" type="error" :bordered="false">{{ error }}</n-alert>
      <div v-else-if="preview" class="preview">
        将导入 {{ preview.requests.length }} 个请求、{{ preview.folders.length }} 个目录到集合
        <span class="mono">{{ preview.collection.name }}</span>
      </div>

      <div class="actions">
        <n-button secondary @click="close">取消</n-button>
        <n-button type="primary" :disabled="!jsonText.trim()" @click="doImport">导入集合</n-button>
      </div>
    </div>
  </n-modal>
</template>

<style scoped>
.import-modal { display: grid; gap: 12px; }
.file-pick {
  display: inline-flex;
  width: max-content;
  cursor: pointer;
  color: var(--accent);
  font-size: var(--fs-xs);
}
.file-pick input { display: none; }
.grid { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
label { display: grid; gap: 5px; font-size: var(--fs-xs); color: var(--text-muted); }
.json-box :deep(textarea) { font-family: var(--font-mono); }
.preview {
  padding: 8px 10px;
  border: 1px solid var(--line);
  border-radius: var(--radius);
  background: var(--bg-elev-2);
  color: var(--text-muted);
  font-size: var(--fs-xs);
}
.mono { font-family: var(--font-mono); color: var(--text); }
.actions { display: flex; justify-content: flex-end; gap: 8px; }
</style>
