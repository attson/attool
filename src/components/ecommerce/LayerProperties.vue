<script setup lang="ts">
import { computed } from 'vue';
import { NButton, NColorPicker, NEmpty, NForm, NInput, NInputNumber, NSelect, NSlider } from 'naive-ui';
import type { TemplateLayer, TextAlign, TextDecoration, TextFontStyle } from '../../types/ecommerceTemplate';

const props = defineProps<{ layer: TemplateLayer | null }>();
const emit = defineEmits<{ update: [layer: TemplateLayer]; 'batch-replace': [layer: TemplateLayer] }>();

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
  <n-form v-else class="template-prop-form" size="small">
    <section class="template-prop-section">
      <h3>基础</h3>
      <div class="template-prop-row">
        <label class="template-prop-field full">
          <span class="template-prop-label">图层名</span>
          <n-input :value="selected.name" :disabled="selected.locked" @update:value="patch({ name: $event })" />
        </label>
        <div class="template-prop-pair">
          <span class="template-prop-label">位置</span>
          <span class="template-prop-pair-cell">
            <span class="template-prop-pair-axis">X</span>
            <n-input-number :value="fixed2(selected.x)" :precision="2" :show-button="false" :disabled="selected.locked" @update:value="patch({ x: $event ?? 0 })" />
          </span>
          <span class="template-prop-pair-cell">
            <span class="template-prop-pair-axis">Y</span>
            <n-input-number :value="fixed2(selected.y)" :precision="2" :show-button="false" :disabled="selected.locked" @update:value="patch({ y: $event ?? 0 })" />
          </span>
        </div>
        <div class="template-prop-pair">
          <span class="template-prop-label">尺寸</span>
          <span class="template-prop-pair-cell">
            <span class="template-prop-pair-axis">W</span>
            <n-input-number :value="fixed2(selected.width)" :precision="2" :min="1" :show-button="false" :disabled="selected.locked" @update:value="patch({ width: $event ?? 1 })" />
          </span>
          <span class="template-prop-pair-cell">
            <span class="template-prop-pair-axis">H</span>
            <n-input-number :value="fixed2(selected.height)" :precision="2" :min="1" :show-button="false" :disabled="selected.locked" @update:value="patch({ height: $event ?? 1 })" />
          </span>
        </div>
        <label class="template-prop-field full">
          <span class="template-prop-label">透明度</span>
          <n-slider :value="selected.opacity" :min="0" :max="1" :step="0.01" :disabled="selected.locked" @update:value="patch({ opacity: Number($event) })" />
        </label>
        <label class="template-prop-field">
          <span class="template-prop-label">旋转</span>
          <n-input-number :value="selected.rotation" :disabled="selected.locked" @update:value="patch({ rotation: $event ?? 0 })" />
        </label>
      </div>
    </section>

    <section class="template-prop-section">
      <h3>绑定</h3>
      <div class="template-prop-row">
        <label class="template-prop-field full">
          <span class="template-prop-label">绑定字段</span>
          <n-input :value="selected.bindingKey" placeholder="例如 title" :disabled="selected.locked" @update:value="patch({ bindingKey: $event || undefined })" />
        </label>
      </div>
    </section>

    <section v-if="selected.type === 'text' && selected.text" class="template-prop-section">
      <h3>文字</h3>
      <div class="template-prop-row">
        <label class="template-prop-field full top">
          <span class="template-prop-label">文字内容</span>
          <n-input :value="selected.text.text" type="textarea" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, text: $event } })" />
        </label>
        <label class="template-prop-field full">
          <span class="template-prop-label">字体</span>
          <n-input :value="selected.text.fontFamily" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, fontFamily: $event } })" />
        </label>
        <label class="template-prop-field">
          <span class="template-prop-label">字号</span>
          <n-input-number :value="selected.text.fontSize" :min="1" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, fontSize: $event ?? 12 } })" />
        </label>
        <label class="template-prop-field">
          <span class="template-prop-label">字重</span>
          <n-input-number :value="Number(selected.text.fontWeight) || 400" :min="100" :max="900" :step="100" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, fontWeight: $event ?? 400 } })" />
        </label>
        <label class="template-prop-field">
          <span class="template-prop-label">样式</span>
          <n-select :value="selected.text.fontStyle ?? 'normal'" :options="fontStyleOptions" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, fontStyle: $event as TextFontStyle } })" />
        </label>
        <label class="template-prop-field">
          <span class="template-prop-label">装饰</span>
          <n-select :value="selected.text.textDecoration ?? 'none'" :options="decorationOptions" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, textDecoration: $event as TextDecoration } })" />
        </label>
        <label class="template-prop-field full">
          <span class="template-prop-label">颜色</span>
          <span class="template-color-field">
            <n-color-picker :value="selected.text.color" :show-alpha="false" :modes="['hex']" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, color: $event } })" />
            <n-input :value="selected.text.color" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, color: $event } })" />
          </span>
        </label>
      </div>
      <h4 class="template-prop-subtitle">排版</h4>
      <div class="template-prop-row">
        <label class="template-prop-field full">
          <span class="template-prop-label">对齐</span>
          <n-select :value="selected.text.align ?? 'left'" :options="alignOptions" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, align: $event as TextAlign } })" />
        </label>
        <label class="template-prop-field">
          <span class="template-prop-label">字距</span>
          <n-input-number :value="selected.text.letterSpacing" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, letterSpacing: $event ?? undefined } })" />
        </label>
        <label class="template-prop-field">
          <span class="template-prop-label">行高</span>
          <n-input-number :value="selected.text.lineHeight" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, lineHeight: $event ?? undefined } })" />
        </label>
        <n-button secondary class="template-prop-batch" @click="emit('batch-replace', selected!)">添加批量替换</n-button>
      </div>
    </section>

    <details v-if="selected.type === 'text' && selected.text" class="template-prop-section template-prop-advanced">
      <summary>填充 / 描边 / 阴影</summary>
      <div class="template-prop-row">
        <label class="template-prop-field full">
          <span class="template-prop-label">背景色</span>
          <span class="template-color-field">
            <n-color-picker :value="selected.text.backgroundColor ?? null" :show-alpha="false" :modes="['hex']" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, backgroundColor: $event || undefined } })" />
            <n-input :value="selected.text.backgroundColor" placeholder="#fff1b8" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, backgroundColor: $event || undefined } })" />
          </span>
        </label>
        <label class="template-prop-field">
          <span class="template-prop-label">圆角</span>
          <n-input-number :value="selected.text.backgroundRadius" :min="0" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, backgroundRadius: $event ?? 0 } })" />
        </label>
        <label class="template-prop-field full">
          <span class="template-prop-label">描边色</span>
          <span class="template-color-field">
            <n-color-picker :value="selected.text.strokeColor ?? null" :show-alpha="false" :modes="['hex']" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, strokeColor: $event || undefined } })" />
            <n-input :value="selected.text.strokeColor" placeholder="#000000" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, strokeColor: $event || undefined } })" />
          </span>
        </label>
        <label class="template-prop-field">
          <span class="template-prop-label">描边宽</span>
          <n-input-number :value="selected.text.strokeWidth" :min="0" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, strokeWidth: $event ?? 0 } })" />
        </label>
        <label class="template-prop-field full">
          <span class="template-prop-label">阴影色</span>
          <span class="template-color-field">
            <n-color-picker :value="selected.text.shadowColor ?? null" :show-alpha="false" :modes="['hex']" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, shadowColor: $event || undefined } })" />
            <n-input :value="selected.text.shadowColor" placeholder="#000000" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, shadowColor: $event || undefined } })" />
          </span>
        </label>
        <label class="template-prop-field">
          <span class="template-prop-label">模糊</span>
          <n-input-number :value="selected.text.shadowBlur" :min="0" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, shadowBlur: $event ?? 0 } })" />
        </label>
        <label class="template-prop-field">
          <span class="template-prop-label">阴影 X</span>
          <n-input-number :value="selected.text.shadowOffsetX" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, shadowOffsetX: $event ?? 0 } })" />
        </label>
        <label class="template-prop-field">
          <span class="template-prop-label">阴影 Y</span>
          <n-input-number :value="selected.text.shadowOffsetY" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, shadowOffsetY: $event ?? 0 } })" />
        </label>
      </div>
    </details>

    <section v-if="selected.type === 'shape' && selected.shape" class="template-prop-section">
      <h3>形状</h3>
      <div class="template-prop-row">
        <label class="template-prop-field full">
          <span class="template-prop-label">填充色</span>
          <span class="template-color-field">
            <n-color-picker :value="selected.shape.fill ?? null" :show-alpha="false" :modes="['hex']" :disabled="selected.locked" @update:value="patch({ shape: { ...selected.shape!, fill: $event || undefined } })" />
            <n-input :value="selected.shape.fill" :disabled="selected.locked" @update:value="patch({ shape: { ...selected.shape!, fill: $event } })" />
          </span>
        </label>
        <label class="template-prop-field full">
          <span class="template-prop-label">描边色</span>
          <span class="template-color-field">
            <n-color-picker :value="selected.shape.stroke ?? null" :show-alpha="false" :modes="['hex']" :disabled="selected.locked" @update:value="patch({ shape: { ...selected.shape!, stroke: $event || undefined } })" />
            <n-input :value="selected.shape.stroke" :disabled="selected.locked" @update:value="patch({ shape: { ...selected.shape!, stroke: $event } })" />
          </span>
        </label>
        <label class="template-prop-field">
          <span class="template-prop-label">描边宽</span>
          <n-input-number :value="selected.shape.strokeWidth" :min="0" :disabled="selected.locked" @update:value="patch({ shape: { ...selected.shape!, strokeWidth: $event ?? 0 } })" />
        </label>
        <label class="template-prop-field">
          <span class="template-prop-label">圆角</span>
          <n-input-number :value="selected.shape.radius" :min="0" :disabled="selected.locked" @update:value="patch({ shape: { ...selected.shape!, radius: $event ?? 0 } })" />
        </label>
      </div>
    </section>

    <section v-if="selected.type === 'image' && selected.image" class="template-prop-section">
      <h3>图片</h3>
      <div class="template-prop-row">
        <label class="template-prop-field full">
          <span class="template-prop-label">裁剪方式</span>
          <n-select :value="selected.image.fit" :options="fitOptions" :disabled="selected.locked" @update:value="patch({ image: { ...selected.image!, fit: $event as 'cover' | 'contain' | 'stretch' } })" />
        </label>
        <n-button secondary class="template-prop-batch" @click="emit('batch-replace', selected!)">添加批量替换</n-button>
      </div>
    </section>
  </n-form>
</template>
