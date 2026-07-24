import type { JsonParseError, JsonParseResult, JsonValue } from '../types/json';

export function parseJson(text: string): JsonParseResult {
  if (!text.trim()) {
    return { ok: false, error: { message: '输入为空' } };
  }
  try {
    const value = JSON.parse(text) as JsonValue;
    return { ok: true, value };
  } catch (error) {
    return { ok: false, error: toParseError(error, text) };
  }
}

export function format(text: string, indent = 2): string {
  const result = parseJson(text);
  if (!result.ok) throw new Error(result.error.message);
  return JSON.stringify(result.value, null, indent);
}

export function minify(text: string): string {
  const result = parseJson(text);
  if (!result.ok) throw new Error(result.error.message);
  return JSON.stringify(result.value);
}

export function sortKeys(text: string, indent = 2): string {
  const result = parseJson(text);
  if (!result.ok) throw new Error(result.error.message);
  return JSON.stringify(sortValue(result.value), null, indent);
}

export function sortValue(value: JsonValue): JsonValue {
  if (Array.isArray(value)) return value.map(sortValue);
  if (value && typeof value === 'object') {
    const sorted: { [key: string]: JsonValue } = {};
    for (const key of Object.keys(value).sort()) {
      sorted[key] = sortValue((value as { [k: string]: JsonValue })[key]);
    }
    return sorted;
  }
  return value;
}

export function formatValue(value: JsonValue, indent = 2): string {
  return JSON.stringify(value, null, indent);
}

export function minifyValue(value: JsonValue): string {
  return JSON.stringify(value);
}

function toParseError(error: unknown, text: string): JsonParseError {
  const message = error instanceof Error ? error.message : String(error);

  // Newer V8 emits "... (line N column M)"; prefer this when present.
  const lineCol = /\(line (\d+) column (\d+)\)/.exec(message);
  if (lineCol) {
    return { message, line: Number(lineCol[1]), column: Number(lineCol[2]) };
  }

  // Older V8 emits "... at position N" without line/column.
  const positional = /position (\d+)/.exec(message);
  if (positional) {
    const position = Number(positional[1]);
    const upto = text.slice(0, position);
    const line = upto.split('\n').length;
    const column = position - upto.lastIndexOf('\n');
    return { message, line, column };
  }

  return { message };
}
