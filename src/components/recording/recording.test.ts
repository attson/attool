import { describe, expect, it } from 'vitest';
import {
  buildRecordingFileName,
  formatRecordingDuration,
  formatRecordingSize,
  selectRecordingProfile
} from './recording';

describe('selectRecordingProfile', () => {
  it('优先选择带 Opus 的 VP9 WebM', () => {
    const supported = new Set(['video/webm;codecs=vp9,opus', 'video/mp4']);

    expect(selectRecordingProfile((mime) => supported.has(mime))).toEqual({
      mimeType: 'video/webm;codecs=vp9,opus',
      extension: 'webm',
      label: 'WebM (VP9)'
    });
  });

  it('WebM 不可用时回退到 MP4', () => {
    expect(selectRecordingProfile((mime) => mime === 'video/mp4')).toEqual({
      mimeType: 'video/mp4',
      extension: 'mp4',
      label: 'MP4 (H.264)'
    });
  });

  it('没有受支持的编码器时返回 null', () => {
    expect(selectRecordingProfile(() => false)).toBeNull();
  });
});

describe('recording formatters', () => {
  it('格式化不足一小时和超过一小时的时长', () => {
    expect(formatRecordingDuration(65_000)).toBe('01:05');
    expect(formatRecordingDuration(3_665_000)).toBe('1:01:05');
  });

  it('格式化录制文件大小', () => {
    expect(formatRecordingSize(512)).toBe('512 B');
    expect(formatRecordingSize(1536)).toBe('1.5 KB');
    expect(formatRecordingSize(2.5 * 1024 * 1024)).toBe('2.5 MB');
  });

  it('生成不含路径非法字符的默认文件名', () => {
    expect(buildRecordingFileName(new Date('2026-10-13T08:09:10.123Z'), 'webm')).toBe(
      'AT-Tool-recording-2026-10-13-08-09-10.webm'
    );
  });
});
