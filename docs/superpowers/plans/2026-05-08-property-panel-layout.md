# Property Panel Layout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refine the e-commerce template editor's property panel so numeric values stop truncating, layer-state toggles move to the panel header, and the 文字 section gains a `排版` subgroup.

**Architecture:** Three additive changes — new CSS classes for compound paired controls + subgroup subtitle in `styles.css`; a `#header-extra` slot on the 属性 `n-card` in `TemplateTool.vue` with two emoji-glyph buttons toggling visibility/lock; a markup-only restructure of `LayerProperties.vue` (no script changes apart from removing now-unused fields). No new dependencies, no data-model changes.

**Tech Stack:** Vue 3 (`<script setup lang="ts">`), naive-ui 2.44, Vite 8, TypeScript 6, Vitest 4. Tauri 2 desktop shell. No icon library.

**Spec:** `docs/superpowers/specs/2026-05-08-property-panel-layout-design.md`

---

## File Map

- `src/styles.css` — append three new rule blocks: `.template-prop-pair*`, `.template-prop-subtitle`, `.template-header-action`. No removals.
- `src/components/ecommerce/TemplateTool.vue` — add `#header-extra` slot inside the existing `<n-card title="属性" ...>` (around line 227) with two `n-button` toggles bound to `selectedLayer.visible` / `selectedLayer.locked`.
- `src/components/ecommerce/LayerProperties.vue` — restructure 基础 section into compound paired rows, remove visible/locked rows, add 排版 subgroup in 文字, promote color rows to full-width in 文字 / 形状 / 填充-描边-阴影 sections.

There are no automated component tests for these files. Verification is via:
1. `npm test` — confirm the existing util test (`src/utils/ecommerceTemplate.test.ts`) still passes.
2. `npm run build` — type-check + production build must succeed.
3. Manual smoke test in the dev server (`npm run dev`) per the spec checklist.

---

## Task 1: Add CSS rules for paired controls, subtitle, and header buttons

**Files:**
- Modify: `src/styles.css` (append at end)

This is purely additive. No existing rules are touched.

- [ ] **Step 1: Append the new rule blocks to `src/styles.css`**

Open `src/styles.css` and append these blocks at the end of the file (after the last existing rule, currently line 808):

```css
.template-prop-pair {
  display: grid;
  grid-template-columns: 48px minmax(0, 1fr) minmax(0, 1fr);
  gap: 4px 6px;
  align-items: center;
  min-width: 0;
  min-height: 26px;
  grid-column: 1 / -1;
}

.template-prop-pair-cell {
  display: grid;
  grid-template-columns: 14px minmax(0, 1fr);
  gap: 4px;
  align-items: center;
  min-width: 0;
}

.template-prop-pair-cell > .n-input-number {
  min-width: 0;
  width: 100%;
}

.template-prop-pair-axis {
  color: var(--muted);
  font-size: 0.72rem;
  text-align: center;
}

.template-prop-subtitle {
  margin: 6px 0 2px;
  color: var(--muted);
  font-size: 0.74rem;
  font-weight: 700;
}

.template-editor-panel .n-card-header__extra {
  display: flex;
  gap: 4px;
}

.template-editor-panel .template-header-action {
  --n-height: 22px;
  padding: 0 6px;
  color: var(--muted);
  font-size: 0.95rem;
  line-height: 1;
}

.template-editor-panel .template-header-action.is-active {
  color: var(--ink);
}
```

- [ ] **Step 2: Run the build to confirm CSS still parses**

Run: `npm run build`
Expected: build succeeds (typescript + vite build complete with no errors). The CSS file is bundled but no markup uses the new classes yet, so no visual change.

- [ ] **Step 3: Run the existing tests**

Run: `npm test`
Expected: all tests pass (only `src/utils/ecommerceTemplate.test.ts` is affected, and CSS changes can't break it).

- [ ] **Step 4: Commit**

```bash
git add src/styles.css
git commit -m "style: add CSS for paired prop rows and panel header actions"
```

---

## Task 2: Add visibility / lock buttons to the 属性 panel header

**Files:**
- Modify: `src/components/ecommerce/TemplateTool.vue` (the `<n-card title="属性" ...>` block around line 227)

The buttons reuse `selectedLayer` (computed, line 36) and `updateLayer` (function, line 85). Both already exist.

- [ ] **Step 1: Replace the 属性 card markup with one that includes the header-extra slot**

In `src/components/ecommerce/TemplateTool.vue`, find this block (around line 227):

```vue
      <n-card title="属性" size="small" :bordered="false" class="panel-card template-editor-panel">
        <LayerProperties :layer="selectedLayer" @update="updateLayer" />
      </n-card>
```

Replace it with:

```vue
      <n-card title="属性" size="small" :bordered="false" class="panel-card template-editor-panel">
        <template #header-extra>
          <n-button
            text
            class="template-header-action"
            :class="{ 'is-active': selectedLayer?.visible ?? true }"
            :disabled="!selectedLayer"
            :title="selectedLayer?.visible === false ? '显示图层' : '隐藏图层'"
            @click="selectedLayer && updateLayer({ ...selectedLayer, visible: !selectedLayer.visible })"
          >
            {{ selectedLayer?.visible === false ? '🚫' : '👁' }}
          </n-button>
          <n-button
            text
            class="template-header-action"
            :class="{ 'is-active': Boolean(selectedLayer?.locked) }"
            :disabled="!selectedLayer"
            :title="selectedLayer?.locked ? '解锁图层' : '锁定图层'"
            @click="selectedLayer && updateLayer({ ...selectedLayer, locked: !selectedLayer.locked })"
          >
            {{ selectedLayer?.locked ? '🔒' : '🔓' }}
          </n-button>
        </template>
        <LayerProperties :layer="selectedLayer" @update="updateLayer" />
      </n-card>
```

`NButton` is already imported on line 6. No script changes needed.

- [ ] **Step 2: Run the build**

Run: `npm run build`
Expected: build succeeds. No type errors.

- [ ] **Step 3: Manual smoke test**

Run: `npm run dev`
Open the e-commerce template tool in the browser. Select any layer (or import a PSD if the layer tree is empty).

Verify:
- Two glyph buttons appear at the top-right of the 属性 panel (`👁` and `🔓`).
- Clicking 👁 hides the layer on the canvas and the glyph flips to `🚫`. Click again to restore.
- Clicking 🔓 locks the layer; the glyph flips to `🔒`. The form fields below disable. The header lock button itself remains clickable.
- With no layer selected, both buttons are disabled and the existing `n-empty` placeholder still renders below.

(Both visibility/lock toggles still also work via the existing checkboxes inside the form — those are removed in Task 3.)

- [ ] **Step 4: Commit**

```bash
git add src/components/ecommerce/TemplateTool.vue
git commit -m "feat: move layer visibility and lock toggles to property panel header"
```

---

## Task 3: Restructure 基础 section into compound paired rows

**Files:**
- Modify: `src/components/ecommerce/LayerProperties.vue` (the 基础 `<section>` block, lines 43-83)

We pair X+Y and W+H into compound rows, drop the now-redundant 显示 and 锁定 checkboxes (Task 2 owns those), and keep 透明度 / 旋转 as before.

- [ ] **Step 1: Replace the 基础 section markup**

In `src/components/ecommerce/LayerProperties.vue`, find the 基础 section (lines 43-83):

```vue
    <section class="template-prop-section">
      <h3>基础</h3>
      <div class="template-prop-row">
        <label class="template-prop-field full">
          <span class="template-prop-label">图层名</span>
          <n-input :value="selected.name" :disabled="selected.locked" @update:value="patch({ name: $event })" />
        </label>
        <label class="template-prop-field">
          <span class="template-prop-label">X</span>
          <n-input-number :value="fixed2(selected.x)" :precision="2" :disabled="selected.locked" @update:value="patch({ x: $event ?? 0 })" />
        </label>
        <label class="template-prop-field">
          <span class="template-prop-label">Y</span>
          <n-input-number :value="fixed2(selected.y)" :precision="2" :disabled="selected.locked" @update:value="patch({ y: $event ?? 0 })" />
        </label>
        <label class="template-prop-field">
          <span class="template-prop-label">宽</span>
          <n-input-number :value="fixed2(selected.width)" :precision="2" :min="1" :disabled="selected.locked" @update:value="patch({ width: $event ?? 1 })" />
        </label>
        <label class="template-prop-field">
          <span class="template-prop-label">高</span>
          <n-input-number :value="fixed2(selected.height)" :precision="2" :min="1" :disabled="selected.locked" @update:value="patch({ height: $event ?? 1 })" />
        </label>
        <label class="template-prop-field full">
          <span class="template-prop-label">透明度</span>
          <n-slider :value="selected.opacity" :min="0" :max="1" :step="0.01" :disabled="selected.locked" @update:value="patch({ opacity: Number($event) })" />
        </label>
        <label class="template-prop-field">
          <span class="template-prop-label">旋转</span>
          <n-input-number :value="selected.rotation" :disabled="selected.locked" @update:value="patch({ rotation: $event ?? 0 })" />
        </label>
        <label class="template-prop-field">
          <span class="template-prop-label">显示</span>
          <n-checkbox :checked="selected.visible" @update:checked="patch({ visible: Boolean($event) })" />
        </label>
        <label class="template-prop-field">
          <span class="template-prop-label">锁定</span>
          <n-checkbox :checked="Boolean(selected.locked)" @update:checked="emit('update', { ...selected, locked: Boolean($event) })" />
        </label>
      </div>
    </section>
```

Replace the entire block with:

```vue
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
```

Notes:
- `:show-button="false"` removes the +/- stepper buttons inside the paired cells. This is what frees space for two-decimal values like `155.00` and `467.00` to render fully. The 旋转 input keeps its steppers (single field, plenty of room).
- Both 显示 and 锁定 checkboxes are removed — they are owned by the panel header in Task 2.

- [ ] **Step 2: Remove the now-unused `NCheckbox` import**

After removing the two checkboxes, `n-checkbox` is no longer used in this file. Check if there are any other uses:

Run: `grep -n "n-checkbox" src/components/ecommerce/LayerProperties.vue`
Expected: no output (no remaining usages).

If there are no remaining usages, edit line 3 of `LayerProperties.vue`:

Find:
```ts
import { NCheckbox, NColorPicker, NEmpty, NForm, NInput, NInputNumber, NSelect, NSlider } from 'naive-ui';
```

Replace with:
```ts
import { NColorPicker, NEmpty, NForm, NInput, NInputNumber, NSelect, NSlider } from 'naive-ui';
```

- [ ] **Step 3: Run the build**

Run: `npm run build`
Expected: build succeeds. TypeScript should not complain about the unused-import removal since we removed it. If `tsc` flags any other unused symbol, leave it for now and re-check after Task 4 (the text section refactor stays in this file).

- [ ] **Step 4: Manual smoke test**

Run: `npm run dev` (if not still running). Select a text layer.

Verify:
- 基础 now shows: 图层名, then 位置 (X 155.00 / Y 326.00 cleanly visible without truncation), 尺寸 (W 467.00 / H 92.80 cleanly visible), 透明度 slider, 旋转.
- The 显示 / 锁定 checkboxes are gone from inside the form. The header-extra buttons from Task 2 are the only way to toggle these now.
- Editing X / Y / W / H still updates the canvas in real time.

- [ ] **Step 5: Commit**

```bash
git add src/components/ecommerce/LayerProperties.vue
git commit -m "style: pair X/Y and W/H into compound prop rows"
```

---

## Task 4: Promote color rows to full-width and add 排版 subgroup

**Files:**
- Modify: `src/components/ecommerce/LayerProperties.vue` — the 文字 section (lines 95-142 in the file BEFORE Task 3; line numbers will have shifted after Task 3, so search by content), the 填充/描边/阴影 details (originally lines 144-189), and the 形状 section (originally lines 191-217).

We make every color row a `.template-prop-field.full` so the hex input has the full row width. Inside 文字, we additionally split the existing flat row into a font subgroup followed by a 排版 subgroup.

- [ ] **Step 1: Replace the 文字 section**

Find the 文字 section in `LayerProperties.vue` (it begins with `<section v-if="selected.type === 'text' && selected.text" class="template-prop-section">`):

```vue
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
        <label class="template-prop-field">
          <span class="template-prop-label">颜色</span>
          <span class="template-color-field">
            <n-color-picker :value="selected.text.color" :show-alpha="false" :modes="['hex']" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, color: $event } })" />
            <n-input :value="selected.text.color" :disabled="selected.locked" @update:value="patch({ text: { ...selected.text!, color: $event } })" />
          </span>
        </label>
        <label class="template-prop-field">
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
      </div>
    </section>
```

Replace it with:

```vue
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
      </div>
    </section>
```

Changes from the original:
- 颜色 field gets `full` class — the color row now spans both grid columns and the hex input has full row width to show `#111111`.
- 对齐 / 字距 / 行高 are split out into a separate `.template-prop-row` preceded by an `<h4 class="template-prop-subtitle">排版</h4>` subtitle.
- 对齐 also gets `full` so it is on its own line for visual breathing room.

- [ ] **Step 2: Replace the 填充 / 描边 / 阴影 details section**

Find:

```vue
    <details v-if="selected.type === 'text' && selected.text" class="template-prop-section template-prop-advanced">
      <summary>填充 / 描边 / 阴影</summary>
      <div class="template-prop-row">
        <label class="template-prop-field">
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
        <label class="template-prop-field">
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
        <label class="template-prop-field">
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
```

Replace with (only difference: the three color fields gain `full`, paired with a numeric companion split across two-per-row groups):

```vue
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
```

The result is: 背景色 (full row) → 圆角 (alone on its row) → 描边色 (full row) → 描边宽 (alone) → 阴影色 (full row) → 模糊 (alone). The trailing 阴影 X / 阴影 Y pair remain as a normal 2-per-row pair.

- [ ] **Step 3: Replace the 形状 section**

Find:

```vue
    <section v-if="selected.type === 'shape' && selected.shape" class="template-prop-section">
      <h3>形状</h3>
      <div class="template-prop-row">
        <label class="template-prop-field">
          <span class="template-prop-label">填充色</span>
          <span class="template-color-field">
            <n-color-picker :value="selected.shape.fill ?? null" :show-alpha="false" :modes="['hex']" :disabled="selected.locked" @update:value="patch({ shape: { ...selected.shape!, fill: $event || undefined } })" />
            <n-input :value="selected.shape.fill" :disabled="selected.locked" @update:value="patch({ shape: { ...selected.shape!, fill: $event } })" />
          </span>
        </label>
        <label class="template-prop-field">
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
```

Replace with:

```vue
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
```

Only difference: 填充色 and 描边色 get `full`. 描边宽 + 圆角 stay paired in a single 2-column row.

- [ ] **Step 4: Run the build**

Run: `npm run build`
Expected: build succeeds with no type or template errors.

- [ ] **Step 5: Run the existing tests**

Run: `npm test`
Expected: existing util test passes.

- [ ] **Step 6: Manual smoke test**

Run: `npm run dev` (or refresh the running dev server). Select a text layer.

Verify against the spec checklist:
1. 颜色 input shows the full hex (`#111111`), not `#111`.
2. 字距 / 行高 placeholders render fully (no `Pleas` truncation).
3. 排版 subtitle appears between font properties and 对齐 / 字距 / 行高.
4. Open the 填充 / 描边 / 阴影 details — 背景色 / 描边色 / 阴影色 each occupy their own full-width row with full hex visible.
5. Select a shape layer — 填充色 / 描边色 are full-width; 描边宽 + 圆角 share a row.
6. Resize the panel narrower (~280px) — no horizontal scroll, inputs degrade.
7. Click 👁 in the panel header — layer hides on the canvas. Click 🔓 — fields disable.

- [ ] **Step 7: Commit**

```bash
git add src/components/ecommerce/LayerProperties.vue
git commit -m "style: promote color rows to full-width and add 排版 subgroup"
```

---

## Self-Review Notes

- **Spec coverage:** Section A → Task 2; Section B/C → Task 3; Section D → unchanged (绑定); Section E → Task 4 step 1; Section F → Task 4 step 2; Section G → Task 4 step 3; Section H → unchanged (only one field, no work); CSS additions → Task 1; Behavior details (locked disables form, header lock stays clickable, n-empty when null) → preserved by the existing logic and verified in Tasks 2 & 3 manual checks; Testing checklist → Task 4 step 6.
- **No placeholders:** every step contains the full code or exact command.
- **Type consistency:** uses existing `selectedLayer` (computed) and `updateLayer(updated: TemplateLayer)` (already defined at line 85 of `TemplateTool.vue`); no new functions introduced.
- **Granularity:** Task 1 is small/safe (CSS only), Task 2 adds the panel chrome before the form refactor so visibility/lock control is never absent, Task 3 trims the 基础 fields, Task 4 finishes the visual cleanup. Each task is independently committable and visually verifiable.
