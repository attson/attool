import { describe, expect, it, vi } from 'vitest';

import { createPluginHostSession, type HostMessagePort, type HostRpcMessage } from './pluginHost';
import type { InstalledPlugin } from './types';

class FakePort implements HostMessagePort {
  onmessage: ((event: MessageEvent<HostRpcMessage>) => void) | null = null;
  onPost: (() => void) | null = null;
  sent: HostRpcMessage[] = [];
  start() {}
  close() {}
  postMessage(message: HostRpcMessage) {
    this.sent.push(message);
    this.onPost?.();
  }
  receive(message: HostRpcMessage) {
    this.onmessage?.({ data: message } as MessageEvent<HostRpcMessage>);
  }
}

const codec: InstalledPlugin = {
  appId: 'com.attson.codec',
  name: '编码转换',
  version: '0.1.0-alpha.0',
  publisherId: 'com.attson',
  publisherName: 'Attson',
  publisherKeyId: 'attool-p0-test',
  entry: 'ui/index.html',
  permissions: ['host.theme.read'],
  enabled: true,
  installedAt: '2026-11-10T00:00:00Z'
};

const http: InstalledPlugin = {
  ...codec,
  appId: 'com.attson.http',
  name: 'HTTP request',
  permissions: ['host.file.save', 'host.http.request', 'host.http.stream', 'host.http.data'],
  nativeService: {
    entry: 'native/athttp-service',
    version: '0.1.0-alpha.0',
    sha256: 'a'.repeat(64),
    protocolVersion: 1
  }
};

function request(id: string, method: string, params: unknown = {}): HostRpcMessage {
  return {
    jsonrpc: '2.0',
    id,
    apiVersion: '1.0',
    appId: codec.appId,
    method,
    params
  };
}

async function flush() {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

describe('plugin host session', () => {
  it('routes HTTP Host API 2.0 methods by declared capability and pushes service events', async () => {
    const port = new FakePort();
    const requestService = vi.fn(async (_method: string, _params: unknown) => ({ ok: true }));
    const session = createPluginHostSession({
      app: http,
      port,
      theme: () => 'dark',
      notify: vi.fn(),
      saveFile: vi.fn(),
      requestHttpService: requestService
    });
    const httpRequest = (id: string, method: string, params: unknown = {}): HostRpcMessage => ({
      jsonrpc: '2.0',
      id,
      apiVersion: '2.0',
      appId: http.appId,
      method,
      params
    });

    port.receive(httpRequest('1', 'host.handshake'));
    await flush();
    port.receive(httpRequest('2', 'host.http.request', {
      method: 'http.send',
      params: { request: { url: 'https://example.com' } }
    }));
    await flush();

    expect(requestService).toHaveBeenCalledWith('http.send', {
      request: { url: 'https://example.com' }
    });
    expect(port.sent[1]).toMatchObject({ id: '2', result: { ok: true } });

    port.receive(httpRequest('3', 'host.http.request', {
      method: 'data.tabs.list',
      params: {}
    }));
    await flush();
    expect(port.sent[2]).toMatchObject({
      id: '3',
      error: { data: { code: 'HOST_HTTP_METHOD_DENIED' } }
    });

    session.notifyHttpEvent({ sessionId: 'sse-1', generation: 1, message: { kind: 'open' } });
    expect(port.sent.at(-1)).toMatchObject({
      apiVersion: '2.0',
      method: 'host.http.streamEvent',
      params: { sessionId: 'sse-1', generation: 1 }
    });
    session.terminate();
  });

  it('exposes multipart files as opaque tokens and rejects unscoped paths', async () => {
    const port = new FakePort();
    const requestService = vi.fn(async (_method: string, _params: unknown) => ({ ok: true }));
    const copyText = vi.fn(async () => {});
    createPluginHostSession({
      app: http,
      port,
      theme: () => 'dark',
      notify: vi.fn(),
      saveFile: vi.fn(),
      pickFile: vi.fn(async () => ({ path: '/home/user/private.txt', name: 'private.txt' })),
      copyText,
      requestHttpService: requestService
    });
    const httpRequest = (id: string, method: string, params: unknown = {}): HostRpcMessage => ({
      jsonrpc: '2.0',
      id,
      apiVersion: '2.0',
      appId: http.appId,
      method,
      params
    });

    port.receive(httpRequest('1', 'host.handshake'));
    await flush();
    port.receive(httpRequest('2', 'host.http.pickFile'));
    await flush();
    const selection = (port.sent[1] as { result: { token: string; name: string } }).result;
    expect(selection.name).toBe('private.txt');
    expect(selection.token).not.toContain('/home/user');

    port.receive(httpRequest('copy', 'host.http.copyText', { text: 'curl example.com' }));
    await flush();
    expect(copyText).toHaveBeenCalledWith('curl example.com');

    port.receive(httpRequest('3', 'host.http.request', {
      method: 'http.send',
      params: {
        request: {
          bodyType: 'multipart',
          multipartFields: [{ key: 'file', kind: 'file', value: selection.token, enabled: true }]
        }
      }
    }));
    await flush();
    expect(requestService).toHaveBeenLastCalledWith('http.send', {
      request: {
        bodyType: 'multipart',
        multipartFields: [{ key: 'file', kind: 'file', value: '/home/user/private.txt', enabled: true }]
      }
    });

    port.receive(httpRequest('4', 'host.http.request', {
      method: 'http.send',
      params: {
        request: {
          bodyType: 'multipart',
          multipartFields: [{ key: 'file', kind: 'file', value: '/etc/passwd', enabled: true }]
        }
      }
    }));
    await flush();
    expect(port.sent.at(-1)).toMatchObject({
      id: '4',
      error: { data: { code: 'HOST_HTTP_FILE_TOKEN_REQUIRED' } }
    });
    expect(requestService).toHaveBeenCalledTimes(1);
  });

  it('delivers a legal five MiB stream snapshot over Host API 2', async () => {
    const port = new FakePort();
    const session = createPluginHostSession({
      app: http,
      port,
      theme: () => 'dark',
      notify: vi.fn(),
      saveFile: vi.fn(),
      requestHttpService: vi.fn(async () => ({ body: 'x'.repeat(5 * 1024 * 1024) }))
    });
    const httpRequest = (id: string, method: string, params: unknown = {}): HostRpcMessage => ({
      jsonrpc: '2.0', id, apiVersion: '2.0', appId: http.appId, method, params
    });

    port.receive(httpRequest('1', 'host.handshake'));
    await flush();
    port.receive(httpRequest('2', 'host.http.stream', {
      method: 'stream.snapshot', params: { sessionId: 'stream-1' }
    }));
    await flush();

    expect(port.sent.at(-1)).toMatchObject({ id: '2', result: { body: expect.any(String) } });
    session.terminate();
  });

  it('requires a handshake and exposes only declared capabilities', async () => {
    const port = new FakePort();
    const session = createPluginHostSession({
      app: codec,
      port,
      sessionId: 'session-1',
      correlationId: 'correlation-1',
      theme: () => 'dark',
      notify: async () => {},
      saveFile: async () => ({ cancelled: false })
    });

    port.receive(request('1', 'host.theme.get'));
    await flush();
    expect(port.sent[0]).toMatchObject({
      id: '1',
      error: { data: { code: 'HOST_HANDSHAKE_REQUIRED' } }
    });

    port.receive(request('2', 'host.handshake'));
    await flush();
    expect(port.sent[1]).toMatchObject({
      id: '2',
      result: {
        apiVersion: '1.0',
        sessionId: 'session-1',
        correlationId: 'correlation-1',
        capabilities: ['host.theme.read'],
        metadata: { appId: codec.appId, name: codec.name, version: codec.version },
        theme: 'dark'
      }
    });

    port.receive(request('3', 'host.file.save', { suggestedName: 'codec.txt', content: 'x' }));
    await flush();
    expect(port.sent[2]).toMatchObject({
      id: '3',
      error: { data: { code: 'HOST_PERMISSION_DENIED' } }
    });
    session.terminate();
  });

  it('reports a completed handshake only after posting the successful response', async () => {
    const port = new FakePort();
    const sequence: string[] = [];
    port.onPost = () => sequence.push('response');
    createPluginHostSession({
      app: codec,
      port,
      theme: () => 'dark',
      notify: vi.fn(),
      saveFile: vi.fn(),
      onHandshake: () => sequence.push('callback')
    });

    port.receive(request('1', 'host.handshake'));
    await vi.waitFor(() => expect(sequence).toHaveLength(2));

    expect(sequence).toEqual(['response', 'callback']);
  });

  it('pushes theme changes after the handshake', async () => {
    const port = new FakePort();
    const session = createPluginHostSession({
      app: codec,
      port,
      theme: () => 'light',
      notify: async () => {},
      saveFile: async () => ({ cancelled: false })
    });
    port.receive(request('1', 'host.handshake'));
    await flush();

    session.notifyTheme('dark');

    expect(port.sent.at(-1)).toEqual({
      jsonrpc: '2.0',
      apiVersion: '1.0',
      appId: codec.appId,
      method: 'host.theme.changed',
      params: { theme: 'dark' }
    });
    session.terminate();
  });
});
