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
