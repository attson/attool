import { describe, it, expect } from 'vitest';
import { useSidebarState, type KVStorage } from './useSidebarState';

function fakeStorage(initial: Record<string, string> = {}) {
  const data = new Map(Object.entries(initial));
  const storage: KVStorage = {
    getItem: (k) => (data.has(k) ? data.get(k)! : null),
    setItem: (k, v) => { data.set(k, v); }
  };
  return { storage, data };
}

describe('useSidebarState', () => {
  it('defaults to expanded when storage is empty', () => {
    const { storage } = fakeStorage();
    const { collapsed } = useSidebarState(storage);
    expect(collapsed.value).toBe(false);
  });

  it('restores collapsed state from storage', () => {
    const { storage } = fakeStorage({ 'attool.sidebar.collapsed': '1' });
    const { collapsed } = useSidebarState(storage);
    expect(collapsed.value).toBe(true);
  });

  it('toggle flips and persists value', () => {
    const { storage, data } = fakeStorage();
    const { collapsed, toggle } = useSidebarState(storage);
    toggle();
    expect(collapsed.value).toBe(true);
    expect(data.get('attool.sidebar.collapsed')).toBe('1');
    toggle();
    expect(collapsed.value).toBe(false);
    expect(data.get('attool.sidebar.collapsed')).toBe('0');
  });
});
