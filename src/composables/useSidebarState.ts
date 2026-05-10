import { ref } from 'vue';

export interface KVStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

const STORAGE_KEY = 'attool.sidebar.collapsed';

export function useSidebarState(storage: KVStorage = localStorage) {
  const initial = storage.getItem(STORAGE_KEY) === '1';
  const collapsed = ref(initial);

  function toggle() {
    collapsed.value = !collapsed.value;
    storage.setItem(STORAGE_KEY, collapsed.value ? '1' : '0');
  }

  return { collapsed, toggle };
}
