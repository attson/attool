import { computed, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { ClipboardFilterKind, ClipboardHistoryItem } from '../types/clipboard';
import { filterClipboardItems } from '../utils/clipboardHistory';

export interface ClipboardHistoryApi {
  listItems(filter: { kind: ClipboardFilterKind; query: string }): Promise<ClipboardHistoryItem[]>;
  deleteItem(id: string): Promise<void>;
  setPinned(id: string, isPinned: boolean): Promise<void>;
  clearHistory(): Promise<void>;
  restoreItem(id: string): Promise<void>;
}

export function createClipboardHistoryApi(invoker = invoke): ClipboardHistoryApi {
  return {
    listItems(filter) {
      return invoker<ClipboardHistoryItem[]>('list_clipboard_items', {
        kind: filter.kind === 'all' ? null : filter.kind,
        query: filter.query,
      });
    },
    deleteItem(id) {
      return invoker('delete_clipboard_item', { id });
    },
    setPinned(id, isPinned) {
      return invoker('set_clipboard_item_pinned', { id, isPinned });
    },
    clearHistory() {
      return invoker('clear_clipboard_history');
    },
    restoreItem(id) {
      return invoker('restore_clipboard_item', { id });
    },
  };
}

export function useClipboardHistory(api: ClipboardHistoryApi = createClipboardHistoryApi()) {
  const items = ref<ClipboardHistoryItem[]>([]);
  const kind = ref<ClipboardFilterKind>('all');
  const query = ref('');
  const loading = ref(false);
  const error = ref<string | null>(null);

  const filteredItems = computed(() => filterClipboardItems(items.value, { kind: kind.value, query: query.value }));

  async function refresh() {
    loading.value = true;
    error.value = null;
    try {
      items.value = await api.listItems({ kind: kind.value, query: query.value });
    } catch (caught) {
      error.value = caught instanceof Error ? caught.message : String(caught);
    } finally {
      loading.value = false;
    }
  }

  async function deleteItem(id: string) {
    await api.deleteItem(id);
    await refresh();
  }

  async function setPinned(id: string, isPinned: boolean) {
    await api.setPinned(id, isPinned);
    await refresh();
  }

  async function clearHistory() {
    await api.clearHistory();
    await refresh();
  }

  async function restoreItem(id: string) {
    await api.restoreItem(id);
    await refresh();
  }

  return { items, filteredItems, kind, query, loading, error, refresh, deleteItem, setPinned, clearHistory, restoreItem };
}
