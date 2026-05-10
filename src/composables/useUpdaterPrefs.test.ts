import { describe, it, expect } from 'vitest';
import { useUpdaterPrefs } from './useUpdaterPrefs';
import type { KVStorage } from './useSidebarState';

function fakeStorage(initial: Record<string, string> = {}) {
  const data = new Map(Object.entries(initial));
  const storage: KVStorage = {
    getItem: (k) => (data.has(k) ? data.get(k)! : null),
    setItem: (k, v) => { data.set(k, v); }
  };
  return { storage, data };
}

describe('useUpdaterPrefs', () => {
  it('autoCheck defaults to true', () => {
    const { storage } = fakeStorage();
    const { autoCheck } = useUpdaterPrefs(storage);
    expect(autoCheck.value).toBe(true);
  });

  it('autoCheck restores false from storage', () => {
    const { storage } = fakeStorage({ 'attool.updater.autoCheck': '0' });
    const { autoCheck } = useUpdaterPrefs(storage);
    expect(autoCheck.value).toBe(false);
  });

  it('setAutoCheck flips and persists', () => {
    const { storage, data } = fakeStorage();
    const { autoCheck, setAutoCheck } = useUpdaterPrefs(storage);
    setAutoCheck(false);
    expect(autoCheck.value).toBe(false);
    expect(data.get('attool.updater.autoCheck')).toBe('0');
    setAutoCheck(true);
    expect(data.get('attool.updater.autoCheck')).toBe('1');
  });

  it('skippedVersion is null by default', () => {
    const { storage } = fakeStorage();
    const { skippedVersion } = useUpdaterPrefs(storage);
    expect(skippedVersion.value).toBeNull();
  });

  it('skipVersion persists value and shouldSkip returns true for it only', () => {
    const { storage, data } = fakeStorage();
    const { skipVersion, shouldSkip } = useUpdaterPrefs(storage);
    skipVersion('0.2.0');
    expect(shouldSkip('0.2.0')).toBe(true);
    expect(shouldSkip('0.3.0')).toBe(false);
    expect(data.get('attool.updater.skipped')).toBe('0.2.0');
  });
});
