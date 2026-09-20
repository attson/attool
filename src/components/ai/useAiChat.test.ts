import { describe, expect, it, vi } from 'vitest';
import { _createAiChatState } from './useAiChat';

function makeFakeApi() {
  const state = { sessions: [] as any[], models: [] as any[], messages: {} as Record<string, any[]> };
  return {
    api: {
      listProviders: vi.fn().mockResolvedValue([]),
      listModels: vi.fn().mockImplementation(async () => state.models.slice()),
      listSessions: vi.fn().mockImplementation(async () => state.sessions.slice()),
      createSession: vi.fn().mockImplementation(async (title: string | null, modelId: string | null) => {
        const s = { id: `s${state.sessions.length}`, title: title ?? '新会话', systemPrompt: '', currentModelId: modelId, createdAt: 0, updatedAt: 0 };
        state.sessions.push(s);
        state.messages[s.id] = [];
        return s;
      }),
      getSession: vi.fn().mockImplementation(async (id: string) => ({
        session: state.sessions.find(s => s.id === id),
        messages: state.messages[id] ?? [],
      })),
      updateSession: vi.fn().mockImplementation(async (id: string, patch: any) => {
        const s = state.sessions.find(x => x.id === id)!;
        if (patch.title !== null) s.title = patch.title;
        if (patch.modelId !== null) s.currentModelId = patch.modelId;
        return s;
      }),
      deleteSession: vi.fn().mockImplementation(async (id: string) => {
        state.sessions = state.sessions.filter(s => s.id !== id);
      }),
      send: vi.fn().mockImplementation(async (sessionId: string, content: any[]) => {
        const userId = `u${Date.now()}`;
        const assistantId = `a${Date.now()}`;
        state.messages[sessionId].push({ id: userId, sessionId, role: 'user', contentJson: JSON.stringify(content), modelId: null, status: 'done', errorMessage: null, promptTokens: null, completionTokens: null, createdAt: 0, finishedAt: 0 });
        state.messages[sessionId].push({ id: assistantId, sessionId, role: 'assistant', contentJson: '[{"type":"text","text":""}]', modelId: null, status: 'streaming', errorMessage: null, promptTokens: null, completionTokens: null, createdAt: 0, finishedAt: null });
        return assistantId;
      }),
      cancel: vi.fn().mockResolvedValue(true),
      retry: vi.fn(),
      exportSession: vi.fn(),
      exportConfig: vi.fn(),
      importConfig: vi.fn(),
      saveAttachment: vi.fn(),
      upsertProvider: vi.fn(),
      deleteProvider: vi.fn(),
      upsertModel: vi.fn(),
      deleteModel: vi.fn(),
      fetchProviderModels: vi.fn(),
    },
    subscribe: vi.fn().mockResolvedValue(() => undefined),
    state,
  };
}

describe('useAiChat', () => {
  it('newSession creates and opens the session', async () => {
    const { api, subscribe } = makeFakeApi();
    const s = _createAiChatState(api as any, subscribe as any);
    const id = await s.newSession();
    await s.loadSessions();
    expect(s.currentSessionId.value).toBe(id);
    expect(s.sessions.value.length).toBe(1);
  });

  it('newSession selects the first available model by default', async () => {
    const { api, subscribe, state } = makeFakeApi();
    state.models = [{ id: 'm1' }, { id: 'm2' }];
    const s = _createAiChatState(api as any, subscribe as any);

    await s.loadModels();
    await s.newSession();

    expect(api.createSession).toHaveBeenCalledWith(undefined, 'm1');
    expect(s.currentSession.value?.session.currentModelId).toBe('m1');
  });

  it('openSession repairs a missing or deleted model selection', async () => {
    const { api, subscribe, state } = makeFakeApi();
    state.models = [{ id: 'm1' }];
    state.sessions.push({
      id: 's1', title: '新会话', systemPrompt: '', currentModelId: 'deleted-model', createdAt: 0, updatedAt: 0,
    });
    state.messages.s1 = [];
    const s = _createAiChatState(api as any, subscribe as any);

    await s.loadModels();
    await s.openSession('s1');

    expect(api.updateSession).toHaveBeenCalledWith('s1', { modelId: 'm1' });
    expect(s.currentSession.value?.session.currentModelId).toBe('m1');
  });

  it('loadModels falls back when the current model was deleted', async () => {
    const { api, subscribe, state } = makeFakeApi();
    state.models = [{ id: 'm1' }, { id: 'm2' }];
    state.sessions.push({
      id: 's1', title: '新会话', systemPrompt: '', currentModelId: 'm2', createdAt: 0, updatedAt: 0,
    });
    state.messages.s1 = [];
    const s = _createAiChatState(api as any, subscribe as any);
    await s.loadModels();
    await s.openSession('s1');

    state.models = [{ id: 'm1' }];
    api.updateSession.mockClear();
    await s.loadModels();

    expect(api.updateSession).toHaveBeenCalledWith('s1', { modelId: 'm1' });
    expect(s.currentSession.value?.session.currentModelId).toBe('m1');
  });

  it('sendMessage tracks streamingMessageId until done fires', async () => {
    const { api, subscribe } = makeFakeApi();
    let doneCb: any = () => {};
    (subscribe as any).mockImplementation(async (_id: string, _onDelta: any, onDone: any) => {
      doneCb = onDone;
      return () => undefined;
    });
    const s = _createAiChatState(api as any, subscribe as any);
    const id = await s.newSession();
    await s.openSession(id);
    await s.sendMessage('hi', []);
    expect(s.streamingMessageId.value).not.toBeNull();
    doneCb({ status: 'done', promptTokens: 3, completionTokens: 5 });
    await new Promise(r => setTimeout(r, 0));
    expect(s.streamingMessageId.value).toBeNull();
  });

  it('cancelStreaming calls api.cancel with current streaming id', async () => {
    const { api, subscribe } = makeFakeApi();
    const s = _createAiChatState(api as any, subscribe as any);
    const id = await s.newSession();
    await s.openSession(id);
    await s.sendMessage('hi', []);
    const streamId = s.streamingMessageId.value;
    await s.cancelStreaming();
    expect(api.cancel).toHaveBeenCalledWith(streamId);
  });

  it('renameSession updates session.title', async () => {
    const { api, subscribe, state } = makeFakeApi();
    const s = _createAiChatState(api as any, subscribe as any);
    const id = await s.newSession();
    await s.renameSession(id, 'foo');
    expect(state.sessions[0].title).toBe('foo');
  });
});
