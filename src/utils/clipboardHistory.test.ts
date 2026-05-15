import { describe, expect, it } from 'vitest';
import type { ClipboardHistoryItem } from '../types/clipboard';
import {
  DEFAULT_CLIPBOARD_SHORTCUT,
  filterClipboardItems,
  formatClipboardPreview,
  formatShortcutLabel,
  isFilePathText,
} from './clipboardHistory';

const ITEMS: ClipboardHistoryItem[] = [
  {
    id: 'text-1',
    kind: 'text',
    preview: 'const total = 42;',
    contentText: 'const total = 42;',
    filePaths: [],
    assetPath: null,
    assetUrl: null,
    isPinned: false,
    createdAt: '2026-05-15T10:00:00+08:00',
    lastCopiedAt: null,
  },
  {
    id: 'image-1',
    kind: 'image',
    preview: 'Image 1200 x 800',
    contentText: '',
    filePaths: [],
    assetPath: null,
    assetUrl: 'asset://image-1.png',
    isPinned: true,
    createdAt: '2026-05-15T10:01:00+08:00',
    lastCopiedAt: null,
  },
  {
    id: 'files-1',
    kind: 'files',
    preview: '2 files',
    contentText: '/Users/a/logo.png\n/Users/a/spec.psd',
    filePaths: ['/Users/a/logo.png', '/Users/a/spec.psd'],
    assetPath: null,
    assetUrl: null,
    isPinned: false,
    createdAt: '2026-05-15T10:02:00+08:00',
    lastCopiedAt: null,
  },
];

describe('clipboardHistory helpers', () => {
  it('filters by type and text query', () => {
    expect(filterClipboardItems(ITEMS, { kind: 'all', query: 'total' }).map((item) => item.id)).toEqual(['text-1']);
    expect(filterClipboardItems(ITEMS, { kind: 'image', query: '' }).map((item) => item.id)).toEqual(['image-1']);
    expect(filterClipboardItems(ITEMS, { kind: 'files', query: 'spec' }).map((item) => item.id)).toEqual(['files-1']);
  });

  it('formats concise previews without losing useful content', () => {
    expect(formatClipboardPreview('  hello\n\nworld  ', 20)).toBe('hello world');
    expect(formatClipboardPreview('a'.repeat(80), 12)).toBe('aaaaaaaaa...');
  });

  it('detects newline separated local file paths', () => {
    expect(isFilePathText('/Users/attson/a.png\n/Users/attson/b.psd')).toBe(true);
    expect(isFilePathText('https://example.com/a.png')).toBe(false);
    expect(isFilePathText('plain copied text')).toBe(false);
  });

  it('uses a default shortcut that avoids paste-without-formatting conflicts', () => {
    expect(DEFAULT_CLIPBOARD_SHORTCUT).toBe('CommandOrControl+Alt+V');
    expect(formatShortcutLabel(DEFAULT_CLIPBOARD_SHORTCUT)).toBe('Command/Ctrl + Alt + V');
  });
});
