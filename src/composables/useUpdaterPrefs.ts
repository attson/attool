import { ref } from 'vue';
import type { KVStorage } from './useSidebarState';

const STORAGE_KEY_AUTO_CHECK = 'attool.updater.autoCheck';
const STORAGE_KEY_SKIPPED = 'attool.updater.skipped';

export function useUpdaterPrefs(storage: KVStorage = localStorage) {
  const autoCheck = ref(storage.getItem(STORAGE_KEY_AUTO_CHECK) !== '0');
  const skippedVersion = ref<string | null>(storage.getItem(STORAGE_KEY_SKIPPED));

  function setAutoCheck(v: boolean) {
    autoCheck.value = v;
    storage.setItem(STORAGE_KEY_AUTO_CHECK, v ? '1' : '0');
  }

  function skipVersion(v: string) {
    skippedVersion.value = v;
    storage.setItem(STORAGE_KEY_SKIPPED, v);
  }

  function shouldSkip(v: string): boolean {
    return skippedVersion.value === v;
  }

  return { autoCheck, setAutoCheck, skippedVersion, skipVersion, shouldSkip };
}
