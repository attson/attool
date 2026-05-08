<script setup lang="ts">
import { computed } from 'vue';
import { NCheckbox, NColorPicker, NEmpty, NForm, NFormItem, NInput, NInputNumber, NSelect, NSlider } from 'naive-ui';
import type { TemplateLayer, TextAlign, TextDecoration, TextFontStyle } from '../../types/ecommerceTemplate';

const props = defineProps<{ layer: TemplateLayer | null }>();
const emit = defineEmits<{ update: [layer: TemplateLayer] }>();

const selected = computed(() => props.layer);
const fitOptions = [
  { label: '覆盖', value: 'cover' },
  { label: '完整显示', value: 'contain' },
  { label: '拉伸', value: 'stretch' }
];
const alignOptions: { label: string; value: TextAlign }[] = [
  { label: '左对齐', value: 'left' },
  { label: '居中', value: 'center' },
  { label: '右对齐', value: 'right' }
];
const fontStyleOptions: { label: string; value: TextFontStyle }[] = [
  { label: '常规', value: 'normal' },
  { label: '斜体', value: 'italic' }
];
const decorationOptions: { label: string; value: TextDecoration }[] = [
  { label: '无', value: 'none' },
  { label: '下划线', value: 'underline' },
  { label: '删除线', value: 'line-through' }
];

function patch(values: Partial<TemplateLayer>) {
  if (!props.layer || props.layer.locked) return;
  emit('update', { ...props.layer, ...values });
}

function fixed2(value: number) {
  return Number(value.toFixed(2));
}
</script>

<template>
  <n-empty v-if="!selected" description="请选择一个图层" />
  <n-form v-else class="template-prop-form" label-placement="left" :label-width="58" size="small">
    <section class="template-prop-section">
      <h3>基础</h3>
      <n-form-item label="图层名">
        <n-input :value="selected.name" :disabled="selected.locked" @update:value="patch({ name: $event })" />
      </n-form-item>
      <n-form-item label="X">
        <n-input-number :value="fixed2(selected.x)" :precision="2" :disabled="selected.locked" @update:value="patch({ x: $event ?? 0 })" />
      </n-form-item>
      <n-form-item label="Y">
        <n-input-number :value="fixed2(selected.y)" :precision="2" :disabled="selected.locked" @update:value="patch({ y: $event ?? 0 })" />
      </n-form-item>
      <n-form-item label="宽">
        <n-input-number :value="fixed2(selected.width)" :precision="2" :min="1" :disabled="selected.locked" @update:value="patch({ width: $event ?? 1 })" />
      </n-form-item>
      <n-form-item label="高">
        <n-input-number :value="fixed2(selected.height)" :precision="2" :min="1" :disabled="selected.locked" @update:value="patch({ height: $event ?? 1 })" />
      </n-form-item>
      <n-form-item label="透明度">
        <n-slider :value="selected.opacity" :min="0" :max="1" :step="0.01" :disabled="selected.locked" @update:value="patch({ opacity: Number($event) })" />
      </n-form-item>
      <n-form-item label="旋转">
        <n-input-number :value="selected.rotation" :disabled="selected.locked" @update:value="patch({ rotation: $event ?? 0 })" />
      </n-form-item>
      <n-form-item label="显示">
        <n-checkbox :checked="selected.visible" @update:checked="patch({ visible: Boolean($event) })" />
      </n-form-item>
      <n-form-item label="锁定">
        <n-checkbox :checked="Boolean(selected.locked)" @update:checked="emit('update', { ...selected, locked: Boolean($event) })" />
      </n-form-item>
    </section>

    <section class="template-prop-section">
      <h3>绑定</h3>
      <n-form-item label="绑定字段">
        <n-input :value="selected.bindingKey" placeholder="例如 title" :disabled="selected.locked" @update:value="patch({ bindingKey: $event || undefined })" />
      </n-form-item>
    </section>

    <section v-if="selected.type === 'text' && selected.text" class="template-prop-section">
      <h3>文字</h3>
      <n-form-item label="文字内容">
        <n-input :value="selected.text.text" type="textarea" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, text: $event } })" />
      </n-form-item>
      <n-form-item label="字体">
        <n-input :value="selected.text.fontFamily" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, fontFamily: $event } })" />
      </n-form-item>
      <n-form-item label="字号">
        <n-input-number :value="selected.text.fontSize" :min="1" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, fontSize: $event ?? 12 } })" />
      </n-form-item>
      <n-form-item label="字重">
        <n-input-number :value="Number(selected.text.fontWeight) || 400" :min="100" :max="900" :step="100" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, fontWeight: $event ?? 400 } })" />
      </n-form-item>
      <n-form-item label="样式">
        <n-select :value="selected.text.fontStyle ?? 'normal'" :options="fontStyleOptions" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, fontStyle: $event as TextFontStyle } })" />
      </n-form-item>
      <n-form-item label="装饰">
        <n-select :value="selected.text.textDecoration ?? 'none'" :options="decorationOptions" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, textDecoration: $event as TextDecoration } })" />
      </n-form-item>
      <n-form-item label="颜色">
        <div class="template-color-field">
          <n-color-picker :value="selected.text.color" :show-alpha="false" :modes="['hex']" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, color: $event } })" />
          <n-input :value="selected.text.color" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, color: $event } })" />
        </div>
      </n-form-item>
      <n-form-item label="对齐">
        <n-select :value="selected.text.align ?? 'left'" :options="alignOptions" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, align: $event as TextAlign } })" />
      </n-form-item>
      <n-form-item label="字距">
        <n-input-number :value="selected.text.letterSpacing" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, letterSpacing: $event ?? undefined } })" />
      </n-form-item>
      <n-form-item label="行高">
        <n-input-number :value="selected.text.lineHeight" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, lineHeight: $event ?? undefined } })" />
      </n-form-item>
    </section>

    <details v-if="selected.type === 'text' && selected.text" class="template-prop-section template-prop-advanced">
      <summary>填充 / 描边 / 阴影</summary>
      <n-form-item label="背景色">
        <div class="template-color-field">
          <n-color-picker :value="selected.text.backgroundColor ?? null" :show-alpha="false" :modes="['hex']" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, backgroundColor: $event || undefined } })" />
          <n-input :value="selected.text.backgroundColor" placeholder="#fff1b8" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, backgroundColor: $event || undefined } })" />
        </div>
      </n-form-item>
      <n-form-item label="圆角">
        <n-input-number :value="selected.text.backgroundRadius" :min="0" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, backgroundRadius: $event ?? 0 } })" />
      </n-form-item>
      <n-form-item label="描边色">
        <div class="template-color-field">
          <n-color-picker :value="selected.text.strokeColor ?? null" :show-alpha="false" :modes="['hex']" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, strokeColor: $event || undefined } })" />
          <n-input :value="selected.text.strokeColor" placeholder="#000000" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, strokeColor: $event || undefined } })" />
        </div>
      </n-form-item>
      <n-form-item label="描边宽">
        <n-input-number :value="selected.text.strokeWidth" :min="0" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, strokeWidth: $event ?? 0 } })" />
      </n-form-item>
      <n-form-item label="阴影色">
        <div class="template-color-field">
          <n-color-picker :value="selected.text.shadowColor ?? null" :show-alpha="false" :modes="['hex']" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, shadowColor: $event || undefined } })" />
          <n-input :value="selected.text.shadowColor" placeholder="#000000" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, shadowColor: $event || undefined } })" />
        </div>
      </n-form-item>
      <n-form-item label="阴影模糊">
        <n-input-number :value="selected.text.shadowBlur" :min="0" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, shadowBlur: $event ?? 0 } })" />
      </n-form-item>
      <n-form-item label="阴影 X">
        <n-input-number :value="selected.text.shadowOffsetX" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, shadowOffsetX: $event ?? 0 } })" />
      </n-form-item>
      <n-form-item label="阴影 Y">
        <n-input-number :value="selected.text.shadowOffsetY" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, shadowOffsetY: $event ?? 0 } })" />
      </n-form-item>
    </details>

    <section v-if="selected.type === 'shape' && selected.shape" class="template-prop-section">
      <h3>形状</h3>
      <n-form-item label="填充色">
        <div class="template-color-field">
          <n-color-picker :value="selected.shape.fill ?? null" :show-alpha="false" :modes="['hex']" :disabled="selected.locked" @update:value="patch({ shape: { ...selected.shape!, fill: $event || undefined } })" />
          <n-input :value="selected.shape.fill" :disabled="selected.locked" @update:value="patch({ shape: { ...selected.shape!, fill: $event } })" />
        </div>
      </n-form-item>
      <n-form-item label="描边色">
        <div class="template-color-field">
          <n-color-picker :value="selected.shape.stroke ?? null" :show-alpha="false" :modes="['hex']" :disabled="selected.locked" @update:value="patch({ shape: { ...selected.shape!, stroke: $event || undefined } })" />
          <n-input :value="selected.shape.stroke" :disabled="selected.locked" @update:value="patch({ shape: { ...selected.shape!, stroke: $event } })" />
        </div>
      </n-form-item>
      <n-form-item label="描边宽">
        <n-input-number :value="selected.shape.strokeWidth" :min="0" :disabled="selected.locked" @update:value="patch({ shape: { ...selected.shape!, strokeWidth: $event ?? 0 } })" />
      </n-form-item>
      <n-form-item label="圆角">
        <n-input-number :value="selected.shape.radius" :min="0" :disabled="selected.locked" @update:value="patch({ shape: { ...selected.shape!, radius: $event ?? 0 } })" />
      </n-form-item>
    </section>

    <section v-if="selected.type === 'image' && selected.image" class="template-prop-section">
      <h3>图片</h3>
      <n-form-item label="裁剪方式">
        <n-select :value="selected.image.fit" :options="fitOptions" :disabled="selected.locked" @update:value="patch({ image: { ...selected.image!, fit: $event as 'cover' | 'contain' | 'stretch' } })" />
      </n-form-item>
    </section>
  </n-form>
</template>
