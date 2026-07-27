import type { OpenApiImportOptions, ImportedOpenApiCollection } from '../components/http/openapiImport';
import { parseOpenApiToCollection } from '../components/http/openapiImport';

export type OpenApiWorkerReq = {
  id: number;
  kind: 'parse';
  text: string;
  options: OpenApiImportOptions;
};

export type OpenApiWorkerRes =
  | { id: number; ok: true; kind: 'parse'; result: ImportedOpenApiCollection; elapsedMs: number }
  | { id: number; ok: false; kind: 'parse'; error: string };

self.onmessage = (event: MessageEvent<OpenApiWorkerReq>) => {
  const req = event.data;
  try {
    const start = performance.now();
    const result = parseOpenApiToCollection(req.text, req.options);
    const elapsedMs = Math.round(performance.now() - start);
    (self as unknown as Worker).postMessage({
      id: req.id, ok: true, kind: 'parse', result, elapsedMs,
    } as OpenApiWorkerRes);
  } catch (e) {
    // Unhandled throw must still resolve the caller's pending promise, or it hangs forever.
    const message = e instanceof Error ? e.message : String(e);
    (self as unknown as Worker).postMessage({
      id: req.id, ok: false, kind: 'parse', error: message,
    } as OpenApiWorkerRes);
  }
};
