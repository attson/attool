import type { InstalledPlugin } from './types';

const API_VERSION = '1.0' as const;
const DEFAULT_MAX_MESSAGE_BYTES = 1024 * 1024;

type RpcBase = {
  jsonrpc: '2.0';
  apiVersion: typeof API_VERSION;
  appId: string;
};

type RpcRequest = RpcBase & {
  id: string;
  method: string;
  params: unknown;
};

type RpcNotification = RpcBase & {
  method: string;
  params: unknown;
};

type RpcSuccess = RpcBase & {
  id: string;
  result: unknown;
};

type RpcFailure = RpcBase & {
  id: string;
  error: {
    code: number;
    message: string;
    data: { code: string };
  };
};

export type HostRpcMessage = RpcRequest | RpcNotification | RpcSuccess | RpcFailure;

export interface HostMessagePort {
  onmessage: ((event: MessageEvent<HostRpcMessage>) => void) | null;
  postMessage(message: HostRpcMessage): void;
  start(): void;
  close(): void;
}

export interface HostFileSaveRequest {
  suggestedName: string;
  content: string;
  encoding: 'utf8' | 'base64';
}

export interface PluginHostSession {
  readonly sessionId: string;
  readonly correlationId: string;
  notifyTheme(theme: 'dark' | 'light'): void;
  terminate(): void;
}

type SessionOptions = {
  app: InstalledPlugin;
  port: HostMessagePort;
  sessionId?: string;
  correlationId?: string;
  theme(): 'dark' | 'light';
  notify(input: { title?: string; message: string; kind: 'info' | 'success' | 'warning' | 'error' }): Promise<void>;
  saveFile(input: HostFileSaveRequest): Promise<{ cancelled: boolean; path?: string }>;
  onHandshake?(): void;
  maxMessageBytes?: number;
};

class RpcDispatchError extends Error {
  constructor(
    readonly stableCode: string,
    message: string,
    readonly rpcCode = -32000
  ) {
    super(message);
  }
}

export function createPluginHostSession(options: SessionOptions): PluginHostSession {
  const maxMessageBytes = options.maxMessageBytes ?? DEFAULT_MAX_MESSAGE_BYTES;
  const sessionId = options.sessionId ?? crypto.randomUUID();
  const correlationId = options.correlationId ?? crypto.randomUUID();
  const cancelled = new Set<string>();
  let handshaken = false;
  let terminated = false;

  function post(message: HostRpcMessage) {
    if (terminated) return;
    const bytes = messageBytes(message);
    if (bytes > maxMessageBytes) return;
    options.port.postMessage(message);
  }

  function success(id: string, result: unknown) {
    const message: RpcSuccess = {
      jsonrpc: '2.0',
      id,
      apiVersion: API_VERSION,
      appId: options.app.appId,
      result
    };
    if (messageBytes(message) > maxMessageBytes) {
      failure(id, new RpcDispatchError('HOST_MESSAGE_TOO_LARGE', 'Host response is too large'));
      return;
    }
    post(message);
  }

  function failure(id: string, error: RpcDispatchError) {
    post({
      jsonrpc: '2.0',
      id,
      apiVersion: API_VERSION,
      appId: options.app.appId,
      error: {
        code: error.rpcCode,
        message: error.message,
        data: { code: error.stableCode }
      }
    });
  }

  function requirePermission(permission: string) {
    if (!options.app.permissions.includes(permission)) {
      throw new RpcDispatchError(
        'HOST_PERMISSION_DENIED',
        `Permission is not granted: ${permission}`,
        -32003
      );
    }
  }

  async function dispatch(request: RpcRequest): Promise<unknown> {
    if (request.method === 'host.handshake') {
      handshaken = true;
      return {
        apiVersion: API_VERSION,
        sessionId,
        correlationId,
        capabilities: [...options.app.permissions],
        metadata: {
          appId: options.app.appId,
          name: options.app.name,
          version: options.app.version
        },
        theme: options.theme()
      };
    }
    if (!handshaken) {
      throw new RpcDispatchError(
        'HOST_HANDSHAKE_REQUIRED',
        'Host handshake is required',
        -32001
      );
    }
    switch (request.method) {
      case 'host.theme.get':
        requirePermission('host.theme.read');
        return { theme: options.theme() };
      case 'host.app.getMetadata':
        return {
          appId: options.app.appId,
          name: options.app.name,
          version: options.app.version,
          publisherId: options.app.publisherId
        };
      case 'host.notification.show': {
        requirePermission('host.notification.show');
        const params = asRecord(request.params);
        const message = requiredString(params.message, 'message', 2000);
        const title = optionalString(params.title, 'title', 200);
        const kind = params.kind ?? 'info';
        if (!['info', 'success', 'warning', 'error'].includes(String(kind))) {
          throw invalidParams('kind');
        }
        await options.notify({
          title,
          message,
          kind: kind as 'info' | 'success' | 'warning' | 'error'
        });
        return { shown: true };
      }
      case 'host.file.save': {
        requirePermission('host.file.save');
        const params = asRecord(request.params);
        const suggestedName = requiredString(params.suggestedName, 'suggestedName', 255);
        if (suggestedName.includes('/') || suggestedName.includes('\\')) {
          throw invalidParams('suggestedName');
        }
        const content = requiredString(params.content, 'content', maxMessageBytes);
        const encoding = params.encoding ?? 'utf8';
        if (encoding !== 'utf8' && encoding !== 'base64') throw invalidParams('encoding');
        return options.saveFile({ suggestedName, content, encoding });
      }
      default:
        throw new RpcDispatchError('HOST_METHOD_NOT_FOUND', 'Host API method was not found', -32601);
    }
  }

  options.port.onmessage = (event) => {
    if (terminated) return;
    const message = event.data;
    if (messageBytes(message) > maxMessageBytes) {
      if (isRequest(message)) {
        failure(
          message.id,
          new RpcDispatchError('HOST_MESSAGE_TOO_LARGE', 'Host API message is too large', -32002)
        );
      }
      return;
    }
    if (
      !isRequestLike(message) ||
      message.jsonrpc !== '2.0' ||
      message.apiVersion !== API_VERSION ||
      message.appId !== options.app.appId
    ) {
      if (isRequest(message)) {
        failure(
          message.id,
          new RpcDispatchError('HOST_REQUEST_INVALID', 'Host API request is invalid', -32600)
        );
      }
      return;
    }
    if (!('id' in message)) {
      if (message.method === '$/cancelRequest') {
        const id = asRecord(message.params).id;
        if (typeof id === 'string') cancelled.add(id);
      }
      return;
    }
    void dispatch(message)
      .then((result) => {
        if (cancelled.delete(message.id) || terminated) return;
        success(message.id, result);
        if (message.method === 'host.handshake') options.onHandshake?.();
      })
      .catch((reason) => {
        if (cancelled.delete(message.id) || terminated) return;
        failure(
          message.id,
          reason instanceof RpcDispatchError
            ? reason
            : new RpcDispatchError('HOST_INTERNAL_ERROR', 'Host API request failed')
        );
      });
  };
  options.port.start();

  return {
    sessionId,
    correlationId,
    notifyTheme(theme) {
      if (!handshaken || terminated || !options.app.permissions.includes('host.theme.read')) return;
      post({
        jsonrpc: '2.0',
        apiVersion: API_VERSION,
        appId: options.app.appId,
        method: 'host.theme.changed',
        params: { theme }
      });
    },
    terminate() {
      if (terminated) return;
      terminated = true;
      cancelled.clear();
      options.port.onmessage = null;
      options.port.close();
    }
  };
}

function messageBytes(value: unknown): number {
  try {
    return new TextEncoder().encode(JSON.stringify(value)).byteLength;
  } catch {
    return Number.POSITIVE_INFINITY;
  }
}

function isRequestLike(value: unknown): value is RpcRequest | RpcNotification {
  if (typeof value !== 'object' || value === null) return false;
  const request = value as Partial<RpcRequest>;
  return typeof request.method === 'string' && 'params' in request;
}

function isRequest(value: unknown): value is RpcRequest {
  return isRequestLike(value) && typeof (value as Partial<RpcRequest>).id === 'string';
}

function asRecord(value: unknown): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw invalidParams('params');
  }
  return value as Record<string, unknown>;
}

function requiredString(value: unknown, field: string, maxLength: number): string {
  if (typeof value !== 'string' || value.length === 0 || value.length > maxLength) {
    throw invalidParams(field);
  }
  return value;
}

function optionalString(value: unknown, field: string, maxLength: number): string | undefined {
  if (value === undefined) return undefined;
  if (typeof value !== 'string' || value.length > maxLength) throw invalidParams(field);
  return value;
}

function invalidParams(field: string) {
  return new RpcDispatchError('HOST_INVALID_PARAMS', `Invalid Host API parameter: ${field}`, -32602);
}
