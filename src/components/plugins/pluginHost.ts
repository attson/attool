import type { InstalledPlugin } from './types';

type ApiVersion = '1.0' | '2.0';
const LEGACY_MAX_MESSAGE_BYTES = 1024 * 1024;
const HTTP_MAX_MESSAGE_BYTES = 8 * 1024 * 1024;

type RpcBase = {
  jsonrpc: '2.0';
  apiVersion: ApiVersion;
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
  notifyHttpEvent(event: unknown): void;
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
  pickFile?(): Promise<{ path: string; name: string } | null>;
  copyText?(text: string): Promise<void>;
  requestHttpService?(method: string, params: unknown): Promise<unknown>;
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
  const apiVersion: ApiVersion = options.app.nativeService ? '2.0' : '1.0';
  const maxMessageBytes = options.maxMessageBytes
    ?? (apiVersion === '2.0' ? HTTP_MAX_MESSAGE_BYTES : LEGACY_MAX_MESSAGE_BYTES);
  const sessionId = options.sessionId ?? crypto.randomUUID();
  const correlationId = options.correlationId ?? crypto.randomUUID();
  const cancelled = new Set<string>();
  const selectedFiles = new Map<string, string>();
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
      apiVersion,
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
      apiVersion,
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
        apiVersion,
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
      case 'host.http.pickFile': {
        requirePermission('host.http.request');
        if (!options.pickFile) {
          throw new RpcDispatchError('HOST_FILE_PICK_UNAVAILABLE', 'File selection is unavailable');
        }
        const selected = await options.pickFile();
        if (!selected) return { cancelled: true };
        const token = `host-file:${crypto.randomUUID()}`;
        selectedFiles.set(token, selected.path);
        return { cancelled: false, token, name: selected.name };
      }
      case 'host.http.copyText': {
        requirePermission('host.http.request');
        if (!options.copyText) {
          throw new RpcDispatchError('HOST_CLIPBOARD_WRITE_UNAVAILABLE', 'Clipboard write is unavailable');
        }
        const params = asRecord(request.params);
        const text = requiredString(params.text, 'text', maxMessageBytes);
        await options.copyText(text);
        return { copied: true };
      }
      case 'host.http.request':
        return dispatchHttpRequest(request, 'host.http.request', (method) =>
          method === 'http.send' || method === 'http.cancel'
        );
      case 'host.http.stream':
        return dispatchHttpRequest(request, 'host.http.stream', (method) =>
          method === 'stream.open'
          || method === 'stream.close'
          || method === 'stream.send'
          || method === 'stream.snapshot'
        );
      case 'host.http.data':
        return dispatchHttpRequest(request, 'host.http.data', (method) => method.startsWith('data.'));
      default:
        throw new RpcDispatchError('HOST_METHOD_NOT_FOUND', 'Host API method was not found', -32601);
    }
  }

  async function dispatchHttpRequest(
    request: RpcRequest,
    permission: string,
    allows: (method: string) => boolean
  ) {
    requirePermission(permission);
    if (!options.requestHttpService) {
      throw new RpcDispatchError('HTTP_SERVICE_NOT_CONNECTED', 'HTTP service is unavailable');
    }
    const params = asRecord(request.params);
    const method = requiredString(params.method, 'method', 128);
    if (!allows(method)) {
      throw new RpcDispatchError(
        'HOST_HTTP_METHOD_DENIED',
        `HTTP service method is not allowed for ${permission}`,
        -32003
      );
    }
    if (!Object.prototype.hasOwnProperty.call(params, 'params')) throw invalidParams('params');
    try {
      return await options.requestHttpService(
        method,
        method === 'http.send' ? resolveSelectedFiles(params.params, selectedFiles) : params.params
      );
    } catch (reason) {
      throw serviceError(reason);
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
      message.apiVersion !== apiVersion ||
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
        apiVersion,
        appId: options.app.appId,
        method: 'host.theme.changed',
        params: { theme }
      });
    },
    notifyHttpEvent(event) {
      if (
        !handshaken
        || terminated
        || apiVersion !== '2.0'
        || !options.app.permissions.includes('host.http.stream')
      ) return;
      post({
        jsonrpc: '2.0',
        apiVersion,
        appId: options.app.appId,
        method: 'host.http.streamEvent',
        params: event
      });
    },
    terminate() {
      if (terminated) return;
      terminated = true;
      cancelled.clear();
      selectedFiles.clear();
      options.port.onmessage = null;
      options.port.close();
    }
  };
}

function resolveSelectedFiles(value: unknown, selectedFiles: Map<string, string>): unknown {
  const params = asRecord(value);
  const request = asRecord(params.request);
  if (request.bodyType !== 'multipart') return value;
  if (!Array.isArray(request.multipartFields)) throw invalidParams('params.request.multipartFields');
  const multipartFields = request.multipartFields.map((value) => {
    const field = asRecord(value);
    if (field.kind !== 'file' || field.enabled === false) return field;
    const token = requiredString(field.value, 'params.request.multipartFields.value', 256);
    const path = selectedFiles.get(token);
    if (!path) {
      throw new RpcDispatchError(
        'HOST_HTTP_FILE_TOKEN_REQUIRED',
        'Multipart files must be selected through the host',
        -32003
      );
    }
    return { ...field, value: path };
  });
  return { ...params, request: { ...request, multipartFields } };
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

function serviceError(reason: unknown) {
  if (reason instanceof RpcDispatchError) return reason;
  if (typeof reason === 'object' && reason !== null) {
    const error = reason as { code?: unknown; message?: unknown };
    if (typeof error.code === 'string' && typeof error.message === 'string') {
      return new RpcDispatchError(error.code, error.message);
    }
  }
  return new RpcDispatchError('HTTP_SERVICE_REQUEST_FAILED', 'HTTP service request failed');
}
