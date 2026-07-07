const IS_MAC = /Mac|iPhone|iPad/.test(navigator.userAgent);

/**
 * Turn a Tauri global-shortcut string (e.g. "CommandOrControl+Shift+A")
 * into a nice display label matching the current platform.
 */
export function formatShortcutForDisplay(raw: string): string {
  if (!raw) return '';
  const parts = raw.split('+').map((p) => p.trim());
  return parts
    .map((p) => {
      const key = p.toLowerCase();
      if (key === 'commandorcontrol') return IS_MAC ? '⌘' : 'Ctrl';
      if (key === 'command' || key === 'cmd' || key === 'super' || key === 'meta') return '⌘';
      if (key === 'control' || key === 'ctrl') return 'Ctrl';
      if (key === 'shift') return '⇧';
      if (key === 'alt' || key === 'option' || key === 'opt') return IS_MAC ? '⌥' : 'Alt';
      if (key === 'space') return 'Space';
      if (key === 'esc' || key === 'escape') return 'Esc';
      if (key.length === 1) return p.toUpperCase();
      return p;
    })
    .join(IS_MAC ? '' : '+');
}

/**
 * Convert a KeyboardEvent into a Tauri-compatible shortcut string, e.g.
 *   "CommandOrControl+Shift+A"
 * Returns null if the combination is not usable (e.g. only modifiers, or bare letter).
 */
export function keyEventToShortcut(event: KeyboardEvent): string | null {
  const parts: string[] = [];
  // On mac, event.metaKey = ⌘; on others, event.ctrlKey = Ctrl. Use CommandOrControl for portability
  // if the user only pressed the platform's primary modifier alongside another.
  const usesCmd = event.metaKey;
  const usesCtrl = event.ctrlKey;
  if (usesCmd && usesCtrl) {
    parts.push('Command');
    parts.push('Control');
  } else if (usesCmd || usesCtrl) {
    parts.push('CommandOrControl');
  }
  if (event.altKey) parts.push('Alt');
  if (event.shiftKey) parts.push('Shift');

  const key = normalizeKey(event.key, event.code);
  if (!key) return null;
  // Reject bare letters (no modifier) — those aren't valid global shortcuts on macOS
  const hasModifier = parts.length > 0;
  if (!hasModifier) return null;
  parts.push(key);
  return parts.join('+');
}

function normalizeKey(key: string, code: string): string | null {
  const modKeys = new Set([
    'Meta',
    'Control',
    'Shift',
    'Alt',
    'CapsLock',
    'ContextMenu',
    'OS',
    'Fn',
    'Dead'
  ]);
  if (modKeys.has(key)) return null;
  if (key === ' ' || key === 'Space' || code === 'Space') return 'Space';
  if (key === 'Escape') return 'Escape';
  if (key === 'Tab') return 'Tab';
  if (key === 'Enter' || key === 'Return') return 'Enter';
  if (key === 'Backspace') return 'Backspace';
  if (key.startsWith('Arrow')) return key.replace('Arrow', ''); // Up/Down/Left/Right
  // Function keys F1..F24
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(key)) return key;
  // Single character keys: normalize to uppercase for Tauri
  if (key.length === 1) return key.toUpperCase();
  return key; // Home/End/Insert/Delete etc — Tauri accepts these
}
