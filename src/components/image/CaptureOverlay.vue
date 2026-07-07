<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { writeImage } from '@tauri-apps/plugin-clipboard-manager';

interface InitPayload {
  imagePath: string;
  screenWidth: number;
  screenHeight: number;
  scaleFactor: number;
}

type Tool = 'rect' | 'arrow' | 'text';

interface Shape {
  tool: Tool;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  color: string;
  lineWidth: number;
  text?: string;
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

const tool = ref<Tool>('rect');
const color = ref('#ef4444');
const lineWidth = ref(3);
const textValue = ref('');
const shapes = ref<Shape[]>([]);
const drawing = ref(false);
const drawStart = ref<{ x: number; y: number } | null>(null);
const drawEnd = ref<{ x: number; y: number } | null>(null);
const showTextInput = ref(false);

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
  const toolbarW = 340;
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
  if (selection.value) return; // Already have a selection; drawing is scoped to canvas layer
  isSelecting.value = true;
  dragStart.value = screenPos(event);
  dragEnd.value = dragStart.value;
}

function onOverlayMouseMove(event: MouseEvent) {
  if (isSelecting.value) {
    dragEnd.value = screenPos(event);
  }
}

function onOverlayMouseUp(event: MouseEvent) {
  if (!isSelecting.value || !dragStart.value) return;
  isSelecting.value = false;
  dragEnd.value = screenPos(event);
  const x = Math.min(dragStart.value.x, dragEnd.value.x);
  const y = Math.min(dragStart.value.y, dragEnd.value.y);
  const w = Math.abs(dragEnd.value.x - dragStart.value.x);
  const h = Math.abs(dragEnd.value.y - dragStart.value.y);
  if (w < 5 || h < 5) {
    dragStart.value = null;
    dragEnd.value = null;
    return;
  }
  selection.value = { x, y, w, h };
  // Prepare drawing canvas sized to selection
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
    if (!textValue.value.trim()) {
      showTextInput.value = true;
      return;
    }
    shapes.value.push({
      tool: 'text',
      x1: p.x,
      y1: p.y,
      x2: p.x,
      y2: p.y,
      color: color.value,
      lineWidth: lineWidth.value,
      text: textValue.value
    });
    redraw();
    return;
  }
  drawing.value = true;
  drawStart.value = p;
  drawEnd.value = p;
}

function onCanvasMouseMove(event: MouseEvent) {
  if (!drawing.value) return;
  event.stopPropagation();
  drawEnd.value = canvasPosFromEvent(event);
  redraw();
}

function onCanvasMouseUp(event: MouseEvent) {
  if (!drawing.value || !drawStart.value) return;
  event.stopPropagation();
  const p = canvasPosFromEvent(event);
  const dx = p.x - drawStart.value.x;
  const dy = p.y - drawStart.value.y;
  if (Math.abs(dx) > 3 || Math.abs(dy) > 3) {
    shapes.value.push({
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
  if (drawing.value && drawStart.value && drawEnd.value) {
    drawShape(
      ctx,
      {
        tool: tool.value,
        x1: drawStart.value.x,
        y1: drawStart.value.y,
        x2: drawEnd.value.x,
        y2: drawEnd.value.y,
        color: color.value,
        lineWidth: lineWidth.value,
        text: tool.value === 'text' ? textValue.value : undefined
      },
      s
    );
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
  } else if (shape.tool === 'text' && shape.text) {
    const fontSize = Math.max(14, shape.lineWidth * 6) * s;
    ctx.font = `${fontSize}px -apple-system, "PingFang SC", "Segoe UI", sans-serif`;
    ctx.textBaseline = 'top';
    ctx.fillText(shape.text, shape.x1, shape.y1);
  }
}

function undo() {
  shapes.value.pop();
  redraw();
}

function reselect() {
  selection.value = null;
  shapes.value = [];
  dragStart.value = null;
  dragEnd.value = null;
}

async function cancel() {
  await invoke('close_capture_overlay').catch(() => {});
  resetState();
}

function resetState() {
  selection.value = null;
  shapes.value = [];
  dragStart.value = null;
  dragEnd.value = null;
  textValue.value = '';
  showTextInput.value = false;
}

async function confirm() {
  if (!selection.value) return;
  const sel = selection.value;
  // Compose: crop the region from the desktop image, then draw the annotations on top.
  const out = document.createElement('canvas');
  out.width = Math.round(sel.w * scale.value);
  out.height = Math.round(sel.h * scale.value);
  const ctx = out.getContext('2d');
  if (!ctx) return;

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
  const s = scale.value;
  for (const shape of shapes.value) {
    drawShape(ctx, shape, s);
  }

  const dataUrl = out.toDataURL('image/png');
  const base64 = dataUrl.split(',')[1];

  // Write to system clipboard as image
  try {
    const bytes = atob(base64)
      .split('')
      .map((c) => c.charCodeAt(0));
    await writeImage(new Uint8Array(bytes));
  } catch (err) {
    console.warn('[capture] clipboard write failed', err);
  }

  // Persist to file + emit capture-completed to main window
  await invoke('commit_capture_overlay', { request: { pngBase64: base64 } }).catch((err) => {
    console.warn('[capture] commit failed', err);
  });

  resetState();
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault();
    cancel();
  } else if ((event.metaKey || event.ctrlKey) && (event.key === 'Enter' || event.key === 'Return')) {
    event.preventDefault();
    if (hasSelection.value) confirm();
  } else if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'z') {
    event.preventDefault();
    undo();
  }
}

function commitTextInput() {
  showTextInput.value = false;
}

onMounted(async () => {
  window.addEventListener('keydown', onKeydown);
  unlistenInit = await listen<InitPayload>('capture-overlay-init', (event) => {
    const p = event.payload;
    bgSrc.value = `${convertFileSrc(p.imagePath)}?t=${Date.now()}`;
    screenW.value = p.screenWidth;
    screenH.value = p.screenHeight;
    scale.value = p.scaleFactor;
    // Reset for a fresh session
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
        <button :class="{ active: tool === 'rect' }" @click="tool = 'rect'" title="矩形">▢</button>
        <button :class="{ active: tool === 'arrow' }" @click="tool = 'arrow'" title="箭头">↗</button>
        <button :class="{ active: tool === 'text' }" @click="tool = 'text'" title="文字">T</button>
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
        <button @click="reselect" title="重选">◇</button>
        <button class="cancel" @click="cancel" title="取消 Esc">✕</button>
        <button class="confirm" @click="confirm" title="完成 ⌘↩">✓</button>
      </div>
    </div>

    <!-- Text input mini-dialog for text tool -->
    <div v-if="showTextInput && selection" class="text-modal" @mousedown.stop>
      <input v-model="textValue" placeholder="输入文字，回车确定" @keydown.enter="commitTextInput" @keydown.esc="showTextInput = false" autofocus />
      <button @click="commitTextInput">确定</button>
    </div>

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
.text-modal {
  position: absolute;
  left: 50%;
  top: 40%;
  transform: translate(-50%, -50%);
  display: flex;
  gap: 6px;
  padding: 12px;
  background: rgba(30, 30, 30, 0.95);
  border-radius: 6px;
  z-index: 30;
}
.text-modal input {
  min-width: 240px;
  padding: 6px 10px;
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 4px;
  background: rgba(0, 0, 0, 0.5);
  color: #fff;
  font-size: 14px;
  outline: none;
}
.text-modal button {
  padding: 6px 14px;
  border: none;
  border-radius: 4px;
  background: #3b82f6;
  color: #fff;
  cursor: pointer;
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
