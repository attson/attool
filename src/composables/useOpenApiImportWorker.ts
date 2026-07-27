import type { OpenApiWorkerReq, OpenApiWorkerRes } from '../workers/openapiWorker';
import type { OpenApiImportOptions, ImportedOpenApiCollection } from '../components/http/openapiImport';

export type OpenApiParseOutcome =
  | { ok: true; result: ImportedOpenApiCollection; elapsedMs: number }
  | { ok: false; error: string };

export interface OpenApiWorkerLike {
  postMessage(msg: OpenApiWorkerReq): void;
  addEventListener(type: 'message', listener: (ev: { data: OpenApiWorkerRes }) => void): void;
  terminate(): void;
}

export interface OpenApiImportWorkerClient {
  parse(text: string, options: OpenApiImportOptions, tag?: string): Promise<OpenApiParseOutcome | null>;
  dispose(): void;
}

type Pending = { resolve: (v: OpenApiParseOutcome | null) => void };

export function createOpenApiImportWorkerClient(
  workerFactory: () => OpenApiWorkerLike,
): OpenApiImportWorkerClient {
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
    if (res.ok) p.resolve({ ok: true, result: res.result, elapsedMs: res.elapsedMs });
    else p.resolve({ ok: false, error: res.error });
  });

  return {
    parse(text, options, tag) {
      const id = nextId++;
      if (tag) {
        const prevId = tagToId.get(tag);
        if (prevId !== undefined && pending.has(prevId)) {
          cancelledIds.add(prevId);
        }
        tagToId.set(tag, id);
      }
      return new Promise((resolve) => {
        pending.set(id, { resolve });
        worker.postMessage({ id, kind: 'parse', text, options });
      });
    },
    dispose() {
      worker.terminate();
      pending.clear();
      cancelledIds.clear();
      tagToId.clear();
    },
  };
}

let singleton: OpenApiImportWorkerClient | null = null;

export function useOpenApiImportWorker(): OpenApiImportWorkerClient {
  if (!singleton) {
    singleton = createOpenApiImportWorkerClient(() =>
      new Worker(
        new URL('../workers/openapiWorker.ts', import.meta.url),
        { type: 'module' },
      ) as unknown as OpenApiWorkerLike,
    );
  }
  return singleton;
}
