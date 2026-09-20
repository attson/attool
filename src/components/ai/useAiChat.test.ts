import { describe, expect, it, vi } from 'vitest';
import { _createAiChatState } from './useAiChat';
import type { KVStorage } from '../../composables/useSidebarState';

function fakeStorage(initial: Record<string, string> = {}) {
  const data = { ...initial };
  const storage: KVStorage = {
    getItem: (key) => data[key] ?? null,
    setItem: (key, value) => { data[key] = value; },
  };
  return { storage, data };
}

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
        if (patch.title != null) s.title = patch.title;
        if (patch.modelId != null) s.currentModelId = patch.modelId;
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

  it('remembers the manually selected model for later sessions', async () => {
    const { api, subscribe, state } = makeFakeApi();
    const { storage, data } = fakeStorage();
    state.models = [{ id: 'm1' }, { id: 'm2' }];
    const s = _createAiChatState(api as any, subscribe as any, storage);
    await s.loadModels();
    const firstSessionId = await s.newSession();

    await s.switchSessionModel(firstSessionId, 'm2');
    await s.newSession();

    expect(data['attool.ai.preferredModelId']).toBe('m2');
    expect(api.createSession).toHaveBeenLastCalledWith(undefined, 'm2');
  });

  it('uses the persisted model preference after state recreation', async () => {
    const { api, subscribe, state } = makeFakeApi();
    const { storage } = fakeStorage({ 'attool.ai.preferredModelId': 'm2' });
    state.models = [{ id: 'm1' }, { id: 'm2' }];
    const s = _createAiChatState(api as any, subscribe as any, storage);

    await s.loadModels();
    await s.newSession();

    expect(api.createSession).toHaveBeenCalledWith(undefined, 'm2');
  });

  it('replaces a deleted model preference with the first available model', async () => {
    const { api, subscribe, state } = makeFakeApi();
    const { storage, data } = fakeStorage({ 'attool.ai.preferredModelId': 'deleted-model' });
    state.models = [{ id: 'm1' }];
    const s = _createAiChatState(api as any, subscribe as any, storage);

    await s.loadModels();
    await s.newSession();

    expect(api.createSession).toHaveBeenCalledWith(undefined, 'm1');
    expect(data['attool.ai.preferredModelId']).toBe('m1');
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
