import { describe, expect, it, vi } from 'vitest';
import { _resetHttpStoreForTest } from './useHttpStore';
import type { HttpRequestSpec } from '../components/http/types';

function makeMockApi(baseUrl?: string) {
  const sendHttp = vi.fn();
  return {
    sendHttp,
    api: {
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
      sendHttp,
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
      async openStream() {},
      async closeStream() {},
      async sendWsMessage() {},
      async listStreamMessages() { return []; },
      async listen() { return vi.fn(); }
    }
  };
}

describe('useHttpStore send', () => {
  it('sends an HTTP relative URL with the active environment baseUrl', async () => {
    const { api, sendHttp } = makeMockApi('https://api.example.com/v1');
    sendHttp.mockResolvedValue({
      status: 200,
      statusText: 'OK',
      headers: [],
      body: '{}',
      bodyBytes: 2,
      elapsedMs: 1,
      finalUrl: 'https://api.example.com/v1/users'
    });
    const store = _resetHttpStoreForTest(api as any);
    await store.init();
    const tab = store.activeTab.value!;
    (tab.spec as HttpRequestSpec).url = '/users';

    await store.send();

    expect(sendHttp).toHaveBeenCalledWith(
      expect.objectContaining({ url: 'https://api.example.com/v1/users' }),
      expect.any(String)
    );
    expect((tab.spec as HttpRequestSpec).url).toBe('/users');
  });

  it('shows a baseUrl error without calling the HTTP API for a relative URL', async () => {
    const { api, sendHttp } = makeMockApi();
    const store = _resetHttpStoreForTest(api as any);
    await store.init();
    const tab = store.activeTab.value!;
    (tab.spec as HttpRequestSpec).url = '/users';

    await expect(store.send()).resolves.toBeUndefined();

    expect(sendHttp).not.toHaveBeenCalled();
    expect(tab.lastError).toBe('相对 URL 需要配置有效的 baseUrl');
    expect(tab.sending).toBe(false);
  });
});
