import { describe, expect, it } from 'vitest';
import { isDuplicateAiModelId } from './ai';
import type { AiModel } from './ai';

function model(id: string, providerId: string, modelId = 'gpt-5.5'): AiModel {
  return {
    id,
    providerId,
    modelId,
    displayName: modelId,
    capabilities: '["text"]',
    temperature: null,
    maxTokens: null,
    sortOrder: 0,
    createdAt: 0,
    updatedAt: 0,
  };
}

describe('isDuplicateAiModelId', () => {
  it('只拒绝同一 provider 下其他记录已使用的规范化模型 ID', () => {
    const models = [model('m1', 'p1'), model('m2', 'p2')];

    expect(isDuplicateAiModelId(models, 'p1', '  gpt-5.5  ', 'new')).toBe(true);
    expect(isDuplicateAiModelId(models, 'p1', 'gpt-5.5', 'm1')).toBe(false);
    expect(isDuplicateAiModelId(models, 'p3', 'gpt-5.5', 'new')).toBe(false);
  });
});
