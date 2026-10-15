<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { save as saveDialog } from '@tauri-apps/plugin-dialog';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useMessage } from 'naive-ui';
import {
  expandCaptureInputRegion,
  mapSelectionToSource,
  positionCaptureToolbar,
  scaleRecordingRegion,
  type CaptureRegion
} from './captureRegion';
import {
  formatRecordingDuration,
  selectRecordingProfile,
  type RecordingProfile
} from '../recording/recording';
import {
  nextRecordingState,
  recordingShortcutAction,
  type CaptureRecordingState
} from '../recording/captureRecording';

interface WindowRect {
  owner: string;
  title: string;
  x: number;
  y: number;
  w: number;
  h: number;
  layer: number;
}

interface InitPayload {
  requestId: string;
  imagePath: string;
  screenWidth: number;
  screenHeight: number;
  usableHeight?: number;
  coversSystemUi?: boolean;
  scaleFactor: number;
  windows: WindowRect[];
  initialMode: CaptureMode;
  recordingOptions: RecordingOptions;
}

type CaptureMode = 'screenshot' | 'recording';
type RecordingSourceKind = 'region' | 'window' | 'fullscreen';

interface RecordingOptions {
  resolution: string;
  frameRate: number;
  systemAudio: boolean;
  microphone: boolean;
}

interface NativeRecordingCapabilities {
  supported: boolean;
  audioSupported: boolean;
  windowSupported: boolean;
}

interface NativeRecordingPrepared {
  sessionId: string;
  sourceLabel: string;
  previewPath: string;
  sourceWidth: number;
  sourceHeight: number;
}

interface RecordingSession {
  sessionId: string;
}

interface RecordingSaved {
  outputPath: string;
  size: number;
}

type Tool = 'cursor' | 'rect' | 'ellipse' | 'line' | 'arrow' | 'pencil' | 'text' | 'number' | 'mosaic';

interface MosaicBlock {
  x: number; // canvas-pixel top-left of block
  y: number;
  color: string; // sampled once from source
}

interface Shape {
  tool: Tool;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  color: string;
  lineWidth: number;
  text?: string;
  number?: number;
  points?: { x: number; y: number }[]; // for pencil
  mosaicBlockSize?: number;
  mosaicBlocks?: MosaicBlock[];
}

interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

const bgSrc = ref('');
const screenshotBgSrc = ref('');
const viewportW = ref(1);
const viewportH = ref(1);
const usableViewportH = ref(1);
const backendUsableHeightRatio = ref(1);
const coversSystemUi = ref(false);
const scale = ref(1);
const message = useMessage();
const overlayWindow = getCurrentWindow();

function updateViewportMetrics() {
  viewportW.value = Math.max(1, window.innerWidth);
  viewportH.value = Math.max(1, window.innerHeight);
  const localAvailableBottom = window.screen.availTop + window.screen.availHeight - window.screenY;
  const browserUsableHeight = !coversSystemUi.value && localAvailableBottom > 0
    ? Math.min(viewportH.value, localAvailableBottom)
    : viewportH.value;
  usableViewportH.value = Math.min(
    browserUsableHeight,
    viewportH.value * backendUsableHeightRatio.value
  );
}

const selection = ref<Rect | null>(null);
const isSelecting = ref(false);
const dragStart = ref<{ x: number; y: number } | null>(null);
const dragEnd = ref<{ x: number; y: number } | null>(null);

const windowList = ref<WindowRect[]>([]);
const hoveredWindow = ref<WindowRect | null>(null);
const cursor = ref<{ x: number; y: number } | null>(null);

/** Front-most window (smallest index in the list) containing point (px, py). */
function windowAt(px: number, py: number): WindowRect | null {
  for (const w of windowList.value) {
    if (px >= w.x && px <= w.x + w.w && py >= w.y && py <= w.y + w.h) {
      return w;
    }
  }
  return null;
}

const tool = ref<Tool>('cursor');
const color = ref('#ef4444');
const lineWidth = ref(3);
const textValue = ref('');
const shapes = ref<Shape[]>([]);
const undoneShapes = ref<Shape[]>([]);
const drawing = ref(false);
const drawStart = ref<{ x: number; y: number } | null>(null);
const drawEnd = ref<{ x: number; y: number } | null>(null);
const pencilPoints = ref<{ x: number; y: number }[]>([]);
const nextNumber = ref(1);
type AdjustMode = 'move' | 'n' | 's' | 'e' | 'w' | 'ne' | 'nw' | 'se' | 'sw';
const adjustMode = ref<AdjustMode | null>(null);
const hoverAdjustMode = ref<AdjustMode | null>(null);
const adjustStart = ref<{ x: number; y: number } | null>(null);
const adjustStartRect = ref<Rect | null>(null);
const canAdjust = computed(() =>
  tool.value === 'cursor'
  && shapes.value.length === 0
  && (captureMode.value === 'screenshot' || recordingPrepared.value)
);
const adjustCursor = computed(() => {
  if (!canAdjust.value) return 'crosshair';
  const m = adjustMode.value ?? hoverAdjustMode.value;
  if (!m) return 'default';
  if (m === 'move') return 'move';
  return `${m}-resize`;
});
// Mosaic brush block size in canvas pixels (scaled up already; tuned so it "reads" as pixelation)
const mosaicBlockSize = ref(14);

// Offscreen canvas holding the desktop screenshot at native resolution — used to sample
// block colors for the mosaic tool. Populated on first mosaic stroke.
let sourceCanvas: HTMLCanvasElement | null = null;
let sourceCtx: CanvasRenderingContext2D | null = null;
let currentMosaicShape: Shape | null = null;

async function ensureSourceSampler(): Promise<CanvasRenderingContext2D | null> {
  if (sourceCtx) return sourceCtx;
  if (!bgSrc.value) return null;
  const img = new Image();
  img.crossOrigin = 'anonymous';
  try {
    await new Promise<void>((resolve, reject) => {
      img.onload = () => resolve();
      img.onerror = () => reject(new Error('source image load failed'));
      img.src = bgSrc.value;
    });
  } catch (err) {
    console.warn('[capture] source sampler load failed', err);
    return null;
  }
  sourceCanvas = document.createElement('canvas');
  sourceCanvas.width = img.naturalWidth;
  sourceCanvas.height = img.naturalHeight;
  const ctx = sourceCanvas.getContext('2d', { willReadFrequently: true });
  if (!ctx) return null;
  ctx.drawImage(img, 0, 0);
  sourceCtx = ctx;
  return ctx;
}

async function sampleMosaicBlock(canvasX: number, canvasY: number): Promise<MosaicBlock | null> {
  const ctx = await ensureSourceSampler();
  if (!ctx || !selection.value) return null;
  const s = scale.value;
  const size = mosaicBlockSize.value;
  const sourceX = Math.round(selection.value.x * s + canvasX);
  const sourceY = Math.round(selection.value.y * s + canvasY);
  const w = size;
  const h = size;
  try {
    const data = ctx.getImageData(sourceX, sourceY, Math.max(1, w), Math.max(1, h)).data;
    let r = 0, g = 0, b = 0, n = 0;
    for (let i = 0; i < data.length; i += 4) {
      r += data[i];
      g += data[i + 1];
      b += data[i + 2];
      n++;
    }
    if (n === 0) return null;
    return {
      x: canvasX,
      y: canvasY,
      color: `rgb(${(r / n) | 0}, ${(g / n) | 0}, ${(b / n) | 0})`
    };
  } catch (err) {
    console.warn('[capture] getImageData failed (CORS?)', err);
    return null;
  }
}

// Text input position — non-null while user is typing a new text label at that spot.
// Coordinates are in canvas-pixel space (already scaled). We translate to viewport CSS coords for the input box.
const textPending = ref<{ canvasX: number; canvasY: number; cssLeft: number; cssTop: number } | null>(null);
const textInputRef = ref<HTMLInputElement | null>(null);

const canvasRef = ref<HTMLCanvasElement | null>(null);
const liveControlsRef = ref<HTMLElement | null>(null);

let unlistenInit: UnlistenFn | undefined;
let unlistenRecordingToggle: UnlistenFn | undefined;
let unlistenNativeError: UnlistenFn | undefined;

const captureMode = ref<CaptureMode>('screenshot');
const recordingSource = ref<RecordingSourceKind>('region');
const recordingState = ref<CaptureRecordingState>('idle');
const recordingOptions = ref<RecordingOptions>({
  resolution: 'original',
  frameRate: 30,
  systemAudio: true,
  microphone: false
});
const nativeCapabilities = ref<NativeRecordingCapabilities>({
  supported: false,
  audioSupported: true,
  windowSupported: false
});
const nativeCapabilitiesLoaded = ref(false);
const capabilitiesError = ref('');
const recordingError = ref('');
const sourceLabel = ref('');
const sourceW = ref(1);
const sourceH = ref(1);
const countdown = ref(0);
const elapsedMs = ref(0);

let nativeSessionId = '';
let displayStream: MediaStream | null = null;
let microphoneStream: MediaStream | null = null;
let recordingStream: MediaStream | null = null;
let audioContext: AudioContext | null = null;
let sourceVideo: HTMLVideoElement | null = null;
let cropCanvas: HTMLCanvasElement | null = null;
let cropFrame = 0;
let recorder: MediaRecorder | null = null;
let recordingProfile: RecordingProfile | null = null;
let fileSessionId = '';
let writeQueue: Promise<void> = Promise.resolve();
let writeError: unknown = null;
let discardWebRecording = false;
let clockTimer: ReturnType<typeof setInterval> | null = null;
let startedAt = 0;
let pausedAt = 0;
let pausedTotal = 0;
let countdownToken = 0;
let sourcePrepareToken = 0;
let componentDisposed = false;
let currentInitRequestId = '';
let inputRegionRevision = 0;

const recordingPrepared = computed(() => recordingState.value === 'selecting-region');
const recordingActive = computed(() => ['countdown', 'recording', 'paused', 'stopping'].includes(recordingState.value));
const isLive = computed(() => recordingState.value === 'recording' || recordingState.value === 'paused');
const modeLocked = computed(() => recordingActive.value || recordingState.value === 'preparing-source');
const recordingStartPending = computed(() => recordingState.value === 'countdown' && countdown.value === 0);
const overlaySelectingEnabled = computed(() =>
  captureMode.value === 'screenshot'
  || (captureMode.value === 'recording' && recordingPrepared.value && recordingSource.value === 'region')
);
const showSelectionUi = computed(() =>
  !isLive.value
  && recordingState.value !== 'stopping'
  && !recordingStartPending.value
);
const elapsedLabel = computed(() => formatRecordingDuration(elapsedMs.value));
const sourceHint = computed(() => {
  if (capabilitiesError.value) return capabilitiesError.value;
  if (recordingState.value === 'preparing-source') return '正在等待系统选择捕获来源';
  if (recordingState.value === 'error') return recordingError.value;
  if (recordingPrepared.value && recordingSource.value === 'region') return '拖动框选录制区域，选区可移动并从八个方向缩放';
  if (recordingPrepared.value) return `${sourceLabel.value} · 将录制完整来源`;
  return recordingSource.value === 'region' ? '拖动框选录制区域' : '选择要录制的捕获来源';
});

function enterRegionSelection() {
  recordingState.value = 'selecting-region';
  recordingError.value = '';
  sourceW.value = Math.max(1, Math.round(viewportW.value * scale.value));
  sourceH.value = Math.max(1, Math.round(viewportH.value * scale.value));
}

const previewRect = computed<Rect | null>(() => {
  if (isSelecting.value && dragStart.value && dragEnd.value) {
    const x = Math.min(dragStart.value.x, dragEnd.value.x);
    const y = Math.min(dragStart.value.y, dragEnd.value.y);
    const w = Math.abs(dragEnd.value.x - dragStart.value.x);
    const h = Math.abs(dragEnd.value.y - dragStart.value.y);
    return { x, y, w, h };
  }
  return selection.value;
});

const hasSelection = computed(() => selection.value !== null && selection.value.w > 4 && selection.value.h > 4);
const previewSourceSize = computed(() => {
  const current = previewRect.value;
  if (!current || captureMode.value !== 'recording' || !recordingPrepared.value) return null;
  try {
    return mapSelectionToSource(
      { x: current.x, y: current.y, width: current.w, height: current.h },
      { width: viewportW.value, height: viewportH.value },
      { width: sourceW.value, height: sourceH.value }
    );
  } catch {
    return null;
  }
});

const toolbarStyle = computed(() => {
  if (!selection.value) return { display: 'none' };
  const sel = selection.value;
  const toolbarW = 1020;
  const toolbarH = 58;
  const margin = 8;
  const position = positionCaptureToolbar(
    { x: sel.x, y: sel.y, width: sel.w, height: sel.h },
    { width: viewportW.value, height: usableViewportH.value },
    { width: toolbarW, height: toolbarH },
    margin,
    'end'
  );
  return { left: `${position.left}px`, top: `${position.top}px` };
});

const sizeLabelStyle = computed(() => {
  const rect = previewRect.value;
  if (!rect) return { display: 'none' };
  const above = rect.y > 26;
  return {
    left: `${rect.x}px`,
    top: `${above ? rect.y - 24 : rect.y + 4}px`
  };
});

function screenPos(event: MouseEvent) {
  return { x: event.clientX, y: event.clientY };
}

function onOverlayMouseDown(event: MouseEvent) {
  if (!overlaySelectingEnabled.value || selection.value) return;
  isSelecting.value = true;
  dragStart.value = screenPos(event);
  dragEnd.value = dragStart.value;
}

function onOverlayMouseMove(event: MouseEvent) {
  const p = screenPos(event);
  cursor.value = p;
  if (isSelecting.value) {
    dragEnd.value = p;
  } else if (!selection.value && captureMode.value === 'screenshot') {
    hoveredWindow.value = windowAt(p.x, p.y);
  }
}

function onOverlayMouseUp(event: MouseEvent) {
  if (!overlaySelectingEnabled.value || !isSelecting.value || !dragStart.value) return;
  isSelecting.value = false;
  dragEnd.value = screenPos(event);
  const dx = Math.abs(dragEnd.value.x - dragStart.value.x);
  const dy = Math.abs(dragEnd.value.y - dragStart.value.y);
  // Treat a click (no meaningful drag) on a hovered window as "snap to window".
  if (captureMode.value === 'screenshot' && dx < 5 && dy < 5 && hoveredWindow.value) {
    const win = hoveredWindow.value;
    selection.value = { x: win.x, y: win.y, w: win.w, h: win.h };
    dragStart.value = null;
    dragEnd.value = null;
    hoveredWindow.value = null;
    requestAnimationFrame(setupCanvas);
    return;
  }
  const x = Math.min(dragStart.value.x, dragEnd.value.x);
  const y = Math.min(dragStart.value.y, dragEnd.value.y);
  const w = dx;
  const h = dy;
  if (w < 5 || h < 5) {
    dragStart.value = null;
    dragEnd.value = null;
    return;
  }
  selection.value = { x, y, w, h };
  hoveredWindow.value = null;
  requestAnimationFrame(setupCanvas);
}

function setupCanvas() {
  const canvas = canvasRef.value;
  const sel = selection.value;
  if (!canvas || !sel) return;
  canvas.width = Math.round(sel.w * scale.value);
  canvas.height = Math.round(sel.h * scale.value);
  redraw();
}

function canvasPosFromEvent(event: MouseEvent): { x: number; y: number } {
  const canvas = canvasRef.value!;
  const rect = canvas.getBoundingClientRect();
  const scaleX = canvas.width / rect.width;
  const scaleY = canvas.height / rect.height;
  return {
    x: (event.clientX - rect.left) * scaleX,
    y: (event.clientY - rect.top) * scaleY
  };
}

const EDGE = 8;

function detectAdjustZone(px: number, py: number, sel: Rect): AdjustMode | null {
  const nearL = px >= sel.x - EDGE && px <= sel.x + EDGE;
  const nearR = px >= sel.x + sel.w - EDGE && px <= sel.x + sel.w + EDGE;
  const nearT = py >= sel.y - EDGE && py <= sel.y + EDGE;
  const nearB = py >= sel.y + sel.h - EDGE && py <= sel.y + sel.h + EDGE;

  if (nearT && nearL) return 'nw';
  if (nearT && nearR) return 'ne';
  if (nearB && nearL) return 'sw';
  if (nearB && nearR) return 'se';
  if (nearT) return 'n';
  if (nearB) return 's';
  if (nearL) return 'w';
  if (nearR) return 'e';
  if (px > sel.x && px < sel.x + sel.w && py > sel.y && py < sel.y + sel.h) return 'move';
  return null;
}

function applyAdjust(px: number, py: number) {
  const start = adjustStart.value;
  const rect0 = adjustStartRect.value;
  const mode = adjustMode.value;
  if (!start || !rect0 || !mode) return;
  const dx = px - start.x;
  const dy = py - start.y;
  const MIN = 20;
  let x = rect0.x;
  let y = rect0.y;
  let w = rect0.w;
  let h = rect0.h;

  if (mode === 'move') {
    x = rect0.x + dx;
    y = rect0.y + dy;
  } else {
    if (mode.includes('n')) { y = rect0.y + dy; h = rect0.h - dy; }
    if (mode.includes('s')) { h = rect0.h + dy; }
    if (mode.includes('w')) { x = rect0.x + dx; w = rect0.w - dx; }
    if (mode.includes('e')) { w = rect0.w + dx; }
  }

  if (w < MIN) {
    if (mode.includes('w')) x = rect0.x + rect0.w - MIN;
    w = MIN;
  }
  if (h < MIN) {
    if (mode.includes('n')) y = rect0.y + rect0.h - MIN;
    h = MIN;
  }

  if (x < 0) {
    if (mode !== 'move') w = w + x;
    x = 0;
  }
  if (y < 0) {
    if (mode !== 'move') h = h + y;
    y = 0;
  }
  if (x + w > viewportW.value) {
    if (mode === 'move') x = viewportW.value - w;
    else w = viewportW.value - x;
  }
  if (y + h > viewportH.value) {
    if (mode === 'move') y = viewportH.value - h;
    else h = viewportH.value - y;
  }

  selection.value = { x, y, w, h };
  requestAnimationFrame(setupCanvas);
}

function onAdjustDragMove(event: MouseEvent) {
  if (!adjustMode.value) return;
  applyAdjust(event.clientX, event.clientY);
}
function onAdjustDragUp() {
  window.removeEventListener('mousemove', onAdjustDragMove);
  window.removeEventListener('mouseup', onAdjustDragUp);
  adjustMode.value = null;
  adjustStart.value = null;
  adjustStartRect.value = null;
}

function onCanvasMouseDown(event: MouseEvent) {
  event.stopPropagation();
  const p = canvasPosFromEvent(event);
  if (canAdjust.value && selection.value) {
    const zone = detectAdjustZone(event.clientX, event.clientY, selection.value);
    if (zone) {
      adjustMode.value = zone;
      adjustStart.value = { x: event.clientX, y: event.clientY };
      adjustStartRect.value = { ...selection.value };
      window.addEventListener('mousemove', onAdjustDragMove);
      window.addEventListener('mouseup', onAdjustDragUp);
      return;
    }
  }
  if (captureMode.value === 'recording') return;
  if (tool.value === 'cursor') return;
  if (tool.value === 'text') {
    if (textPending.value && textValue.value.trim()) {
      commitPendingText();
    }
    textValue.value = '';
    textPending.value = {
      canvasX: p.x,
      canvasY: p.y,
      cssLeft: event.clientX,
      cssTop: event.clientY
    };
    requestAnimationFrame(() => textInputRef.value?.focus());
    return;
  }
  if (tool.value === 'number') {
    pushShape({
      tool: 'number',
      x1: p.x,
      y1: p.y,
      x2: p.x,
      y2: p.y,
      color: color.value,
      lineWidth: lineWidth.value,
      number: nextNumber.value
    });
    nextNumber.value += 1;
    redraw();
    return;
  }
  if (tool.value === 'pencil') {
    drawing.value = true;
    pencilPoints.value = [p];
    return;
  }
  if (tool.value === 'mosaic') {
    drawing.value = true;
    currentMosaicShape = {
      tool: 'mosaic',
      x1: p.x,
      y1: p.y,
      x2: p.x,
      y2: p.y,
      color: '#000',
      lineWidth: 1,
      mosaicBlockSize: mosaicBlockSize.value,
      mosaicBlocks: []
    };
    shapes.value.push(currentMosaicShape);
    addMosaicBlock(p);
    return;
  }
  drawing.value = true;
  drawStart.value = p;
  drawEnd.value = p;
}

async function addMosaicBlock(p: { x: number; y: number }) {
  if (!currentMosaicShape) return;
  const size = mosaicBlockSize.value;
  const bx = Math.floor(p.x / size) * size;
  const by = Math.floor(p.y / size) * size;
  // Skip if this block is already in the current stroke
  const blocks = currentMosaicShape.mosaicBlocks!;
  if (blocks.some((b) => b.x === bx && b.y === by)) return;
  const block = await sampleMosaicBlock(bx, by);
  if (block) {
    blocks.push(block);
    redraw();
  }
}

function commitPendingText() {
  const pos = textPending.value;
  const value = textValue.value.trim();
  if (pos && value) {
    shapes.value.push({
      tool: 'text',
      x1: pos.canvasX,
      y1: pos.canvasY,
      x2: pos.canvasX,
      y2: pos.canvasY,
      color: color.value,
      lineWidth: lineWidth.value,
      text: value
    });
    redraw();
  }
  textPending.value = null;
  textValue.value = '';
}

function cancelPendingText() {
  textPending.value = null;
  textValue.value = '';
}

function onCanvasMouseMove(event: MouseEvent) {
  if (adjustMode.value) return;
  if (canAdjust.value && selection.value) {
    hoverAdjustMode.value = detectAdjustZone(event.clientX, event.clientY, selection.value);
    return;
  }
  hoverAdjustMode.value = null;
  if (captureMode.value === 'recording') return;
  if (!drawing.value) return;
  event.stopPropagation();
  const p = canvasPosFromEvent(event);
  if (tool.value === 'pencil') {
    pencilPoints.value.push(p);
    redraw();
  } else if (tool.value === 'mosaic') {
    addMosaicBlock(p);
    // redraw is called inside addMosaicBlock once the block is sampled
  } else {
    drawEnd.value = p;
    redraw();
  }
}

function onCanvasMouseUp(event: MouseEvent) {
  if (adjustMode.value) return;
  if (captureMode.value === 'recording') return;
  if (!drawing.value) return;
  event.stopPropagation();
  const p = canvasPosFromEvent(event);
  if (tool.value === 'pencil') {
    if (pencilPoints.value.length > 1) {
      pushShape({
        tool: 'pencil',
        x1: pencilPoints.value[0].x,
        y1: pencilPoints.value[0].y,
        x2: p.x,
        y2: p.y,
        color: color.value,
        lineWidth: lineWidth.value,
        points: [...pencilPoints.value]
      });
    }
    pencilPoints.value = [];
    drawing.value = false;
    redraw();
    return;
  }
  if (tool.value === 'mosaic') {
    if (currentMosaicShape && (currentMosaicShape.mosaicBlocks?.length ?? 0) === 0) {
      // Empty stroke — drop it
      shapes.value.pop();
    } else if (currentMosaicShape) {
      // Any new drawing clears the redo stack — mirror pushShape semantics
      undoneShapes.value = [];
    }
    currentMosaicShape = null;
    drawing.value = false;
    redraw();
    return;
  }
  if (!drawStart.value) {
    drawing.value = false;
    return;
  }
  const dx = p.x - drawStart.value.x;
  const dy = p.y - drawStart.value.y;
  if (Math.abs(dx) > 3 || Math.abs(dy) > 3) {
    pushShape({
      tool: tool.value,
      x1: drawStart.value.x,
      y1: drawStart.value.y,
      x2: p.x,
      y2: p.y,
      color: color.value,
      lineWidth: lineWidth.value
    });
  }
  drawing.value = false;
  drawStart.value = null;
  drawEnd.value = null;
  redraw();
}

function pushShape(shape: Shape) {
  shapes.value.push(shape);
  // Any new drawing invalidates the redo stack (standard editor behavior)
  undoneShapes.value = [];
}

function redraw() {
  const canvas = canvasRef.value;
  if (!canvas) return;
  const ctx = canvas.getContext('2d');
  if (!ctx) return;
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  const s = scale.value;
  for (const shape of shapes.value) {
    drawShape(ctx, shape, s);
  }
  // Live preview of in-progress shape
  if (drawing.value) {
    if (tool.value === 'pencil' && pencilPoints.value.length > 0) {
      drawShape(
        ctx,
        {
          tool: 'pencil',
          x1: pencilPoints.value[0].x,
          y1: pencilPoints.value[0].y,
          x2: pencilPoints.value[pencilPoints.value.length - 1].x,
          y2: pencilPoints.value[pencilPoints.value.length - 1].y,
          color: color.value,
          lineWidth: lineWidth.value,
          points: pencilPoints.value
        },
        s
      );
    } else if (drawStart.value && drawEnd.value) {
      drawShape(
        ctx,
        {
          tool: tool.value,
          x1: drawStart.value.x,
          y1: drawStart.value.y,
          x2: drawEnd.value.x,
          y2: drawEnd.value.y,
          color: color.value,
          lineWidth: lineWidth.value
        },
        s
      );
    }
  }
}

function drawShape(ctx: CanvasRenderingContext2D, shape: Shape, s: number) {
  ctx.strokeStyle = shape.color;
  ctx.fillStyle = shape.color;
  ctx.lineWidth = Math.max(1, shape.lineWidth * s);
  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';

  if (shape.tool === 'rect') {
    ctx.strokeRect(shape.x1, shape.y1, shape.x2 - shape.x1, shape.y2 - shape.y1);
  } else if (shape.tool === 'ellipse') {
    const cx = (shape.x1 + shape.x2) / 2;
    const cy = (shape.y1 + shape.y2) / 2;
    const rx = Math.abs(shape.x2 - shape.x1) / 2;
    const ry = Math.abs(shape.y2 - shape.y1) / 2;
    ctx.beginPath();
    ctx.ellipse(cx, cy, rx, ry, 0, 0, Math.PI * 2);
    ctx.stroke();
  } else if (shape.tool === 'line') {
    ctx.beginPath();
    ctx.moveTo(shape.x1, shape.y1);
    ctx.lineTo(shape.x2, shape.y2);
    ctx.stroke();
  } else if (shape.tool === 'arrow') {
    const dx = shape.x2 - shape.x1;
    const dy = shape.y2 - shape.y1;
    const angle = Math.atan2(dy, dx);
    const headLen = Math.max(10, ctx.lineWidth * 4);
    ctx.beginPath();
    ctx.moveTo(shape.x1, shape.y1);
    ctx.lineTo(shape.x2, shape.y2);
    ctx.stroke();
    ctx.beginPath();
    ctx.moveTo(shape.x2, shape.y2);
    ctx.lineTo(
      shape.x2 - headLen * Math.cos(angle - Math.PI / 6),
      shape.y2 - headLen * Math.sin(angle - Math.PI / 6)
    );
    ctx.lineTo(
      shape.x2 - headLen * Math.cos(angle + Math.PI / 6),
      shape.y2 - headLen * Math.sin(angle + Math.PI / 6)
    );
    ctx.closePath();
    ctx.fill();
  } else if (shape.tool === 'pencil' && shape.points && shape.points.length > 1) {
    ctx.beginPath();
    ctx.moveTo(shape.points[0].x, shape.points[0].y);
    for (let i = 1; i < shape.points.length; i++) {
      ctx.lineTo(shape.points[i].x, shape.points[i].y);
    }
    ctx.stroke();
  } else if (shape.tool === 'number' && typeof shape.number === 'number') {
    const r = Math.max(12, shape.lineWidth * 4) * s;
    ctx.beginPath();
    ctx.arc(shape.x1, shape.y1, r, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillStyle = '#fff';
    ctx.font = `bold ${Math.round(r * 1.1)}px -apple-system, sans-serif`;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(String(shape.number), shape.x1, shape.y1 + 1);
    ctx.textAlign = 'start';
    ctx.textBaseline = 'alphabetic';
  } else if (shape.tool === 'mosaic' && shape.mosaicBlocks && shape.mosaicBlockSize) {
    const size = shape.mosaicBlockSize;
    for (const b of shape.mosaicBlocks) {
      ctx.fillStyle = b.color;
      ctx.fillRect(b.x, b.y, size, size);
    }
  } else if (shape.tool === 'text' && shape.text) {
    const fontSize = Math.max(14, shape.lineWidth * 6) * s;
    ctx.font = `${fontSize}px -apple-system, "PingFang SC", "Segoe UI", sans-serif`;
    ctx.textBaseline = 'top';
    ctx.fillText(shape.text, shape.x1, shape.y1);
  }
}

function undo() {
  const popped = shapes.value.pop();
  if (popped) {
    undoneShapes.value.push(popped);
    // keep number counter in sync if we just undid a number shape
    if (popped.tool === 'number') {
      nextNumber.value = Math.max(1, nextNumber.value - 1);
    }
  }
  redraw();
}

function redo() {
  const popped = undoneShapes.value.pop();
  if (popped) {
    shapes.value.push(popped);
    if (popped.tool === 'number') {
      nextNumber.value = Math.max(nextNumber.value, (popped.number ?? 0) + 1);
    }
  }
  redraw();
}

function reselect() {
  resetState();
}

async function composeCanvas(): Promise<HTMLCanvasElement | null> {
  const sel = selection.value;
  if (!sel) return null;
  const out = document.createElement('canvas');
  out.width = Math.round(sel.w * scale.value);
  out.height = Math.round(sel.h * scale.value);
  const ctx = out.getContext('2d');
  if (!ctx) return null;
  const img = new Image();
  img.crossOrigin = 'anonymous';
  await new Promise<void>((resolve, reject) => {
    img.onload = () => resolve();
    img.onerror = () => reject(new Error('load desktop image failed'));
    img.src = bgSrc.value;
  });
  ctx.drawImage(
    img,
    sel.x * scale.value,
    sel.y * scale.value,
    sel.w * scale.value,
    sel.h * scale.value,
    0,
    0,
    out.width,
    out.height
  );
  for (const shape of shapes.value) {
    drawShape(ctx, shape, scale.value);
  }
  return out;
}

async function saveToFile() {
  const canvas = await composeCanvas().catch((err) => {
    message.error(`截图合成失败：${String(err)}`);
    return null;
  });
  if (!canvas) return;
  const ts = new Date().toISOString().replace(/[:.]/g, '-');
  const target = await saveDialog({
    defaultPath: `attool-${ts}.png`,
    filters: [{ name: 'PNG', extensions: ['png'] }]
  }).catch(() => null);
  if (!target) return;
  const dataUrl = canvas.toDataURL('image/png');
  const base64 = dataUrl.split(',')[1];
  try {
    await invoke('write_binary_file', { path: target, base64 });
  } catch (err) {
    console.warn('[capture] save failed', err);
    message.error(`保存失败：${String(err)}`);
    return;
  }
  await invoke('close_capture_overlay').catch(() => {});
  resetState();
}

function setRecordingState(event: Parameters<typeof nextRecordingState>[1]) {
  recordingState.value = nextRecordingState(recordingState.value, event);
}

function isPickerCancellation(error: unknown) {
  if (error instanceof DOMException && ['NotAllowedError', 'AbortError'].includes(error.name)) return true;
  return /cancel|取消|denied by user/i.test(String(error));
}

function stopClock() {
  if (clockTimer) clearInterval(clockTimer);
  clockTimer = null;
}

function startClock() {
  startedAt = performance.now();
  pausedAt = 0;
  pausedTotal = 0;
  elapsedMs.value = 0;
  clockTimer = setInterval(() => {
    const end = pausedAt || performance.now();
    elapsedMs.value = end - startedAt - pausedTotal;
  }, 200);
}

function stopCropLoop() {
  if (cropFrame) cancelAnimationFrame(cropFrame);
  cropFrame = 0;
}

function stopWebStreams() {
  stopCropLoop();
  displayStream?.getTracks().forEach((track) => track.stop());
  microphoneStream?.getTracks().forEach((track) => track.stop());
  recordingStream?.getTracks().forEach((track) => track.stop());
  displayStream = null;
  microphoneStream = null;
  recordingStream = null;
  sourceVideo = null;
  cropCanvas = null;
  if (audioContext) void audioContext.close();
  audioContext = null;
}

async function cancelFileSession() {
  const current = fileSessionId;
  fileSessionId = '';
  if (current) await invoke('cancel_recording_file', { sessionId: current }).catch(() => {});
}

async function blobToBase64(blob: Blob): Promise<string> {
  return await new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result).split(',')[1] ?? '');
    reader.onerror = () => reject(reader.error ?? new Error('读取录制数据失败'));
    reader.readAsDataURL(blob);
  });
}

async function uploadBlob(blob: Blob) {
  const chunkSize = 512 * 1024;
  for (let offset = 0; offset < blob.size; offset += chunkSize) {
    const base64 = await blobToBase64(blob.slice(offset, offset + chunkSize));
    await invoke('append_recording_chunk', { sessionId: fileSessionId, base64 });
  }
}

async function composeRecordingStream(videoTrack: MediaStreamTrack): Promise<MediaStream> {
  const audioTracks = [...(displayStream?.getAudioTracks() ?? [])];
  if (recordingOptions.value.microphone) {
    microphoneStream = await navigator.mediaDevices.getUserMedia({ audio: true });
    audioTracks.push(...microphoneStream.getAudioTracks());
  }
  if (audioTracks.length <= 1) return new MediaStream([videoTrack, ...audioTracks]);

  audioContext = new AudioContext();
  const destination = audioContext.createMediaStreamDestination();
  for (const track of audioTracks) {
    audioContext.createMediaStreamSource(new MediaStream([track])).connect(destination);
  }
  return new MediaStream([videoTrack, ...destination.stream.getAudioTracks()]);
}

async function prepareWebSource() {
  const video = {
    frameRate: { ideal: recordingOptions.value.frameRate, max: recordingOptions.value.frameRate },
    displaySurface: recordingSource.value === 'window' ? 'window' : 'monitor'
  } as MediaTrackConstraints;
  displayStream = await navigator.mediaDevices.getDisplayMedia({
    video,
    audio: recordingOptions.value.systemAudio
  });
  const track = displayStream.getVideoTracks()[0];
  if (!track) throw new Error('系统没有返回可录制的视频轨道');
  const displaySurface = track.getSettings().displaySurface;
  if (recordingSource.value === 'window' && displaySurface && displaySurface !== 'window') {
    stopWebStreams();
    throw new Error('请选择一个窗口作为录制来源');
  }
  if (recordingSource.value === 'fullscreen' && displaySurface && displaySurface !== 'monitor') {
    stopWebStreams();
    throw new Error('请选择一个显示器作为全屏录制来源');
  }

  sourceVideo = document.createElement('video');
  sourceVideo.muted = true;
  sourceVideo.playsInline = true;
  sourceVideo.srcObject = displayStream;
  await sourceVideo.play();
  if (!sourceVideo.videoWidth || !sourceVideo.videoHeight) {
    await new Promise<void>((resolve) => {
      sourceVideo?.addEventListener('loadedmetadata', () => resolve(), { once: true });
    });
  }
  sourceW.value = sourceVideo.videoWidth;
  sourceH.value = sourceVideo.videoHeight;
  sourceLabel.value = track.label || '共享画面';

  if (recordingSource.value !== 'region') updateWebPreview();
  track.addEventListener('ended', () => {
    if (recordingState.value === 'recording' || recordingState.value === 'paused') {
      void stopRecording();
    }
  }, { once: true });
}

function updateWebPreview() {
  if (!sourceVideo) return;
  const preview = document.createElement('canvas');
  preview.width = sourceW.value;
  preview.height = sourceH.value;
  preview.getContext('2d')?.drawImage(sourceVideo, 0, 0);
  bgSrc.value = preview.toDataURL('image/png');
}

async function prepareSource(preserveSelection = false): Promise<boolean> {
  if (
    recordingState.value !== 'idle'
    && recordingState.value !== 'error'
    && !(preserveSelection && recordingState.value === 'selecting-region')
  ) return false;
  if (!nativeCapabilitiesLoaded.value || capabilitiesError.value) return false;
  if (recordingSource.value === 'window' && !nativeCapabilities.value.windowSupported) return false;
  recordingError.value = '';
  setRecordingState('prepare');
  const prepareToken = ++sourcePrepareToken;
  const retainedSelection = preserveSelection && selection.value
    ? { ...selection.value }
    : null;
  if (!preserveSelection) resetState();
  let hiddenForPicker = false;
  try {
    if (nativeCapabilities.value.supported) {
      await overlayWindow.hide();
      hiddenForPicker = true;
      await new Promise((resolve) => setTimeout(resolve, 220));
      const prepared = await invoke<NativeRecordingPrepared>('prepare_native_recording');
      if (prepareToken !== sourcePrepareToken || componentDisposed) {
        await invoke('cancel_native_recording', { sessionId: prepared.sessionId }).catch(() => {});
        return false;
      }
      nativeSessionId = prepared.sessionId;
      sourceLabel.value = prepared.sourceLabel;
      sourceW.value = prepared.sourceWidth;
      sourceH.value = prepared.sourceHeight;
      if (recordingSource.value !== 'region') {
        bgSrc.value = `${convertFileSrc(prepared.previewPath)}?t=${Date.now()}`;
      }
    } else {
      await prepareWebSource();
      if (prepareToken !== sourcePrepareToken || componentDisposed) {
        stopWebStreams();
        return false;
      }
      if (recordingSource.value !== 'region') {
        await overlayWindow.hide();
        hiddenForPicker = true;
        await new Promise((resolve) => setTimeout(resolve, 220));
        updateWebPreview();
      }
    }
    if (recordingSource.value !== 'region') {
      selection.value = { x: 0, y: 0, w: viewportW.value, h: viewportH.value };
      requestAnimationFrame(setupCanvas);
    } else if (retainedSelection) {
      selection.value = retainedSelection;
      requestAnimationFrame(setupCanvas);
    }
    setRecordingState('source-ready');
    return true;
  } catch (error) {
    if (prepareToken !== sourcePrepareToken || componentDisposed) return false;
    stopWebStreams();
    if (isPickerCancellation(error)) {
      if (preserveSelection) enterRegionSelection();
      else setRecordingState('source-cancelled');
      bgSrc.value = screenshotBgSrc.value;
      return false;
    }
    recordingError.value = String(error);
    if (preserveSelection) {
      message.error(recordingError.value);
      enterRegionSelection();
      bgSrc.value = screenshotBgSrc.value;
    } else {
      setRecordingState('fail');
    }
    return false;
  } finally {
    if (hiddenForPicker && !componentDisposed) {
      await overlayWindow.show().catch(() => {});
      await overlayWindow.setFocus().catch(() => {});
    }
  }
}

function recordingRegion(): CaptureRegion {
  if (recordingSource.value !== 'region') {
    return { x: 0, y: 0, width: sourceW.value, height: sourceH.value };
  }
  const current = selection.value;
  if (!current) throw new Error('请先框选录制区域');
  return mapSelectionToSource(
    { x: current.x, y: current.y, width: current.w, height: current.h },
    { width: viewportW.value, height: viewportH.value },
    { width: sourceW.value, height: sourceH.value }
  );
}

function drawCropFrame(region: CaptureRegion) {
  if (!cropCanvas || !sourceVideo) return;
  const context = cropCanvas.getContext('2d');
  if (!context) return;
  context.drawImage(
    sourceVideo,
    region.x,
    region.y,
    region.width,
    region.height,
    0,
    0,
    cropCanvas.width,
    cropCanvas.height
  );
  cropFrame = requestAnimationFrame(() => drawCropFrame(region));
}

async function startWebRecording(region: CaptureRegion) {
  if (!sourceVideo || !displayStream) throw new Error('录制来源已经失效，请重新选择');
  recordingProfile = selectRecordingProfile((mimeType) => MediaRecorder.isTypeSupported(mimeType));
  if (!recordingProfile) throw new Error('当前系统不支持可用的录屏编码格式');

  const target = scaleRecordingRegion(region, recordingOptions.value.resolution);
  cropCanvas = document.createElement('canvas');
  cropCanvas.width = target.width;
  cropCanvas.height = target.height;
  drawCropFrame(region);
  const canvasStream = cropCanvas.captureStream(recordingOptions.value.frameRate);
  const videoTrack = canvasStream.getVideoTracks()[0];
  if (!videoTrack) throw new Error('无法创建区域录制轨道');
  recordingStream = await composeRecordingStream(videoTrack);
  const session = await invoke<RecordingSession>('begin_recording_file');
  fileSessionId = session.sessionId;
  writeQueue = Promise.resolve();
  writeError = null;
  discardWebRecording = false;
  recorder = new MediaRecorder(recordingStream, {
    mimeType: recordingProfile.mimeType,
    videoBitsPerSecond: target.height <= 720 ? 4_000_000 : 8_000_000,
    audioBitsPerSecond: 128_000
  });
  recorder.addEventListener('dataavailable', (event) => {
    if (!event.data.size || !fileSessionId) return;
    writeQueue = writeQueue.then(() => uploadBlob(event.data)).catch((error) => {
      writeError = error;
      if (recorder?.state !== 'inactive') recorder?.stop();
    });
  });
  recorder.addEventListener('stop', () => void finishWebRecording(), { once: true });
  recorder.addEventListener('error', (event) => {
    writeError = event.error;
    if (recorder?.state !== 'inactive') recorder?.stop();
  });
  recorder.start(1000);
}

async function finishWebRecording() {
  stopClock();
  await writeQueue;
  stopWebStreams();
  if (discardWebRecording) {
    await cancelFileSession();
    return;
  }
  if (writeError) {
    await cancelFileSession();
    recordingError.value = String(writeError);
    recordingState.value = 'error';
    return;
  }
  const current = fileSessionId;
  fileSessionId = '';
  let saved: RecordingSaved;
  try {
    saved = await invoke<RecordingSaved>('finish_recording_file', {
      sessionId: current,
      extension: recordingProfile?.extension ?? 'webm'
    });
  } catch (error) {
    recordingError.value = String(error);
    recordingState.value = 'error';
    return;
  }
  setRecordingState('complete');
  try {
    await presentRecording(saved, recordingProfile?.extension ?? 'webm');
  } catch (error) {
    await invoke('discard_recording_file', { path: saved.outputPath }).catch(() => {});
    recordingError.value = String(error);
    recordingState.value = 'error';
  }
}

async function presentRecording(saved: RecordingSaved, extension: 'webm' | 'mp4') {
  await invoke('present_recording_result', {
    result: {
      outputPath: saved.outputPath,
      size: saved.size,
      sourceLabel: sourceLabel.value,
      elapsedMs: Math.round(elapsedMs.value),
      extension
    }
  });
}

async function waitForRecordingOverlayToClear() {
  countdown.value = 0;
  await nextTick();
  await new Promise<void>((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
  });
  await new Promise((resolve) => setTimeout(resolve, 100));
}

async function startRecording() {
  if (!recordingPrepared.value) return;
  if (!nativeCapabilitiesLoaded.value || capabilitiesError.value) return;
  if (recordingSource.value === 'region' && !nativeSessionId && !displayStream) {
    const ready = await prepareSource(true);
    if (!ready) return;
  }
  let region: CaptureRegion;
  try {
    region = recordingRegion();
  } catch (error) {
    message.error(String(error));
    return;
  }

  setRecordingState('start');
  const token = ++countdownToken;
  try {
    for (const value of [3, 2, 1]) {
      countdown.value = value;
      await new Promise((resolve) => setTimeout(resolve, 1000));
      if (token !== countdownToken) return;
    }
    await waitForRecordingOverlayToClear();
    if (token !== countdownToken) return;
    if (nativeCapabilities.value.supported) {
      await invoke('start_native_recording', {
        sessionId: nativeSessionId,
        region,
        resolution: recordingOptions.value.resolution,
        frameRate: recordingOptions.value.frameRate
      });
    } else {
      await startWebRecording(region);
    }
    setRecordingState('countdown-finished');
    startClock();
  } catch (error) {
    recordingError.value = String(error);
    await cleanupRecordingSource();
    recordingState.value = 'error';
  }
}

async function pauseRecording() {
  if (recordingState.value !== 'recording') return;
  try {
    if (nativeCapabilities.value.supported) {
      await invoke('pause_native_recording', { sessionId: nativeSessionId });
    } else {
      recorder?.pause();
    }
    pausedAt = performance.now();
    setRecordingState('pause');
  } catch (error) {
    recordingError.value = String(error);
    await cleanupRecordingSource();
    recordingState.value = 'error';
  }
}

async function resumeRecording() {
  if (recordingState.value !== 'paused') return;
  try {
    if (nativeCapabilities.value.supported) {
      await invoke('resume_native_recording', { sessionId: nativeSessionId });
    } else {
      recorder?.resume();
    }
    pausedTotal += performance.now() - pausedAt;
    pausedAt = 0;
    setRecordingState('resume');
  } catch (error) {
    recordingError.value = String(error);
    await cleanupRecordingSource();
    recordingState.value = 'error';
  }
}

async function stopRecording() {
  if (recordingState.value !== 'recording' && recordingState.value !== 'paused') return;
  setRecordingState('stop');
  stopClock();
  if (nativeCapabilities.value.supported) {
    const current = nativeSessionId;
    nativeSessionId = '';
    try {
      const saved = await invoke<RecordingSaved>('stop_native_recording', { sessionId: current });
      setRecordingState('complete');
      try {
        await presentRecording(saved, 'webm');
      } catch (error) {
        await invoke('discard_recording_file', { path: saved.outputPath }).catch(() => {});
        throw error;
      }
    } catch (error) {
      recordingError.value = String(error);
      recordingState.value = 'error';
    }
    return;
  }
  if (recorder && recorder.state !== 'inactive') {
    recorder.stop();
  } else {
    recordingError.value = '录制会话已经结束，请重新录制';
    await cleanupRecordingSource();
    recordingState.value = 'error';
  }
}

async function cleanupRecordingSource() {
  sourcePrepareToken += 1;
  countdownToken += 1;
  countdown.value = 0;
  stopClock();
  if (nativeSessionId) {
    const current = nativeSessionId;
    nativeSessionId = '';
    await invoke('cancel_native_recording', { sessionId: current }).catch(() => {});
  }
  if (recorder && recorder.state !== 'inactive') {
    discardWebRecording = true;
    recorder.stop();
  }
  recorder = null;
  stopWebStreams();
  await writeQueue.catch(() => {});
  await cancelFileSession();
}

async function resetRecordingSource() {
  await cleanupRecordingSource();
  recordingState.value = 'idle';
  recordingError.value = '';
  sourceLabel.value = '';
  elapsedMs.value = 0;
  bgSrc.value = screenshotBgSrc.value;
  resetState();
  if (recordingSource.value === 'region') enterRegionSelection();
}

async function selectCaptureMode(next: CaptureMode) {
  if (next === captureMode.value || modeLocked.value) return;
  if (captureMode.value === 'recording') await resetRecordingSource();
  captureMode.value = next;
  bgSrc.value = screenshotBgSrc.value;
  resetState();
  if (next === 'recording' && recordingSource.value === 'region') enterRegionSelection();
}

async function selectRecordingSource(next: RecordingSourceKind) {
  if (modeLocked.value || next === recordingSource.value) return;
  if (next === 'window' && (!nativeCapabilitiesLoaded.value || !nativeCapabilities.value.windowSupported)) return;
  await resetRecordingSource();
  recordingSource.value = next;
  recordingState.value = 'idle';
  if (next === 'region') enterRegionSelection();
}

const recordingToolbarStyle = computed(() => {
  const sel = selection.value;
  if (!sel) return { display: 'none' };
  const width = isLive.value ? 220 : 310;
  const height = 48;
  const gap = 10;
  const position = positionCaptureToolbar(
    { x: sel.x, y: sel.y, width: sel.w, height: sel.h },
    { width: viewportW.value, height: usableViewportH.value },
    { width, height },
    gap,
    'center'
  );
  return { left: `${position.left}px`, top: `${position.top}px` };
});

async function syncOverlayInputRegion(restrictToControls: boolean) {
  const revision = ++inputRegionRevision;
  await nextTick();
  if (revision !== inputRegionRevision || componentDisposed) return;

  let region: { x: number; y: number; width: number; height: number } | null = null;
  if (restrictToControls) {
    const bounds = liveControlsRef.value?.getBoundingClientRect();
    if (!bounds) return;
    region = expandCaptureInputRegion(
      { x: bounds.left, y: bounds.top, width: bounds.width, height: bounds.height },
      { width: viewportW.value, height: viewportH.value },
      4
    );
  }

  await invoke('set_capture_overlay_input_region', { region }).catch((error) => {
    console.warn('[capture] failed to update overlay input region', error);
  });
}

async function cancel() {
  if (captureMode.value === 'recording') await cleanupRecordingSource();
  await invoke('close_capture_overlay').catch(() => {});
  resetState();
}

function resetState() {
  window.removeEventListener('mousemove', onAdjustDragMove);
  window.removeEventListener('mouseup', onAdjustDragUp);
  adjustMode.value = null;
  adjustStart.value = null;
  adjustStartRect.value = null;
  hoverAdjustMode.value = null;
  selection.value = null;
  shapes.value = [];
  undoneShapes.value = [];
  nextNumber.value = 1;
  dragStart.value = null;
  dragEnd.value = null;
  textValue.value = '';
  textPending.value = null;
  pencilPoints.value = [];
  hoveredWindow.value = null;
  tool.value = 'cursor';
}

async function confirm() {
  const canvas = await composeCanvas().catch((err) => {
    message.error(`截图合成失败：${String(err)}`);
    return null;
  });
  if (!canvas) return;
  const dataUrl = canvas.toDataURL('image/png');
  const base64 = dataUrl.split(',')[1];
  // Rust side saves file AND writes clipboard image — one round trip, no flaky JS Image.fromBytes.
  try {
    await invoke('commit_capture_overlay', { request: { pngBase64: base64 } });
  } catch (err) {
    console.warn('[capture] commit failed', err);
    message.error(`提交失败：${String(err)}`);
    return;
  }
  resetState();
}

async function pinIt() {
  const canvas = await composeCanvas().catch((err) => {
    message.error(`截图合成失败：${String(err)}`);
    return null;
  });
  if (!canvas) return;
  const dataUrl = canvas.toDataURL('image/png');
  const base64 = dataUrl.split(',')[1];
  try {
    await invoke('pin_capture_overlay', { request: { pngBase64: base64 } });
  } catch (err) {
    console.warn('[capture] pin failed', err);
    message.error(`钉图失败：${String(err)}`);
    return;
  }
  resetState();
}

function onKeydown(event: KeyboardEvent) {
  if (textPending.value) return;
  if (event.key === 'Escape') {
    event.preventDefault();
    void cancel();
  } else if (captureMode.value === 'recording') {
    if ((event.metaKey || event.ctrlKey) && (event.key === 'Enter' || event.key === 'Return')) {
      event.preventDefault();
      if (recordingPrepared.value && hasSelection.value) void startRecording();
    } else if (event.code === 'Space' && recordingState.value === 'recording') {
      event.preventDefault();
      void pauseRecording();
    } else if (event.code === 'Space' && recordingState.value === 'paused') {
      event.preventDefault();
      void resumeRecording();
    }
  } else if ((event.metaKey || event.ctrlKey) && (event.key === 'Enter' || event.key === 'Return')) {
    event.preventDefault();
    if (hasSelection.value) confirm();
  } else if ((event.metaKey || event.ctrlKey) && event.shiftKey && event.key.toLowerCase() === 'z') {
    event.preventDefault();
    redo();
  } else if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'z') {
    event.preventDefault();
    undo();
  } else if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 's') {
    event.preventDefault();
    if (hasSelection.value) saveToFile();
  }
}

watch(() => tool.value, (v) => {
  if (v !== 'cursor') hoverAdjustMode.value = null;
});

watch(recordingState, (state, previous) => {
  if (state === 'recording' || state === 'paused') {
    void syncOverlayInputRegion(true);
  } else if (state !== 'stopping' && previous !== undefined) {
    void syncOverlayInputRegion(false);
  }
});

onMounted(async () => {
  updateViewportMetrics();
  window.addEventListener('keydown', onKeydown);
  window.addEventListener('resize', updateViewportMetrics);
  invoke<NativeRecordingCapabilities>('get_native_recording_capabilities')
    .then((capabilities) => {
      nativeCapabilities.value = capabilities;
      nativeCapabilitiesLoaded.value = true;
      capabilitiesError.value = '';
      if (!capabilities.audioSupported) {
        recordingOptions.value.systemAudio = false;
        recordingOptions.value.microphone = false;
      }
    })
    .catch((error) => {
      nativeCapabilitiesLoaded.value = true;
      capabilitiesError.value = `读取录屏能力失败：${String(error)}`;
      recordingError.value = capabilitiesError.value;
      recordingState.value = 'error';
    });
  unlistenInit = await listen<InitPayload>('capture-overlay-init', async (event) => {
    const p = event.payload;
    if (p.requestId === currentInitRequestId) return;
    currentInitRequestId = p.requestId;
    await cleanupRecordingSource();
    screenshotBgSrc.value = p.imagePath
      ? `${convertFileSrc(p.imagePath)}?t=${Date.now()}`
      : '';
    bgSrc.value = screenshotBgSrc.value;
    const reportedUsableHeight = Number.isFinite(p.usableHeight)
      ? p.usableHeight as number
      : p.screenHeight;
    backendUsableHeightRatio.value = p.screenHeight > 0
      ? Math.min(1, Math.max(0.1, reportedUsableHeight / p.screenHeight))
      : 1;
    coversSystemUi.value = p.coversSystemUi === true;
    await nextTick();
    updateViewportMetrics();
    scale.value = p.scaleFactor;
    windowList.value = p.windows ?? [];
    captureMode.value = p.initialMode === 'recording' ? 'recording' : 'screenshot';
    recordingOptions.value = p.recordingOptions ?? recordingOptions.value;
    if (nativeCapabilities.value.supported && !nativeCapabilities.value.audioSupported) {
      recordingOptions.value.systemAudio = false;
      recordingOptions.value.microphone = false;
    }
    recordingState.value = 'idle';
    recordingError.value = '';
    resetState();
    if (captureMode.value === 'recording' && recordingSource.value === 'region') {
      enterRegionSelection();
    }
  });
  unlistenRecordingToggle = await listen('capture-overlay-recording-toggle', () => {
    const action = recordingShortcutAction(recordingState.value);
    if (action === 'stop') {
      void stopRecording();
    } else if (action === 'open') {
      if (captureMode.value === 'recording' && ['completed', 'error'].includes(recordingState.value)) {
        void resetRecordingSource();
      } else {
        void selectCaptureMode('recording');
      }
    }
  });
  unlistenNativeError = await listen<{ sessionId: string; message: string }>('recording://native-error', (event) => {
    if (event.payload.sessionId !== nativeSessionId) return;
    nativeSessionId = '';
    stopClock();
    recordingError.value = event.payload.message;
    recordingState.value = 'error';
  });
  await invoke('mark_capture_overlay_ready');
});

onUnmounted(() => {
  componentDisposed = true;
  inputRegionRevision += 1;
  void invoke('set_capture_overlay_input_region', { region: null }).catch(() => {});
  window.removeEventListener('keydown', onKeydown);
  window.removeEventListener('resize', updateViewportMetrics);
  if (unlistenInit) unlistenInit();
  if (unlistenRecordingToggle) unlistenRecordingToggle();
  if (unlistenNativeError) unlistenNativeError();
  window.removeEventListener('mousemove', onAdjustDragMove);
  window.removeEventListener('mouseup', onAdjustDragUp);
  void cleanupRecordingSource();
});
</script>

<template>
  <div class="overlay-root">
    <img v-if="bgSrc && !isLive && !recordingStartPending" class="bg" :src="bgSrc" draggable="false" />

    <div v-if="!recordingActive" class="mode-panel" @mousedown.stop>
      <div class="mode-switch" role="tablist" aria-label="捕获模式">
        <button
          :class="{ active: captureMode === 'screenshot' }"
          :disabled="modeLocked"
          role="tab"
          :aria-selected="captureMode === 'screenshot'"
          @click="selectCaptureMode('screenshot')"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h3l1.5-2h7L17 7h3a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V9a2 2 0 0 1 2-2Z"/><circle cx="12" cy="13" r="4"/></svg>
          截图
        </button>
        <button
          :class="{ active: captureMode === 'recording' }"
          :disabled="modeLocked"
          role="tab"
          :aria-selected="captureMode === 'recording'"
          @click="selectCaptureMode('recording')"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="5" width="14" height="14" rx="2"/><path d="m17 10 4-2v8l-4-2Z"/></svg>
          录屏
        </button>
      </div>

      <template v-if="captureMode === 'recording'">
        <div class="panel-divider" />
        <div class="source-switch" role="radiogroup" aria-label="录屏来源">
          <button :class="{ active: recordingSource === 'region' }" @click="selectRecordingSource('region')">区域</button>
          <button
            :class="{ active: recordingSource === 'window' }"
            :disabled="!nativeCapabilitiesLoaded || !nativeCapabilities.windowSupported"
            :title="nativeCapabilitiesLoaded && nativeCapabilities.windowSupported ? '录制窗口' : '当前会话不支持窗口录制'"
            @click="selectRecordingSource('window')"
          >窗口</button>
          <button :class="{ active: recordingSource === 'fullscreen' }" @click="selectRecordingSource('fullscreen')">全屏</button>
        </div>
        <button
          v-if="recordingSource !== 'region' && !recordingPrepared"
          class="prepare-button"
          :disabled="!nativeCapabilitiesLoaded || !!capabilitiesError || recordingState === 'preparing-source'"
          @click="prepareSource()"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="4" width="18" height="14" rx="2"/><path d="M8 22h8M12 18v4"/></svg>
          {{ !nativeCapabilitiesLoaded ? '检测录屏能力' : recordingState === 'preparing-source' ? '等待系统选择' : '选择捕获来源' }}
        </button>
        <template v-else-if="recordingSource !== 'region'">
          <button class="secondary-button" @click="resetRecordingSource">重新选择</button>
        </template>
      </template>
    </div>

    <!-- Fullscreen mouse catcher -->
    <div
      v-if="showSelectionUi"
      class="catcher"
      @mousedown="onOverlayMouseDown"
      @mousemove="onOverlayMouseMove"
      @mouseup="onOverlayMouseUp"
    />

    <!-- Dim: solid before selection, punched-out box-shadow trick during/after -->
    <div v-if="showSelectionUi && !previewRect" class="fullscreen-dim" />
    <div
      v-else-if="showSelectionUi"
      class="selection-cutout"
      :style="{
        left: previewRect.x + 'px',
        top: previewRect.y + 'px',
        width: previewRect.w + 'px',
        height: previewRect.h + 'px'
      }"
    />

    <!-- Size label on top-left of selection -->
    <div v-if="previewRect && showSelectionUi" class="size-label" :style="sizeLabelStyle">
      {{ previewSourceSize?.width ?? Math.round(previewRect.w) }}
      ×
      {{ previewSourceSize?.height ?? Math.round(previewRect.h) }}
    </div>

    <!-- Window highlight (only before user starts dragging / makes a selection) -->
    <div
      v-if="captureMode === 'screenshot' && hoveredWindow && !selection && !isSelecting"
      class="window-hint"
      :style="{
        left: hoveredWindow.x + 'px',
        top: hoveredWindow.y + 'px',
        width: hoveredWindow.w + 'px',
        height: hoveredWindow.h + 'px'
      }"
    />
    <div
      v-if="captureMode === 'screenshot' && hoveredWindow && !selection && !isSelecting"
      class="window-label"
      :style="{ left: hoveredWindow.x + 'px', top: Math.max(6, hoveredWindow.y - 22) + 'px' }"
    >
      <span class="mono">{{ hoveredWindow.owner }}</span>
      <span v-if="hoveredWindow.title" class="title">— {{ hoveredWindow.title }}</span>
      <span class="hint-key">click 选中</span>
    </div>

    <!-- Drawing canvas: sized/positioned exactly over the selection -->
    <canvas
      v-if="selection && showSelectionUi"
      ref="canvasRef"
      class="draw-canvas"
      :style="{
        left: selection.x + 'px',
        top: selection.y + 'px',
        width: selection.w + 'px',
        height: selection.h + 'px',
        cursor: adjustCursor
      }"
      @mousedown="onCanvasMouseDown"
      @mousemove="onCanvasMouseMove"
      @mouseup="onCanvasMouseUp"
    />

    <!-- Toolbar under the selection -->
    <div v-if="captureMode === 'screenshot' && selection" class="toolbar" :style="toolbarStyle" @mousedown.stop>
      <div class="tools">
        <button :class="{ active: tool === 'cursor' }" @click="tool = 'cursor'" title="光标（拖动/调整选区）">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
            <path d="M5 3 L5 17 L9 13 L11.5 20 L14 19 L11.5 12 L17 12 Z"/>
          </svg>
        </button>
        <button :class="{ active: tool === 'rect' }" @click="tool = 'rect'" title="矩形">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
            <rect x="4" y="6" width="16" height="12" rx="1"/>
          </svg>
        </button>
        <button :class="{ active: tool === 'ellipse' }" @click="tool = 'ellipse'" title="椭圆">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="7"/>
          </svg>
        </button>
        <button :class="{ active: tool === 'line' }" @click="tool = 'line'" title="直线">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
            <line x1="5" y1="19" x2="19" y2="5"/>
          </svg>
        </button>
        <button :class="{ active: tool === 'arrow' }" @click="tool = 'arrow'" title="箭头">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
            <line x1="5" y1="19" x2="18" y2="6"/>
            <polyline points="11,6 18,6 18,13"/>
          </svg>
        </button>
        <button :class="{ active: tool === 'pencil' }" @click="tool = 'pencil'" title="铅笔">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
            <path d="M4 17 Q7 10 11 14 T18 8"/>
            <path d="M17 8 L20 5 L21 6 L18 9 Z" fill="currentColor" stroke="none"/>
          </svg>
        </button>
        <button :class="{ active: tool === 'mosaic' }" @click="tool = 'mosaic'" title="马赛克">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
            <rect x="4" y="4" width="16" height="16" rx="1"/>
            <line x1="9.33" y1="4" x2="9.33" y2="20"/>
            <line x1="14.67" y1="4" x2="14.67" y2="20"/>
            <line x1="4" y1="9.33" x2="20" y2="9.33"/>
            <line x1="4" y1="14.67" x2="20" y2="14.67"/>
          </svg>
        </button>
        <button :class="{ active: tool === 'text' }" @click="tool = 'text'" title="文字">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
            <line x1="6" y1="6" x2="18" y2="6"/>
            <line x1="12" y1="6" x2="12" y2="19"/>
          </svg>
        </button>
        <button :class="{ active: tool === 'number' }" @click="tool = 'number'" title="序号">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="8"/>
            <path d="M10 9 L12 8 L12 16"/>
            <line x1="9" y1="16" x2="15" y2="16"/>
          </svg>
        </button>
      </div>
      <div class="palette">
        <button
          v-for="c in ['#ef4444', '#f59e0b', '#10b981', '#3b82f6', '#a855f7', '#f8fafc', '#111827']"
          :key="c"
          class="swatch"
          :class="{ active: color === c }"
          :style="{ background: c }"
          @click="color = c"
        />
      </div>
      <div class="width">
        <button v-for="w in [2, 4, 8]" :key="w" :class="{ active: lineWidth === w }" @click="lineWidth = w">
          <span class="dot" :style="{ width: w * 1.5 + 'px', height: w * 1.5 + 'px' }" />
        </button>
      </div>
      <div class="ops">
        <button @click="undo" :disabled="shapes.length === 0" title="撤销 ⌘Z">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
            <path d="M9 14 L4 9 L9 4"/>
            <path d="M4 9 H14 A6 6 0 0 1 20 15 V16"/>
          </svg>
        </button>
        <button @click="redo" :disabled="undoneShapes.length === 0" title="重做 ⌘⇧Z">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
            <path d="M15 14 L20 9 L15 4"/>
            <path d="M20 9 H10 A6 6 0 0 0 4 15 V16"/>
          </svg>
        </button>
        <button @click="reselect" title="重选">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 3 L21 12 L12 21 L3 12 Z"/>
          </svg>
        </button>
        <button @click="saveToFile" title="保存到文件 ⌘S">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
            <line x1="12" y1="4" x2="12" y2="16"/>
            <polyline points="7,11 12,16 17,11"/>
            <line x1="5" y1="20" x2="19" y2="20"/>
          </svg>
        </button>
        <button class="pin" @click="pinIt" title="钉在桌面（浮窗置顶）">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
            <path d="M9 4 L15 4 L14 10 L18 14 L6 14 L10 10 Z"/>
            <line x1="12" y1="14" x2="12" y2="20" stroke="currentColor" fill="none"/>
          </svg>
        </button>
        <button class="cancel" @click="cancel" title="取消 Esc">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
            <line x1="6" y1="6" x2="18" y2="18"/>
            <line x1="18" y1="6" x2="6" y2="18"/>
          </svg>
        </button>
        <button class="confirm" @click="confirm" title="完成 ⌘↩">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="5,13 10,18 19,6"/>
          </svg>
        </button>
      </div>
    </div>

    <!-- Floating text input anchored to the click point, always fresh per click -->
    <input
      v-if="captureMode === 'screenshot' && textPending"
      ref="textInputRef"
      v-model="textValue"
      class="text-input"
      :style="{
        left: textPending.cssLeft + 'px',
        top: textPending.cssTop + 'px',
        color: color,
        fontSize: Math.max(14, lineWidth * 6) + 'px'
      }"
      placeholder="输入文字，回车下笔，Esc 取消"
      @keydown.enter.stop="commitPendingText"
      @keydown.esc.stop="cancelPendingText"
      @mousedown.stop
      @blur="commitPendingText"
    />

    <!-- Instructions when nothing selected yet -->
    <div v-if="captureMode === 'screenshot' && !selection && !isSelecting" class="hint-banner">
      拖动选择区域 · Esc 取消 · Ctrl/⌘ + Enter 完成
    </div>

    <div
      v-if="captureMode === 'recording' && !recordingActive"
      class="recording-hint"
      :class="{ error: recordingState === 'error' }"
    >
      {{ sourceHint }}
      <span v-if="nativeCapabilitiesLoaded && recordingSource === 'window' && !nativeCapabilities.windowSupported">当前平台仅支持区域与全屏</span>
    </div>

    <div
      v-if="captureMode === 'recording' && recordingPrepared && selection"
      class="recording-toolbar"
      :style="recordingToolbarStyle"
      @mousedown.stop
    >
      <div class="recording-selection-summary">
        <span class="record-dot" />
        <span>{{ recordingSource === 'region' ? '区域录屏' : recordingSource === 'window' ? '窗口录屏' : '全屏录制' }}</span>
      </div>
      <button v-if="recordingSource === 'region'" class="icon-button" title="重新框选" @click="reselect">
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 8V4h4M20 8V4h-4M4 16v4h4M20 16v4h-4"/></svg>
      </button>
      <button class="icon-button danger" title="取消" @click="cancel">
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18"/></svg>
      </button>
      <button
        class="start-button"
        :disabled="!nativeCapabilitiesLoaded || !!capabilitiesError"
        @click="startRecording"
      >
        <span class="record-dot" />
        开始录制
      </button>
    </div>

    <div v-if="recordingState === 'countdown' && countdown > 0" class="countdown" :style="selection ? {
      left: selection.x + selection.w / 2 + 'px',
      top: selection.y + selection.h / 2 + 'px'
    } : undefined">
      {{ countdown }}
    </div>

    <div
      v-if="isLive && selection"
      class="recording-frame"
      :class="{ paused: recordingState === 'paused' }"
      :style="{
        left: selection.x + 'px',
        top: selection.y + 'px',
        width: selection.w + 'px',
        height: selection.h + 'px'
      }"
    />

    <div
      v-if="isLive && selection"
      ref="liveControlsRef"
      class="live-controls"
      :style="recordingToolbarStyle"
      @mousedown.stop
    >
      <span class="record-dot" :class="{ paused: recordingState === 'paused' }" />
      <span class="live-time">{{ elapsedLabel }}</span>
      <button v-if="recordingState === 'recording'" class="icon-button" title="暂停" @click="pauseRecording">
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 5v14M16 5v14"/></svg>
      </button>
      <button v-else class="icon-button" title="继续" @click="resumeRecording">
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m8 5 11 7-11 7Z"/></svg>
      </button>
      <button class="stop-button" title="停止并完成" @click="stopRecording">
        <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="6" y="6" width="12" height="12" rx="1"/></svg>
      </button>
      <button class="icon-button danger" title="取消录制" @click="cancel">
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18"/></svg>
      </button>
    </div>
  </div>
</template>

<style scoped>
:global(html),
:global(body),
:global(#root) {
  background: transparent !important;
}
.overlay-root {
  position: fixed;
  inset: 0;
  overflow: hidden;
  user-select: none;
  cursor: crosshair;
  background: transparent;
}
.bg {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  -webkit-user-drag: none;
}
.catcher {
  position: absolute;
  inset: 0;
  z-index: 1;
}
.fullscreen-dim {
  position: absolute;
  inset: 0;
  background: rgba(0, 0, 0, 0.45);
  pointer-events: none;
  z-index: 2;
}
.selection-cutout {
  position: absolute;
  border: 2px solid #3b82f6;
  box-shadow: 0 0 0 9999px rgba(0, 0, 0, 0.45);
  background: transparent;
  box-sizing: border-box;
  pointer-events: none;
  z-index: 2;
}
.size-label {
  position: absolute;
  padding: 2px 8px;
  background: rgba(20, 20, 20, 0.85);
  color: #fff;
  font-size: 11px;
  font-family: -apple-system, "PingFang SC", sans-serif;
  border-radius: 3px;
  pointer-events: none;
  z-index: 10;
}
.window-hint {
  position: absolute;
  border: 2px solid rgba(59, 130, 246, 0.9);
  background: rgba(59, 130, 246, 0.12);
  box-sizing: border-box;
  pointer-events: none;
  z-index: 3;
  border-radius: 2px;
}
.window-label {
  position: absolute;
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 3px 10px;
  background: rgba(59, 130, 246, 0.9);
  color: #fff;
  font-size: 11px;
  font-family: -apple-system, "PingFang SC", sans-serif;
  border-radius: 3px;
  max-width: 60vw;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  pointer-events: none;
  z-index: 11;
}
.window-label .title { opacity: 0.85; }
.window-label .hint-key {
  padding: 1px 6px;
  background: rgba(255, 255, 255, 0.2);
  border-radius: 2px;
  font-size: 10px;
}
.draw-canvas {
  position: absolute;
  cursor: crosshair;
  z-index: 5;
  background: transparent;
}
.toolbar {
  position: absolute;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 10px;
  background: rgba(30, 30, 30, 0.95);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 6px;
  color: #fff;
  z-index: 20;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4);
}
.tools, .palette, .width, .ops {
  display: flex;
  gap: 4px;
  align-items: center;
  padding: 0 6px;
  border-right: 1px solid rgba(255, 255, 255, 0.1);
}
.ops { border-right: none; }
.toolbar button {
  min-width: 36px;
  height: 36px;
  padding: 0 6px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: #fff;
  font-size: 14px;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}
.toolbar button:hover { background: rgba(255, 255, 255, 0.12); }
.toolbar button.active { background: rgba(59, 130, 246, 0.4); }
.toolbar button:disabled { opacity: 0.35; cursor: not-allowed; }
.toolbar button.cancel { color: #ef4444; }
.toolbar button.confirm { color: #10b981; }
.toolbar button.pin { color: #ef4444; }
.swatch {
  width: 22px !important;
  min-width: 22px !important;
  height: 22px !important;
  border-radius: 50% !important;
  padding: 0 !important;
  border: 2px solid transparent !important;
}
.swatch.active { border-color: #fff !important; }
.dot {
  display: inline-block;
  background: currentColor;
  border-radius: 50%;
}
.text-input {
  position: absolute;
  min-width: 100px;
  padding: 2px 6px;
  border: 1px dashed rgba(255, 255, 255, 0.6);
  border-radius: 2px;
  background: rgba(0, 0, 0, 0.35);
  font-family: -apple-system, "PingFang SC", "Segoe UI", sans-serif;
  outline: none;
  z-index: 25;
  transform: translateY(-2px);
}
.hint-banner {
  position: absolute;
  left: 50%;
  bottom: 24px;
  transform: translateX(-50%);
  padding: 6px 14px;
  background: rgba(30, 30, 30, 0.8);
  color: #fff;
  font-size: 12px;
  border-radius: 4px;
  pointer-events: none;
  z-index: 15;
}
.mode-panel {
  position: absolute;
  top: 18px;
  left: 50%;
  z-index: 40;
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 44px;
  max-width: calc(100vw - 32px);
  padding: 6px;
  border: 1px solid var(--line-strong);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
  color: var(--text);
  box-shadow: var(--shadow-pop);
  transform: translateX(-50%);
  cursor: default;
}
.mode-switch,
.source-switch {
  display: flex;
  align-items: center;
  gap: 2px;
  flex: none;
}
.mode-panel button,
.recording-toolbar button,
.live-controls button {
  height: 32px;
  border: 0;
  border-radius: var(--radius);
  background: transparent;
  color: var(--text-muted);
  font: 500 var(--fs-sm)/1 var(--font-sans);
  letter-spacing: 0;
  cursor: pointer;
}
.mode-panel button:hover,
.recording-toolbar button:hover,
.live-controls button:hover {
  background: var(--bg-elev-2);
  color: var(--text);
}
.mode-panel button:disabled {
  cursor: not-allowed;
  opacity: 0.42;
}
.mode-switch button {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 0 11px;
}
.mode-switch button.active,
.source-switch button.active {
  background: var(--accent-soft);
  color: var(--accent);
}
.mode-switch svg,
.prepare-button svg,
.recording-toolbar svg,
.live-controls svg {
  width: 17px;
  height: 17px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.8;
  stroke-linecap: round;
  stroke-linejoin: round;
}
.panel-divider {
  width: 1px;
  height: 22px;
  background: var(--line-strong);
}
.source-switch button {
  min-width: 44px;
  padding: 0 9px;
}
.prepare-button,
.start-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  padding: 0 13px !important;
  background: var(--accent) !important;
  color: var(--accent-fg) !important;
}
.start-button:disabled {
  cursor: not-allowed;
  opacity: 0.42;
}
.secondary-button {
  padding: 0 11px;
  border: 1px solid var(--line-strong) !important;
}
.recording-hint {
  position: absolute;
  left: 50%;
  bottom: 24px;
  z-index: 30;
  display: flex;
  gap: 12px;
  max-width: calc(100vw - 32px);
  padding: 8px 12px;
  border: 1px solid var(--line-strong);
  border-radius: var(--radius);
  background: var(--bg-elevated);
  color: var(--text-muted);
  box-shadow: var(--shadow-pop);
  font: var(--fs-xs)/1.4 var(--font-sans);
  transform: translateX(-50%);
  pointer-events: none;
}
.recording-hint.error {
  border-color: color-mix(in srgb, var(--error) 45%, transparent);
  color: var(--error);
}
.recording-toolbar,
.live-controls {
  position: absolute;
  z-index: 45;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  width: 310px;
  height: 48px;
  padding: 7px 8px;
  box-sizing: border-box;
  border: 1px solid var(--line-strong);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
  color: var(--text);
  box-shadow: var(--shadow-pop);
  cursor: default;
}
.recording-selection-summary {
  display: flex;
  align-items: center;
  gap: 7px;
  min-width: 0;
  margin-right: auto;
  padding: 0 5px;
  color: var(--text-muted);
  font: 500 var(--fs-xs)/1 var(--font-sans);
  white-space: nowrap;
}
.record-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  flex: none;
  border-radius: var(--radius-pill);
  background: var(--error);
}
.record-dot.paused {
  background: var(--warning);
}
.icon-button,
.stop-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  min-width: 32px;
  padding: 0;
}
.icon-button.danger {
  color: var(--error);
}
.stop-button {
  background: var(--error-soft) !important;
  color: var(--error) !important;
}
.countdown {
  position: absolute;
  z-index: 50;
  display: grid;
  place-items: center;
  width: 92px;
  height: 92px;
  border: 1px solid var(--accent-line);
  border-radius: var(--radius-pill);
  background: var(--bg-elevated);
  color: var(--accent);
  box-shadow: var(--shadow-pop);
  font: 600 46px/1 var(--font-mono);
  letter-spacing: 0;
  transform: translate(-50%, -50%);
  pointer-events: none;
}
.recording-frame {
  position: absolute;
  z-index: 35;
  box-sizing: border-box;
  outline: 2px solid var(--error);
  outline-offset: 2px;
  pointer-events: none;
}
.recording-frame.paused {
  outline-color: var(--warning);
}
.live-controls {
  width: 220px;
}
.live-time {
  min-width: 54px;
  margin-right: auto;
  color: var(--text);
  font: 500 var(--fs-sm)/1 var(--font-mono);
}
@media (max-width: 680px) {
  .mode-panel {
    flex-wrap: wrap;
    justify-content: center;
  }
  .panel-divider {
    display: none;
  }
}
</style>
