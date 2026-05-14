export type ClipboardItemKind = 'text' | 'image' | 'files';
export type ClipboardFilterKind = ClipboardItemKind | 'all';

export interface ClipboardHistoryItem {
  id: string;
  kind: ClipboardItemKind;
  preview: string;
  contentText: string;
  filePaths: string[];
  assetPath: string | null;
  assetUrl: string | null;
  isPinned: boolean;
  createdAt: string;
  lastCopiedAt: string | null;
}

export interface ClipboardHistoryFilter {
  kind: ClipboardFilterKind;
  query: string;
}

export interface ClipboardHistorySettings {
  captureEnabled: boolean;
  retentionLimit: number;
  shortcut: string;
}
