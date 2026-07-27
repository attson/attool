import { describe, it, expect } from 'vitest';
import {
  useSidebarWidth,
  DEFAULT_WIDTH,
  MIN_WIDTH,
  MAX_WIDTH
} from './useSidebarWidth';
import type { KVStorage } from './useSidebarState';

function fakeStorage(initial: Record<string, string> = {}) {
  const data = new Map(Object.entries(initial));
  const storage: KVStorage = {
    getItem: (k) => (data.has(k) ? data.get(k)! : null),
    setItem: (k, v) => { data.set(k, v); }
  };
  return { storage, data };
}

const KEY = 'attool.http.sidebar.width';

describe('useSidebarWidth', () => {
  it('defaults to DEFAULT_WIDTH when storage is empty', () => {
    const { storage } = fakeStorage();
    const { width } = useSidebarWidth(storage);
    expect(width.value).toBe(DEFAULT_WIDTH);
  });

  it('restores a valid stored width', () => {
    const { storage } = fakeStorage({ [KEY]: '320' });
    const { width } = useSidebarWidth(storage);
    expect(width.value).toBe(320);
  });

  it('clamps a stored width below MIN', () => {
    const { storage } = fakeStorage({ [KEY]: '50' });
    const { width } = useSidebarWidth(storage);
    expect(width.value).toBe(MIN_WIDTH);
  });

  it('clamps a stored width above MAX', () => {
    const { storage } = fakeStorage({ [KEY]: '9999' });
    const { width } = useSidebarWidth(storage);
    expect(width.value).toBe(MAX_WIDTH);
  });

  it('falls back to default for a non-numeric stored value', () => {
    const { storage } = fakeStorage({ [KEY]: 'abc' });
    const { width } = useSidebarWidth(storage);
    expect(width.value).toBe(DEFAULT_WIDTH);
  });

  it('setWidth clamps and persists', () => {
    const { storage, data } = fakeStorage();
    const { width, setWidth } = useSidebarWidth(storage);
    setWidth(300);
    expect(width.value).toBe(300);
    expect(data.get(KEY)).toBe('300');
    setWidth(10_000);
    expect(width.value).toBe(MAX_WIDTH);
    expect(data.get(KEY)).toBe(String(MAX_WIDTH));
  });

  it('reset restores default width and persists', () => {
    const { storage, data } = fakeStorage({ [KEY]: '400' });
    const { width, reset } = useSidebarWidth(storage);
    expect(width.value).toBe(400);
    reset();
    expect(width.value).toBe(DEFAULT_WIDTH);
    expect(data.get(KEY)).toBe(String(DEFAULT_WIDTH));
  });
});
