import { describe, expect, it } from 'vitest';
import { createOpenApiImportWorkerClient, type OpenApiWorkerLike } from './useOpenApiImportWorker';
import type { OpenApiWorkerReq, OpenApiWorkerRes } from '../workers/openapiWorker';

function makeMockWorker() {
  let listener: ((ev: { data: OpenApiWorkerRes }) => void) | null = null;
  const posted: OpenApiWorkerReq[] = [];
  const worker: OpenApiWorkerLike = {
    postMessage(msg) { posted.push(msg); },
    addEventListener(_type, cb) { listener = cb; },
    terminate() {},
  };
  return {
    worker,
    posted,
    respond(res: OpenApiWorkerRes) { listener?.({ data: res }); },
  };
}

describe('createOpenApiImportWorkerClient', () => {
  it('resolves with parsed result on success', async () => {
    const mock = makeMockWorker();
    const client = createOpenApiImportWorkerClient(() => mock.worker);
    const promise = client.parse('{"openapi":"3.0.0"}', {});
    const req = mock.posted[0];
    mock.respond({
      id: req.id, ok: true, kind: 'parse', elapsedMs: 1,
      result: { collection: { id: 'c', name: 'X', orderIndex: 0 }, folders: [], requests: [], baseUrl: '' },
    });
    const out = await promise;
    expect(out).not.toBeNull();
    expect(out!.ok).toBe(true);
    if (out!.ok) expect(out!.result.collection.name).toBe('X');
  });

  it('resolves with error on failure', async () => {
    const mock = makeMockWorker();
    const client = createOpenApiImportWorkerClient(() => mock.worker);
    const promise = client.parse('bad', {});
    const req = mock.posted[0];
    mock.respond({ id: req.id, ok: false, kind: 'parse', error: '只支持 OpenAPI 3.x JSON' });
    const out = await promise;
    expect(out!.ok).toBe(false);
    if (!out!.ok) expect(out!.error).toBe('只支持 OpenAPI 3.x JSON');
  });

  it('formats JSON off the main thread', async () => {
    const mock = makeMockWorker();
    const client = createOpenApiImportWorkerClient(() => mock.worker);
    const promise = client.format('{"openapi":"3.0.0"}', 2, 'format');
    const req = mock.posted[0];
    expect(req).toMatchObject({ kind: 'format', indent: 2 });
    mock.respond({
      id: req.id,
      ok: true,
      kind: 'format',
      text: '{\n  "openapi": "3.0.0"\n}',
      elapsedMs: 1,
    });
    await expect(promise).resolves.toEqual({
      ok: true,
      text: '{\n  "openapi": "3.0.0"\n}',
      elapsedMs: 1,
    });
  });

  it('surfaces JSON formatting errors', async () => {
    const mock = makeMockWorker();
    const client = createOpenApiImportWorkerClient(() => mock.worker);
    const promise = client.format('{', 2);
    const req = mock.posted[0];
    mock.respond({ id: req.id, ok: false, kind: 'format', error: 'Unexpected end of JSON input' });
    await expect(promise).resolves.toEqual({ ok: false, error: 'Unexpected end of JSON input' });
  });

  it('surfaces a mismatched worker response instead of silently dropping it', async () => {
    const mock = makeMockWorker();
    const client = createOpenApiImportWorkerClient(() => mock.worker);
    const promise = client.format('{"openapi":"3.0.0"}');
    const req = mock.posted[0];
    mock.respond({
      id: req.id,
      ok: true,
      kind: 'parse',
      result: { collection: { id: 'c', name: 'X', orderIndex: 0 }, folders: [], requests: [], baseUrl: '' },
      elapsedMs: 1,
    });
    await expect(promise).resolves.toEqual({
      ok: false,
      error: 'OpenAPI Worker 响应类型不匹配：期望 format，收到 parse',
    });
  });

  it('drops a superseded JSON formatting result', async () => {
    const mock = makeMockWorker();
    const client = createOpenApiImportWorkerClient(() => mock.worker);
    const first = client.format('{"value":1}', 2, 'format');
    const second = client.format('{"value":2}', 2, 'format');
    const [req1, req2] = mock.posted;
    mock.respond({ id: req1.id, ok: true, kind: 'format', text: 'old', elapsedMs: 1 });
    mock.respond({ id: req2.id, ok: true, kind: 'format', text: 'new', elapsedMs: 1 });
    await expect(first).resolves.toBeNull();
    await expect(second).resolves.toEqual({ ok: true, text: 'new', elapsedMs: 1 });
  });

  it('drops the superseded result when same tag is reissued', async () => {
    const mock = makeMockWorker();
    const client = createOpenApiImportWorkerClient(() => mock.worker);
    const first = client.parse('a', {}, 'preview');
    const second = client.parse('b', {}, 'preview');
    const [req1, req2] = mock.posted;
    mock.respond({ id: req1.id, ok: true, kind: 'parse', elapsedMs: 1,
      result: { collection: { id: '1', name: 'A', orderIndex: 0 }, folders: [], requests: [], baseUrl: '' } });
    mock.respond({ id: req2.id, ok: true, kind: 'parse', elapsedMs: 1,
      result: { collection: { id: '2', name: 'B', orderIndex: 0 }, folders: [], requests: [], baseUrl: '' } });
    expect(await first).toBeNull();
    const out2 = await second;
    expect(out2!.ok && out2!.result.collection.name).toBe('B');
  });

  it('resolves null when superseded request returns an error response', async () => {
    const mock = makeMockWorker();
    const client = createOpenApiImportWorkerClient(() => mock.worker);
    const first = client.parse('a', {}, 'preview');
    const second = client.parse('b', {}, 'preview');
    const [req1, req2] = mock.posted;
    mock.respond({ id: req1.id, ok: false, kind: 'parse', error: '被取消' });
    mock.respond({ id: req2.id, ok: true, kind: 'parse', elapsedMs: 1,
      result: { collection: { id: '2', name: 'B', orderIndex: 0 }, folders: [], requests: [], baseUrl: '' } });
    expect(await first).toBeNull();
    const out2 = await second;
    expect(out2!.ok && out2!.result.collection.name).toBe('B');
  });
});
