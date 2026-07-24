import type { WorkerReq, WorkerRes } from '../workers/jsonWorker';
import type { JsonValue, JsonParseError, ConvertFormat } from '../types/json';
import type { SerializeMode } from '../utils/jsonWorkerHandlers';

export type ParseOutcome = { value: JsonValue | null; error?: JsonParseError; elapsedMs: number };
export type SerializeOutcome = { ok: true; text: string; elapsedMs: number } | { ok: false; error: string };
export type JsonPathOutcome =
  | { ok: true; matches: JsonValue[]; text: string; elapsedMs: number }
  | { ok: false; error: string };
export type DiffOutcome = {
  equal: boolean; delta: unknown | null; html: string | null; elapsedMs: number;
  leftError?: string; rightError?: string;
};
export type ConvertOutcome = { ok: true; text: string; elapsedMs: number } | { ok: false; error: string };

export interface WorkerLike {
  postMessage(msg: WorkerReq): void;
  addEventListener(type: 'message', listener: (ev: { data: WorkerRes }) => void): void;
  terminate(): void;
}

type Pending = { resolve: (v: unknown) => void };

export interface JsonWorkerClient {
  parse(text: string, tag?: string): Promise<ParseOutcome | null>;
  serialize(value: JsonValue, mode: SerializeMode, indent?: number, tag?: string): Promise<SerializeOutcome | null>;
  jsonpath(value: JsonValue, expr: string, tag?: string): Promise<JsonPathOutcome | null>;
  diff(leftText: string, rightText: string, withHtml: boolean, tag?: string): Promise<DiffOutcome | null>;
  convert(text: string, from: ConvertFormat, to: ConvertFormat, tag?: string): Promise<ConvertOutcome | null>;
  dispose(): void;
}

export function createJsonWorkerClient(workerFactory: () => WorkerLike): JsonWorkerClient {
  const worker = workerFactory();
  const pending = new Map<number, Pending>();
  const cancelledIds = new Set<number>();
  const tagToId = new Map<string, number>();
  let nextId = 1;

  worker.addEventListener('message', (ev) => {
    const res = ev.data;
    const p = pending.get(res.id);
    if (!p) return;
    pending.delete(res.id);
    if (cancelledIds.has(res.id)) {
      cancelledIds.delete(res.id);
      p.resolve(null);
      return;
    }
    p.resolve(res);
  });

  function issue<K extends WorkerReq['kind']>(
    kind: K,
    build: (id: number) => Extract<WorkerReq, { kind: K }>,
    tag: string | undefined,
    map: (res: WorkerRes) => unknown,
  ): Promise<unknown> {
    const id = nextId++;
    if (tag) {
      const prevId = tagToId.get(tag);
      if (prevId !== undefined && pending.has(prevId)) {
        cancelledIds.add(prevId);
        // Worker is single-thread FIFO run-to-completion; a worker-side cancel
        // message is guaranteed to arrive after the response for `prevId`, so
        // aborting on the worker adds no value. We only mark prevId so that
        // the eventual response is dropped on the main thread below.
      }
      tagToId.set(tag, id);
    }
    return new Promise((resolve) => {
      pending.set(id, {
        resolve: (res: unknown) => {
          if (res === null) { resolve(null); return; }
          resolve(map(res as WorkerRes));
        },
      });
      worker.postMessage(build(id));
    });
  }

  return {
    parse(text, tag) {
      return issue('parse',
        (id) => ({ id, kind: 'parse', text }),
        tag,
        (res) => {
          if (res.kind !== 'parse') return null;
          if (res.ok) return { value: res.value, elapsedMs: res.elapsedMs } satisfies ParseOutcome;
          return { value: null, error: res.error as JsonParseError, elapsedMs: 0 } satisfies ParseOutcome;
        }) as Promise<ParseOutcome | null>;
    },
    serialize(value, mode, indent, tag) {
      return issue('serialize',
        (id) => ({ id, kind: 'serialize', value, mode, indent }),
        tag,
        (res) => {
          if (res.kind !== 'serialize') return null;
          if (res.ok) return { ok: true, text: res.text, elapsedMs: res.elapsedMs } satisfies SerializeOutcome;
          return { ok: false, error: (res.error as { message: string }).message } satisfies SerializeOutcome;
        }) as Promise<SerializeOutcome | null>;
    },
    jsonpath(value, expr, tag) {
      return issue('jsonpath',
        (id) => ({ id, kind: 'jsonpath', value, expr }),
        tag,
        (res) => {
          if (res.kind !== 'jsonpath') return null;
          if (res.ok) return { ok: true, matches: res.matches, text: res.text, elapsedMs: res.elapsedMs } satisfies JsonPathOutcome;
          return { ok: false, error: (res.error as { message: string }).message } satisfies JsonPathOutcome;
        }) as Promise<JsonPathOutcome | null>;
    },
    diff(leftText, rightText, withHtml, tag) {
      return issue('diff',
        (id) => ({ id, kind: 'diff', leftText, rightText, withHtml }),
        tag,
        (res) => {
          if (res.kind !== 'diff') return null;
          if (res.ok) return {
            equal: res.equal, delta: res.delta, html: res.html,
            elapsedMs: res.elapsedMs, leftError: res.leftError, rightError: res.rightError,
          } satisfies DiffOutcome;
          return null;
        }) as Promise<DiffOutcome | null>;
    },
    convert(text, from, to, tag) {
      return issue('convert',
        (id) => ({ id, kind: 'convert', text, from, to }),
        tag,
        (res) => {
          if (res.kind !== 'convert') return null;
          if (res.ok) return { ok: true, text: res.text, elapsedMs: res.elapsedMs } satisfies ConvertOutcome;
          return { ok: false, error: (res.error as { message: string }).message } satisfies ConvertOutcome;
        }) as Promise<ConvertOutcome | null>;
    },
    dispose() {
      worker.terminate();
      pending.clear();
      cancelledIds.clear();
      tagToId.clear();
    },
  };
}

let singleton: JsonWorkerClient | null = null;

export function useJsonWorker(): JsonWorkerClient {
  if (!singleton) {
    singleton = createJsonWorkerClient(() =>
      new Worker(
        new URL('../workers/jsonWorker.ts', import.meta.url),
        { type: 'module' },
      ) as unknown as WorkerLike,
    );
  }
  return singleton;
}
