import { beforeEach, describe, expect, it, vi } from 'vitest';
import { _resetHttpStoreForTest } from './useHttpStore';
import type { HttpCollection, HttpCollectionRequest, HttpRequestSpec } from '../components/http/types';
import { makeEmptySpec } from '../components/http/types';

function specJson(url: string): HttpRequestSpec {
  return { ...makeEmptySpec(), method: 'GET', url };
}

function buildOpenApi(paths: string[]): string {
  const pathsObj: Record<string, unknown> = {};
  for (const p of paths) pathsObj[p] = { get: { summary: `get ${p}` } };
  return JSON.stringify({
    openapi: '3.0.0',
    info: { title: 'A' },
    servers: [{ url: 'https://api.example.com' }],
    paths: pathsObj
  });
}

function makeMockApi(seed?: {
  collections?: HttpCollection[];
  requests?: HttpCollectionRequest[];
  fetchResult?: string | (() => Promise<string>);
}) {
  const calls: Array<{ fn: string; args: unknown }> = [];
  const collections = [...(seed?.collections ?? [])];
  const requests = [...(seed?.requests ?? [])];
  const fetchImpl = seed?.fetchResult;
  return {
    calls,
    api: {
      async listTabs() { return []; },
      async upsertTab() {}, async deleteTab() {}, async setActiveTab() {},
      async listHistory() { return []; }, async insertHistory() {},
      async deleteHistory() {}, async clearHistory() {},
      async listEnvs() { return []; }, async upsertEnv() {},
      async deleteEnv() {}, async setActiveEnv() {},
      async listEnvVars() { return []; }, async upsertEnvVar() {}, async deleteEnvVar() {},
      async sendHttp() { throw new Error('unused'); },
      async cancelHttp() { return true; },
      async listCollections() { return collections; },
      async listCollectionFolders() { return []; },
      async listCollectionRequests() { return requests; },
      async upsertCollection(row: HttpCollection) {
        calls.push({ fn: 'upsertCollection', args: row });
        const idx = collections.findIndex((c) => c.id === row.id);
        if (idx >= 0) collections[idx] = row; else collections.push(row);
      },
      async upsertCollectionFolder() {},
      async upsertCollectionRequest(row: HttpCollectionRequest) {
        calls.push({ fn: 'upsertCollectionRequest', args: row });
      },
      async deleteCollection() {},
      async deleteCollectionRequest(id: string) { calls.push({ fn: 'deleteCollectionRequest', args: id }); },
      async deleteCollectionFolder(id: string) { calls.push({ fn: 'deleteCollectionFolder', args: id }); },
      async fetchOpenApiUrl(url: string, headers: unknown) {
        calls.push({ fn: 'fetchOpenApiUrl', args: { url, headers } });
        if (typeof fetchImpl === 'function') return await fetchImpl();
        if (typeof fetchImpl === 'string') return fetchImpl;
        return '';
      },
      async openStream() {}, async closeStream() {},
      async sendWsMessage() {}, async listStreamMessages() { return []; },
      async listen() { return vi.fn(); }
    }
  };
}

describe('useHttpStore sync', () => {
  beforeEach(() => vi.clearAllMocks());

  it('手动 syncCollection：fetch → merge → 落库 → 返回 diff', async () => {
    const { api, calls } = makeMockApi({
      collections: [{
        id: 'c1', name: 'A', orderIndex: 0, updatedAt: 1,
        sourceUrl: 'https://example.com/openapi.json',
        sourceHeaders: [], syncIntervalSecs: 1800,
        lastSyncedAt: null, lastSyncError: null, baseUrl: null
      }],
      fetchResult: buildOpenApi(['/users', '/orders'])
    });
    const store = _resetHttpStoreForTest(api as any);
    await store.init();

    const diff = await store.syncCollection('c1', { manual: true });

    expect(diff).toEqual({ added: 2, updated: 0, deleted: 0 });
    const fetchCall = calls.find((c) => c.fn === 'fetchOpenApiUrl');
    expect(fetchCall).toBeTruthy();
    const upsertCol = calls.filter((c) => c.fn === 'upsertCollection').pop()!;
    expect((upsertCol.args as HttpCollection).lastSyncedAt).toBeTruthy();
    expect((upsertCol.args as HttpCollection).lastSyncError).toBeNull();
  });

  it('fetch 抛错：lastSyncError 落库，手动触发时 rethrow', async () => {
    const { api, calls } = makeMockApi({
      collections: [{
        id: 'c1', name: 'A', orderIndex: 0, updatedAt: 1,
        sourceUrl: 'https://x/openapi.json', sourceHeaders: [], syncIntervalSecs: 1800,
        lastSyncedAt: null, lastSyncError: null, baseUrl: null
      }],
      fetchResult: async () => { throw new Error('boom'); }
    });
    const store = _resetHttpStoreForTest(api as any);
    await store.init();

    await expect(store.syncCollection('c1', { manual: true })).rejects.toThrow('boom');
    const upsertCol = calls.filter((c) => c.fn === 'upsertCollection').pop()!;
    expect((upsertCol.args as HttpCollection).lastSyncError).toContain('boom');
  });

  it('并发保护：同 id 重复调只跑一次 fetch', async () => {
    let resolveFetch: (v: string) => void;
    const pending = new Promise<string>((res) => { resolveFetch = res; });
    const { api, calls } = makeMockApi({
      collections: [{
        id: 'c1', name: 'A', orderIndex: 0, updatedAt: 1,
        sourceUrl: 'https://x/o.json', sourceHeaders: [], syncIntervalSecs: 1800,
        lastSyncedAt: null, lastSyncError: null, baseUrl: null
      }],
      fetchResult: () => pending
    });
    const store = _resetHttpStoreForTest(api as any);
    await store.init();

    const p1 = store.syncCollection('c1', { manual: true });
    const p2 = store.syncCollection('c1', { manual: true });
    resolveFetch!(buildOpenApi(['/x']));
    await Promise.all([p1, p2]);
    const fetchCalls = calls.filter((c) => c.fn === 'fetchOpenApiUrl');
    expect(fetchCalls).toHaveLength(1);
  });
});
