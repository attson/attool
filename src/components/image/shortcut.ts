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

  // Prefer `code` (physical key) for letters/digits/function keys.
  // With Alt/Option held on macOS, `event.key` becomes a "special" char (⌥A → Å),
  // which Tauri's registrar rejects. `event.code` stays "KeyA" regardless.
  if (code) {
    const letter = code.match(/^Key([A-Z])$/);
    if (letter) return letter[1];
    const digit = code.match(/^Digit([0-9])$/);
    if (digit) return digit[1];
    const numpad = code.match(/^Numpad([0-9])$/);
    if (numpad) return `Num${numpad[1]}`;
    const fn = code.match(/^F([1-9]|1[0-9]|2[0-4])$/);
    if (fn) return `F${fn[1]}`;
    if (code === 'Space') return 'Space';
    if (code === 'Escape') return 'Escape';
    if (code === 'Tab') return 'Tab';
    if (code === 'Enter' || code === 'NumpadEnter') return 'Enter';
    if (code === 'Backspace') return 'Backspace';
    if (code === 'ArrowUp') return 'Up';
    if (code === 'ArrowDown') return 'Down';
    if (code === 'ArrowLeft') return 'Left';
    if (code === 'ArrowRight') return 'Right';
    if (code === 'Home' || code === 'End' || code === 'PageUp' || code === 'PageDown') return code;
    if (code === 'Insert' || code === 'Delete') return code;
    if (code === 'Minus') return '-';
    if (code === 'Equal') return '=';
    if (code === 'BracketLeft') return '[';
    if (code === 'BracketRight') return ']';
    if (code === 'Backslash') return '\\';
    if (code === 'Semicolon') return ';';
    if (code === 'Quote') return "'";
    if (code === 'Comma') return ',';
    if (code === 'Period') return '.';
    if (code === 'Slash') return '/';
    if (code === 'Backquote') return '`';
  }

  // Fallback to `key` when we don't recognize `code`
  if (key === ' ') return 'Space';
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(key)) return key;
  if (key.length === 1) return key.toUpperCase();
  return key;
}
