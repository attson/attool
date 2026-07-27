import { describe, it, expect } from 'vitest';
import { useCollapsedFolders } from './useCollapsedFolders';
import type { KVStorage } from './useSidebarState';

function fakeStorage(initial: Record<string, string> = {}) {
  const data = new Map(Object.entries(initial));
  const storage: KVStorage = {
    getItem: (k) => (data.has(k) ? data.get(k)! : null),
    setItem: (k, v) => { data.set(k, v); }
  };
  return { storage, data };
}

const KEY = 'attool.http.sidebar.expandedFolders';

describe('useCollapsedFolders', () => {
  it('defaults to collapsed for every folder', () => {
    const { storage } = fakeStorage();
    const { isCollapsed } = useCollapsedFolders(storage);
    expect(isCollapsed('a')).toBe(true);
  });

  it('restores expanded ids from storage', () => {
    const { storage } = fakeStorage({ [KEY]: JSON.stringify(['a', 'b']) });
    const { isCollapsed } = useCollapsedFolders(storage);
    expect(isCollapsed('a')).toBe(false);
    expect(isCollapsed('b')).toBe(false);
    expect(isCollapsed('c')).toBe(true);
  });

  it('toggle expands then collapses, persisting the expanded set', () => {
    const { storage, data } = fakeStorage();
    const { isCollapsed, toggle } = useCollapsedFolders(storage);
    toggle('a');
    expect(isCollapsed('a')).toBe(false);
    expect(JSON.parse(data.get(KEY)!)).toEqual(['a']);
    toggle('a');
    expect(isCollapsed('a')).toBe(true);
    expect(JSON.parse(data.get(KEY)!)).toEqual([]);
  });

  it('ignores malformed stored value (stays collapsed)', () => {
    const { storage } = fakeStorage({ [KEY]: 'not-json' });
    const { isCollapsed } = useCollapsedFolders(storage);
    expect(isCollapsed('a')).toBe(true);
  });
});
