<script setup lang="ts">
import { computed } from 'vue';
import { NEmpty, NForm, NFormItem, NInput, NInputNumber, NSelect } from 'naive-ui';
import type { TemplateLayer } from '../../types/ecommerceTemplate';

const props = defineProps<{ layer: TemplateLayer | null }>();
const emit = defineEmits<{ update: [layer: TemplateLayer] }>();

const selected = computed(() => props.layer);
const fitOptions = [
  { label: '覆盖', value: 'cover' },
  { label: '完整显示', value: 'contain' },
  { label: '拉伸', value: 'stretch' }
];

function patch(values: Partial<TemplateLayer>) {
  if (!props.layer) return;
  emit('update', { ...props.layer, ...values });
}
</script>

<template>
  <n-empty v-if="!selected" description="请选择一个图层" />
  <n-form v-else label-placement="top" size="small">
    <n-form-item label="图层名">
      <n-input :value="selected.name" @update:value="patch({ name: $event })" />
    </n-form-item>
    <n-form-item label="绑定字段">
      <n-input :value="selected.bindingKey" placeholder="例如 title" @update:value="patch({ bindingKey: $event || undefined })" />
    </n-form-item>
    <n-form-item label="X / Y / 宽 / 高">
      <div class="template-prop-grid">
        <n-input-number :value="selected.x" @update:value="patch({ x: $event ?? 0 })" />
        <n-input-number :value="selected.y" @update:value="patch({ y: $event ?? 0 })" />
        <n-input-number :value="selected.width" :min="1" @update:value="patch({ width: $event ?? 1 })" />
        <n-input-number :value="selected.height" :min="1" @update:value="patch({ height: $event ?? 1 })" />
      </div>
    </n-form-item>
    <template v-if="selected.type === 'text' && selected.text">
      <n-form-item label="文字内容">
        <n-input :value="selected.text.text" type="textarea" @update:value="patch({ text: { ...selected.text!, text: $event } })" />
      </n-form-item>
      <n-form-item label="字号">
        <n-input-number :value="selected.text.fontSize" :min="1" @update:value="patch({ text: { ...selected.text!, fontSize: $event ?? 12 } })" />
      </n-form-item>
      <n-form-item label="颜色">
        <n-input :value="selected.text.color" @update:value="patch({ text: { ...selected.text!, color: $event } })" />
      </n-form-item>
    </template>
    <template v-if="selected.type === 'image' && selected.image">
      <n-form-item label="裁剪方式">
        <n-select :value="selected.image.fit" :options="fitOptions" @update:value="patch({ image: { ...selected.image!, fit: $event } })" />
      </n-form-item>
    </template>
  </n-form>
</template>
