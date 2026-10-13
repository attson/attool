export interface RecordingProfile {
  mimeType: string;
  extension: 'webm' | 'mp4';
  label: string;
}

const RECORDING_PROFILES: RecordingProfile[] = [
  { mimeType: 'video/webm;codecs=vp9,opus', extension: 'webm', label: 'WebM (VP9)' },
  { mimeType: 'video/webm;codecs=vp8,opus', extension: 'webm', label: 'WebM (VP8)' },
  { mimeType: 'video/webm', extension: 'webm', label: 'WebM' },
  { mimeType: 'video/mp4;codecs=avc1.42E01E,mp4a.40.2', extension: 'mp4', label: 'MP4 (H.264)' },
  { mimeType: 'video/mp4', extension: 'mp4', label: 'MP4 (H.264)' }
];

export function selectRecordingProfile(
  isSupported: (mimeType: string) => boolean
): RecordingProfile | null {
  return RECORDING_PROFILES.find((profile) => isSupported(profile.mimeType)) ?? null;
}

export function formatRecordingDuration(milliseconds: number): string {
  const totalSeconds = Math.max(0, Math.floor(milliseconds / 1000));
  const seconds = totalSeconds % 60;
  const minutes = Math.floor(totalSeconds / 60) % 60;
  const hours = Math.floor(totalSeconds / 3600);
  const parts = [minutes, seconds].map((value) => String(value).padStart(2, '0'));
  return hours > 0 ? `${hours}:${parts.join(':')}` : parts.join(':');
}

export function formatRecordingSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${trimDecimal(bytes / 1024)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${trimDecimal(bytes / 1024 / 1024)} MB`;
  return `${trimDecimal(bytes / 1024 / 1024 / 1024)} GB`;
}

function trimDecimal(value: number): string {
  return value.toFixed(1).replace(/\.0$/, '');
}

export function buildRecordingFileName(now: Date, extension: string): string {
  const timestamp = now.toISOString().slice(0, 19).replace(/[T:]/g, '-');
  return `AT-Tool-recording-${timestamp}.${extension}`;
}
