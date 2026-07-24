import './globalShim';

import type { JsonValue, JsonParseError, ConvertFormat } from '../types/json';
import {
  handleParse, handleSerialize, handleJsonpath, handleDiff, handleConvert,
  type SerializeMode,
} from '../utils/jsonWorkerHandlers';

export type WorkerReq =
  | { id: number; kind: 'parse'; text: string }
  | { id: number; kind: 'serialize'; value: JsonValue; mode: SerializeMode; indent?: number }
  | { id: number; kind: 'jsonpath'; value: JsonValue; expr: string }
  | { id: number; kind: 'diff'; leftText: string; rightText: string; withHtml: boolean }
  | { id: number; kind: 'convert'; text: string; from: ConvertFormat; to: ConvertFormat };

export type WorkerRes =
  | { id: number; ok: true;  kind: 'parse';     value: JsonValue; elapsedMs: number }
  | { id: number; ok: true;  kind: 'serialize'; text: string; elapsedMs: number }
  | { id: number; ok: true;  kind: 'jsonpath';  matches: JsonValue[]; text: string; elapsedMs: number }
  | { id: number; ok: true;  kind: 'diff';      equal: boolean; delta: unknown | null; html: string | null; elapsedMs: number; leftError?: string; rightError?: string }
  | { id: number; ok: true;  kind: 'convert';   text: string; elapsedMs: number }
  | { id: number; ok: false; kind: WorkerReq['kind']; error: JsonParseError | { message: string } };

self.onmessage = (event: MessageEvent<WorkerReq>) => {
  const req = event.data;
  try {
    const res = dispatch(req);
    (self as unknown as Worker).postMessage(res);
  } catch (e) {
    // Unhandled throw must still resolve the caller's pending promise, or it hangs forever.
    const message = e instanceof Error ? e.message : String(e);
    (self as unknown as Worker).postMessage({
      id: req.id,
      ok: false,
      kind: req.kind,
      error: { message },
    } as WorkerRes);
  }
};

function dispatch(req: WorkerReq): WorkerRes {
  switch (req.kind) {
    case 'parse': {
      const r = handleParse(req.text);
      if (r.ok) return { id: req.id, ok: true, kind: 'parse', value: r.value, elapsedMs: r.elapsedMs };
      return { id: req.id, ok: false, kind: 'parse', error: r.error };
    }
    case 'serialize': {
      const r = handleSerialize(req.value, req.mode, req.indent);
      if (r.ok) return { id: req.id, ok: true, kind: 'serialize', text: r.text, elapsedMs: r.elapsedMs };
      return { id: req.id, ok: false, kind: 'serialize', error: r.error };
    }
    case 'jsonpath': {
      const r = handleJsonpath(req.value, req.expr);
      if (r.ok) return { id: req.id, ok: true, kind: 'jsonpath', matches: r.matches, text: r.text, elapsedMs: r.elapsedMs };
      return { id: req.id, ok: false, kind: 'jsonpath', error: r.error };
    }
    case 'diff': {
      const r = handleDiff(req.leftText, req.rightText, req.withHtml);
      return { id: req.id, ok: true, kind: 'diff', ...r };
    }
    case 'convert': {
      const r = handleConvert(req.text, req.from, req.to);
      if (r.ok) return { id: req.id, ok: true, kind: 'convert', text: r.text, elapsedMs: r.elapsedMs };
      return { id: req.id, ok: false, kind: 'convert', error: r.error };
    }
  }
}
