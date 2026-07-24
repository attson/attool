import { JSONPath } from 'jsonpath-plus';
import type { JsonParseError, JsonValue } from '../types/json';
import type { ConvertFormat } from '../types/json';
import { parseJson, formatValue, minifyValue, sortValue } from './jsonFormat';
import { diffJson, diffJsonHtml } from './jsondiff';
import { convert as convertFormats } from './jsonConvert';

type Ok<T> = T & { ok: true };
type Err = { ok: false; error: { message: string } | JsonParseError };

export type SerializeMode = 'format' | 'minify' | 'sort';

export function handleParse(text: string):
  | Ok<{ value: JsonValue; elapsedMs: number }>
  | { ok: false; error: JsonParseError } {
  const start = performance.now();
  const r = parseJson(text);
  const elapsedMs = Math.round(performance.now() - start);
  if (r.ok) return { ok: true, value: (r.value ?? null) as JsonValue, elapsedMs };
  return { ok: false, error: r.error ?? { message: '解析失败' } };
}

export function handleSerialize(
  value: JsonValue,
  mode: SerializeMode,
  indent = 2,
): Ok<{ text: string; elapsedMs: number }> | Err {
  const start = performance.now();
  try {
    let text: string;
    if (mode === 'format') text = formatValue(value, indent);
    else if (mode === 'minify') text = minifyValue(value);
    else text = formatValue(sortValue(value), indent);
    return { ok: true, text, elapsedMs: Math.round(performance.now() - start) };
  } catch (e) {
    return { ok: false, error: { message: (e as Error).message } };
  }
}

export function handleJsonpath(
  value: JsonValue,
  expr: string,
): Ok<{ matches: JsonValue[]; text: string; elapsedMs: number }> | Err {
  const start = performance.now();
  try {
    assertValidJsonpath(expr);
    const matches = JSONPath({ path: expr || '$', json: value }) as JsonValue[];
    const text = JSON.stringify(matches, null, 2);
    return { ok: true, matches, text, elapsedMs: Math.round(performance.now() - start) };
  } catch (e) {
    return { ok: false, error: { message: (e as Error).message } };
  }
}

// jsonpath-plus's grammar tolerates near-arbitrary garbage (e.g. "][[[" quietly
// resolves to an empty match) instead of throwing, so obviously malformed
// expressions need to be rejected before reaching it.
function assertValidJsonpath(expr: string): void {
  const trimmed = expr.trim();
  if (!trimmed) return;
  if (!/^[$@]/.test(trimmed)) throw new Error('无效的 JSONPath 表达式');
  let depth = 0;
  for (const ch of trimmed) {
    if (ch === '[') depth++;
    else if (ch === ']') {
      depth--;
      if (depth < 0) throw new Error('无效的 JSONPath 表达式');
    }
  }
  if (depth !== 0) throw new Error('无效的 JSONPath 表达式');
}

export function handleDiff(
  leftText: string,
  rightText: string,
  withHtml: boolean,
): {
  equal: boolean;
  delta: unknown | null;
  html: string | null;
  elapsedMs: number;
  leftError?: string;
  rightError?: string;
} {
  const start = performance.now();
  const r = diffJson(leftText || '{}', rightText || '{}');
  const result = {
    equal: false,
    delta: null as unknown | null,
    html: null as string | null,
    leftError: r.leftError,
    rightError: r.rightError,
    elapsedMs: 0,
  };
  if (r.leftError || r.rightError) {
    result.elapsedMs = Math.round(performance.now() - start);
    return result;
  }
  if (r.delta === null) {
    result.equal = true;
    result.elapsedMs = Math.round(performance.now() - start);
    return result;
  }
  result.equal = false;
  result.delta = r.delta;
  if (withHtml) result.html = diffJsonHtml(leftText || '{}', rightText || '{}');
  result.elapsedMs = Math.round(performance.now() - start);
  return result;
}

export function handleConvert(
  text: string,
  from: ConvertFormat,
  to: ConvertFormat,
): Ok<{ text: string; elapsedMs: number }> | Err {
  const start = performance.now();
  const r = convertFormats(text, from, to);
  if (r.ok) {
    return { ok: true, text: r.value ?? '', elapsedMs: Math.round(performance.now() - start) };
  }
  return { ok: false, error: { message: r.error ?? '转换失败' } };
}
