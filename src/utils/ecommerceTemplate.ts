import type { ShapeKind, TemplateAsset, TemplateLayer, TemplateProject } from '../types/ecommerceTemplate';

const BINDING_PATTERN = /\{\{\s*([a-zA-Z][a-zA-Z0-9_]*)\s*\}\}/;

export function extractBindingKey(layerName: string): string | undefined {
  return layerName.match(BINDING_PATTERN)?.[1];
}

export function flattenLayers(layers: TemplateLayer[]): TemplateLayer[] {
  const flattened: TemplateLayer[] = [];
  const visit = (items: TemplateLayer[]) => {
    for (const layer of items) {
      flattened.push(layer);
      if (layer.children?.length) {
        visit(layer.children);
      }
    }
  };
  visit(layers);
  return flattened;
}

export function collectBindingKeys(layers: TemplateLayer[]): string[] {
  const keys: string[] = [];
  for (const layer of flattenLayers(layers)) {
    if (layer.bindingKey && !keys.includes(layer.bindingKey)) {
      keys.push(layer.bindingKey);
    }
  }
  return keys;
}

type CanvasSize = { canvasWidth: number; canvasHeight: number };
type TextPreset = 'title' | 'subtitle' | 'body' | 'price';

const textPresets: Record<TextPreset, Pick<TemplateLayer, 'name' | 'width' | 'height'> & { text: TemplateLayer['text'] }> = {
  title: {
    name: '标题文字',
    width: 420,
    height: 96,
    text: {
      text: '双击编辑标题',
      fontFamily: 'PingFang SC',
      fontSize: 64,
      fontWeight: 800,
      color: '#111111',
      align: 'left',
      fontStyle: 'normal',
      textDecoration: 'none',
      backgroundRadius: 0,
      shadowBlur: 0,
      shadowOffsetX: 0,
      shadowOffsetY: 0
    }
  },
  subtitle: {
    name: '副标题文字',
    width: 360,
    height: 64,
    text: {
      text: '输入副标题',
      fontFamily: 'PingFang SC',
      fontSize: 36,
      fontWeight: 700,
      color: '#333333',
      align: 'left',
      fontStyle: 'normal',
      textDecoration: 'none',
      backgroundRadius: 0,
      shadowBlur: 0,
      shadowOffsetX: 0,
      shadowOffsetY: 0
    }
  },
  body: {
    name: '正文文字',
    width: 320,
    height: 48,
    text: {
      text: '输入正文',
      fontFamily: 'PingFang SC',
      fontSize: 24,
      fontWeight: 500,
      color: '#333333',
      align: 'left',
      fontStyle: 'normal',
      textDecoration: 'none',
      backgroundRadius: 0,
      shadowBlur: 0,
      shadowOffsetX: 0,
      shadowOffsetY: 0
    }
  },
  price: {
    name: '价格文字',
    width: 280,
    height: 72,
    text: {
      text: '¥99',
      fontFamily: 'PingFang SC',
      fontSize: 52,
      fontWeight: 900,
      color: '#d63f2f',
      align: 'left',
      fontStyle: 'normal',
      textDecoration: 'none',
      backgroundRadius: 0,
      shadowBlur: 0,
      shadowOffsetX: 0,
      shadowOffsetY: 0
    }
  }
};

function centeredPosition(canvas: CanvasSize, width: number, height: number) {
  return {
    x: Math.max(0, Math.round((canvas.canvasWidth - width) / 2)),
    y: Math.max(0, Math.round((canvas.canvasHeight - height) / 2))
  };
}

export function createTextLayer(options: CanvasSize & { preset: TextPreset }): TemplateLayer {
  const preset = textPresets[options.preset];
  const position = centeredPosition(options, preset.width, preset.height);
  return {
    id: `layer-${crypto.randomUUID()}`,
    name: preset.name,
    type: 'text',
    ...position,
    width: preset.width,
    height: preset.height,
    visible: true,
    opacity: 1,
    rotation: 0,
    locked: false,
    text: { ...preset.text! }
  };
}

export function createShapeLayer(options: CanvasSize & { shape: ShapeKind }): TemplateLayer {
  const width = options.shape === 'line' ? 360 : 300;
  const height = options.shape === 'line' ? 8 : 160;
  const position = centeredPosition(options, width, height);
  return {
    id: `layer-${crypto.randomUUID()}`,
    name: options.shape === 'line' ? '线条' : '形状',
    type: 'shape',
    ...position,
    width,
    height,
    visible: true,
    opacity: 1,
    rotation: 0,
    locked: false,
    shape: {
      shape: options.shape,
      fill: options.shape === 'line' ? '#17211b' : '#f5d36b',
      stroke: '#17211b',
      strokeWidth: 0,
      radius: options.shape === 'roundRect' ? 24 : 0
    }
  };
}

export function createTemplateAsset(input: { path: string; name: string; width: number; height: number; mimeType?: string }): TemplateAsset {
  return {
    id: `asset-${crypto.randomUUID()}`,
    name: input.name,
    path: input.path,
    mimeType: input.mimeType ?? 'image/png',
    width: input.width,
    height: input.height
  };
}

export function createImageLayer(options: CanvasSize & { asset: TemplateAsset }): TemplateLayer {
  const size = Math.round(Math.min(options.canvasWidth, options.canvasHeight) * 0.5);
  const position = centeredPosition(options, size, size);
  return {
    id: `layer-${crypto.randomUUID()}`,
    name: options.asset.name,
    type: 'image',
    ...position,
    width: size,
    height: size,
    visible: true,
    opacity: 1,
    rotation: 0,
    locked: false,
    image: { assetId: options.asset.id, fit: 'contain', replaceable: true }
  };
}

export function insertLayer(project: TemplateProject, layer: TemplateLayer): TemplateProject {
  return { ...project, layers: [...project.layers, layer], updatedAt: new Date().toLocaleString() };
}

export function updateLayerById(layers: TemplateLayer[], layerId: string, updater: (layer: TemplateLayer) => TemplateLayer): TemplateLayer[] {
  return layers.map((layer) => {
    if (layer.id === layerId) return updater(layer);
    if (layer.children) return { ...layer, children: updateLayerById(layer.children, layerId, updater) };
    return layer;
  });
}

export function deleteLayerById(layers: TemplateLayer[], layerId: string): TemplateLayer[] {
  return layers
    .filter((layer) => layer.id !== layerId)
    .map((layer) => (layer.children ? { ...layer, children: deleteLayerById(layer.children, layerId) } : layer));
}

export function removeSelectedLayer(project: TemplateProject, selectedLayerId: string | null): { project: TemplateProject; selectedLayerId: string | null } {
  if (!selectedLayerId) {
    return { project, selectedLayerId: null };
  }
  const nextLayers = deleteLayerById(project.layers, selectedLayerId);
  return {
    project: { ...project, layers: nextLayers, updatedAt: new Date().toLocaleString() },
    selectedLayerId: null
  };
}

function cloneLayer(layer: TemplateLayer): TemplateLayer {
  return {
    ...layer,
    id: `layer-${crypto.randomUUID()}`,
    name: `${layer.name} 副本`,
    x: layer.x + 24,
    y: layer.y + 24,
    children: layer.children?.map(cloneLayer),
    text: layer.text ? { ...layer.text } : undefined,
    image: layer.image ? { ...layer.image } : undefined,
    shape: layer.shape ? { ...layer.shape } : undefined
  };
}

export function duplicateLayer(layers: TemplateLayer[], layerId: string): TemplateLayer[] {
  const result: TemplateLayer[] = [];
  for (const layer of layers) {
    result.push(layer);
    if (layer.id === layerId) {
      result.push(cloneLayer(layer));
      continue;
    }
    if (layer.children) {
      result[result.length - 1] = { ...layer, children: duplicateLayer(layer.children, layerId) };
    }
  }
  return result;
}

export type LayerMoveDirection = 'forward' | 'backward' | 'front' | 'back';

export function moveLayer(layers: TemplateLayer[], layerId: string, direction: LayerMoveDirection): TemplateLayer[] {
  const index = layers.findIndex((layer) => layer.id === layerId);
  if (index >= 0) {
    const next = [...layers];
    const [layer] = next.splice(index, 1);
    const target = direction === 'front' ? next.length : direction === 'back' ? 0 : direction === 'forward' ? Math.min(next.length, index + 1) : Math.max(0, index - 1);
    next.splice(target, 0, layer);
    return next;
  }
  return layers.map((layer) => (layer.children ? { ...layer, children: moveLayer(layer.children, layerId, direction) } : layer));
}

function hasDescendant(layer: TemplateLayer, layerId: string): boolean {
  return Boolean(layer.children?.some((child) => child.id === layerId || hasDescendant(child, layerId)));
}

function removeLayerFromTree(layers: TemplateLayer[], layerId: string): { layers: TemplateLayer[]; removed: TemplateLayer | null } {
  const index = layers.findIndex((layer) => layer.id === layerId);
  if (index >= 0) {
    const next = [...layers];
    const [removed] = next.splice(index, 1);
    return { layers: next, removed };
  }

  let removed: TemplateLayer | null = null;
  const next = layers.map((layer) => {
    if (!layer.children || removed) return layer;
    const result = removeLayerFromTree(layer.children, layerId);
    removed = result.removed;
    return removed ? { ...layer, children: result.layers } : layer;
  });

  return { layers: next, removed };
}

export type LayerDropPlacement = 'before' | 'after';

function insertLayerNear(layers: TemplateLayer[], targetLayerId: string, layerToInsert: TemplateLayer, placement: LayerDropPlacement): { layers: TemplateLayer[]; inserted: boolean } {
  const index = layers.findIndex((layer) => layer.id === targetLayerId);
  if (index >= 0) {
    const next = [...layers];
    next.splice(placement === 'after' ? index + 1 : index, 0, layerToInsert);
    return { layers: next, inserted: true };
  }

  let inserted = false;
  const next = layers.map((layer) => {
    if (!layer.children || inserted) return layer;
    const result = insertLayerNear(layer.children, targetLayerId, layerToInsert, placement);
    inserted = result.inserted;
    return inserted ? { ...layer, children: result.layers } : layer;
  });

  return { layers: next, inserted };
}

export function reorderLayer(layers: TemplateLayer[], draggedLayerId: string, targetLayerId: string, placement: LayerDropPlacement = 'before'): TemplateLayer[] {
  if (draggedLayerId === targetLayerId) return layers;
  const dragged = flattenLayers(layers).find((layer) => layer.id === draggedLayerId);
  if (!dragged || hasDescendant(dragged, targetLayerId)) return layers;

  const removed = removeLayerFromTree(layers, draggedLayerId);
  if (!removed.removed) return layers;

  const inserted = insertLayerNear(removed.layers, targetLayerId, removed.removed, placement);
  return inserted.inserted ? inserted.layers : layers;
}

export function reorderLayerBefore(layers: TemplateLayer[], draggedLayerId: string, targetLayerId: string): TemplateLayer[] {
  return reorderLayer(layers, draggedLayerId, targetLayerId, 'before');
}

export function validateBatchFields(requiredFields: string[], incomingFields: string[]) {
  return {
    missingFields: requiredFields.filter((field) => !incomingFields.includes(field)),
    unusedFields: incomingFields.filter((field) => !requiredFields.includes(field))
  };
}

export function makeExportFileName(values: Record<string, string>, rowIndex: number): string {
  const rawName = values.name || values.title || String(rowIndex + 1).padStart(3, '0');
  const safeName = rawName
    .trim()
    .replace(/[\\/:*?"<>|]+/g, '_')
    .replace(/\s+/g, '_')
    .replace(/^\.+$/, '')
    .slice(0, 80);

  return `${safeName || String(rowIndex + 1).padStart(3, '0')}.png`;
}

function scaledPx(value: number | undefined, scale: number): string | undefined {
  if (value === undefined) {
    return undefined;
  }
  return `${Number((value * scale).toFixed(4))}px`;
}

function normalizedLetterSpacingPx(layer: TemplateLayer): number | undefined {
  const spacing = layer.text?.letterSpacing;
  if (spacing === undefined) {
    return undefined;
  }
  const fontSize = layer.text?.fontSize ?? 24;
  const looksLikePsdTracking = Math.abs(spacing) > Math.max(10, fontSize * 0.5);
  return looksLikePsdTracking ? (spacing / 1000) * fontSize : spacing;
}

export function textLayerPreviewStyle(layer: TemplateLayer, canvasScale: number) {
  const text = layer.text;
  const scale = Number.isFinite(canvasScale) && canvasScale > 0 ? canvasScale : 1;
  const strokeWidth = text?.strokeWidth ? text.strokeWidth * scale : 0;

  return {
    color: text?.color,
    fontFamily: text?.fontFamily,
    fontSize: scaledPx(text?.fontSize ?? 24, scale),
    fontWeight: text?.fontWeight,
    fontStyle: text?.fontStyle,
    lineHeight: scaledPx(text?.lineHeight, scale),
    letterSpacing: scaledPx(normalizedLetterSpacingPx(layer), scale),
    textAlign: text?.align,
    textDecoration: text?.textDecoration && text.textDecoration !== 'none' ? text.textDecoration : undefined,
    backgroundColor: text?.backgroundColor,
    borderRadius: scaledPx(text?.backgroundRadius, scale),
    textShadow:
      text?.shadowColor && ((text.shadowBlur ?? 0) > 0 || (text.shadowOffsetX ?? 0) !== 0 || (text.shadowOffsetY ?? 0) !== 0)
        ? `${Number(((text.shadowOffsetX ?? 0) * scale).toFixed(4))}px ${Number(((text.shadowOffsetY ?? 0) * scale).toFixed(4))}px ${Number(((text.shadowBlur ?? 0) * scale).toFixed(4))}px ${text.shadowColor}`
        : undefined,
    WebkitTextStroke: text?.strokeColor && strokeWidth > 0 ? `${Number(strokeWidth.toFixed(4))}px ${text.strokeColor}` : undefined
  };
}
