# Clipboard History Tool Design

## Goal

Build a Paste-style clipboard history tool inside AT Tool. The tool should run while AT Tool is open, capture system clipboard changes, and let the user summon a searchable history panel with a global shortcut.

The v1 scope includes text, images, and file paths. The experience should feel like a local desktop utility, not a form-heavy admin page.

## User Experience

- AT Tool gains a ready tool entry: `剪贴板工具`.
- When AT Tool is running, a backend clipboard watcher records new clipboard items.
- A global shortcut opens a compact history window centered on screen.
- The history window shows recent items in a Paste-like grid/card layout.
- Search filters across text content, file names, paths, and item type.
- Type filters let the user switch between all, text, image, and files.
- Selecting an item restores it to the system clipboard.
- The panel supports delete and favorite/pin actions.
- Duplicate consecutive clipboard values are ignored.
- A retention limit prevents unbounded storage growth; v1 default is 500 items.

## Non-Goals for v1

- Cloud sync or multi-device sync.
- OCR or AI classification for images.
- Menu bar app mode.
- Recording the source application name.
- Rule-based text transformations.
- Background capture when AT Tool is fully quit.

## Architecture

### Frontend

Add a dedicated clipboard feature module instead of expanding `src/App.vue` with business logic.

Proposed files:

- `src/components/clipboard/ClipboardTool.vue` — main in-app tool view with status, shortcut hint, settings, and recent history.
- `src/components/clipboard/ClipboardHistoryWindow.vue` — reusable history panel layout for the global shortcut window.
- `src/components/clipboard/ClipboardItemCard.vue` — card rendering for text, image, and file items.
- `src/composables/useClipboardHistory.ts` — history loading, search/filter state, item actions.
- `src/types/clipboard.ts` — item models and request/response types.

`src/App.vue` should only register the tool route and render `ClipboardTool`, following the existing “new tool minimal path”.

### Backend

Add a Rust clipboard module under `src-tauri/src/clipboard/`.

Proposed files:

- `models.rs` — serializable clipboard item types.
- `storage.rs` — SQLite tables and retention cleanup.
- `watcher.rs` — polling loop and deduplication.
- `commands.rs` — Tauri commands for listing, searching, restoring, deleting, pinning, and settings.
- `mod.rs` — module exports.

Use Tauri's clipboard APIs or platform clipboard integration from Rust for read/write operations. If the chosen API cannot cover images and file URLs consistently, implement v1 in layers: text first, then image, then file paths, each behind the same model and tests.

### Global Shortcut and Window

Use Tauri 2 global shortcut support to register a default shortcut. The default should avoid common system shortcuts; use `CommandOrControl+Shift+V` unless testing shows a conflict.

Opening the shortcut should show a lightweight Tauri window, not navigate the main tool shell. The window should:

- Be initially hidden and created at startup.
- Use a fixed compact size suitable for quick selection.
- Center on the current display when shown.
- Close on Escape or after selecting an item.
- Reuse the same frontend components as the in-app tool where practical.

### Storage Model

SQLite remains the source of truth.

A single `clipboard_items` table should store:

- `id`
- `kind`: `text | image | files`
- `preview`
- `content_text` for text items and searchable file path text
- `asset_path` for image payloads/thumbnails
- `file_paths_json` for file items
- `content_hash` for deduplication
- `is_pinned`
- `created_at`
- `last_copied_at`

Image payloads should live under the app data directory, referenced by SQLite path. Store a thumbnail for grid rendering and retain enough original data to restore the image clipboard item when possible.

Retention cleanup removes oldest unpinned items beyond the configured limit and deletes orphaned image assets.

## Data Flow

1. App startup initializes clipboard storage and starts the watcher.
2. Watcher polls clipboard at a modest interval.
3. On change, watcher normalizes the clipboard value into a typed item.
4. Watcher hashes normalized content and skips duplicates.
5. New items are persisted to SQLite and emitted to frontend windows.
6. History UI queries recent items with search and type filters.
7. Restore action writes the selected item back to the system clipboard and closes the quick panel.

## Privacy and Safety

Clipboard history can contain secrets. v1 should make this explicit in UI copy and keep all data local.

Controls:

- A visible pause/resume capture toggle.
- A clear-all action with confirmation.
- A retention limit setting.
- No network sync.
- No logging of clipboard content.

## Error Handling

- If clipboard read fails, keep the watcher alive and surface a non-blocking status message.
- If image capture fails but text/file capture works, record only supported types and show the unsupported state in settings.
- If restoring an item fails, show an actionable error and keep the panel open.
- If the global shortcut registration fails, the in-app tool remains usable and displays the failure.

## Testing Strategy

Follow the project rule: pure logic first, no jsdom.

Frontend tests:

- Type filtering and search helpers.
- Item preview formatting.
- Retention/settings composables with injected storage.

Backend tests:

- Storage insert/list/delete/pin behavior.
- Deduplication hash behavior.
- Retention cleanup preserving pinned items.
- Asset cleanup for deleted image items.

Manual verification:

- Start Tauri dev app.
- Copy text, image, and file paths.
- Confirm shortcut opens history window.
- Confirm selecting each type restores it to the clipboard.
- Confirm pause, delete, favorite, and clear-all behavior.

## Implementation Notes

- Do not add third-party frontend icon packages; use inline SVG for any new iconography.
- Keep colors, border radius, and spacing on existing design tokens.
- Do not add business logic to `src/App.vue` beyond tool registration and render branch.
- Prefer a backend abstraction for clipboard access so tests can inject fake clipboard data.
- If image/file clipboard support is uneven across macOS, Windows, and Linux, land the typed model and UI with capability flags instead of pretending all platforms behave the same.
