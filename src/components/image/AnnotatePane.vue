<script setup lang="ts">
import { computed, onActivated, onMounted, ref, watch } from 'vue';
import { NAlert, NButton, NColorPicker, NInput, NInputNumber, NSelect } from 'naive-ui';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { open, save as saveDialog } from '@tauri-apps/plugin-dialog';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import Panel from '../ui/Panel.vue';
import { usePendingAnnotateImage } from './imageBus';

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

const canvasRef = ref<HTMLCanvasElement | null>(null);
const tool = ref<Tool>('rect');
const color = ref('#ef4444');
const lineWidth = ref(3);
const textValue = ref('');
const notice = ref('');
const shapes = ref<Shape[]>([]);
const inputPath = ref('');
const image = ref<HTMLImageElement | null>(null);
const drawing = ref(false);
const startPoint = ref<{ x: number; y: number } | null>(null);
const previewPoint = ref<{ x: number; y: number } | null>(null);

const toolOptions = [
  { label: '矩形', value: 'rect' },
  { label: '箭头', value: 'arrow' },
  { label: '文字', value: 'text' }
];

const canDraw = computed(() => !!image.value);

async function pickFile() {
  notice.value = '';
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'Images', extensions: ['jpg', 'jpeg', 'png', 'webp'] }]
    });
    if (typeof selected === 'string') {
      inputPath.value = selected;
      await loadImage(selected);
    }
  } catch (err) {
    notice.value = String(err);
  }
}

async function loadImage(path: string) {
  return new Promise<void>((resolve, reject) => {
    const img = new Image();
    img.crossOrigin = 'anonymous';
    img.onload = () => {
      image.value = img;
      shapes.value = [];
      draw();
      resolve();
    };
    img.onerror = () => {
      notice.value = '加载图片失败';
      reject(new Error('load failed'));
    };
    img.src = convertFileSrc(path);
  });
}

function fitCanvas(img: HTMLImageElement) {
  const canvas = canvasRef.value;
  if (!canvas) return;
  const maxW = 900;
  const maxH = 620;
  let w = img.naturalWidth;
  let h = img.naturalHeight;
  const scale = Math.min(maxW / w, maxH / h, 1);
  canvas.width = Math.round(w * scale);
  canvas.height = Math.round(h * scale);
}

function draw() {
  const canvas = canvasRef.value;
  if (!canvas || !image.value) return;
  fitCanvas(image.value);
  const ctx = canvas.getContext('2d');
  if (!ctx) return;
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  ctx.drawImage(image.value, 0, 0, canvas.width, canvas.height);
  for (const shape of shapes.value) {
    drawShape(ctx, shape);
  }
  if (drawing.value && startPoint.value && previewPoint.value) {
    const preview: Shape = {
      tool: tool.value,
      x1: startPoint.value.x,
      y1: startPoint.value.y,
      x2: previewPoint.value.x,
      y2: previewPoint.value.y,
      color: color.value,
      lineWidth: lineWidth.value,
      text: tool.value === 'text' ? textValue.value : undefined
    };
    drawShape(ctx, preview);
  }
}

function drawShape(ctx: CanvasRenderingContext2D, shape: Shape) {
  ctx.strokeStyle = shape.color;
  ctx.fillStyle = shape.color;
  ctx.lineWidth = shape.lineWidth;
  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';

  if (shape.tool === 'rect') {
    ctx.strokeRect(
      shape.x1,
      shape.y1,
      shape.x2 - shape.x1,
      shape.y2 - shape.y1
    );
  } else if (shape.tool === 'arrow') {
    const dx = shape.x2 - shape.x1;
    const dy = shape.y2 - shape.y1;
    const angle = Math.atan2(dy, dx);
    const headLen = Math.max(10, shape.lineWidth * 4);
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
    const fontSize = Math.max(14, shape.lineWidth * 6);
    ctx.font = `${fontSize}px -apple-system, "PingFang SC", "Segoe UI", sans-serif`;
    ctx.textBaseline = 'top';
    ctx.fillText(shape.text, shape.x1, shape.y1);
  }
}

function canvasPos(event: MouseEvent) {
  const canvas = canvasRef.value;
  if (!canvas) return { x: 0, y: 0 };
  const rect = canvas.getBoundingClientRect();
  const scaleX = canvas.width / rect.width;
  const scaleY = canvas.height / rect.height;
  return {
    x: (event.clientX - rect.left) * scaleX,
    y: (event.clientY - rect.top) * scaleY
  };
}

function onMouseDown(event: MouseEvent) {
  if (!canDraw.value) return;
  const p = canvasPos(event);
  if (tool.value === 'text') {
    if (!textValue.value.trim()) {
      notice.value = '请先输入要标注的文字';
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
    draw();
    return;
  }
  drawing.value = true;
  startPoint.value = p;
  previewPoint.value = p;
}

function onMouseMove(event: MouseEvent) {
  if (!drawing.value) return;
  previewPoint.value = canvasPos(event);
  draw();
}

function onMouseUp(event: MouseEvent) {
  if (!drawing.value || !startPoint.value) return;
  const p = canvasPos(event);
  if (
    Math.abs(p.x - startPoint.value.x) > 3 ||
    Math.abs(p.y - startPoint.value.y) > 3
  ) {
    shapes.value.push({
      tool: tool.value,
      x1: startPoint.value.x,
      y1: startPoint.value.y,
      x2: p.x,
      y2: p.y,
      color: color.value,
      lineWidth: lineWidth.value
    });
  }
  drawing.value = false;
  startPoint.value = null;
  previewPoint.value = null;
  draw();
}

function undo() {
  shapes.value.pop();
  draw();
}

function clearAll() {
  shapes.value = [];
  draw();
}

async function exportPng() {
  const canvas = canvasRef.value;
  if (!canvas) return;
  notice.value = '';
  try {
    const target = await saveDialog({
      defaultPath: 'annotated.png',
      filters: [{ name: 'PNG', extensions: ['png'] }]
    });
    if (!target) return;
    const dataUrl = canvas.toDataURL('image/png');
    const base64 = dataUrl.split(',')[1];
    await invoke('write_binary_file', { path: target, base64 });
    notice.value = `已导出：${target}`;
  } catch (err) {
    notice.value = String(err);
  }
}

async function copyToClipboard() {
  const canvas = canvasRef.value;
  if (!canvas) return;
  notice.value = '';
  try {
    canvas.toBlob(async (blob) => {
      if (!blob) return;
      // Fallback: copy data URL as text (limited but works cross-platform)
      const url = canvas.toDataURL('image/png');
      await writeText(url);
      notice.value = '已复制标注图为 data URL（粘贴到浏览器可查看）';
    });
  } catch (err) {
    notice.value = String(err);
  }
}

const { pending, consume } = usePendingAnnotateImage();

async function drainPending() {
  if (!pending.value) return;
  const path = consume();
  if (path) {
    inputPath.value = path;
    await loadImage(path);
  }
}

onMounted(() => {
  const canvas = canvasRef.value;
  if (canvas) {
    canvas.width = 900;
    canvas.height = 480;
    const ctx = canvas.getContext('2d');
    if (ctx) {
      ctx.fillStyle = '#111827';
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      ctx.fillStyle = '#6b7280';
      ctx.font = '14px -apple-system, sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText('拖入图片或点选择图片开始', canvas.width / 2, canvas.height / 2);
    }
  }
  drainPending();
});
onActivated(drainPending);
// Also pick up handoff when the pane is already mounted and pending is set later
watch(pending, (val) => { if (val) drainPending(); });

watch([color, lineWidth, textValue, tool], () => draw());
</script>

<template>
  <div class="annotate-pane">
    <div class="left">
      <Panel title="工具">
        <div class="form">
          <label class="field">
            <span class="lbl">图片</span>
            <n-button secondary block @click="pickFile">选择图片</n-button>
          </label>

          <label class="field">
            <span class="lbl">工具</span>
            <n-select v-model:value="tool" :options="toolOptions" />
          </label>

          <label class="field">
            <span class="lbl">颜色</span>
            <n-color-picker v-model:value="color" :show-alpha="false" />
          </label>

          <label class="field">
            <span class="lbl">粗细</span>
            <n-input-number v-model:value="lineWidth" :min="1" :max="20" style="width: 100%" />
          </label>

          <label v-if="tool === 'text'" class="field">
            <span class="lbl">文字内容</span>
            <n-input v-model:value="textValue" placeholder="要写入的文字" />
          </label>

          <div class="actions">
            <n-button secondary block @click="undo" :disabled="shapes.length === 0">撤销</n-button>
            <n-button secondary block @click="clearAll" :disabled="shapes.length === 0">全清</n-button>
          </div>

          <div class="actions">
            <n-button type="primary" block @click="exportPng" :disabled="!canDraw">
              导出 PNG
            </n-button>
            <n-button secondary block @click="copyToClipboard" :disabled="!canDraw">
              复制到剪贴板
            </n-button>
          </div>

          <n-alert v-if="notice" type="info" :bordered="false">{{ notice }}</n-alert>
        </div>
      </Panel>
    </div>

    <div class="right">
      <Panel title="画布">
        <template #right>
          <span class="mono">{{ shapes.length }} 个标注</span>
        </template>
        <div class="canvas-wrap">
          <canvas
            ref="canvasRef"
            @mousedown="onMouseDown"
            @mousemove="onMouseMove"
            @mouseup="onMouseUp"
            @mouseleave="onMouseUp"
          />
        </div>
      </Panel>
    </div>
  </div>
</template>

<style scoped>
.annotate-pane {
  display: grid;
  grid-template-columns: 260px 1fr;
  gap: 16px;
  padding: 16px;
  height: 100%;
  overflow: hidden;
}
.left { min-width: 0; }
.right { min-width: 0; overflow: hidden; }
.form { display: grid; gap: 12px; }
.field { display: grid; gap: 6px; }
.lbl { font-size: var(--fs-xxs); color: var(--text-muted); }
.actions {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}
.canvas-wrap {
  display: grid;
  place-items: center;
  min-height: 500px;
  background: var(--bg-elev);
  border-radius: var(--radius-md);
  overflow: auto;
  padding: 12px;
}
canvas {
  max-width: 100%;
  height: auto;
  cursor: crosshair;
  background: #000;
  border-radius: 4px;
}
.mono { font-family: var(--font-mono, monospace); font-size: var(--fs-xs); }
</style>
