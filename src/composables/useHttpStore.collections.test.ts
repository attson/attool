import { beforeEach, describe, expect, it, vi } from 'vitest';
import { _resetHttpStoreForTest } from './useHttpStore';
import type {
  HttpCollection,
  HttpCollectionFolder,
  HttpCollectionRequest,
  HttpRequestSpec
} from '../components/http/types';
import { makeEmptySpec } from '../components/http/types';
import type { ImportedOpenApiCollection } from '../components/http/openapiImport';

function makeRequest(url: string): HttpRequestSpec {
  return { ...makeEmptySpec(), method: 'GET', url };
}

function makeMockApi(seed?: {
  collections?: HttpCollection[];
  folders?: HttpCollectionFolder[];
  requests?: HttpCollectionRequest[];
}) {
  const calls: Array<{ fn: string; args: unknown }> = [];
  const collections = [...(seed?.collections ?? [])];
  const folders = [...(seed?.folders ?? [])];
  const requests = [...(seed?.requests ?? [])];
  const api = {
    async listTabs() { return []; },
    async upsertTab(tab: unknown) { calls.push({ fn: 'upsertTab', args: tab }); },
    async deleteTab() {},
    async setActiveTab(id: string) { calls.push({ fn: 'setActiveTab', args: id }); },
    async listHistory() { return []; },
    async insertHistory() {},
    async deleteHistory() {},
    async clearHistory() {},
    async listEnvs() { return []; },
    async upsertEnv() {},
    async deleteEnv() {},
    async setActiveEnv() {},
    async listEnvVars() { return []; },
    async upsertEnvVar() {},
    async deleteEnvVar() {},
    async sendHttp() { throw new Error('not used'); },
    async cancelHttp() { return true; },
    async listCollections() { return collections; },
    async listCollectionFolders() { return folders; },
    async listCollectionRequests() { return requests; },
    async upsertCollection(row: HttpCollection) {
      calls.push({ fn: 'upsertCollection', args: row });
      collections.push(row);
    },
    async upsertCollectionFolder(row: HttpCollectionFolder) {
      calls.push({ fn: 'upsertCollectionFolder', args: row });
      folders.push(row);
    },
    async upsertCollectionRequest(row: HttpCollectionRequest) {
      calls.push({ fn: 'upsertCollectionRequest', args: row });
      requests.push(row);
    },
    async deleteCollection(id: string) { calls.push({ fn: 'deleteCollection', args: id }); },
    async deleteCollectionRequest(id: string) { calls.push({ fn: 'deleteCollectionRequest', args: id }); },
    async openStream() {},
    async closeStream() {},
    async sendWsMessage() {},
    async listStreamMessages() { return []; },
    async listen() { return vi.fn(); }
  };
  return { api, calls };
}

describe('useHttpStore collections', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('loads persisted collections during init', async () => {
    const { api } = makeMockApi({
      collections: [{ id: 'c1', name: 'Admin API', orderIndex: 0, updatedAt: 1 }],
      folders: [{ id: 'f1', collectionId: 'c1', parentId: null, name: 'users', orderIndex: 0, updatedAt: 1 }],
      requests: [{ id: 'r1', collectionId: 'c1', folderId: 'f1', name: 'GET users', method: 'GET', spec: makeRequest('/users'), orderIndex: 0, updatedAt: 1 }]
    });
    const store = _resetHttpStoreForTest(api as any);

    await store.init();

    expect(store.state.collections).toHaveLength(1);
    expect(store.state.collectionFolders).toHaveLength(1);
    expect(store.state.collectionRequests).toHaveLength(1);
  });

  it('opens collection requests into a new tab', async () => {
    const request = { id: 'r1', collectionId: 'c1', folderId: null, name: 'GET users', method: 'GET' as const, spec: makeRequest('/users'), orderIndex: 0, updatedAt: 1 };
    const { api } = makeMockApi({ requests: [request] });
    const store = _resetHttpStoreForTest(api as any);
    await store.init();

    await store.openCollectionRequest(request, 'new');

    expect(store.state.tabs).toHaveLength(2);
    expect(store.activeTab.value?.title).toBe('GET users');
    expect((store.activeTab.value?.spec as HttpRequestSpec).url).toBe('/users');
  });

  it('persists imported OpenAPI collection nodes', async () => {
    const { api, calls } = makeMockApi();
    const store = _resetHttpStoreForTest(api as any);
    await store.init();
    const imported: ImportedOpenApiCollection = {
      baseUrl: '{{baseUrl}}',
      collection: { id: 'c1', name: 'Admin API', orderIndex: 0 },
      folders: [{ id: 'f1', collectionId: 'c1', parentId: null, name: 'users', orderIndex: 0 }],
      requests: [{ id: 'r1', collectionId: 'c1', folderId: 'f1', name: 'GET users', method: 'GET', spec: makeRequest('{{baseUrl}}/users'), orderIndex: 0 }]
    };

    await store.importCollection(imported);

    expect(calls.map((c) => c.fn)).toEqual([
      'upsertTab',
      'upsertCollection',
      'upsertCollectionFolder',
      'upsertCollectionRequest'
    ]);
    expect(store.state.collections[0].name).toBe('Admin API');
    expect(store.state.collectionRequests[0].spec.url).toBe('{{baseUrl}}/users');
  });
});
