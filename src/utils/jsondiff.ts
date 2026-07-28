import { DiffPatcher } from 'jsondiffpatch';
import { format as formatHtml } from 'jsondiffpatch/formatters/html';
import type { JsonValue } from '../types/json';
import { sortValue } from './jsonFormat';

const patcher = new DiffPatcher({
  objectHash: (item: unknown, index?: number) => {
    if (item && typeof item === 'object' && 'id' in item) {
      return String((item as { id: unknown }).id);
    }
    return `$$index:${index ?? 0}`;
  },
});

export interface DiffResult {
  delta: unknown | null;
  leftError?: string;
  rightError?: string;
}

export interface JsonDiffView {
  equal: boolean;
  delta: unknown | null;
  changeCount: number;
  leftText: string;
  rightText: string;
  leftError?: string;
  rightError?: string;
}

export function diffJson(left: string, right: string): DiffResult {
  const result: DiffResult = { delta: null };
  let leftValue: unknown;
  let rightValue: unknown;
  try {
    leftValue = JSON.parse(left);
  } catch (error) {
    result.leftError = errorMessage(error);
  }
  try {
    rightValue = JSON.parse(right);
  } catch (error) {
    result.rightError = errorMessage(error);
  }
  if (result.leftError || result.rightError) return result;
  result.delta = patcher.diff(leftValue, rightValue) ?? null;
  return result;
}

export function diffJsonHtml(left: string, right: string): string {
  const result = diffJson(left, right);
  if (result.leftError || result.rightError || !result.delta) return '';
  let leftValue: unknown;
  try {
    leftValue = JSON.parse(left);
  } catch {
    return '';
  }
  return formatHtml(result.delta as Parameters<typeof formatHtml>[0], leftValue) ?? '';
}

export function prepareJsonDiffView(left: string, right: string): JsonDiffView {
  const result = diffJson(left, right);
  const view: JsonDiffView = {
    equal: result.delta === null && !result.leftError && !result.rightError,
    delta: result.delta,
    changeCount: countJsonDiffChanges(result.delta),
    leftText: '',
    rightText: '',
    leftError: result.leftError,
    rightError: result.rightError,
  };
  if (result.leftError || result.rightError) return view;

  view.leftText = formatSortedJson(left);
  view.rightText = formatSortedJson(right);
  return view;
}

export function countJsonDiffChanges(delta: unknown): number {
  if (delta === null || delta === undefined) return 0;
  if (Array.isArray(delta)) return 1;
  if (typeof delta !== 'object') return 0;

  let count = 0;
  for (const [key, value] of Object.entries(delta as Record<string, unknown>)) {
    if (key === '_t') continue;
    count += countJsonDiffChanges(value);
  }
  return count;
}

function formatSortedJson(text: string): string {
  const value = JSON.parse(text) as JsonValue;
  return JSON.stringify(sortValue(value), null, 2);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
