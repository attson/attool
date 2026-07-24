import type { JsonValue } from '../types/json';

export interface FlatNode {
  key: string;
  parentKey: string | null;
  depth: number;
  label: string;
  path: string;
  kind: 'object' | 'array' | 'primitive';
  size: number;
  primitiveText?: string;
  primitiveClass?: 'string' | 'number' | 'boolean' | 'null';
  hasChildren: boolean;
}

const ROOT_KEY = '$';
const DEFAULT_PRIMITIVE_MAX = 512;

function kindOf(v: JsonValue): FlatNode['kind'] {
  if (Array.isArray(v)) return 'array';
  if (v && typeof v === 'object') return 'object';
  return 'primitive';
}

function sizeOf(v: JsonValue, kind: FlatNode['kind']): number {
  if (kind === 'array') return (v as JsonValue[]).length;
  if (kind === 'object') return Object.keys(v as Record<string, JsonValue>).length;
  return 0;
}

export function primitiveDisplay(
  value: JsonValue,
  max = DEFAULT_PRIMITIVE_MAX,
): { text: string; klass: FlatNode['primitiveClass'] } {
  if (value === null) return { text: 'null', klass: 'null' };
  if (typeof value === 'string') {
    if (value.length > max) return { text: `"…(${value.length} chars)"`, klass: 'string' };
    return { text: JSON.stringify(value), klass: 'string' };
  }
  if (typeof value === 'number') return { text: String(value), klass: 'number' };
  if (typeof value === 'boolean') return { text: String(value), klass: 'boolean' };
  return { text: String(value), klass: undefined };
}

function makeNode(
  key: string,
  parentKey: string | null,
  depth: number,
  label: string,
  path: string,
  value: JsonValue,
  primitiveMax: number,
): FlatNode {
  const kind = kindOf(value);
  const size = sizeOf(value, kind);
  const node: FlatNode = {
    key, parentKey, depth, label, path, kind, size,
    hasChildren: kind !== 'primitive' && size > 0,
  };
  if (kind === 'primitive') {
    const { text, klass } = primitiveDisplay(value, primitiveMax);
    node.primitiveText = text;
    node.primitiveClass = klass;
  }
  return node;
}

interface Frame {
  key: string;
  parentKey: string | null;
  depth: number;
  label: string;
  path: string;
  value: JsonValue;
}

export function flatten(
  root: JsonValue,
  openKeys: Set<string>,
  opts?: { primitiveMax?: number },
): FlatNode[] {
  const primitiveMax = opts?.primitiveMax ?? DEFAULT_PRIMITIVE_MAX;
  const out: FlatNode[] = [];
  const rootFrame: Frame = { key: ROOT_KEY, parentKey: null, depth: 0, label: '', path: ROOT_KEY, value: root };
  const stack: Frame[] = [rootFrame];

  while (stack.length) {
    const f = stack.pop()!;
    const node = makeNode(f.key, f.parentKey, f.depth, f.label, f.path, f.value, primitiveMax);
    out.push(node);
    if (node.kind === 'primitive' || !openKeys.has(node.key)) continue;
    if (node.kind === 'array') {
      const arr = f.value as JsonValue[];
      const frames: Frame[] = [];
      for (let i = 0; i < arr.length; i++) {
        const childKey = `${node.key}[${i}]`;
        frames.push({
          key: childKey,
          parentKey: node.key,
          depth: node.depth + 1,
          label: String(i),
          path: `${node.path}[${i}]`,
          value: arr[i],
        });
      }
      for (let i = frames.length - 1; i >= 0; i--) stack.push(frames[i]);
    } else if (node.kind === 'object') {
      const obj = f.value as Record<string, JsonValue>;
      const keys = Object.keys(obj);
      const frames: Frame[] = [];
      for (const k of keys) {
        const childKey = `${node.key}.${k}`;
        frames.push({
          key: childKey,
          parentKey: node.key,
          depth: node.depth + 1,
          label: k,
          path: `${node.path}.${k}`,
          value: obj[k],
        });
      }
      for (let i = frames.length - 1; i >= 0; i--) stack.push(frames[i]);
    }
  }
  return out;
}

export function allExpandableKeys(root: JsonValue): Set<string> {
  const keys = new Set<string>();
  const stack: Frame[] = [{ key: ROOT_KEY, parentKey: null, depth: 0, label: '', path: ROOT_KEY, value: root }];
  while (stack.length) {
    const f = stack.pop()!;
    const kind = kindOf(f.value);
    if (kind === 'primitive') continue;
    keys.add(f.key);
    if (kind === 'array') {
      const arr = f.value as JsonValue[];
      for (let i = 0; i < arr.length; i++) {
        stack.push({
          key: `${f.key}[${i}]`, parentKey: f.key, depth: f.depth + 1,
          label: String(i), path: `${f.path}[${i}]`, value: arr[i],
        });
      }
    } else {
      const obj = f.value as Record<string, JsonValue>;
      for (const k of Object.keys(obj)) {
        stack.push({
          key: `${f.key}.${k}`, parentKey: f.key, depth: f.depth + 1,
          label: k, path: `${f.path}.${k}`, value: obj[k],
        });
      }
    }
  }
  return keys;
}
