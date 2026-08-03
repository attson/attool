import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type {
  AiProvider, AiModel, AiSession, AiSessionSummary, AiMessage,
  AiContentPart, ProviderModelInfo, AiImportSummary
} from '../../types/ai';

export function createAiApi() {
  return {
    listProviders: () => invoke<AiProvider[]>('ai_list_providers'),
    upsertProvider: (provider: AiProvider) => invoke<AiProvider>('ai_upsert_provider', { provider }),
    deleteProvider: (id: string) => invoke<void>('ai_delete_provider', { id }),
    listModels: (providerId?: string) => invoke<AiModel[]>('ai_list_models', { providerId: providerId ?? null }),
    upsertModel: (model: AiModel) => invoke<AiModel>('ai_upsert_model', { model }),
    deleteModel: (id: string) => invoke<void>('ai_delete_model', { id }),
    fetchProviderModels: (providerId: string) => invoke<ProviderModelInfo[]>('ai_fetch_provider_models', { providerId }),
    listSessions: (search?: string) => invoke<AiSession[]>('ai_list_sessions', { search: search ?? null }),
    createSession: (title?: string, modelId?: string) => invoke<AiSession>('ai_create_session', { title: title ?? null, modelId: modelId ?? null }),
    getSession: (id: string) => invoke<AiSessionSummary>('ai_get_session', { id }),
    updateSession: (id: string, patch: { title?: string; systemPrompt?: string; modelId?: string }) =>
      invoke<AiSession>('ai_update_session', {
        id,
        title: patch.title ?? null,
        systemPrompt: patch.systemPrompt ?? null,
        modelId: patch.modelId ?? null,
      }),
    deleteSession: (id: string) => invoke<void>('ai_delete_session', { id }),
    send: (sessionId: string, userContent: AiContentPart[]) =>
      invoke<string>('ai_send', { sessionId, userContent }),
    cancel: (assistantMessageId: string) =>
      invoke<boolean>('ai_cancel', { assistantMessageId }),
    retry: (assistantMessageId: string) =>
      invoke<string>('ai_retry', { assistantMessageId }),
    exportSession: (sessionId: string, format: 'markdown' | 'json') =>
      invoke<string>('ai_export_session', { sessionId, format }),
    exportConfig: (includeKeys: boolean) =>
      invoke<string>('ai_export_config', { includeKeys }),
    importConfig: (json: string) => invoke<AiImportSummary>('ai_import_config', { json }),
    saveAttachment: (bytes: Uint8Array, mime: string) =>
      invoke<string>('ai_save_attachment', { bytes: Array.from(bytes), mime }),
  };
}

export type AiApi = ReturnType<typeof createAiApi>;

export async function subscribeAiDelta(
  messageId: string,
  onDelta: (text: string) => void,
  onDone: (payload: { status: string; promptTokens?: number; completionTokens?: number; error?: string }) => void,
): Promise<() => void> {
  const un1 = await listen<{ text: string }>(`ai-chat-delta-${messageId}`, e => onDelta(e.payload.text));
  const un2 = await listen<any>(`ai-chat-done-${messageId}`, e => onDone(e.payload));
  return () => { un1(); un2(); };
}
