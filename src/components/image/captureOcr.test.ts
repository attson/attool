import { describe, expect, it } from 'vitest';
import {
  captureOcrAllowsGlobalShortcut,
  initialCaptureOcrState,
  nextCaptureOcrState
} from './captureOcr';

describe('capture OCR state', () => {
  it('shows a non-empty recognition result without discarding it on close', () => {
    const recognizing = nextCaptureOcrState(initialCaptureOcrState(), { type: 'start' });
    expect(recognizing).toEqual({ phase: 'recognizing', text: '', error: '', requestId: 1 });

    const success = nextCaptureOcrState(recognizing, {
      type: 'resolve',
      requestId: recognizing.requestId,
      text: '人员与组织'
    });
    expect(success).toEqual({ phase: 'success', text: '人员与组织', error: '', requestId: 1 });

    expect(nextCaptureOcrState(success, { type: 'close' })).toEqual({
      phase: 'idle',
      text: '人员与组织',
      error: '',
      requestId: 1
    });
  });

  it('distinguishes empty recognition from a failed request', () => {
    const recognizing = nextCaptureOcrState(initialCaptureOcrState(), { type: 'start' });

    expect(nextCaptureOcrState(recognizing, {
      type: 'resolve',
      requestId: recognizing.requestId,
      text: '  \n'
    })).toEqual({
      phase: 'empty',
      text: '',
      error: '',
      requestId: 1
    });
    expect(nextCaptureOcrState(recognizing, {
      type: 'fail',
      requestId: recognizing.requestId,
      error: 'OCR 不可用'
    })).toEqual({
      phase: 'error',
      text: '',
      error: 'OCR 不可用',
      requestId: 1
    });
  });

  it('keeps the previous result unless a later recognition succeeds', () => {
    const previous = { phase: 'success' as const, text: '旧结果', error: '', requestId: 4 };
    const recognizing = nextCaptureOcrState(previous, { type: 'start' });
    const failed = nextCaptureOcrState(recognizing, {
      type: 'fail',
      requestId: recognizing.requestId,
      error: '识别失败'
    });

    expect(failed.text).toBe('旧结果');
    const retrying = nextCaptureOcrState(failed, { type: 'start' });
    expect(nextCaptureOcrState(retrying, {
      type: 'resolve',
      requestId: retrying.requestId,
      text: '新结果'
    }).text).toBe('新结果');
  });

  it('ignores a result from a request invalidated by resetting the capture', () => {
    const firstRequest = nextCaptureOcrState(initialCaptureOcrState(), { type: 'start' });
    const reset = nextCaptureOcrState(firstRequest, { type: 'reset' });
    const secondRequest = nextCaptureOcrState(reset, { type: 'start' });

    const staleResult = nextCaptureOcrState(secondRequest, {
      type: 'resolve',
      requestId: firstRequest.requestId,
      text: '旧选区结果'
    });

    expect(staleResult).toEqual(secondRequest);
    expect(nextCaptureOcrState(secondRequest, {
      type: 'resolve',
      requestId: secondRequest.requestId,
      text: '新选区结果'
    }).text).toBe('新选区结果');
  });

  it('keeps editing keys inside the result panel while preserving Escape cancellation', () => {
    expect(captureOcrAllowsGlobalShortcut('success', 'Enter')).toBe(false);
    expect(captureOcrAllowsGlobalShortcut('success', 's')).toBe(false);
    expect(captureOcrAllowsGlobalShortcut('recognizing', 'Enter')).toBe(false);
    expect(captureOcrAllowsGlobalShortcut('success', 'Escape')).toBe(true);
    expect(captureOcrAllowsGlobalShortcut('idle', 'Enter')).toBe(true);
  });
});
