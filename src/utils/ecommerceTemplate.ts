import type { TemplateLayer } from '../types/ecommerceTemplate';

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
    lineHeight: scaledPx(text?.lineHeight, scale),
    letterSpacing: scaledPx(normalizedLetterSpacingPx(layer), scale),
    textAlign: text?.align,
    WebkitTextStroke: text?.strokeColor && strokeWidth > 0 ? `${Number(strokeWidth.toFixed(4))}px ${text.strokeColor}` : undefined
  };
}
