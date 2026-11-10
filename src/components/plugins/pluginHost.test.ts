import { describe, expect, it } from 'vitest';

import { createPluginHostSession, type HostMessagePort, type HostRpcMessage } from './pluginHost';
import type { InstalledPlugin } from './types';

class FakePort implements HostMessagePort {
  onmessage: ((event: MessageEvent<HostRpcMessage>) => void) | null = null;
  sent: HostRpcMessage[] = [];
  start() {}
  close() {}
  postMessage(message: HostRpcMessage) {
    this.sent.push(message);
  }
  receive(message: HostRpcMessage) {
    this.onmessage?.({ data: message } as MessageEvent<HostRpcMessage>);
  }
}

const codec: InstalledPlugin = {
  appId: 'com.attson.codec',
  name: 'AT Codec',
  version: '0.1.0-alpha.0',
  publisherId: 'com.attson',
  publisherName: 'Attson',
  publisherKeyId: 'attool-p0-test',
  entry: 'ui/index.html',
  permissions: ['host.theme.read'],
  enabled: true,
  installedAt: '2026-11-10T00:00:00Z'
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
  it('requires a handshake and exposes only declared capabilities', async () => {
    const port = new FakePort();
    const session = createPluginHostSession({
      app: codec,
      port,
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
