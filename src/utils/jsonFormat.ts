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
  if (!result.ok) throw new Error(result.error?.message ?? 'JSON 解析失败');
  return JSON.stringify(result.value, null, indent);
}

export function minify(text: string): string {
  const result = parseJson(text);
  if (!result.ok) throw new Error(result.error?.message ?? 'JSON 解析失败');
  return JSON.stringify(result.value);
}

export function sortKeys(text: string, indent = 2): string {
  const result = parseJson(text);
  if (!result.ok) throw new Error(result.error?.message ?? 'JSON 解析失败');
  return JSON.stringify(sortValue(result.value as JsonValue), null, indent);
}

function sortValue(value: JsonValue): JsonValue {
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

function toParseError(error: unknown, text: string): JsonParseError {
  const message = error instanceof Error ? error.message : String(error);
  const match = /position (\d+)/.exec(message);
  if (!match) return { message };
  const position = Number(match[1]);
  const upto = text.slice(0, position);
  const line = upto.split('\n').length;
  const column = position - upto.lastIndexOf('\n');
  return { message, line, column };
}
