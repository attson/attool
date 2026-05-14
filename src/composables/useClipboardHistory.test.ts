import { describe, expect, it } from 'vitest';
import type { ClipboardHistoryItem } from '../types/clipboard';
import { createClipboardHistoryApi, useClipboardHistory } from './useClipboardHistory';

const item: ClipboardHistoryItem = {
  id: 'text-1',
  kind: 'text',
  preview: 'hello',
  contentText: 'hello',
  filePaths: [],
  assetUrl: null,
  isPinned: false,
  createdAt: '2026-05-15T10:00:00+08:00',
  lastCopiedAt: null,
};

describe('useClipboardHistory', () => {
  it('loads items and filters locally', async () => {
    const history = useClipboardHistory({
      listItems: async () => [item],
      deleteItem: async () => undefined,
      setPinned: async () => undefined,
      clearHistory: async () => undefined,
    });

    await history.refresh();
    history.query.value = 'hello';

    expect(history.items.value).toEqual([item]);
    expect(history.filteredItems.value.map((entry) => entry.id)).toEqual(['text-1']);
  });

  it('refreshes after destructive actions', async () => {
    let deleted = '';
    let calls = 0;
    const history = useClipboardHistory({
      listItems: async () => {
        calls += 1;
        return [item];
      },
      deleteItem: async (id) => {
        deleted = id;
      },
      setPinned: async () => undefined,
      clearHistory: async () => undefined,
    });

    await history.deleteItem('text-1');

    expect(deleted).toBe('text-1');
    expect(calls).toBe(1);
  });

  it('creates an invoke-backed api with stable command names', () => {
    const calls: Array<{ command: string; args?: unknown }> = [];
    const api = createClipboardHistoryApi(async (command, args) => {
      calls.push({ command, args });
      return [];
    });

    api.listItems({ kind: 'text', query: 'abc' });

    expect(calls[0]).toEqual({ command: 'list_clipboard_items', args: { kind: 'text', query: 'abc' } });
  });
});
