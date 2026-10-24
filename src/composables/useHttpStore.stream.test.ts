import { describe, it, expect, vi, beforeEach } from 'vitest';
import { _resetHttpStoreForTest } from './useHttpStore';
import type { StreamMessage } from '../components/http/types';

interface CapturedListener {
  event: string;
  cb: (m: StreamMessage) => void;
}

function makeMockApi(baseUrl?: string) {
  const calls: Array<{ fn: string; args: unknown }> = [];
  const listeners: CapturedListener[] = [];
  const unlisten = vi.fn();
  const api = {
    // http 部分给最小实现，让 init 能跑
    async listTabs() { return []; },
    async upsertTab() {},
    async deleteTab() {},
    async setActiveTab() {},
    async listHistory() { return []; },
    async insertHistory() {},
    async deleteHistory() {},
    async clearHistory() {},
    async listEnvs() {
      return baseUrl ? [{ id: 'dev', name: 'dev', isActive: true, orderIndex: 0 }] : [];
    },
    async upsertEnv() {},
    async deleteEnv() {},
    async setActiveEnv() {},
    async listEnvVars(envId: string) {
      return baseUrl && envId === 'dev'
        ? [{ id: 'base-url', envId, key: 'baseUrl', value: baseUrl, enabled: true, orderIndex: 0 }]
        : [];
    },
    async upsertEnvVar() {},
    async deleteEnvVar() {},
    async sendHttp() { throw new Error('not used'); },
    async cancelHttp() { return true; },
    async listCollections() { return []; },
    async listCollectionFolders() { return []; },
    async listCollectionRequests() { return []; },
    async upsertCollection() {},
    async upsertCollectionFolder() {},
    async upsertCollectionRequest() {},
    async deleteCollection() {},
    async deleteCollectionFolder() {},
    async deleteCollectionRequest() {},
    async fetchOpenApiUrl() { return ''; },
    // stream 部分
    async openStream(sessionId: string, kind: 'sse' | 'ws', spec: unknown) {
      calls.push({ fn: 'openStream', args: { sessionId, kind, spec } });
    },
    async closeStream(sessionId: string) {
      calls.push({ fn: 'closeStream', args: { sessionId } });
    },
    async sendWsMessage(sessionId: string, text: string) {
      calls.push({ fn: 'sendWsMessage', args: { sessionId, text } });
    },
    async listStreamMessages(sessionId: string) {
      calls.push({ fn: 'listStreamMessages', args: { sessionId } });
      return [];
    },
    async listen(sessionId: string, cb: (m: StreamMessage) => void) {
      listeners.push({ event: `http-stream-message-${sessionId}`, cb });
      return unlisten;
    },
  };
  return { api, calls, listeners, unlisten };
}

describe('useHttpStore stream', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('openStream sets status connecting then open on Open message', async () => {
    const { api, listeners } = makeMockApi();
    const store = _resetHttpStoreForTest(api as any);
    await store.init();
    const tab = await store.newTab(undefined, undefined, 'sse');
    await store.openStream(tab.id, 'sse', { url: 'https://example.com', headers: [], queryParams: [], auth: { type: 'none' }, verifySsl: true } as any);
    expect(tab.session?.status).toBe('connecting');
    // 模拟 Open 事件
    listeners[0].cb({ kind: 'open', atMs: 1, status: 200, headers: [] });
    expect(tab.session?.status).toBe('open');
  });

  it('Closed message flips status to closed and pushes into messages', async () => {
    const { api, listeners } = makeMockApi();
    const store = _resetHttpStoreForTest(api as any);
    await store.init();
    const tab = await store.newTab(undefined, undefined, 'sse');
    await store.openStream(tab.id, 'sse', { url: 'https://x' } as any);
    listeners[0].cb({ kind: 'open', atMs: 1, status: 200, headers: [] });
    listeners[0].cb({ kind: 'closed', atMs: 2, code: null, reason: 'bye' });
    expect(tab.session?.status).toBe('closed');
    expect(tab.messages).toHaveLength(2);
  });

  it('sendWsMessage only fires when status=open', async () => {
    const { api, calls, listeners } = makeMockApi();
    const store = _resetHttpStoreForTest(api as any);
    await store.init();
    const tab = await store.newTab(undefined, undefined, 'ws');
    await store.openStream(tab.id, 'ws', { url: 'ws://x' } as any);
    // idle 阶段：不发
    await expect(store.sendWsMessage(tab.id, 'a')).rejects.toBeTruthy();
    listeners[0].cb({ kind: 'open', atMs: 1, status: null, headers: [] });
    await store.sendWsMessage(tab.id, 'b');
    expect(calls.some(c => c.fn === 'sendWsMessage' && (c.args as any).text === 'b')).toBe(true);
  });

  it('opens a WebSocket relative URL with the active environment baseUrl', async () => {
    const { api, calls } = makeMockApi('https://api.example.com/v1');
    const store = _resetHttpStoreForTest(api as any);
    await store.init();
    const tab = await store.newTab(undefined, undefined, 'ws');

    await store.openStream(tab.id, 'ws', { url: '/socket' } as any);

    const call = calls.find((entry) => entry.fn === 'openStream');
    expect((call?.args as any).spec.url).toBe('wss://api.example.com/v1/socket');
  });

  it('closeTab triggers closeStream + unlisten for stream tab', async () => {
    const { api, calls, unlisten } = makeMockApi();
    const store = _resetHttpStoreForTest(api as any);
    await store.init();
    const tab = await store.newTab(undefined, undefined, 'ws');
    await store.openStream(tab.id, 'ws', { url: 'ws://x' } as any);
    await store.closeTab(tab.id);
    expect(calls.some(c => c.fn === 'closeStream' && (c.args as any).sessionId === tab.id)).toBe(true);
    expect(unlisten).toHaveBeenCalled();
  });

  it('rejects a relative stream URL without baseUrl and cleans up the listener', async () => {
    const { api, calls, listeners, unlisten } = makeMockApi();
    const store = _resetHttpStoreForTest(api as any);
    await store.init();
    const tab = await store.newTab(undefined, undefined, 'sse');

    await expect(store.openStream(tab.id, 'sse', { url: '/events' } as any)).rejects.toThrow(
      '相对 URL 需要配置有效的 baseUrl'
    );

    expect(calls.some((call) => call.fn === 'openStream')).toBe(false);
    expect(tab.session).toEqual({ status: 'error', error: '相对 URL 需要配置有效的 baseUrl' });
    expect(listeners).toHaveLength(0);
    expect(unlisten).not.toHaveBeenCalled();
  });
});
