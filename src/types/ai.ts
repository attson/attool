export type AiCapability = 'text' | 'image' | 'audio' | 'video';
export type AiProviderKind = 'openai' | 'anthropic' | 'ollama';
export type AiMessageRole = 'user' | 'assistant' | 'system';
export type AiMessageStatus = 'pending' | 'streaming' | 'done' | 'error' | 'cancelled';

export interface AiProvider {
    id: string; name: string; kind: AiProviderKind;
    baseUrl: string; apiKey: string; extraJson: string;
    sortOrder: number; createdAt: number; updatedAt: number;
}
export interface AiModel {
    id: string; providerId: string; modelId: string; displayName: string;
    capabilities: string;  // JSON array 字符串
    temperature: number | null; maxTokens: number | null;
    sortOrder: number; createdAt: number; updatedAt: number;
}
export interface AiSession {
    id: string; title: string; systemPrompt: string;
    currentModelId: string | null;
    createdAt: number; updatedAt: number;
}
export interface AiMessage {
    id: string; sessionId: string; role: AiMessageRole;
    contentJson: string;
    modelId: string | null; status: AiMessageStatus;
    errorMessage: string | null;
    promptTokens: number | null; completionTokens: number | null;
    createdAt: number; finishedAt: number | null;
}
export interface AiSessionSummary { session: AiSession; messages: AiMessage[] }
export interface ProviderModelInfo { id: string; displayName: string }
export type AiContentPart =
    | { type: 'text'; text: string }
    | { type: 'image'; path: string; mime: string };
export interface AiImportSummary { providersUpserted: number; modelsUpserted: number }

export function parseCapabilities(json: string): AiCapability[] {
    try {
        const v = JSON.parse(json);
        return Array.isArray(v) ? (v as AiCapability[]) : ['text'];
    } catch { return ['text']; }
}
export function parseContent(json: string): AiContentPart[] {
    try { const v = JSON.parse(json); return Array.isArray(v) ? v as AiContentPart[] : []; }
    catch { return []; }
}
