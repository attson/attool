<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { save as saveDialog } from '@tauri-apps/plugin-dialog';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

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
  imagePath: string;
  screenWidth: number;
  screenHeight: number;
  scaleFactor: number;
  windows: WindowRect[];
}

type Tool = 'rect' | 'ellipse' | 'line' | 'arrow' | 'pencil' | 'text' | 'number' | 'mosaic';

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
const screenW = ref(1);
const screenH = ref(1);
const scale = ref(1);

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

const tool = ref<Tool>('rect');
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

let unlistenInit: UnlistenFn | undefined;

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

const toolbarStyle = computed(() => {
  if (!selection.value) return { display: 'none' };
  const sel = selection.value;
  const toolbarW = 760;
  const toolbarH = 44;
  const margin = 8;
  let left = sel.x + sel.w - toolbarW;
  let top = sel.y + sel.h + margin;
  // Wrap left if it'd go off-screen right
  if (left < 8) left = 8;
  if (left + toolbarW > screenW.value - 8) left = screenW.value - toolbarW - 8;
  // If it would go below screen, put it above the selection instead
  if (top + toolbarH > screenH.value - 8) top = Math.max(8, sel.y - toolbarH - margin);
  return { left: `${left}px`, top: `${top}px` };
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
  if (selection.value) return;
  isSelecting.value = true;
  dragStart.value = screenPos(event);
  dragEnd.value = dragStart.value;
}

function onOverlayMouseMove(event: MouseEvent) {
  const p = screenPos(event);
  cursor.value = p;
  if (isSelecting.value) {
    dragEnd.value = p;
  } else if (!selection.value) {
    hoveredWindow.value = windowAt(p.x, p.y);
  }
}

function onOverlayMouseUp(event: MouseEvent) {
  if (!isSelecting.value || !dragStart.value) return;
  isSelecting.value = false;
  dragEnd.value = screenPos(event);
  const dx = Math.abs(dragEnd.value.x - dragStart.value.x);
  const dy = Math.abs(dragEnd.value.y - dragStart.value.y);
  // Treat a click (no meaningful drag) on a hovered window as "snap to window".
  if (dx < 5 && dy < 5 && hoveredWindow.value) {
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

function onCanvasMouseDown(event: MouseEvent) {
  event.stopPropagation();
  const p = canvasPosFromEvent(event);
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
  selection.value = null;
  shapes.value = [];
  undoneShapes.value = [];
  nextNumber.value = 1;
  dragStart.value = null;
  dragEnd.value = null;
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
  const canvas = await composeCanvas();
  if (!canvas) return;
  const ts = new Date().toISOString().replace(/[:.]/g, '-');
  const target = await saveDialog({
    defaultPath: `attool-${ts}.png`,
    filters: [{ name: 'PNG', extensions: ['png'] }]
  }).catch(() => null);
  if (!target) return;
  const dataUrl = canvas.toDataURL('image/png');
  const base64 = dataUrl.split(',')[1];
  await invoke('write_binary_file', { path: target, base64 }).catch((err) => {
    console.warn('[capture] save failed', err);
  });
  await invoke('close_capture_overlay').catch(() => {});
  resetState();
}

async function cancel() {
  await invoke('close_capture_overlay').catch(() => {});
  resetState();
}

function resetState() {
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
}

async function confirm() {
  const canvas = await composeCanvas();
  if (!canvas) return;
  const dataUrl = canvas.toDataURL('image/png');
  const base64 = dataUrl.split(',')[1];
  // Rust side saves file AND writes clipboard image — one round trip, no flaky JS Image.fromBytes.
  await invoke('commit_capture_overlay', { request: { pngBase64: base64 } }).catch((err) => {
    console.warn('[capture] commit failed', err);
  });
  resetState();
}

async function pinIt() {
  const canvas = await composeCanvas();
  if (!canvas) return;
  const dataUrl = canvas.toDataURL('image/png');
  const base64 = dataUrl.split(',')[1];
  await invoke('pin_capture_overlay', { request: { pngBase64: base64 } }).catch((err) => {
    console.warn('[capture] pin failed', err);
  });
  resetState();
}

function onKeydown(event: KeyboardEvent) {
  if (textPending.value) return;
  if (event.key === 'Escape') {
    event.preventDefault();
    cancel();
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

onMounted(async () => {
  window.addEventListener('keydown', onKeydown);
  unlistenInit = await listen<InitPayload>('capture-overlay-init', (event) => {
    const p = event.payload;
    bgSrc.value = `${convertFileSrc(p.imagePath)}?t=${Date.now()}`;
    screenW.value = p.screenWidth;
    screenH.value = p.screenHeight;
    scale.value = p.scaleFactor;
    windowList.value = p.windows ?? [];
    resetState();
  });
});

onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown);
  if (unlistenInit) unlistenInit();
});
</script>

<template>
  <div class="overlay-root">
    <img v-if="bgSrc" class="bg" :src="bgSrc" draggable="false" />

    <!-- Fullscreen mouse catcher -->
    <div
      class="catcher"
      @mousedown="onOverlayMouseDown"
      @mousemove="onOverlayMouseMove"
      @mouseup="onOverlayMouseUp"
    />

    <!-- Dim: solid before selection, punched-out box-shadow trick during/after -->
    <div v-if="!previewRect" class="fullscreen-dim" />
    <div
      v-else
      class="selection-cutout"
      :style="{
        left: previewRect.x + 'px',
        top: previewRect.y + 'px',
        width: previewRect.w + 'px',
        height: previewRect.h + 'px'
      }"
    />

    <!-- Size label on top-left of selection -->
    <div v-if="previewRect" class="size-label" :style="sizeLabelStyle">
      {{ Math.round(previewRect.w) }} × {{ Math.round(previewRect.h) }}
    </div>

    <!-- Window highlight (only before user starts dragging / makes a selection) -->
    <div
      v-if="hoveredWindow && !selection && !isSelecting"
      class="window-hint"
      :style="{
        left: hoveredWindow.x + 'px',
        top: hoveredWindow.y + 'px',
        width: hoveredWindow.w + 'px',
        height: hoveredWindow.h + 'px'
      }"
    />
    <div
      v-if="hoveredWindow && !selection && !isSelecting"
      class="window-label"
      :style="{ left: hoveredWindow.x + 'px', top: Math.max(6, hoveredWindow.y - 22) + 'px' }"
    >
      <span class="mono">{{ hoveredWindow.owner }}</span>
      <span v-if="hoveredWindow.title" class="title">— {{ hoveredWindow.title }}</span>
      <span class="hint-key">click 选中</span>
    </div>

    <!-- Drawing canvas: sized/positioned exactly over the selection -->
    <canvas
      v-if="selection"
      ref="canvasRef"
      class="draw-canvas"
      :style="{
        left: selection.x + 'px',
        top: selection.y + 'px',
        width: selection.w + 'px',
        height: selection.h + 'px'
      }"
      @mousedown="onCanvasMouseDown"
      @mousemove="onCanvasMouseMove"
      @mouseup="onCanvasMouseUp"
    />

    <!-- Toolbar under the selection -->
    <div v-if="selection" class="toolbar" :style="toolbarStyle" @mousedown.stop>
      <div class="tools">
        <button :class="{ active: tool === 'rect' }" @click="tool = 'rect'" title="矩形">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
            <rect x="4" y="6" width="16" height="12" rx="1"/>
          </svg>
        </button>
        <button :class="{ active: tool === 'ellipse' }" @click="tool = 'ellipse'" title="椭圆">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="7"/>
          </svg>
        </button>
        <button :class="{ active: tool === 'line' }" @click="tool = 'line'" title="直线">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
            <line x1="5" y1="19" x2="19" y2="5"/>
          </svg>
        </button>
        <button :class="{ active: tool === 'arrow' }" @click="tool = 'arrow'" title="箭头">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
            <line x1="5" y1="19" x2="18" y2="6"/>
            <polyline points="11,6 18,6 18,13"/>
          </svg>
        </button>
        <button :class="{ active: tool === 'pencil' }" @click="tool = 'pencil'" title="铅笔">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
            <path d="M4 17 Q7 10 11 14 T18 8"/>
            <path d="M17 8 L20 5 L21 6 L18 9 Z" fill="currentColor" stroke="none"/>
          </svg>
        </button>
        <button :class="{ active: tool === 'mosaic' }" @click="tool = 'mosaic'" title="马赛克">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
            <rect x="4" y="4" width="16" height="16" rx="1"/>
            <line x1="9.33" y1="4" x2="9.33" y2="20"/>
            <line x1="14.67" y1="4" x2="14.67" y2="20"/>
            <line x1="4" y1="9.33" x2="20" y2="9.33"/>
            <line x1="4" y1="14.67" x2="20" y2="14.67"/>
          </svg>
        </button>
        <button :class="{ active: tool === 'text' }" @click="tool = 'text'" title="文字">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
            <line x1="6" y1="6" x2="18" y2="6"/>
            <line x1="12" y1="6" x2="12" y2="19"/>
          </svg>
        </button>
        <button :class="{ active: tool === 'number' }" @click="tool = 'number'" title="序号">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
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
        <button @click="undo" :disabled="shapes.length === 0" title="撤销 ⌘Z">↺</button>
        <button @click="redo" :disabled="undoneShapes.length === 0" title="重做 ⌘⇧Z">↻</button>
        <button @click="reselect" title="重选">◇</button>
        <button @click="saveToFile" title="保存到文件 ⌘S">⬇</button>
        <button @click="pinIt" title="钉在桌面（浮窗置顶）">📌</button>
        <button class="cancel" @click="cancel" title="取消 Esc">✕</button>
        <button class="confirm" @click="confirm" title="完成 ⌘↩">✓</button>
      </div>
    </div>

    <!-- Floating text input anchored to the click point, always fresh per click -->
    <input
      v-if="textPending"
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
    <div v-if="!selection && !isSelecting" class="hint-banner">
      拖动选择区域 · Esc 取消 · ⌘↩ 完成
    </div>
  </div>
</template>

<style scoped>
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
  padding: 6px 8px;
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
  min-width: 26px;
  height: 26px;
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
.swatch {
  width: 18px !important;
  min-width: 18px !important;
  height: 18px !important;
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
  top: 24px;
  transform: translateX(-50%);
  padding: 6px 14px;
  background: rgba(30, 30, 30, 0.8);
  color: #fff;
  font-size: 12px;
  border-radius: 4px;
  pointer-events: none;
  z-index: 15;
}
</style>
