export type DownloadStatus = 'queued' | 'running' | 'completed' | 'failed' | 'cancelled';

export type DownloadEventPayload = {
  id: string;
  status: DownloadStatus;
  progress: number;
  speed?: string | null;
  eta?: string | null;
  message?: string | null;
};

export type DownloadTask = DownloadEventPayload & {
  url: string;
  fileName?: string;
  createdAt: string;
};

export type StartDownloadRequest = {
  url: string;
  downloadDir: string;
  fileName?: string;
  connections: number;
  split: number;
  minSplitSize: string;
};
