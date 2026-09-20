import { readonly, ref, shallowRef } from 'vue';
import type { Ref } from 'vue';
import { createAiApi, subscribeAiDelta, type AiApi } from './aiApi';
import type {
  AiContentPart, AiModel, AiProvider, AiSession, AiSessionSummary,
} from '../../types/ai';

type Subscribe = typeof subscribeAiDelta;

export function _createAiChatState(api: AiApi, subscribe: Subscribe) {
  const providers = ref<AiProvider[]>([]);
  const models = ref<AiModel[]>([]);
  const sessions = ref<AiSession[]>([]);
  const currentSessionId = ref<string | null>(null);
  const currentSession = shallowRef<AiSessionSummary | null>(null);
  const streamingMessageId = ref<string | null>(null);
  let unsubscribe: (() => void) | null = null;

  async function loadProviders() { providers.value = await api.listProviders(); }
  async function loadModels() {
    models.value = await api.listModels();
    const sid = currentSessionId.value;
    const selectedModelId = currentSession.value?.session.currentModelId;
    if (
      sid
      && currentSession.value
      && (!selectedModelId || !models.value.some((model) => model.id === selectedModelId))
    ) {
      await openSession(sid);
    }
  }
  async function loadSessions(search?: string) { sessions.value = await api.listSessions(search); }

  async function openSession(id: string) {
    currentSessionId.value = id;
    let summary = await api.getSession(id);
    const selectedModelId = summary.session.currentModelId;
    const defaultModelId = models.value[0]?.id;
    if (
      defaultModelId
      && (!selectedModelId || !models.value.some((model) => model.id === selectedModelId))
    ) {
      await api.updateSession(id, { modelId: defaultModelId });
      summary = await api.getSession(id);
    }
    currentSession.value = summary;
  }

  async function newSession(): Promise<string> {
    const s = await api.createSession(undefined, models.value[0]?.id);
    await loadSessions();
    await openSession(s.id);
    return s.id;
  }

  async function renameSession(id: string, title: string) {
    await api.updateSession(id, { title });
    await loadSessions();
    if (currentSessionId.value === id) await openSession(id);
  }

  async function deleteSession(id: string) {
    await api.deleteSession(id);
    if (currentSessionId.value === id) {
      currentSessionId.value = null;
      currentSession.value = null;
    }
    await loadSessions();
  }

  async function updateSystemPrompt(sessionId: string, systemPrompt: string) {
    await api.updateSession(sessionId, { systemPrompt });
    if (currentSessionId.value === sessionId) await openSession(sessionId);
  }

  async function switchSessionModel(sessionId: string, modelId: string) {
    await api.updateSession(sessionId, { modelId });
    if (currentSessionId.value === sessionId) await openSession(sessionId);
  }

  async function sendMessage(text: string, attachments: AiContentPart[]) {
    const sid = currentSessionId.value;
    if (!sid) return;
    const parts: AiContentPart[] = [];
    if (text) parts.push({ type: 'text', text });
    parts.push(...attachments);
    const assistantId = await api.send(sid, parts);
    streamingMessageId.value = assistantId;
    await openSession(sid);
    unsubscribe?.();
    unsubscribe = await subscribe(assistantId,
      async () => { await openSession(sid); },
      async () => {
        streamingMessageId.value = null;
        await openSession(sid);
        await loadSessions();
        unsubscribe?.(); unsubscribe = null;
      });
  }

  async function cancelStreaming() {
    if (!streamingMessageId.value) return;
    await api.cancel(streamingMessageId.value);
  }

  async function retryMessage(assistantId: string) {
    const sid = currentSessionId.value;
    if (!sid) return;
    const newId = await api.retry(assistantId);
    streamingMessageId.value = newId;
    await openSession(sid);
    unsubscribe?.();
    unsubscribe = await subscribe(newId,
      async () => { await openSession(sid); },
      async () => {
        streamingMessageId.value = null;
        await openSession(sid);
        await loadSessions();
        unsubscribe?.(); unsubscribe = null;
      });
  }

  return {
    providers: readonly(providers),
    models: readonly(models),
    sessions: readonly(sessions),
    currentSessionId: readonly(currentSessionId),
    currentSession: readonly(currentSession),
    streamingMessageId: readonly(streamingMessageId),
    loadProviders, loadModels, loadSessions,
    openSession, newSession, renameSession, deleteSession,
    updateSystemPrompt, switchSessionModel,
    sendMessage, cancelStreaming, retryMessage,
  };
}

let _singleton: ReturnType<typeof _createAiChatState> | null = null;

export function useAiChat() {
  if (!_singleton) _singleton = _createAiChatState(createAiApi(), subscribeAiDelta);
  return _singleton;
}

export async function openInAI(text: string, opts?: { title?: string; systemPrompt?: string }): Promise<string> {
  const api = createAiApi();
  const s = await api.createSession(opts?.title, undefined);
  if (opts?.systemPrompt) await api.updateSession(s.id, { systemPrompt: opts.systemPrompt });
  await api.send(s.id, [{ type: 'text', text }]);
  return s.id;
}
