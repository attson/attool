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

export interface JsonDiffOptions {
  keyOnly?: boolean;
  parseNestedJsonStrings?: boolean;
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

export function prepareJsonDiffView(
  left: string,
  right: string,
  options: JsonDiffOptions = {},
): JsonDiffView {
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

  let leftValue = JSON.parse(left) as JsonValue;
  let rightValue = JSON.parse(right) as JsonValue;
  if (options.parseNestedJsonStrings) {
    [leftValue, rightValue] = expandNestedJsonPair(leftValue, rightValue);
  }

  if (options.keyOnly) {
    const keyDiff = buildKeyDiff(leftValue, rightValue);
    const leftSkeleton = keyDiff.left ?? emptyKeySkeleton(leftValue);
    const rightSkeleton = keyDiff.right ?? emptyKeySkeleton(rightValue);
    view.equal = keyDiff.count === 0;
    view.delta = patcher.diff(leftSkeleton, rightSkeleton) ?? null;
    view.changeCount = keyDiff.count;
    view.leftText = JSON.stringify(leftSkeleton, null, 2);
    view.rightText = JSON.stringify(rightSkeleton, null, 2);
    return view;
  }

  view.delta = patcher.diff(leftValue, rightValue) ?? null;
  view.equal = view.delta === null;
  view.changeCount = countJsonDiffChanges(view.delta);
  view.leftText = JSON.stringify(sortValue(leftValue), null, 2);
  view.rightText = JSON.stringify(sortValue(rightValue), null, 2);
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

function expandNestedJsonPair(left: JsonValue, right: JsonValue): [JsonValue, JsonValue] {
  if (typeof left === 'string' && typeof right === 'string') {
    const parsedLeft = parseNestedJsonContainer(left);
    const parsedRight = parseNestedJsonContainer(right);
    if (parsedLeft !== null && parsedRight !== null) {
      return expandNestedJsonPair(parsedLeft, parsedRight);
    }
    return [left, right];
  }

  if (Array.isArray(left) && Array.isArray(right)) {
    const expandedLeft = [...left];
    const expandedRight = [...right];
    const sharedLength = Math.min(left.length, right.length);
    for (let index = 0; index < sharedLength; index++) {
      [expandedLeft[index], expandedRight[index]] = expandNestedJsonPair(left[index], right[index]);
    }
    return [expandedLeft, expandedRight];
  }

  if (isJsonObject(left) && isJsonObject(right)) {
    const expandedLeft = { ...left };
    const expandedRight = { ...right };
    for (const key of Object.keys(left)) {
      if (!Object.hasOwn(right, key)) continue;
      [expandedLeft[key], expandedRight[key]] = expandNestedJsonPair(left[key], right[key]);
    }
    return [expandedLeft, expandedRight];
  }

  return [left, right];
}

function parseNestedJsonContainer(text: string): JsonValue[] | Record<string, JsonValue> | null {
  try {
    const parsed = JSON.parse(text) as JsonValue;
    if (Array.isArray(parsed) || isJsonObject(parsed)) return parsed;
  } catch {
    // Invalid nested JSON is an ordinary string value.
  }
  return null;
}

type KeyDiff = {
  left?: JsonValue;
  right?: JsonValue;
  count: number;
};

function buildKeyDiff(left: JsonValue, right: JsonValue): KeyDiff {
  if (Array.isArray(left) && Array.isArray(right)) {
    const leftSkeleton: JsonValue[] = [];
    const rightSkeleton: JsonValue[] = [];
    let count = 0;
    const sharedLength = Math.min(left.length, right.length);
    for (let index = 0; index < sharedLength; index++) {
      const nested = buildKeyDiff(left[index], right[index]);
      if (nested.count === 0) continue;
      while (leftSkeleton.length < index) leftSkeleton.push(null);
      while (rightSkeleton.length < index) rightSkeleton.push(null);
      leftSkeleton[index] = nested.left ?? emptyKeySkeleton(left[index]);
      rightSkeleton[index] = nested.right ?? emptyKeySkeleton(right[index]);
      count += nested.count;
    }
    return {
      left: count > 0 ? leftSkeleton : undefined,
      right: count > 0 ? rightSkeleton : undefined,
      count,
    };
  }

  if (!isJsonObject(left) || !isJsonObject(right)) {
    const leftProjection = projectKeyShape(left);
    const rightProjection = projectKeyShape(right);
    const count = leftProjection.count + rightProjection.count;
    return count > 0
      ? { left: leftProjection.value, right: rightProjection.value, count }
      : { count: 0 };
  }

  const leftSkeleton: Record<string, JsonValue> = {};
  const rightSkeleton: Record<string, JsonValue> = {};
  let count = 0;
  const keys = [...new Set([...Object.keys(left), ...Object.keys(right)])].sort();

  for (const key of keys) {
    const inLeft = Object.hasOwn(left, key);
    const inRight = Object.hasOwn(right, key);
    if (!inLeft) {
      const projected = projectKeyShape(right[key]);
      rightSkeleton[key] = projected.value;
      count += 1 + projected.count;
      continue;
    }
    if (!inRight) {
      const projected = projectKeyShape(left[key]);
      leftSkeleton[key] = projected.value;
      count += 1 + projected.count;
      continue;
    }

    const nested = buildKeyDiff(left[key], right[key]);
    if (nested.count === 0) continue;
    leftSkeleton[key] = nested.left ?? emptyKeySkeleton(left[key]);
    rightSkeleton[key] = nested.right ?? emptyKeySkeleton(right[key]);
    count += nested.count;
  }

  return {
    left: Object.keys(leftSkeleton).length > 0 ? leftSkeleton : undefined,
    right: Object.keys(rightSkeleton).length > 0 ? rightSkeleton : undefined,
    count,
  };
}

function projectKeyShape(value: JsonValue): { value: JsonValue; count: number } {
  if (Array.isArray(value)) {
    const projected: JsonValue[] = [];
    let count = 0;
    for (const item of value) {
      const child = projectKeyShape(item);
      projected.push(child.value);
      count += child.count;
    }
    return { value: projected, count };
  }
  if (!isJsonObject(value)) return { value: null, count: 0 };

  const projected: Record<string, JsonValue> = {};
  let count = 0;
  for (const key of Object.keys(value).sort()) {
    const child = projectKeyShape(value[key]);
    projected[key] = child.value;
    count += 1 + child.count;
  }
  return { value: projected, count };
}

function emptyKeySkeleton(value: JsonValue): JsonValue {
  if (Array.isArray(value)) return [];
  if (isJsonObject(value)) return {};
  return null;
}

function isJsonObject(value: JsonValue): value is Record<string, JsonValue> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
