export interface CaptureSize {
  width: number;
  height: number;
}

export interface CaptureRegion extends CaptureSize {
  x: number;
  y: number;
}

function ceilInward(value: number): number {
  return Math.max(0, Math.ceil(value - 1e-7));
}

export function mapSelectionToSource(
  selection: CaptureRegion,
  viewport: CaptureSize,
  source: CaptureSize
): CaptureRegion {
  if (viewport.width <= 0 || viewport.height <= 0 || source.width <= 0 || source.height <= 0) {
    throw new Error('捕获源尺寸无效');
  }
  const scaleX = source.width / viewport.width;
  const scaleY = source.height / viewport.height;
  const left = ceilInward(Math.max(0, selection.x) * scaleX);
  const top = ceilInward(Math.max(0, selection.y) * scaleY);
  const right = Math.floor(Math.min(viewport.width, selection.x + selection.width) * scaleX);
  const bottom = Math.floor(Math.min(viewport.height, selection.y + selection.height) * scaleY);
  const width = Math.max(0, right - left) & ~1;
  const height = Math.max(0, bottom - top) & ~1;
  if (width < 2 || height < 2) {
    throw new Error('录制区域至少需要 2 × 2 像素');
  }
  return { x: left, y: top, width, height };
}

export function scaleRecordingRegion(region: CaptureSize, resolution: string): CaptureSize {
  const maxHeight = resolution === '1080' ? 1080 : resolution === '720' ? 720 : 0;
  if (!maxHeight || region.height <= maxHeight) return { ...region };
  const width = Math.max(2, Math.floor(region.width * maxHeight / region.height) & ~1);
  return { width, height: maxHeight };
}

export function positionCaptureToolbar(
  selection: CaptureRegion,
  viewport: CaptureSize,
  toolbar: CaptureSize,
  gap: number,
  alignment: 'center' | 'end'
): { left: number; top: number } {
  const margin = 8;
  const preferredLeft = alignment === 'end'
    ? selection.x + selection.width - toolbar.width
    : selection.x + (selection.width - toolbar.width) / 2;
  const left = Math.max(
    margin,
    Math.min(viewport.width - toolbar.width - margin, preferredLeft)
  );
  const below = selection.y + selection.height + gap;
  const above = selection.y - toolbar.height - gap;
  if (below + toolbar.height <= viewport.height - margin) return { left, top: below };
  if (above >= margin) return { left, top: above };
  return {
    left,
    top: Math.max(margin, viewport.height - toolbar.height - gap)
  };
}

export function expandCaptureInputRegion(
  bounds: CaptureRegion,
  viewport: CaptureSize,
  padding: number
): CaptureRegion {
  const maxX = Math.max(0, Math.floor(viewport.width) - 1);
  const maxY = Math.max(0, Math.floor(viewport.height) - 1);
  const x = Math.max(0, Math.min(maxX, Math.floor(bounds.x) - padding));
  const y = Math.max(0, Math.min(maxY, Math.floor(bounds.y) - padding));
  const right = Math.max(x + 1, Math.min(viewport.width, Math.ceil(bounds.x + bounds.width) + padding));
  const bottom = Math.max(y + 1, Math.min(viewport.height, Math.ceil(bounds.y + bounds.height) + padding));
  return { x, y, width: right - x, height: bottom - y };
}
