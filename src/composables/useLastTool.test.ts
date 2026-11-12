import { describe, it, expect } from 'vitest';
import { useLastTool } from './useLastTool';
import type { KVStorage } from './useSidebarState';

function fakeStorage(initial: Record<string, string> = {}) {
  const data = new Map(Object.entries(initial));
  const storage: KVStorage = {
    getItem: (k) => (data.has(k) ? data.get(k)! : null),
    setItem: (k, v) => { data.set(k, v); }
  };
  return { storage, data };
}

describe('useLastTool', () => {
  it('returns null when no last tool stored', () => {
    const { storage } = fakeStorage();
    const { lastToolId } = useLastTool(storage);
    expect(lastToolId.value).toBeNull();
  });

  it('returns stored last tool id', () => {
    const { storage } = fakeStorage({ 'attool.lastTool': 'aria2' });
    const { lastToolId } = useLastTool(storage);
    expect(lastToolId.value).toBe('aria2');
  });

  it('maps the removed recording tool to the integrated image tool', () => {
    const { storage } = fakeStorage({ 'attool.lastTool': 'recording' });
    const { lastToolId } = useLastTool(storage);
    expect(lastToolId.value).toBe('image');
  });

  it('maps the removed codec tool to the external application', () => {
    const { storage } = fakeStorage({ 'attool.lastTool': 'codec' });
    const { lastToolId } = useLastTool(storage);
    expect(lastToolId.value).toBe('plugin:com.attson.codec');
  });

  it('maps the removed time tool to the external application', () => {
    const { storage } = fakeStorage({ 'attool.lastTool': 'time' });
    const { lastToolId } = useLastTool(storage);
    expect(lastToolId.value).toBe('plugin:com.attson.time');
  });

  it('maps the removed JSON tool to the external application', () => {
    const { storage } = fakeStorage({ 'attool.lastTool': 'json' });
    const { lastToolId } = useLastTool(storage);
    expect(lastToolId.value).toBe('plugin:com.attson.json');
  });

  it('remember persists tool id and updates state', () => {
    const { storage, data } = fakeStorage();
    const { lastToolId, remember } = useLastTool(storage);
    remember('template');
    expect(lastToolId.value).toBe('template');
    expect(data.get('attool.lastTool')).toBe('template');
  });
});
