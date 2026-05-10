import { describe, it, expect } from 'vitest';
import { useTheme, type ThemeRoot } from './useTheme';
import type { KVStorage } from './useSidebarState';

function fakeStorage(initial: Record<string, string> = {}) {
  const data = new Map(Object.entries(initial));
  const storage: KVStorage = {
    getItem: (k) => (data.has(k) ? data.get(k)! : null),
    setItem: (k, v) => { data.set(k, v); }
  };
  return { storage, data };
}

function fakeRoot() {
  const attrs = new Map<string, string>();
  const root: ThemeRoot = {
    setAttribute: (n, v) => { attrs.set(n, v); }
  };
  return { root, attrs };
}

describe('useTheme', () => {
  it('defaults to dark when storage is empty', () => {
    const { storage } = fakeStorage();
    const { root } = fakeRoot();
    const { theme } = useTheme(storage, root);
    expect(theme.value).toBe('dark');
  });

  it('restores light from storage', () => {
    const { storage } = fakeStorage({ 'attool.theme': 'light' });
    const { root } = fakeRoot();
    const { theme } = useTheme(storage, root);
    expect(theme.value).toBe('light');
  });

  it('applies theme attribute to root on init', () => {
    const { storage } = fakeStorage({ 'attool.theme': 'light' });
    const { root, attrs } = fakeRoot();
    useTheme(storage, root);
    expect(attrs.get('data-theme')).toBe('light');
  });

  it('toggle flips theme, persists, and updates root', () => {
    const { storage, data } = fakeStorage();
    const { root, attrs } = fakeRoot();
    const { theme, toggle } = useTheme(storage, root);

    toggle();
    expect(theme.value).toBe('light');
    expect(data.get('attool.theme')).toBe('light');
    expect(attrs.get('data-theme')).toBe('light');

    toggle();
    expect(theme.value).toBe('dark');
    expect(data.get('attool.theme')).toBe('dark');
    expect(attrs.get('data-theme')).toBe('dark');
  });
});
