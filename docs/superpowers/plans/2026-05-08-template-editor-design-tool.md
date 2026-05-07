# Template Editor Design Tool Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first version of a manual ecommerce template editor with left resource navigation, canvas editing, contextual properties, and preview/export parity for text, image, and shape styles.

**Architecture:** Keep state ownership in `TemplateTool.vue`, move resource selection into a new focused resource panel component, and keep reusable layer operations in pure TypeScript helpers with Vitest coverage. Extend the Rust model and renderer so every style exposed by the UI is persisted and exported to PNG.

**Tech Stack:** Vue 3 `<script setup>`, Naive UI, Vitest, Tauri 2, Rust `image`, `imageproc`, `ab_glyph`, `fontdb`.

---

## File Structure

- Create `src/components/ecommerce/TemplateResourcePanel.vue`: left rail and resource panel for Text, Images, Shapes, and Layers tabs.
- Modify `src/components/ecommerce/TemplateTool.vue`: editor layout state, layer insertion, image asset creation, toolbar action handling, resource panel wiring.
- Modify `src/components/ecommerce/TemplateCanvas.vue`: selected layer toolbar, lock handling, image fit preview, shape preview improvements.
- Modify `src/components/ecommerce/LayerProperties.vue`: expanded contextual property sections for basic, text, image, and shape controls.
- Modify `src/types/ecommerceTemplate.ts`: optional text style fields and text decoration/font style types.
- Modify `src/utils/ecommerceTemplate.ts`: pure layer factory, project mutation, reorder, style, and image fit helpers.
- Modify `src/utils/ecommerceTemplate.test.ts`: Vitest coverage for new pure helpers and preview styles.
- Modify `src/styles.css`: fixed workbench layout, rail/resource/property styling, canvas toolbar, material preset cards.
- Modify `src-tauri/src/ecommerce/models.rs`: optional text style fields for serde compatibility.
- Modify `src-tauri/src/ecommerce/render.rs`: export parity for text background, rounded background, stroke, shadow, decoration, opacity, shape stroke, and image fit modes.
- Modify `src-tauri/tests/ecommerce_render.rs`: rendering coverage for text visual styles, shape stroke/fill, opacity, and image fit.

---

### Task 1: Extend TypeScript Types And Pure Layer Factories

**Files:**
- Modify: `src/types/ecommerceTemplate.ts`
- Modify: `src/utils/ecommerceTemplate.ts`
- Modify: `src/utils/ecommerceTemplate.test.ts`

- [ ] **Step 1: Write failing tests for layer factories and insertion**

Add these imports in `src/utils/ecommerceTemplate.test.ts`:

```ts
import type { TemplateAsset, TemplateProject } from '../types/ecommerceTemplate';
import {
  collectBindingKeys,
  createImageLayer,
  createShapeLayer,
  createTemplateAsset,
  createTextLayer,
  extractBindingKey,
  flattenLayers,
  insertLayer,
  makeExportFileName,
  textLayerPreviewStyle,
  validateBatchFields
} from './ecommerceTemplate';
```

Add this helper after the `layers` constant:

```ts
function makeProject(): TemplateProject {
  return {
    id: 'tpl-test',
    name: '测试模板',
    canvasWidth: 1000,
    canvasHeight: 1000,
    layers: [...layers],
    assets: [],
    createdAt: '2026-05-08 00:00:00',
    updatedAt: '2026-05-08 00:00:00'
  };
}
```

Add these tests before the filename test:

```ts
it('creates default text, shape, image assets, and image layers', () => {
  const textLayer = createTextLayer({ canvasWidth: 1000, canvasHeight: 1000, preset: 'title' });
  expect(textLayer).toMatchObject({
    type: 'text',
    visible: true,
    opacity: 1,
    rotation: 0,
    width: 420,
    height: 96,
    text: {
      text: '双击编辑标题',
      fontSize: 64,
      fontWeight: 800,
      fontStyle: 'normal',
      textDecoration: 'none',
      backgroundRadius: 0,
      shadowBlur: 0,
      shadowOffsetX: 0,
      shadowOffsetY: 0
    }
  });
  expect(textLayer.x).toBe(290);
  expect(textLayer.y).toBe(452);

  const shapeLayer = createShapeLayer({ canvasWidth: 1000, canvasHeight: 1000, shape: 'roundRect' });
  expect(shapeLayer).toMatchObject({
    type: 'shape',
    x: 350,
    y: 420,
    width: 300,
    height: 160,
    shape: { shape: 'roundRect', fill: '#f5d36b', stroke: '#17211b', strokeWidth: 0, radius: 24 }
  });

  const asset = createTemplateAsset({ path: '/tmp/chair.png', name: 'chair.png', width: 640, height: 480 });
  expect(asset).toMatchObject({ name: 'chair.png', path: '/tmp/chair.png', mimeType: 'image/png', width: 640, height: 480 });
  expect(asset.id).toMatch(/^asset-/);

  const imageLayer = createImageLayer({ canvasWidth: 1000, canvasHeight: 1000, asset });
  expect(imageLayer).toMatchObject({
    type: 'image',
    x: 250,
    y: 250,
    width: 500,
    height: 500,
    image: { assetId: asset.id, fit: 'contain', replaceable: true }
  });
});

it('inserts layers immutably at the top of paint order', () => {
  const project = makeProject();
  const layer = createTextLayer({ canvasWidth: project.canvasWidth, canvasHeight: project.canvasHeight, preset: 'body' });
  const next = insertLayer(project, layer);

  expect(next).not.toBe(project);
  expect(next.layers.at(-1)).toEqual(layer);
  expect(project.layers).toHaveLength(2);
  expect(next.layers).toHaveLength(3);
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
npm test -- src/utils/ecommerceTemplate.test.ts
```

Expected: FAIL with TypeScript/Vitest errors that `createTextLayer`, `createShapeLayer`, `createTemplateAsset`, `createImageLayer`, and `insertLayer` are not exported.

- [ ] **Step 3: Extend TypeScript layer style types**

Modify `src/types/ecommerceTemplate.ts` near the existing text types:

```ts
export type TextAlign = 'left' | 'center' | 'right';
export type TextFontStyle = 'normal' | 'italic';
export type TextDecoration = 'none' | 'underline' | 'line-through';
```

Extend `TextLayerData`:

```ts
export type TextLayerData = {
  text: string;
  fontFamily: string;
  fontSize: number;
  fontWeight: number | string;
  color: string;
  strokeColor?: string;
  strokeWidth?: number;
  letterSpacing?: number;
  lineHeight?: number;
  align?: TextAlign;
  fontStyle?: TextFontStyle;
  textDecoration?: TextDecoration;
  backgroundColor?: string;
  backgroundRadius?: number;
  shadowColor?: string;
  shadowBlur?: number;
  shadowOffsetX?: number;
  shadowOffsetY?: number;
};
```

- [ ] **Step 4: Implement minimal layer factory helpers**

Add these imports and helpers to `src/utils/ecommerceTemplate.ts`:

```ts
import type { ShapeKind, TemplateAsset, TemplateLayer, TemplateProject } from '../types/ecommerceTemplate';
```

If the file already imports `TemplateLayer`, replace that import with the combined import above.

Add these helpers after `collectBindingKeys`:

```ts
type CanvasSize = { canvasWidth: number; canvasHeight: number };
type TextPreset = 'title' | 'subtitle' | 'body' | 'price';

const textPresets: Record<TextPreset, Pick<TemplateLayer, 'name' | 'width' | 'height'> & { text: TemplateLayer['text'] }> = {
  title: {
    name: '标题文字',
    width: 420,
    height: 96,
    text: {
      text: '双击编辑标题',
      fontFamily: 'PingFang SC',
      fontSize: 64,
      fontWeight: 800,
      color: '#111111',
      align: 'left',
      fontStyle: 'normal',
      textDecoration: 'none',
      backgroundRadius: 0,
      shadowBlur: 0,
      shadowOffsetX: 0,
      shadowOffsetY: 0
    }
  },
  subtitle: {
    name: '副标题文字',
    width: 360,
    height: 64,
    text: {
      text: '输入副标题',
      fontFamily: 'PingFang SC',
      fontSize: 36,
      fontWeight: 700,
      color: '#333333',
      align: 'left',
      fontStyle: 'normal',
      textDecoration: 'none',
      backgroundRadius: 0,
      shadowBlur: 0,
      shadowOffsetX: 0,
      shadowOffsetY: 0
    }
  },
  body: {
    name: '正文文字',
    width: 320,
    height: 48,
    text: {
      text: '输入正文',
      fontFamily: 'PingFang SC',
      fontSize: 24,
      fontWeight: 500,
      color: '#333333',
      align: 'left',
      fontStyle: 'normal',
      textDecoration: 'none',
      backgroundRadius: 0,
      shadowBlur: 0,
      shadowOffsetX: 0,
      shadowOffsetY: 0
    }
  },
  price: {
    name: '价格文字',
    width: 280,
    height: 72,
    text: {
      text: '¥99',
      fontFamily: 'PingFang SC',
      fontSize: 52,
      fontWeight: 900,
      color: '#d63f2f',
      align: 'left',
      fontStyle: 'normal',
      textDecoration: 'none',
      backgroundRadius: 0,
      shadowBlur: 0,
      shadowOffsetX: 0,
      shadowOffsetY: 0
    }
  }
};

function centeredPosition(canvas: CanvasSize, width: number, height: number) {
  return {
    x: Math.max(0, Math.round((canvas.canvasWidth - width) / 2)),
    y: Math.max(0, Math.round((canvas.canvasHeight - height) / 2))
  };
}

export function createTextLayer(options: CanvasSize & { preset: TextPreset }): TemplateLayer {
  const preset = textPresets[options.preset];
  const position = centeredPosition(options, preset.width, preset.height);
  return {
    id: `layer-${crypto.randomUUID()}`,
    name: preset.name,
    type: 'text',
    ...position,
    width: preset.width,
    height: preset.height,
    visible: true,
    opacity: 1,
    rotation: 0,
    locked: false,
    text: { ...preset.text! }
  };
}

export function createShapeLayer(options: CanvasSize & { shape: ShapeKind }): TemplateLayer {
  const width = options.shape === 'line' ? 360 : 300;
  const height = options.shape === 'line' ? 8 : 160;
  const position = centeredPosition(options, width, height);
  return {
    id: `layer-${crypto.randomUUID()}`,
    name: options.shape === 'line' ? '线条' : '形状',
    type: 'shape',
    ...position,
    width,
    height,
    visible: true,
    opacity: 1,
    rotation: 0,
    locked: false,
    shape: {
      shape: options.shape,
      fill: options.shape === 'line' ? '#17211b' : '#f5d36b',
      stroke: '#17211b',
      strokeWidth: 0,
      radius: options.shape === 'roundRect' ? 24 : 0
    }
  };
}

export function createTemplateAsset(input: { path: string; name: string; width: number; height: number; mimeType?: string }): TemplateAsset {
  return {
    id: `asset-${crypto.randomUUID()}`,
    name: input.name,
    path: input.path,
    mimeType: input.mimeType ?? 'image/png',
    width: input.width,
    height: input.height
  };
}

export function createImageLayer(options: CanvasSize & { asset: TemplateAsset }): TemplateLayer {
  const size = Math.round(Math.min(options.canvasWidth, options.canvasHeight) * 0.5);
  const position = centeredPosition(options, size, size);
  return {
    id: `layer-${crypto.randomUUID()}`,
    name: options.asset.name,
    type: 'image',
    ...position,
    width: size,
    height: size,
    visible: true,
    opacity: 1,
    rotation: 0,
    locked: false,
    image: { assetId: options.asset.id, fit: 'contain', replaceable: true }
  };
}

export function insertLayer(project: TemplateProject, layer: TemplateLayer): TemplateProject {
  return { ...project, layers: [...project.layers, layer], updatedAt: new Date().toLocaleString() };
}
```

- [ ] **Step 5: Run focused tests**

Run:

```bash
npm test -- src/utils/ecommerceTemplate.test.ts
```

Expected: PASS for all tests in `src/utils/ecommerceTemplate.test.ts`.

- [ ] **Step 6: Commit Task 1**

```bash
git add src/types/ecommerceTemplate.ts src/utils/ecommerceTemplate.ts src/utils/ecommerceTemplate.test.ts
git commit -m "feat: add template layer factory helpers"
```

---

### Task 2: Add Project Mutation Helpers For Toolbar Actions

**Files:**
- Modify: `src/utils/ecommerceTemplate.ts`
- Modify: `src/utils/ecommerceTemplate.test.ts`

- [ ] **Step 1: Write failing tests for duplicate, delete, update, and reorder**

Add these imports in `src/utils/ecommerceTemplate.test.ts`:

```ts
import {
  deleteLayerById,
  duplicateLayer,
  moveLayer,
  updateLayerById
} from './ecommerceTemplate';
```

Merge those names into the existing import from `./ecommerceTemplate` instead of creating a second import.

Add these tests before the filename test:

```ts
it('updates, deletes, duplicates, and reorders layers immutably', () => {
  const project = makeProject();
  const updated = updateLayerById(project.layers, 'text-1', (layer) => ({ ...layer, name: '新标题' }));
  expect(flattenLayers(updated).find((layer) => layer.id === 'text-1')?.name).toBe('新标题');
  expect(flattenLayers(project.layers).find((layer) => layer.id === 'text-1')?.name).toBe('{{title}} 大标题');

  const duplicated = duplicateLayer(project.layers, 'image-1');
  expect(duplicated).toHaveLength(3);
  expect(duplicated[2]).toMatchObject({ name: '{{product_image}} 商品图 副本', type: 'image' });
  expect(duplicated[2].id).not.toBe('image-1');

  const moved = moveLayer(duplicated, duplicated[2].id, 'backward');
  expect(moved.map((layer) => layer.id)).toEqual(['group-1', duplicated[2].id, 'image-1']);

  const deleted = deleteLayerById(moved, duplicated[2].id);
  expect(deleted.map((layer) => layer.id)).toEqual(['group-1', 'image-1']);
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
npm test -- src/utils/ecommerceTemplate.test.ts
```

Expected: FAIL with missing exports for `updateLayerById`, `deleteLayerById`, `duplicateLayer`, and `moveLayer`.

- [ ] **Step 3: Implement mutation helpers**

Add to `src/utils/ecommerceTemplate.ts` after `insertLayer`:

```ts
export function updateLayerById(layers: TemplateLayer[], layerId: string, updater: (layer: TemplateLayer) => TemplateLayer): TemplateLayer[] {
  return layers.map((layer) => {
    if (layer.id === layerId) return updater(layer);
    if (layer.children) return { ...layer, children: updateLayerById(layer.children, layerId, updater) };
    return layer;
  });
}

export function deleteLayerById(layers: TemplateLayer[], layerId: string): TemplateLayer[] {
  return layers
    .filter((layer) => layer.id !== layerId)
    .map((layer) => (layer.children ? { ...layer, children: deleteLayerById(layer.children, layerId) } : layer));
}

function cloneLayer(layer: TemplateLayer): TemplateLayer {
  return {
    ...layer,
    id: `layer-${crypto.randomUUID()}`,
    name: `${layer.name} 副本`,
    x: layer.x + 24,
    y: layer.y + 24,
    children: layer.children?.map(cloneLayer),
    text: layer.text ? { ...layer.text } : undefined,
    image: layer.image ? { ...layer.image } : undefined,
    shape: layer.shape ? { ...layer.shape } : undefined
  };
}

export function duplicateLayer(layers: TemplateLayer[], layerId: string): TemplateLayer[] {
  const result: TemplateLayer[] = [];
  for (const layer of layers) {
    result.push(layer);
    if (layer.id === layerId) {
      result.push(cloneLayer(layer));
      continue;
    }
    if (layer.children) {
      result[result.length - 1] = { ...layer, children: duplicateLayer(layer.children, layerId) };
    }
  }
  return result;
}

export type LayerMoveDirection = 'forward' | 'backward' | 'front' | 'back';

export function moveLayer(layers: TemplateLayer[], layerId: string, direction: LayerMoveDirection): TemplateLayer[] {
  const index = layers.findIndex((layer) => layer.id === layerId);
  if (index >= 0) {
    const next = [...layers];
    const [layer] = next.splice(index, 1);
    const target = direction === 'front' ? next.length : direction === 'back' ? 0 : direction === 'forward' ? Math.min(next.length, index + 1) : Math.max(0, index - 1);
    next.splice(target, 0, layer);
    return next;
  }
  return layers.map((layer) => (layer.children ? { ...layer, children: moveLayer(layer.children, layerId, direction) } : layer));
}
```

- [ ] **Step 4: Run focused tests**

Run:

```bash
npm test -- src/utils/ecommerceTemplate.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit Task 2**

```bash
git add src/utils/ecommerceTemplate.ts src/utils/ecommerceTemplate.test.ts
git commit -m "feat: add template layer mutation helpers"
```

---

### Task 3: Expand Preview Style Helpers

**Files:**
- Modify: `src/utils/ecommerceTemplate.ts`
- Modify: `src/utils/ecommerceTemplate.test.ts`

- [ ] **Step 1: Write failing preview style test**

Add this test before the filename test:

```ts
it('maps complete text styles to preview CSS', () => {
  const textLayer: TemplateLayer = {
    ...layers[0].children![0],
    text: {
      ...layers[0].children![0].text!,
      fontStyle: 'italic',
      textDecoration: 'underline',
      backgroundColor: '#fff1b8',
      backgroundRadius: 16,
      shadowColor: '#000000',
      shadowBlur: 8,
      shadowOffsetX: 3,
      shadowOffsetY: 4
    }
  };

  expect(textLayerPreviewStyle(textLayer, 0.5)).toMatchObject({
    fontStyle: 'italic',
    textDecoration: 'underline',
    backgroundColor: '#fff1b8',
    borderRadius: '8px',
    textShadow: '1.5px 2px 4px #000000'
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
npm test -- src/utils/ecommerceTemplate.test.ts
```

Expected: FAIL because `textLayerPreviewStyle` does not return `fontStyle`, `textDecoration`, `backgroundColor`, `borderRadius`, or `textShadow`.

- [ ] **Step 3: Implement preview style mapping**

Modify `textLayerPreviewStyle` in `src/utils/ecommerceTemplate.ts` so the returned object includes:

```ts
    fontStyle: text?.fontStyle,
    textDecoration: text?.textDecoration && text.textDecoration !== 'none' ? text.textDecoration : undefined,
    backgroundColor: text?.backgroundColor,
    borderRadius: scaledPx(text?.backgroundRadius, scale),
    textShadow:
      text?.shadowColor && ((text.shadowBlur ?? 0) > 0 || (text.shadowOffsetX ?? 0) !== 0 || (text.shadowOffsetY ?? 0) !== 0)
        ? `${Number(((text.shadowOffsetX ?? 0) * scale).toFixed(4))}px ${Number(((text.shadowOffsetY ?? 0) * scale).toFixed(4))}px ${Number(((text.shadowBlur ?? 0) * scale).toFixed(4))}px ${text.shadowColor}`
        : undefined,
```

Keep existing `WebkitTextStroke`, `lineHeight`, and normalized letter spacing behavior intact.

- [ ] **Step 4: Run focused tests**

Run:

```bash
npm test -- src/utils/ecommerceTemplate.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit Task 3**

```bash
git add src/utils/ecommerceTemplate.ts src/utils/ecommerceTemplate.test.ts
git commit -m "feat: preview complete text styles"
```

---

### Task 4: Build Resource Panel And Fixed Editor Layout

**Files:**
- Create: `src/components/ecommerce/TemplateResourcePanel.vue`
- Modify: `src/components/ecommerce/TemplateTool.vue`
- Modify: `src/styles.css`

- [ ] **Step 1: Create resource panel component**

Create `src/components/ecommerce/TemplateResourcePanel.vue`:

```vue
<script setup lang="ts">
import { NButton, NEmpty, NSpace } from 'naive-ui';
import type { ShapeKind, TemplateLayer } from '../../types/ecommerceTemplate';
import LayerTree from './LayerTree.vue';

export type ResourceTab = 'text' | 'image' | 'shape' | 'layers';

const props = defineProps<{
  activeTab: ResourceTab;
  layers: TemplateLayer[];
  selectedLayerId: string | null;
}>();

const emit = defineEmits<{
  'update:activeTab': [tab: ResourceTab];
  'add-text': [preset: 'title' | 'subtitle' | 'body' | 'price'];
  'add-shape': [shape: ShapeKind];
  'add-image': [];
  select: [layerId: string];
  update: [layer: TemplateLayer];
}>();

const tabs: { key: ResourceTab; label: string; icon: string }[] = [
  { key: 'text', label: '文字', icon: 'T' },
  { key: 'image', label: '图片', icon: '图' },
  { key: 'shape', label: '素材', icon: '形' },
  { key: 'layers', label: '图层', icon: '层' }
];

const textPresets = [
  { key: 'title', title: '标题', sample: '双击编辑标题' },
  { key: 'subtitle', title: '副标题', sample: '输入副标题' },
  { key: 'body', title: '正文', sample: '输入正文' },
  { key: 'price', title: '价格', sample: '¥99' }
] as const;

const shapePresets: { key: ShapeKind; title: string }[] = [
  { key: 'rect', title: '矩形' },
  { key: 'roundRect', title: '圆角矩形' },
  { key: 'ellipse', title: '椭圆/圆形' },
  { key: 'line', title: '线条' }
];
</script>

<template>
  <div class="template-workbench-rail">
    <button v-for="tab in tabs" :key="tab.key" type="button" :class="['template-rail-button', { active: activeTab === tab.key }]" @click="emit('update:activeTab', tab.key)">
      <b>{{ tab.icon }}</b>
      <span>{{ tab.label }}</span>
    </button>
  </div>

  <aside class="template-resource-panel">
    <template v-if="props.activeTab === 'text'">
      <div class="template-resource-heading">
        <h3>添加文字</h3>
        <p>选择一个文字样式插入画布</p>
      </div>
      <div class="template-preset-grid">
        <button v-for="preset in textPresets" :key="preset.key" type="button" class="template-text-preset" @click="emit('add-text', preset.key)">
          <strong>{{ preset.title }}</strong>
          <span>{{ preset.sample }}</span>
        </button>
      </div>
    </template>

    <template v-else-if="props.activeTab === 'image'">
      <div class="template-resource-heading">
        <h3>添加图片</h3>
        <p>导入本地图片作为模板素材</p>
      </div>
      <n-button type="primary" block @click="emit('add-image')">选择本地图片</n-button>
    </template>

    <template v-else-if="props.activeTab === 'shape'">
      <div class="template-resource-heading">
        <h3>素材 / 形状</h3>
        <p>插入基础形状搭建模板</p>
      </div>
      <div class="template-preset-grid">
        <button v-for="shape in shapePresets" :key="shape.key" type="button" class="template-shape-preset" @click="emit('add-shape', shape.key)">
          <span :class="['shape-preview', shape.key]" />
          <strong>{{ shape.title }}</strong>
        </button>
      </div>
    </template>

    <template v-else>
      <div class="template-resource-heading">
        <h3>图层</h3>
        <p>选择和管理当前模板图层</p>
      </div>
      <LayerTree :layers="props.layers" :selected-layer-id="props.selectedLayerId" @select="emit('select', $event)" @update="emit('update', $event)" />
      <n-empty v-if="!props.layers.length" description="暂无图层" />
    </template>
  </aside>
</template>
```

- [ ] **Step 2: Wire fixed layout in TemplateTool**

Modify imports in `src/components/ecommerce/TemplateTool.vue`:

```ts
import { computed, onMounted, ref } from 'vue';
import { basename } from '@tauri-apps/api/path';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { NAlert, NButton, NCard, NPageHeader, NSpace, NTag } from 'naive-ui';
import type { ShapeKind, TemplateAsset, TemplateLayer, TemplateProject, TemplateSummary } from '../../types/ecommerceTemplate';
import {
  collectBindingKeys,
  createImageLayer,
  createShapeLayer,
  createTemplateAsset,
  createTextLayer,
  flattenLayers,
  insertLayer,
  updateLayerById
} from '../../utils/ecommerceTemplate';
import LayerProperties from './LayerProperties.vue';
import TemplateResourcePanel, { type ResourceTab } from './TemplateResourcePanel.vue';
import TemplateCanvas from './TemplateCanvas.vue';
import BatchPanel from './BatchPanel.vue';
import { createEmptyTemplateProject } from './templateDefaults';
```

Add state:

```ts
const activeResourceTab = ref<ResourceTab>('text');
```

Replace `updateLayer` with:

```ts
function touch(next: TemplateProject): TemplateProject {
  return { ...next, updatedAt: new Date().toLocaleString() };
}

function updateLayer(updated: TemplateLayer) {
  project.value = touch({ ...project.value, layers: updateLayerById(project.value.layers, updated.id, () => updated) });
}
```

Add insertion handlers:

```ts
function addTextLayer(preset: 'title' | 'subtitle' | 'body' | 'price') {
  const layer = createTextLayer({ canvasWidth: project.value.canvasWidth, canvasHeight: project.value.canvasHeight, preset });
  project.value = insertLayer(project.value, layer);
  selectedLayerId.value = layer.id;
}

function addShapeLayer(shape: ShapeKind) {
  const layer = createShapeLayer({ canvasWidth: project.value.canvasWidth, canvasHeight: project.value.canvasHeight, shape });
  project.value = insertLayer(project.value, layer);
  selectedLayerId.value = layer.id;
}

async function addImageLayer() {
  notice.value = '';
  try {
    const selected = await open({ multiple: false, filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp'] }] });
    if (typeof selected !== 'string') return;
    const name = await basename(selected);
    const asset: TemplateAsset = createTemplateAsset({ path: selected, name, width: 1, height: 1 });
    const layer = createImageLayer({ canvasWidth: project.value.canvasWidth, canvasHeight: project.value.canvasHeight, asset });
    project.value = touch({ ...insertLayer(project.value, layer), assets: [...project.value.assets, asset] });
    selectedLayerId.value = layer.id;
  } catch (error) {
    notice.value = String(error);
  }
}
```

Replace the `<n-grid>` editor section with:

```vue
<div class="template-workbench">
  <TemplateResourcePanel
    v-model:active-tab="activeResourceTab"
    :layers="project.layers"
    :selected-layer-id="selectedLayerId"
    @add-text="addTextLayer"
    @add-shape="addShapeLayer"
    @add-image="addImageLayer"
    @select="selectLayer"
    @update="updateLayer"
  />

  <n-card title="画布" size="small" :bordered="false" class="panel-card template-canvas-card">
    <TemplateCanvas :canvas-width="project.canvasWidth" :canvas-height="project.canvasHeight" :layers="project.layers" :assets="project.assets" :selected-layer-id="selectedLayerId" @select="selectLayer" @update="updateLayer" />
  </n-card>

  <n-card title="属性" size="small" :bordered="false" class="panel-card template-editor-panel">
    <LayerProperties :layer="selectedLayer" @update="updateLayer" />
  </n-card>
</div>
```

- [ ] **Step 3: Add layout styles**

Append to `src/styles.css` near the existing ecommerce template styles:

```css
.template-workbench {
  display: grid;
  grid-template-columns: 64px minmax(220px, 280px) minmax(0, 1fr) minmax(260px, 320px);
  gap: 12px;
  align-items: stretch;
}

.template-workbench-rail,
.template-resource-panel {
  border-radius: 18px;
  background: rgba(255, 250, 238, 0.78);
  box-shadow: 0 12px 32px rgba(23, 33, 27, 0.08);
}

.template-workbench-rail {
  display: grid;
  gap: 8px;
  align-content: start;
  padding: 10px 8px;
}

.template-rail-button {
  display: grid;
  place-items: center;
  gap: 3px;
  min-height: 54px;
  border: 0;
  border-radius: 14px;
  background: transparent;
  color: var(--muted);
  cursor: pointer;
}

.template-rail-button.active {
  background: var(--ink);
  color: #fff7e9;
}

.template-rail-button span {
  font-size: 0.72rem;
}

.template-resource-panel {
  min-height: 72vh;
  max-height: 72vh;
  overflow: auto;
  padding: 14px;
}

.template-resource-heading h3 {
  margin: 0;
  font-size: 1rem;
}

.template-resource-heading p {
  margin: 4px 0 12px;
  color: var(--muted);
  font-size: 0.82rem;
}

.template-preset-grid {
  display: grid;
  gap: 10px;
}

.template-text-preset,
.template-shape-preset {
  display: grid;
  gap: 6px;
  border: 1px solid rgba(23, 33, 27, 0.08);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.72);
  color: var(--ink);
  padding: 12px;
  text-align: left;
  cursor: pointer;
}

.template-text-preset span {
  color: var(--muted);
}

.shape-preview {
  display: block;
  width: 56px;
  height: 38px;
  background: #f5d36b;
  border: 2px solid #17211b;
}

.shape-preview.roundRect { border-radius: 12px; }
.shape-preview.ellipse { border-radius: 999px; }
.shape-preview.line { height: 4px; margin: 17px 0; }

@media (max-width: 1100px) {
  .template-workbench {
    grid-template-columns: 58px minmax(200px, 260px) minmax(0, 1fr);
  }

  .template-workbench > .template-editor-panel {
    grid-column: 1 / -1;
  }
}
```

- [ ] **Step 4: Run build**

Run:

```bash
npm run build
```

Expected: TypeScript and Vite build succeed.

- [ ] **Step 5: Commit Task 4**

```bash
git add src/components/ecommerce/TemplateResourcePanel.vue src/components/ecommerce/TemplateTool.vue src/styles.css
git commit -m "feat: add template editor resource workbench"
```

---

### Task 5: Expand Layer Properties Panel

**Files:**
- Modify: `src/components/ecommerce/LayerProperties.vue`

- [ ] **Step 1: Replace the properties script**

Replace the `<script setup lang="ts">` block in `src/components/ecommerce/LayerProperties.vue` with:

```vue
<script setup lang="ts">
import { computed } from 'vue';
import { NCheckbox, NEmpty, NForm, NFormItem, NInput, NInputNumber, NSelect, NSlider, NSwitch } from 'naive-ui';
import type { TemplateLayer, TextDecoration, TextFontStyle } from '../../types/ecommerceTemplate';

const props = defineProps<{ layer: TemplateLayer | null }>();
const emit = defineEmits<{ update: [layer: TemplateLayer] }>();

const selected = computed(() => props.layer);
const fitOptions = [
  { label: '覆盖', value: 'cover' },
  { label: '完整显示', value: 'contain' },
  { label: '拉伸', value: 'stretch' }
];
const alignOptions = [
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
</script>
```

- [ ] **Step 2: Replace the properties template**

Replace the `<template>` in `src/components/ecommerce/LayerProperties.vue` with:

```vue
<template>
  <n-empty v-if="!selected" description="请选择一个图层" />
  <n-form v-else label-placement="top" size="small">
    <section class="template-prop-section">
      <h3>基础</h3>
      <n-form-item label="图层名">
        <n-input :value="selected.name" :disabled="selected.locked" @update:value="patch({ name: $event })" />
      </n-form-item>
      <n-form-item label="X / Y / 宽 / 高">
        <div class="template-prop-grid">
          <n-input-number :value="selected.x" :disabled="selected.locked" @update:value="patch({ x: $event ?? 0 })" />
          <n-input-number :value="selected.y" :disabled="selected.locked" @update:value="patch({ y: $event ?? 0 })" />
          <n-input-number :value="selected.width" :min="1" :disabled="selected.locked" @update:value="patch({ width: $event ?? 1 })" />
          <n-input-number :value="selected.height" :min="1" :disabled="selected.locked" @update:value="patch({ height: $event ?? 1 })" />
        </div>
      </n-form-item>
      <n-form-item label="透明度">
        <n-slider :value="selected.opacity" :min="0" :max="1" :step="0.01" :disabled="selected.locked" @update:value="patch({ opacity: Number($event) })" />
      </n-form-item>
      <n-form-item label="旋转">
        <n-input-number :value="selected.rotation" :disabled="selected.locked" @update:value="patch({ rotation: $event ?? 0 })" />
      </n-form-item>
      <div class="template-switch-row">
        <n-checkbox :checked="selected.visible" @update:checked="patch({ visible: Boolean($event) })">显示</n-checkbox>
        <n-checkbox :checked="Boolean(selected.locked)" @update:checked="emit('update', { ...selected, locked: Boolean($event) })">锁定</n-checkbox>
      </div>
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
      <n-form-item label="字号 / 字重">
        <div class="template-prop-grid">
          <n-input-number :value="selected.text.fontSize" :min="1" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, fontSize: $event ?? 12 } })" />
          <n-input-number :value="Number(selected.text.fontWeight) || 400" :min="100" :max="900" :step="100" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, fontWeight: $event ?? 400 } })" />
        </div>
      </n-form-item>
      <n-form-item label="样式 / 装饰">
        <div class="template-prop-grid">
          <n-select :value="selected.text.fontStyle ?? 'normal'" :options="fontStyleOptions" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, fontStyle: $event } })" />
          <n-select :value="selected.text.textDecoration ?? 'none'" :options="decorationOptions" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, textDecoration: $event } })" />
        </div>
      </n-form-item>
      <n-form-item label="颜色 / 对齐">
        <div class="template-prop-grid">
          <n-input :value="selected.text.color" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, color: $event } })" />
          <n-select :value="selected.text.align ?? 'left'" :options="alignOptions" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, align: $event } })" />
        </div>
      </n-form-item>
      <n-form-item label="字距 / 行高">
        <div class="template-prop-grid">
          <n-input-number :value="selected.text.letterSpacing" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, letterSpacing: $event ?? undefined } })" />
          <n-input-number :value="selected.text.lineHeight" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, lineHeight: $event ?? undefined } })" />
        </div>
      </n-form-item>
    </section>

    <section v-if="selected.type === 'text' && selected.text" class="template-prop-section">
      <h3>填充 / 描边 / 阴影</h3>
      <n-form-item label="背景色 / 圆角">
        <div class="template-prop-grid">
          <n-input :value="selected.text.backgroundColor" placeholder="#fff1b8" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, backgroundColor: $event || undefined } })" />
          <n-input-number :value="selected.text.backgroundRadius" :min="0" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, backgroundRadius: $event ?? 0 } })" />
        </div>
      </n-form-item>
      <n-form-item label="描边色 / 描边宽">
        <div class="template-prop-grid">
          <n-input :value="selected.text.strokeColor" placeholder="#000000" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, strokeColor: $event || undefined } })" />
          <n-input-number :value="selected.text.strokeWidth" :min="0" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, strokeWidth: $event ?? 0 } })" />
        </div>
      </n-form-item>
      <n-form-item label="阴影色 / 模糊">
        <div class="template-prop-grid">
          <n-input :value="selected.text.shadowColor" placeholder="#000000" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, shadowColor: $event || undefined } })" />
          <n-input-number :value="selected.text.shadowBlur" :min="0" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, shadowBlur: $event ?? 0 } })" />
        </div>
      </n-form-item>
      <n-form-item label="阴影 X / Y">
        <div class="template-prop-grid">
          <n-input-number :value="selected.text.shadowOffsetX" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, shadowOffsetX: $event ?? 0 } })" />
          <n-input-number :value="selected.text.shadowOffsetY" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, shadowOffsetY: $event ?? 0 } })" />
        </div>
      </n-form-item>
    </section>

    <section v-if="selected.type === 'shape' && selected.shape" class="template-prop-section">
      <h3>形状</h3>
      <n-form-item label="填充 / 描边">
        <div class="template-prop-grid">
          <n-input :value="selected.shape.fill" :disabled="selected.locked" @update:value="patch({ shape: { ...selected.shape!, fill: $event } })" />
          <n-input :value="selected.shape.stroke" :disabled="selected.locked" @update:value="patch({ shape: { ...selected.shape!, stroke: $event } })" />
        </div>
      </n-form-item>
      <n-form-item label="描边宽 / 圆角">
        <div class="template-prop-grid">
          <n-input-number :value="selected.shape.strokeWidth" :min="0" :disabled="selected.locked" @update:value="patch({ shape: { ...selected.shape!, strokeWidth: $event ?? 0 } })" />
          <n-input-number :value="selected.shape.radius" :min="0" :disabled="selected.locked" @update:value="patch({ shape: { ...selected.shape!, radius: $event ?? 0 } })" />
        </div>
      </n-form-item>
    </section>

    <section v-if="selected.type === 'image' && selected.image" class="template-prop-section">
      <h3>图片</h3>
      <n-form-item label="裁剪方式">
        <n-select :value="selected.image.fit" :options="fitOptions" :disabled="selected.locked" @update:value="patch({ image: { ...selected.image!, fit: $event } })" />
      </n-form-item>
    </section>
  </n-form>
</template>
```

- [ ] **Step 3: Add property styles**

Append to `src/styles.css`:

```css
.template-prop-section {
  display: grid;
  gap: 8px;
  border-bottom: 1px solid rgba(23, 33, 27, 0.08);
  padding-bottom: 12px;
  margin-bottom: 12px;
}

.template-prop-section h3 {
  margin: 0;
  color: var(--ink);
  font-size: 0.92rem;
  font-weight: 900;
}

.template-switch-row {
  display: flex;
  gap: 12px;
  align-items: center;
}
```

- [ ] **Step 4: Run build**

Run:

```bash
npm run build
```

Expected: PASS.

- [ ] **Step 5: Commit Task 5**

```bash
git add src/components/ecommerce/LayerProperties.vue src/styles.css
git commit -m "feat: expand template layer properties"
```

---

### Task 6: Add Canvas Toolbar, Lock Handling, And Image Fit Preview

**Files:**
- Modify: `src/components/ecommerce/TemplateCanvas.vue`
- Modify: `src/components/ecommerce/TemplateTool.vue`
- Modify: `src/styles.css`

- [ ] **Step 1: Add toolbar emits to canvas**

Modify emits in `src/components/ecommerce/TemplateCanvas.vue`:

```ts
const emit = defineEmits<{
  select: [layerId: string];
  update: [layer: TemplateLayer];
  action: [action: 'duplicate' | 'delete' | 'front' | 'back' | 'lock' | 'toggle-visible', layer: TemplateLayer];
}>();
```

- [ ] **Step 2: Respect locked layers and image fit**

Modify `startMove` and `startResize` to return early after selection if `layer.locked` is true:

```ts
function startMove(event: PointerEvent, layer: TemplateLayer) {
  emit('select', layer.id);
  if (layer.locked) return;
  interaction.value = {
    mode: 'move',
    layer,
    startX: event.clientX,
    startY: event.clientY,
    startLayerX: layer.x,
    startLayerY: layer.y,
    startWidth: layer.width,
    startHeight: layer.height
  };
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}

function startResize(event: PointerEvent, layer: TemplateLayer) {
  event.stopPropagation();
  if (layer.locked) return;
  interaction.value = {
    mode: 'resize',
    layer,
    startX: event.clientX,
    startY: event.clientY,
    startLayerX: layer.x,
    startLayerY: layer.y,
    startWidth: layer.width,
    startHeight: layer.height
  };
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}
```

Add helper:

```ts
function imageStyle(layer: TemplateLayer) {
  const fit = layer.image?.fit ?? 'stretch';
  return { objectFit: fit === 'stretch' ? 'fill' : fit };
}
```

Change the image element:

```vue
<img v-else-if="layer.type === 'image' && assetSrc(layer)" :src="assetSrc(layer)" :style="imageStyle(layer)" alt="模板图片图层" draggable="false" />
```

- [ ] **Step 3: Render selected toolbar**

Inside selected layer button in `TemplateCanvas.vue`, before the resize handle, add:

```vue
<span v-if="layer.id === selectedLayerId" class="template-layer-toolbar" @pointerdown.stop>
  <button type="button" @click.stop="emit('action', 'duplicate', layer)">复制</button>
  <button type="button" @click.stop="emit('action', 'delete', layer)">删除</button>
  <button type="button" @click.stop="emit('action', 'front', layer)">置顶</button>
  <button type="button" @click.stop="emit('action', 'back', layer)">置底</button>
  <button type="button" @click.stop="emit('action', 'lock', layer)">{{ layer.locked ? '解锁' : '锁定' }}</button>
  <button type="button" @click.stop="emit('action', 'toggle-visible', layer)">{{ layer.visible ? '隐藏' : '显示' }}</button>
</span>
<span v-if="layer.id === selectedLayerId && !layer.locked" class="template-resize-handle" @pointerdown="startResize($event, layer)" />
```

Remove the old standalone resize handle line to avoid rendering it twice.

- [ ] **Step 4: Handle actions in TemplateTool**

Add imports in `TemplateTool.vue`:

```ts
import { deleteLayerById, duplicateLayer, moveLayer } from '../../utils/ecommerceTemplate';
```

Merge those names into the existing utility import.

Add handler:

```ts
function handleLayerAction(action: 'duplicate' | 'delete' | 'front' | 'back' | 'lock' | 'toggle-visible', layer: TemplateLayer) {
  if (action === 'delete') {
    project.value = touch({ ...project.value, layers: deleteLayerById(project.value.layers, layer.id) });
    selectedLayerId.value = null;
    return;
  }
  if (action === 'duplicate') {
    project.value = touch({ ...project.value, layers: duplicateLayer(project.value.layers, layer.id) });
    return;
  }
  if (action === 'front' || action === 'back') {
    project.value = touch({ ...project.value, layers: moveLayer(project.value.layers, layer.id, action) });
    return;
  }
  if (action === 'lock') {
    updateLayer({ ...layer, locked: !layer.locked });
    return;
  }
  if (action === 'toggle-visible') {
    updateLayer({ ...layer, visible: !layer.visible });
  }
}
```

Wire it on `TemplateCanvas`:

```vue
@action="handleLayerAction"
```

- [ ] **Step 5: Add toolbar styles**

Append to `src/styles.css`:

```css
.template-layer-toolbar {
  position: absolute;
  left: 0;
  top: -38px;
  z-index: 4;
  display: flex;
  gap: 4px;
  align-items: center;
  width: max-content;
  max-width: 360px;
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.96);
  box-shadow: 0 8px 24px rgba(23, 33, 27, 0.18);
  padding: 6px;
}

.template-layer-toolbar button {
  border: 0;
  border-radius: 9px;
  background: rgba(23, 33, 27, 0.07);
  color: var(--ink);
  padding: 4px 7px;
  font-size: 0.72rem;
  cursor: pointer;
}

.template-canvas-layer img {
  object-position: center;
}
```

- [ ] **Step 6: Run build**

Run:

```bash
npm run build
```

Expected: PASS.

- [ ] **Step 7: Commit Task 6**

```bash
git add src/components/ecommerce/TemplateCanvas.vue src/components/ecommerce/TemplateTool.vue src/styles.css
git commit -m "feat: add canvas layer toolbar"
```

---

### Task 7: Extend Rust Models For Text Styles

**Files:**
- Modify: `src-tauri/src/ecommerce/models.rs`
- Modify: `src-tauri/tests/ecommerce_storage.rs`

- [ ] **Step 1: Write failing storage test assertions**

In `src-tauri/tests/ecommerce_storage.rs`, extend the `TextLayerData` fixture with:

```rust
font_style: Some(TextFontStyle::Italic),
text_decoration: Some(TextDecoration::Underline),
background_color: Some("#fff1b8".to_string()),
background_radius: Some(12.0),
shadow_color: Some("#000000".to_string()),
shadow_blur: Some(4.0),
shadow_offset_x: Some(2.0),
shadow_offset_y: Some(3.0),
```

Add imports at the top if needed:

```rust
use attool_lib::ecommerce::models::{TextDecoration, TextFontStyle};
```

After loading the template, assert:

```rust
let loaded_text = loaded.layers[0].text.as_ref().unwrap();
assert_eq!(loaded_text.font_style, Some(TextFontStyle::Italic));
assert_eq!(loaded_text.text_decoration, Some(TextDecoration::Underline));
assert_eq!(loaded_text.background_color.as_deref(), Some("#fff1b8"));
assert_eq!(loaded_text.background_radius, Some(12.0));
assert_eq!(loaded_text.shadow_color.as_deref(), Some("#000000"));
assert_eq!(loaded_text.shadow_blur, Some(4.0));
assert_eq!(loaded_text.shadow_offset_x, Some(2.0));
assert_eq!(loaded_text.shadow_offset_y, Some(3.0));
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test ecommerce_storage
```

Expected: FAIL to compile because `TextFontStyle`, `TextDecoration`, and new `TextLayerData` fields do not exist.

- [ ] **Step 3: Extend Rust models**

Modify `src-tauri/src/ecommerce/models.rs` by adding enums after `TextAlign`:

```rust
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TextFontStyle {
    Normal,
    Italic,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TextDecoration {
    None,
    Underline,
    LineThrough,
}
```

Add fields to `TextLayerData`:

```rust
pub font_style: Option<TextFontStyle>,
pub text_decoration: Option<TextDecoration>,
pub background_color: Option<String>,
pub background_radius: Option<f32>,
pub shadow_color: Option<String>,
pub shadow_blur: Option<f32>,
pub shadow_offset_x: Option<f32>,
pub shadow_offset_y: Option<f32>,
```

- [ ] **Step 4: Update all Rust test fixtures**

For every `TextLayerData { ... }` literal in `src-tauri/tests/*.rs`, add the new fields with `None` unless a test needs a value:

```rust
font_style: None,
text_decoration: None,
background_color: None,
background_radius: None,
shadow_color: None,
shadow_blur: None,
shadow_offset_x: None,
shadow_offset_y: None,
```

- [ ] **Step 5: Run storage test**

Run:

```bash
cargo test --test ecommerce_storage
```

Expected: PASS.

- [ ] **Step 6: Commit Task 7**

```bash
git add src-tauri/src/ecommerce/models.rs src-tauri/tests/ecommerce_storage.rs src-tauri/tests/ecommerce_render.rs src-tauri/tests/ecommerce_psd_bridge.rs
git commit -m "feat: persist extended text styles"
```

---

### Task 8: Render Text Background, Shadow, Stroke, And Decoration In PNG Export

**Files:**
- Modify: `src-tauri/src/ecommerce/render.rs`
- Modify: `src-tauri/tests/ecommerce_render.rs`

- [ ] **Step 1: Write failing text render test**

Add a test to `src-tauri/tests/ecommerce_render.rs`:

```rust
#[test]
fn exports_text_with_background_shadow_and_decoration() {
    let output = tempfile::tempdir().unwrap();
    let store = EcommerceStore::new(output.path().join("store")).unwrap();
    let template = TemplateProject {
        id: "styled-text".to_string(),
        name: "Styled Text".to_string(),
        canvas_width: 240,
        canvas_height: 140,
        layers: vec![TemplateLayer {
            id: "text".to_string(),
            name: "Styled".to_string(),
            r#type: TemplateLayerType::Text,
            x: 30.0,
            y: 36.0,
            width: 180.0,
            height: 72.0,
            visible: true,
            opacity: 1.0,
            rotation: 0.0,
            binding_key: None,
            locked: None,
            children: None,
            text: Some(TextLayerData {
                text: "SALE".to_string(),
                font_family: "PingFang SC".to_string(),
                font_size: 34.0,
                font_weight: serde_json::json!(900),
                color: "#111111".to_string(),
                stroke_color: Some("#ffffff".to_string()),
                stroke_width: Some(2.0),
                letter_spacing: None,
                line_height: None,
                align: Some(TextAlign::Left),
                font_style: None,
                text_decoration: Some(TextDecoration::Underline),
                background_color: Some("#ffdd66".to_string()),
                background_radius: Some(10.0),
                shadow_color: Some("#000000".to_string()),
                shadow_blur: Some(0.0),
                shadow_offset_x: Some(5.0),
                shadow_offset_y: Some(5.0),
            }),
            image: None,
            shape: None,
        }],
        assets: vec![],
        source_psd_path: None,
        preview_path: None,
        created_at: "now".to_string(),
        updated_at: "now".to_string(),
    };
    store.save_template(template).unwrap();

    let result = export_images(&store, ExportRequest {
        template_id: "styled-text".to_string(),
        output_dir: output.path().join("exports").to_string_lossy().into_owned(),
        rows: vec![BatchRow { id: "row".to_string(), index: 0, values: Default::default() }],
    }).unwrap();

    let image = image::open(&result.outputs[0]).unwrap().to_rgba8();
    assert_eq!(image.get_pixel(34, 40).0, [255, 221, 102, 255], "background should be rendered");
    assert_ne!(image.get_pixel(70, 80).0, [255, 255, 255, 255], "text or decoration should affect pixels");
    assert_ne!(image.get_pixel(120, 98).0, [255, 255, 255, 255], "underline should affect pixels below text");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test ecommerce_render exports_text_with_background_shadow_and_decoration
```

Expected: FAIL because background, shadow, stroke, or decoration pixels are not rendered.

- [ ] **Step 3: Implement text rendering helpers**

Modify imports in `src-tauri/src/ecommerce/render.rs`:

```rust
use imageproc::drawing::{draw_filled_ellipse_mut, draw_filled_rect_mut, draw_hollow_ellipse_mut, draw_hollow_rect_mut, draw_line_segment_mut, draw_text_mut};
use imageproc::rect::Rect;
```

Add helpers before `draw_text_layer`:

```rust
fn layer_rect(layer: &TemplateLayer) -> Rect {
    Rect::at(layer.x.round() as i32, layer.y.round() as i32).of_size(layer.width.max(1.0) as u32, layer.height.max(1.0) as u32)
}

fn with_layer_alpha(mut color: Rgba<u8>, opacity: f32) -> Rgba<u8> {
    color.0[3] = ((color.0[3] as f32) * opacity.clamp(0.0, 1.0)).round() as u8;
    color
}

fn draw_text_background(canvas: &mut RgbaImage, layer: &TemplateLayer, text_data: &TextLayerData) {
    if let Some(background) = &text_data.background_color {
        draw_filled_rect_mut(canvas, layer_rect(layer), with_layer_alpha(parse_hex(background), layer.opacity));
    }
}

fn draw_text_decoration(canvas: &mut RgbaImage, layer: &TemplateLayer, text_data: &TextLayerData, color: Rgba<u8>) {
    let Some(decoration) = &text_data.text_decoration else { return; };
    if matches!(decoration, TextDecoration::None) { return; }
    let y = match decoration {
        TextDecoration::Underline => layer.y + text_data.font_size + 6.0,
        TextDecoration::LineThrough => layer.y + text_data.font_size * 0.55,
        TextDecoration::None => return,
    };
    draw_line_segment_mut(canvas, (layer.x, y), (layer.x + layer.width, y), color);
}
```

Replace `draw_text_layer` with:

```rust
fn draw_text_layer(canvas: &mut RgbaImage, layer: &TemplateLayer, row: &BatchRow) {
    let Some(text_data) = &layer.text else {
        return;
    };
    let Some(font) = load_font(&text_data.font_family).or_else(|| load_font("PingFang SC")).or_else(default_font) else {
        return;
    };
    let text = layer
        .binding_key
        .as_ref()
        .and_then(|key| row.values.get(key))
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| text_data.text.clone());
    draw_text_background(canvas, layer, text_data);
    let fill = with_layer_alpha(parse_hex(&text_data.color), layer.opacity);
    let scale = PxScale::from(text_data.font_size.max(1.0));

    if let Some(shadow_color) = &text_data.shadow_color {
        let shadow = with_layer_alpha(parse_hex(shadow_color), layer.opacity);
        let mut shadow_layer = RgbaImage::from_pixel(canvas.width(), canvas.height(), Rgba([0, 0, 0, 0]));
        draw_text_mut(
            &mut shadow_layer,
            shadow,
            (layer.x + text_data.shadow_offset_x.unwrap_or(0.0)).round() as i32,
            (layer.y + text_data.shadow_offset_y.unwrap_or(0.0)).round() as i32,
            scale,
            &font,
            &text,
        );
        let blur = text_data.shadow_blur.unwrap_or(0.0).max(0.0);
        let shadow_layer = if blur > 0.0 { imageops::blur(&shadow_layer, blur) } else { shadow_layer };
        imageops::overlay(canvas, &shadow_layer, 0, 0);
    }

    if let (Some(stroke_color), Some(stroke_width)) = (&text_data.stroke_color, text_data.stroke_width) {
        let stroke = with_layer_alpha(parse_hex(stroke_color), layer.opacity);
        let width = stroke_width.max(0.0).round() as i32;
        for dy in -width..=width {
            for dx in -width..=width {
                if dx == 0 && dy == 0 { continue; }
                draw_text_mut(canvas, stroke, layer.x.round() as i32 + dx, layer.y.round() as i32 + dy, scale, &font, &text);
            }
        }
    }

    draw_text_mut(canvas, fill, layer.x.round() as i32, layer.y.round() as i32, scale, &font, &text);
    draw_text_decoration(canvas, layer, text_data, fill);
}
```

The shadow implementation renders shadow text to an intermediate transparent layer, applies `imageops::blur` when `shadowBlur` is positive, then overlays the result onto the canvas.

- [ ] **Step 4: Run focused render test**

Run:

```bash
cargo test --test ecommerce_render exports_text_with_background_shadow_and_decoration
```

Expected: PASS.

- [ ] **Step 5: Commit Task 8**

```bash
git add src-tauri/src/ecommerce/render.rs src-tauri/tests/ecommerce_render.rs
git commit -m "feat: render styled text exports"
```

---

### Task 9: Render Shape Stroke, Opacity, And Image Fit In PNG Export

**Files:**
- Modify: `src-tauri/src/ecommerce/render.rs`
- Modify: `src-tauri/tests/ecommerce_render.rs`

- [ ] **Step 1: Write failing render tests for image fit and shape stroke**

Add this test to `src-tauri/tests/ecommerce_render.rs`:

```rust
#[test]
fn exports_shape_stroke_and_contained_image() {
    let output = tempfile::tempdir().unwrap();
    let image_path = output.path().join("wide.png");
    image::RgbaImage::from_pixel(80, 20, image::Rgba([255, 0, 0, 255])).save(&image_path).unwrap();

    let store = EcommerceStore::new(output.path().join("store")).unwrap();
    let template = TemplateProject {
        id: "shape-image".to_string(),
        name: "Shape Image".to_string(),
        canvas_width: 120,
        canvas_height: 120,
        layers: vec![
            TemplateLayer {
                id: "shape".to_string(),
                name: "Shape".to_string(),
                r#type: TemplateLayerType::Shape,
                x: 10.0,
                y: 10.0,
                width: 50.0,
                height: 40.0,
                visible: true,
                opacity: 1.0,
                rotation: 0.0,
                binding_key: None,
                locked: None,
                children: None,
                text: None,
                image: None,
                shape: Some(ShapeLayerData { shape: ShapeKind::Rect, fill: Some("#00ff00".to_string()), stroke: Some("#0000ff".to_string()), stroke_width: Some(3.0), radius: None }),
            },
            TemplateLayer {
                id: "image".to_string(),
                name: "Image".to_string(),
                r#type: TemplateLayerType::Image,
                x: 10.0,
                y: 70.0,
                width: 80.0,
                height: 40.0,
                visible: true,
                opacity: 1.0,
                rotation: 0.0,
                binding_key: None,
                locked: None,
                children: None,
                text: None,
                image: Some(ImageLayerData { asset_id: "asset".to_string(), fit: ImageFit::Contain, replaceable: true }),
                shape: None,
            },
        ],
        assets: vec![TemplateAsset { id: "asset".to_string(), name: "wide".to_string(), path: image_path.to_string_lossy().into_owned(), source_layer_id: None, mime_type: "image/png".to_string(), width: 80, height: 20 }],
        source_psd_path: None,
        preview_path: None,
        created_at: "now".to_string(),
        updated_at: "now".to_string(),
    };
    store.save_template(template).unwrap();

    let result = export_images(&store, ExportRequest {
        template_id: "shape-image".to_string(),
        output_dir: output.path().join("exports").to_string_lossy().into_owned(),
        rows: vec![BatchRow { id: "row".to_string(), index: 0, values: Default::default() }],
    }).unwrap();

    let exported = image::open(&result.outputs[0]).unwrap().to_rgba8();
    assert_eq!(exported.get_pixel(10, 10).0, [0, 0, 255, 255], "shape border should be blue");
    assert_eq!(exported.get_pixel(30, 30).0, [0, 255, 0, 255], "shape interior should be green");
    assert_eq!(exported.get_pixel(10, 70).0, [255, 255, 255, 255], "contain fit should letterbox top area");
    assert_eq!(exported.get_pixel(10, 80).0, [255, 0, 0, 255], "contain fit should center resized image");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test ecommerce_render exports_shape_stroke_and_contained_image
```

Expected: FAIL because shape stroke and/or contain fit are not implemented.

- [ ] **Step 3: Implement image fit rendering**

Replace `draw_image_layer` resizing logic in `src-tauri/src/ecommerce/render.rs` with:

```rust
let target_w = layer.width.max(1.0) as u32;
let target_h = layer.height.max(1.0) as u32;
let (resized, offset_x, offset_y) = match image_data.fit {
    ImageFit::Stretch => (image.resize_exact(target_w, target_h, imageops::FilterType::Lanczos3).to_rgba8(), 0_i64, 0_i64),
    ImageFit::Contain => {
        let scale = (target_w as f32 / image.width() as f32).min(target_h as f32 / image.height() as f32);
        let w = (image.width() as f32 * scale).round().max(1.0) as u32;
        let h = (image.height() as f32 * scale).round().max(1.0) as u32;
        let offset_x = ((target_w - w) / 2) as i64;
        let offset_y = ((target_h - h) / 2) as i64;
        (image.resize_exact(w, h, imageops::FilterType::Lanczos3).to_rgba8(), offset_x, offset_y)
    }
    ImageFit::Cover => {
        let scale = (target_w as f32 / image.width() as f32).max(target_h as f32 / image.height() as f32);
        let w = (image.width() as f32 * scale).round().max(1.0) as u32;
        let h = (image.height() as f32 * scale).round().max(1.0) as u32;
        let resized = image.resize_exact(w, h, imageops::FilterType::Lanczos3).to_rgba8();
        let crop_x = ((w - target_w) / 2).min(w.saturating_sub(1));
        let crop_y = ((h - target_h) / 2).min(h.saturating_sub(1));
        (imageops::crop_imm(&resized, crop_x, crop_y, target_w.min(w), target_h.min(h)).to_image(), 0_i64, 0_i64)
    }
};
imageops::overlay(canvas, &resized, layer.x.round() as i64 + offset_x, layer.y.round() as i64 + offset_y);
```

- [ ] **Step 4: Implement shape stroke rendering**

Modify `draw_shape_layer`:

```rust
fn draw_shape_layer(canvas: &mut RgbaImage, layer: &TemplateLayer) {
    let Some(shape) = &layer.shape else { return; };
    let fill = with_layer_alpha(parse_hex(shape.fill.as_deref().unwrap_or("#000000")), layer.opacity);
    let stroke = shape.stroke.as_deref().map(|value| with_layer_alpha(parse_hex(value), layer.opacity));
    let stroke_width = shape.stroke_width.unwrap_or(0.0).max(0.0) as u32;
    let rect = layer_rect(layer);

    match shape.shape {
        ShapeKind::Rect | ShapeKind::RoundRect => {
            draw_filled_rect_mut(canvas, rect, fill);
            if let Some(stroke) = stroke {
                for inset in 0..stroke_width {
                    let x = rect.left() + inset as i32;
                    let y = rect.top() + inset as i32;
                    let w = rect.width().saturating_sub(inset * 2);
                    let h = rect.height().saturating_sub(inset * 2);
                    if w > 0 && h > 0 {
                        draw_hollow_rect_mut(canvas, Rect::at(x, y).of_size(w, h), stroke);
                    }
                }
            }
        }
        ShapeKind::Line => {
            draw_line_segment_mut(canvas, (layer.x, layer.y), (layer.x + layer.width, layer.y + layer.height), stroke.unwrap_or(fill));
        }
        ShapeKind::Ellipse => {
            let center = (layer.x + layer.width / 2.0, layer.y + layer.height / 2.0);
            let radius_x = (layer.width / 2.0).max(1.0) as i32;
            let radius_y = (layer.height / 2.0).max(1.0) as i32;
            draw_filled_ellipse_mut(canvas, (center.0.round() as i32, center.1.round() as i32), radius_x, radius_y, fill);
            if let Some(stroke) = stroke {
                for inset in 0..stroke_width {
                    draw_hollow_ellipse_mut(canvas, (center.0.round() as i32, center.1.round() as i32), radius_x - inset as i32, radius_y - inset as i32, stroke);
                }
            }
        }
    }
}
```

Ellipse rendering uses imageproc ellipse helpers so ellipse and circle exports match the UI shape choice.

- [ ] **Step 5: Run focused test**

Run:

```bash
cargo test --test ecommerce_render exports_shape_stroke_and_contained_image
```

Expected: PASS.

- [ ] **Step 6: Commit Task 9**

```bash
git add src-tauri/src/ecommerce/render.rs src-tauri/tests/ecommerce_render.rs
git commit -m "feat: render shape strokes and image fit"
```

---

### Task 10: Full Verification And Cleanup

**Files:**
- Check: all modified files

- [ ] **Step 1: Run TypeScript tests**

Run:

```bash
npm test
```

Expected: all Vitest suites pass.

- [ ] **Step 2: Run frontend build**

Run:

```bash
npm run build
```

Expected: TypeScript and Vite build succeed. Vite chunk-size warnings are acceptable if exit code is 0.

- [ ] **Step 3: Run Python bridge tests**

Run:

```bash
python3 -m unittest src-tauri/python/psd_template_bridge_test.py
```

Expected: OK.

- [ ] **Step 4: Run Rust tests**

Run:

```bash
cargo test
```

Expected: all Rust unit, integration, and doc tests pass.

- [ ] **Step 5: Check diff hygiene**

Run:

```bash
git diff --check
git status --short
```

Expected: `git diff --check` prints nothing. `git status --short` shows only intentional source/test/doc changes and no `.superpowers/` artifacts.

- [ ] **Step 6: Add `.superpowers/` to gitignore if still unignored**

If `git status --short` shows `.superpowers/brainstorm/...`, append this line to `.gitignore`:

```gitignore
.superpowers/
```

Then run:

```bash
git status --short
```

Expected: `.superpowers/` no longer appears.

- [ ] **Step 7: Final commit**

If Step 6 changed `.gitignore` or verification cleanup changed files, commit:

```bash
git add .gitignore
git commit -m "chore: ignore superpowers brainstorm artifacts"
```

If there are no remaining uncommitted changes, skip this commit and record that the branch is clean.
