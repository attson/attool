import type { ClipboardHistoryFilter, ClipboardHistoryItem } from '../types/clipboard';

export function formatClipboardPreview(value: string, maxLength = 120): string {
  const normalized = value.replace(/\s+/g, ' ').trim();
  if (normalized.length <= maxLength) return normalized;
  return `${normalized.slice(0, Math.max(0, maxLength - 3))}...`;
}

export function isFilePathText(value: string): boolean {
  const lines = value
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
  if (lines.length === 0) return false;
  return lines.every((line) => line.startsWith('/') || line.startsWith('~/') || /^file:\/\//i.test(line) || /^[A-Za-z]:\\/.test(line));
}

export function filterClipboardItems(
  items: ClipboardHistoryItem[],
  filter: ClipboardHistoryFilter,
): ClipboardHistoryItem[] {
  const query = filter.query.trim().toLocaleLowerCase();
  return items.filter((item) => {
    if (filter.kind !== 'all' && item.kind !== filter.kind) return false;
    if (!query) return true;
    const haystack = [item.preview, item.contentText, ...item.filePaths].join('\n').toLocaleLowerCase();
    return haystack.includes(query);
  });
}
