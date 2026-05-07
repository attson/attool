# Template Editor Design Tool

Date: 2026-05-08
Status: Approved for implementation planning

## Goal

Turn the current ecommerce template tool into a manual template-making editor similar to lightweight Canva/Gaoding workflows. The first implementation focuses on making templates by hand: adding text, shapes, and local images; editing common visual styles; saving templates; and exporting PNGs with preview/export consistency.

Existing PSD import, template persistence, layer selection, drag/resize, and batch export stay in place. The new editor experience builds on those capabilities instead of replacing the ecommerce workflow.

## Scope

### In Scope

- Fixed editor layout: left vertical navigation, left resource panel, center canvas, right property panel.
- Left navigation tabs: Text, Images, Shapes/Materials, Layers.
- Add new layers from the resource panel:
  - Text presets: title, subtitle, body, price/promo text.
  - Shape presets: rectangle, rounded rectangle, ellipse/circle, line.
  - Local image import as a template asset and inserted image layer.
- Canvas interactions:
  - Select, drag, resize existing and newly inserted layers.
  - Quick toolbar for common actions: duplicate, delete, move forward/backward or top/bottom, lock/unlock, show/hide.
- Right property panel:
  - Shared layer properties: name, X/Y/width/height, opacity, rotation, lock, visibility, binding field.
  - Text properties: content, font family, font size, font weight, italic, underline, strikethrough, text color, alignment, letter spacing, line height, background color, background radius, stroke, shadow.
  - Image properties: fit mode and local replacement.
  - Shape properties: type, fill, stroke color, stroke width, radius.
- PNG export supports the styles exposed in the editor, including text background, rounded background, stroke, shadow, underline, and strikethrough.
- Automated tests for the pure TypeScript layer/style helpers and Rust rendering behavior.

### Out of Scope for This First Version

- Large built-in stock material library.
- Combination components such as one-click product hero blocks or bottom title bars.
- Photoshop-level text layout fidelity for every glyph and transform.
- Cloud accounts, online assets, collaboration, or marketplace templates.
- Multi-page documents.

## User Experience

### Layout

The editor uses a fixed three-panel workbench:

1. Left vertical rail: compact navigation icons for Text, Images, Shapes/Materials, and Layers.
2. Left resource panel: content changes based on the active rail tab.
3. Center canvas: the editable template canvas with selectable layers.
4. Right property panel: contextual controls for the selected layer.

This matches the user's selected layout direction: a stable, always-visible resource panel and an always-visible property panel for fast template creation.

### Left Resource Panel

Text tab:

- Shows text presets grouped by common ecommerce uses: title, subtitle, body, price, promo badge text.
- Clicking a preset inserts a text layer centered in the visible canvas area.
- Presets define initial content, font size, weight, color, and optional style defaults.

Images tab:

- Offers a local image picker.
- Importing creates a `TemplateAsset` and inserts an image layer referencing that asset.
- If an image layer is selected, the same picker can replace its asset.

Shapes/Materials tab:

- Offers rectangle, rounded rectangle, ellipse/circle, and line.
- Clicking a preset inserts a shape layer with sane default size, fill, stroke, and radius.

Layers tab:

- Reuses the existing layer tree behavior but moves it into the left resource panel.
- Supports selecting layers and toggling visibility/lock state.

### Canvas

The center canvas keeps the current preview behavior and supports layers from imported PSDs and manually inserted layers. A selected layer shows resize handles and a compact floating toolbar. The toolbar contains only fast actions; detailed editing stays in the right property panel.

Default insertion behavior:

- New layers appear near the center of the canvas.
- New layers are selected immediately.
- New layer dimensions are clamped to the canvas bounds.
- Layer IDs and asset IDs use the same project-local ID style as current templates.

### Right Property Panel

The right panel is grouped into sections:

- Basic: layer name, position, size, opacity, rotation, lock, visible.
- Binding: optional binding key for batch replacement/export.
- Text: text content, font, size, weight, style, alignment, spacing, line height, decoration.
- Fill: text background or shape fill.
- Stroke: text stroke or shape stroke.
- Shadow: text shadow settings.
- Image: fit mode and replace image.

The panel must remain usable in a narrow right column. Long groups use clear section headers in the first implementation; collapsible sections are optional polish after the required controls work.

## Data Model

Extend `TextLayerData` with optional fields:

- `fontStyle?: 'normal' | 'italic'`
- `textDecoration?: 'none' | 'underline' | 'line-through'`
- `backgroundColor?: string`
- `backgroundRadius?: number`
- `shadowColor?: string`
- `shadowBlur?: number`
- `shadowOffsetX?: number`
- `shadowOffsetY?: number`

Existing fields remain the source of truth for font family, font size, font weight, color, stroke, letter spacing, line height, and alignment.

No new top-level layer type is required. Text, image, and shape layers already cover the first-version needs.

## TypeScript Architecture

Add small pure helpers in the ecommerce template utility layer for behavior that should be tested without mounting Vue components:

- Create default text, shape, and image layers.
- Create image assets from selected local file metadata.
- Insert a layer into a project and select it.
- Duplicate, delete, and reorder layers.
- Convert text style data to canvas preview CSS.

Vue components should stay focused on rendering and event handling:

- `TemplateTool.vue` owns project state, selected layer, active resource tab, and save/import orchestration.
- A new resource panel component owns the left tab content and emits `add-text`, `add-shape`, `add-image`, and `replace-image` events.
- `LayerProperties.vue` remains the contextual editor but is expanded and organized into sections.
- `TemplateCanvas.vue` renders the floating toolbar and emits layer action events.

## Rust Export Architecture

The export renderer must support the styles exposed by the editor and must not silently ignore primary controls shown in the UI.

Text export requirements:

- Draw text background rectangle before text when `backgroundColor` is set.
- Respect `backgroundRadius` for text backgrounds. A radius of `0` renders a rectangle; a positive radius renders rounded corners.
- Draw text shadow before stroke/fill when shadow settings are present.
- Draw text stroke, then fill text.
- Draw underline or strikethrough based on text metrics or a conservative approximation.
- Continue supporting binding-key replacement for text content.

Shape export requirements:

- Render fill and stroke for rectangle, rounded rectangle, ellipse, and line.
- Respect layer opacity for text, shape, and image layers during export by compositing each layer with alpha.

Image export requirements:

- Implement distinct image fit behavior for `cover`, `contain`, and `stretch` in both preview and export.

## Error Handling

- If a local image cannot be loaded, show the existing notice/error pattern and do not mutate the template.
- If no layer is selected, the property panel shows an empty state.
- If an action cannot apply to a locked layer, ignore it or show a short notice; do not mutate locked layers.
- Clamp numeric inputs to safe values: size greater than zero, opacity within 0-1, stroke/shadow/radius non-negative.
- Keep missing font behavior consistent with current export fallback fonts.

## Testing Strategy

TypeScript tests:

- Creating default text, shape, and image layers produces valid dimensions, IDs, visibility, opacity, and initial style data.
- Inserting a layer updates the project and selects the inserted layer.
- Duplicating/deleting/reordering layers preserves nested layer structure where applicable.
- Text preview CSS includes font style, decoration, background, radius, shadow, stroke, letter spacing, and line height.

Rust tests:

- Exporting a text layer with background and text changes pixels in both background and text regions.
- Exporting a text layer with shadow changes pixels offset from the text region.
- Exporting underline or strikethrough changes pixels near the expected decoration position.
- Exporting a shape with fill and stroke changes expected interior and border pixels.
- Existing storage, PSD import, and batch export tests continue to pass.

Verification commands:

- `npm test`
- `npm run build`
- `python3 -m unittest src-tauri/python/psd_template_bridge_test.py`
- `cargo test`

## Implementation Notes

- Keep the first implementation in the existing ecommerce feature area.
- Preserve existing project JSON compatibility by making all new style fields optional.
- Keep the batch generation card below the editor for this version.
- Keep visual styling consistent with the current app's warm panel aesthetic rather than copying the reference app pixel-for-pixel.
- The `.superpowers/` brainstorming artifacts are local design aids and should not be committed.
