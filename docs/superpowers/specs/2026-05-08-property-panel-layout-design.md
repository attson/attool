# Property Panel Layout Refinement — Design

Date: 2026-05-08
Component: `src/components/ecommerce/LayerProperties.vue` (electronic-commerce template editor)

## Problem

The property panel for the e-commerce template editor has visible layout issues at the current viewport width:

- Numeric fields (X / Y / 宽 / 高) truncate values like `155.0` → `155.` and `467.00` → `467.`. The 48px label column plus the n-input-number stepper buttons leave too little room for the value.
- The 颜色 field shows truncated hex (`#111` instead of `#111111`) because swatch + hex compete for narrow space in a 2-column grid.
- The 字距 / 行高 placeholders display as `Pleas` (truncated `Please...`).
- The 旋转 / 显示 / 锁定 row is uneven: `显示` shrinks to a checkbox icon next to its label, and `锁定` falls onto its own row.
- The 透明度 slider takes a full-width row that contrasts visually with the dense surrounding fields.
- Section grouping is flat: there is no clear separation between identity (image-name, position, size) and behavioral state (visibility, lock), nor between text content, font, color, and paragraph layout.

## Goals

1. Eliminate value truncation in numeric and color fields.
2. Stop layer-state controls (visible, locked) from competing with properties for grid cells.
3. Make the 文字 section's internal grouping legible (font properties vs. paragraph properties).
4. Preserve the current vertical density — the panel should not become noticeably taller.

Non-goals: redesigning the icon set for visibility/lock; changing the underlying `TemplateLayer` data model; re-skinning the canvas or layer tree.

## Solution

### A. Move visibility / lock to the panel header

The visibility (`visible`) and lock (`locked`) flags are layer-level state, not properties. They belong on the panel chrome.

- In `TemplateTool.vue`, add a `#header-extra` slot to the `n-card title="属性"` containing two icon-only `n-button` controls bound to `selectedLayer.visible` and `selectedLayer.locked`.
- The buttons are `n-button` with `text` + `quaternary` mode, ~22px tall, rendering emoji glyphs as labels: `👁` / `🚫` for visible/hidden, `🔓` / `🔒` for unlocked/locked. (No icon library is installed and we are not adding one for this change.)
- Disabled when no layer is selected.
- Removing the buttons from `LayerProperties.vue` removes the awkward third small field after `旋转` and gives the 基础 section a clean tail.

### B. Compound paired controls in 基础

Two new compound rows replace the four separate X / Y / 宽 / 高 fields:

```
位置  X [ 155.00 ]  Y [ 326.00 ]
尺寸  W [ 467.00 ]  H [  92.80 ]
```

DOM shape (per row):

```html
<div class="template-prop-pair">
  <span class="template-prop-label">位置</span>
  <span class="template-prop-pair-cell">
    <span class="template-prop-pair-axis">X</span>
    <n-input-number ... />
  </span>
  <span class="template-prop-pair-cell">
    <span class="template-prop-pair-axis">Y</span>
    <n-input-number ... />
  </span>
</div>
```

Layout: CSS grid with `grid-template-columns: <label-col> minmax(0,1fr) minmax(0,1fr)`. The two cells split available width equally. The mini-label `X` / `Y` (or `W` / `H`) sits inline before its input as a 12-14px-wide leading character.

Result: each numeric input gets ~2× its current width without changing overall row height, fitting two-decimal values cleanly.

### C. 基础 section final shape

```
图层名  [ 标题文字                ]
位置    X [ 155.00 ]  Y [ 326.00 ]
尺寸    W [ 467.00 ]  H [  92.80 ]
透明度  ●━━━━━━━━━━━━━━━━ 100%
旋转    [   0  ]
```

The `显示` and `锁定` checkboxes are removed (now on the panel header). Opacity stays as a full-width slider.

### D. 绑定 section

Unchanged.

### E. 文字 section: subgroup for paragraph properties

Within the existing `<section>`, introduce a lightweight subtitle to separate font properties from paragraph properties. The subtitle is **not** a new section (no h3, no border) — just a small grey label with a top margin.

```
文字内容
[ 仅配件，勿拍错               ]

字体    [ PingFang SC          ]
字号    [ 64 ]   字重 [ 800 ]
样式    [常规▾]  装饰 [无▾]
颜色    [■]  [ #111111         ]

排版                                ← subgroup title
对齐    [左对齐▾]
字距    [   ]   行高 [   ]
```

Key changes:
- 颜色 is promoted to a **full-width row** (`template-prop-field.full`). The hex input stops being squeezed into half of half a row and shows the full `#111111`.
- 对齐 moves out of the same row as 颜色 and into the new 排版 subgroup. 字距 and 行高 share a row but their (now-smaller) input widths are still wide enough that the placeholder `请输入` no longer truncates to `Pleas`.
- Subtitle DOM:
  ```html
  <h4 class="template-prop-subtitle">排版</h4>
  ```
  styled with `font-size: 0.74rem; color: var(--muted); margin: 6px 0 2px; font-weight: 700;`.

### F. 填充 / 描边 / 阴影 (collapsible details)

Each color field becomes a full-width row so the hex input always shows complete values. Numeric companion fields (圆角, 描边宽, 模糊, 阴影 X, 阴影 Y) stay two-per-row.

### G. 形状 section

Same color treatment: 填充色 and 描边色 each take a full-width row. 描边宽 + 圆角 share a row.

### H. 图片 section

Unchanged (only one field, 裁剪方式).

## CSS additions / changes (`src/styles.css`)

New classes:

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
```

Panel header button styling (minimal — relies on `n-button` `text` + `quaternary`):

```css
.template-editor-panel .n-card-header__extra {
  display: flex;
  gap: 4px;
}

.template-editor-panel .template-header-action {
  --n-height: 22px;
  padding: 0 6px;
  color: var(--muted);
}

.template-editor-panel .template-header-action.is-active {
  color: var(--ink);
}
```

No removals; existing `.template-prop-field`, `.template-prop-row`, `.template-color-field` rules stay in place.

## Behavior details

- When `layer.locked` is true, the form continues to render with `:disabled` on every input (existing behavior, untouched). The header lock button itself stays clickable so the user can unlock.
- When `layer` is null, the header buttons are disabled; `LayerProperties` continues to show `n-empty`.
- Toggling visibility from the header issues the same `update` event the rest of the form uses, so undo/redo behaviour is preserved.

## Testing

Manual verification checklist (no automated tests exist for this component):

1. Open the e-commerce template tool, select a text layer.
2. Confirm `155.00` / `326.00` / `467.00` / `92.80` show fully without truncation in the position/size rows.
3. Confirm 颜色 hex displays as `#111111`, not `#111`.
4. Confirm 字距 and 行高 placeholder text is fully visible.
5. Toggle the eye icon in the panel header — layer hides on canvas; toggle the lock icon — form fields disable, lock button stays enabled.
6. Select a shape layer; confirm 填充色 and 描边色 rows show full hex values.
7. Open the 填充/描边/阴影 details section; confirm color rows are full-width and hex values complete.
8. Resize the editor panel narrower (~280px wide) and confirm no horizontal scrollbar appears and inputs degrade gracefully.

## Out of scope

- Replacing `n-input-number` steppers with a different control.
- Adding alignment-as-icons (`左 / 中 / 右` icon group) — current select is fine.
- Restyling the section header (`基础` / `文字` / etc.) — current h3 visual is fine.
- Touching `BatchPanel.vue`, `LayerTree.vue`, `TemplateCanvas.vue`, `TemplateResourcePanel.vue`.

## Files changed

- `src/components/ecommerce/LayerProperties.vue` — restructure 基础 section into compound paired rows, remove visible/locked fields, promote 颜色/形状色 to full rows, add 排版 subtitle.
- `src/components/ecommerce/TemplateTool.vue` — add `#header-extra` slot to the 属性 `n-card` with two icon buttons wired to visibility/lock.
- `src/styles.css` — add `.template-prop-pair*`, `.template-prop-subtitle`, `.template-header-action` rules; no removals.
