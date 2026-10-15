import { describe, expect, it } from 'vitest';
import {
  expandCaptureInputRegion,
  mapSelectionToSource,
  positionCaptureToolbar,
  scaleRecordingRegion
} from './captureRegion';

describe('mapSelectionToSource', () => {
  it('maps CSS coordinates to source pixels and shrinks dimensions to even values', () => {
    expect(mapSelectionToSource(
      { x: 100.4, y: 50.6, width: 400.9, height: 200.8 },
      { width: 1000, height: 500 },
      { width: 2000, height: 1000 }
    )).toEqual({ x: 201, y: 102, width: 800, height: 400 });
  });

  it('clamps a selection to the source bounds', () => {
    expect(mapSelectionToSource(
      { x: -20, y: 450, width: 1100, height: 100 },
      { width: 1000, height: 500 },
      { width: 1920, height: 1080 }
    )).toEqual({ x: 0, y: 972, width: 1920, height: 108 });
  });

  it('rejects a selection that cannot produce a two-pixel crop', () => {
    expect(() => mapSelectionToSource(
      { x: 10, y: 10, width: 0.4, height: 20 },
      { width: 1000, height: 500 },
      { width: 1000, height: 500 }
    )).toThrow('录制区域至少需要 2 × 2 像素');
  });
});

describe('scaleRecordingRegion', () => {
  it('scales after cropping without upscaling', () => {
    expect(scaleRecordingRegion({ width: 2560, height: 1440 }, '1080')).toEqual({
      width: 1920,
      height: 1080
    });
    expect(scaleRecordingRegion({ width: 1918, height: 1078 }, '1080')).toEqual({
      width: 1918,
      height: 1078
    });
  });
});

describe('positionCaptureToolbar', () => {
  it('moves the toolbar above a selection when the desktop work area ends first', () => {
    expect(positionCaptureToolbar(
      { x: 80, y: 168, width: 1537, height: 700 },
      { width: 1713, height: 893 },
      { width: 1020, height: 58 },
      8,
      'end'
    )).toEqual({ left: 597, top: 102 });
  });

  it('keeps a full-screen toolbar inside the usable work area', () => {
    expect(positionCaptureToolbar(
      { x: 0, y: 0, width: 1713, height: 933 },
      { width: 1713, height: 893 },
      { width: 310, height: 48 },
      10,
      'center'
    )).toEqual({ left: 701.5, top: 835 });
  });
});

describe('expandCaptureInputRegion', () => {
  it('adds hit padding without extending beyond the overlay', () => {
    expect(expandCaptureInputRegion(
      { x: 1690.4, y: 880.2, width: 40, height: 30 },
      { width: 1713, height: 893 },
      4
    )).toEqual({ x: 1686, y: 876, width: 27, height: 17 });
  });

  it('keeps a partially off-screen control region valid', () => {
    expect(expandCaptureInputRegion(
      { x: -3, y: -2, width: 20, height: 20 },
      { width: 100, height: 80 },
      4
    )).toEqual({ x: 0, y: 0, width: 21, height: 22 });
  });
});
