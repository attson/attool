import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { SseSpec, StreamMessage, WsSpec } from './types';

export interface StreamApi {
  openStream(sessionId: string, kind: 'sse' | 'ws', spec: SseSpec | WsSpec): Promise<void>;
  closeStream(sessionId: string): Promise<void>;
  sendWsMessage(sessionId: string, text: string): Promise<void>;
  listStreamMessages(sessionId: string): Promise<StreamMessage[]>;
  listen(sessionId: string, onMessage: (m: StreamMessage) => void): Promise<UnlistenFn>;
}

export function createStreamApi(invoker: typeof invoke = invoke): StreamApi {
  return {
    async openStream(sessionId, kind, spec) {
      await invoker('open_stream', { sessionId, kind, spec });
    },
    async closeStream(sessionId) {
      await invoker('close_stream', { sessionId });
    },
    async sendWsMessage(sessionId, text) {
      await invoker('send_ws_message', { sessionId, text });
    },
    async listStreamMessages(sessionId) {
      return await invoker<StreamMessage[]>('list_stream_messages', { sessionId });
    },
    async listen(sessionId, onMessage) {
      return await listen<StreamMessage>(
        `http-stream-message-${sessionId}`,
        (event) => onMessage(event.payload)
      );
    },
  };
}
