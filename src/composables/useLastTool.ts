import { ref } from 'vue';
import type { KVStorage } from './useSidebarState';

const STORAGE_KEY = 'attool.lastTool';

export function useLastTool(storage: KVStorage = localStorage) {
  const lastToolId = ref<string | null>(storage.getItem(STORAGE_KEY));

  function remember(id: string) {
    lastToolId.value = id;
    storage.setItem(STORAGE_KEY, id);
  }

  return { lastToolId, remember };
}
