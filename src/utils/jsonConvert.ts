import yaml from 'js-yaml';
import toml from '@iarna/toml';
import type { ConvertFormat, JsonValue } from '../types/json';

export interface ConvertResult {
  ok: boolean;
  value?: string;
  error?: string;
}

export function convert(text: string, from: ConvertFormat, to: ConvertFormat): ConvertResult {
  if (from === to) return { ok: true, value: text };
  let parsed: JsonValue;
  try {
    parsed = parseFormat(text, from);
  } catch (error) {
    return { ok: false, error: errorMessage(error) };
  }
  try {
    return { ok: true, value: stringifyFormat(parsed, to) };
  } catch (error) {
    return { ok: false, error: errorMessage(error) };
  }
}

function parseFormat(text: string, format: ConvertFormat): JsonValue {
  switch (format) {
    case 'json':
      return JSON.parse(text) as JsonValue;
    case 'yaml':
      return yaml.load(text) as JsonValue;
    case 'toml':
      return toml.parse(text) as unknown as JsonValue;
    case 'csv':
      return parseCsv(text);
  }
}

function stringifyFormat(value: JsonValue, format: ConvertFormat): string {
  switch (format) {
    case 'json':
      return JSON.stringify(value, null, 2);
    case 'yaml':
      return yaml.dump(value);
    case 'toml':
      return toml.stringify(value as Parameters<typeof toml.stringify>[0]);
    case 'csv':
      return toCsv(value);
  }
}

function toCsv(value: JsonValue): string {
  if (!Array.isArray(value)) {
    throw new Error('CSV 仅支持顶层为数组的结构');
  }
  if (value.length === 0) return '';
  const keys = new Set<string>();
  for (const row of value) {
    if (row === null || typeof row !== 'object' || Array.isArray(row)) {
      throw new Error('CSV 仅支持 array-of-flat-objects');
    }
    for (const [k, v] of Object.entries(row)) {
      if (v !== null && typeof v === 'object') {
        throw new Error('CSV 不支持嵌套结构');
      }
      keys.add(k);
    }
  }
  const cols = Array.from(keys);
  const lines = [cols.map(escapeCsv).join(',')];
  for (const row of value as Array<{ [k: string]: JsonValue }>) {
    lines.push(cols.map((c) => escapeCsv(formatScalar(row[c]))).join(','));
  }
  return lines.join('\n');
}

function formatScalar(v: JsonValue | undefined): string {
  if (v === undefined || v === null) return '';
  return String(v);
}

function escapeCsv(s: string): string {
  if (/[",\n]/.test(s)) return `"${s.replace(/"/g, '""')}"`;
  return s;
}

function parseCsv(text: string): JsonValue {
  const rows = parseCsvRows(text);
  if (rows.length === 0) return [];
  const header = rows[0];
  return rows.slice(1).map((row) => {
    const obj: { [k: string]: JsonValue } = {};
    header.forEach((key, i) => {
      obj[key] = row[i] ?? '';
    });
    return obj;
  });
}

function parseCsvRows(text: string): string[][] {
  const rows: string[][] = [];
  let field = '';
  let row: string[] = [];
  let i = 0;
  let inQuotes = false;
  while (i < text.length) {
    const ch = text[i];
    if (inQuotes) {
      if (ch === '"') {
        if (text[i + 1] === '"') {
          field += '"';
          i += 2;
          continue;
        }
        inQuotes = false;
        i++;
        continue;
      }
      field += ch;
      i++;
      continue;
    }
    if (ch === '"') {
      inQuotes = true;
      i++;
      continue;
    }
    if (ch === ',') {
      row.push(field);
      field = '';
      i++;
      continue;
    }
    if (ch === '\n' || ch === '\r') {
      row.push(field);
      rows.push(row);
      row = [];
      field = '';
      if (ch === '\r' && text[i + 1] === '\n') i += 2;
      else i++;
      continue;
    }
    field += ch;
    i++;
  }
  if (field !== '' || row.length > 0) {
    row.push(field);
    rows.push(row);
  }
  return rows.filter((r) => r.length > 0 && !(r.length === 1 && r[0] === ''));
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
