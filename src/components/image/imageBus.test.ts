import { describe, expect, it } from 'vitest';
import { requestCaptureMode, useRequestedCaptureMode } from './imageBus';

describe('capture mode requests', () => {
  it('hands a recording toggle request to the capture workspace once', () => {
    requestCaptureMode('recording', true);
    const request = useRequestedCaptureMode();

    expect(request.consume()).toEqual({ mode: 'recording', toggle: true });
    expect(request.consume()).toBeNull();
  });
});
