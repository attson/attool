import { DiffPatcher } from 'jsondiffpatch';
import { format as formatHtml } from 'jsondiffpatch/formatters/html';

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

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
