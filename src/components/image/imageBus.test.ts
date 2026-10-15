import { describe, expect, it } from 'vitest';
import {
  publishRecordingResult,
  requestCaptureMode,
  usePendingRecordingResult,
  useRequestedCaptureMode
} from './imageBus';

describe('capture mode requests', () => {
  it('hands a recording toggle request to the capture workspace once', () => {
    requestCaptureMode('recording', true);
    const request = useRequestedCaptureMode();

    expect(request.consume()).toEqual({ mode: 'recording', toggle: true });
    expect(request.consume()).toBeNull();
  });
});

describe('recording result handoff', () => {
  it('hands a completed overlay recording to the capture workspace once', () => {
    publishRecordingResult({
      outputPath: '/tmp/capture.webm',
      size: 4096,
      sourceLabel: '区域 800 × 600',
      elapsedMs: 3200,
      extension: 'webm'
    });
    const result = usePendingRecordingResult();

    expect(result.consume()).toEqual({
      outputPath: '/tmp/capture.webm',
      size: 4096,
      sourceLabel: '区域 800 × 600',
      elapsedMs: 3200,
      extension: 'webm'
    });
    expect(result.consume()).toBeNull();
  });
});
