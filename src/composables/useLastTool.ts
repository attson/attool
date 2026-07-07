import { ref } from 'vue';
import type { KVStorage } from './useSidebarState';

const STORAGE_KEY = 'attool.lastTool';

const LEGACY_ID_MAP: Record<string, string> = {
  douyin: 'video-link'
};

export function useLastTool(storage: KVStorage = localStorage) {
  const stored = storage.getItem(STORAGE_KEY);
  const mapped = stored ? LEGACY_ID_MAP[stored] ?? stored : null;
  const lastToolId = ref<string | null>(mapped);

  function remember(id: string) {
    lastToolId.value = id;
    storage.setItem(STORAGE_KEY, id);
  }

  return { lastToolId, remember };
}
