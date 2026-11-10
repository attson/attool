import { describe, it, expect } from 'vitest';
import { builtinTools } from '../builtinTools';
import type { ToolGroup } from './tool';

const knownGroups: ToolGroup[] = ['download', 'edit', 'network', 'utility'];
const tools = builtinTools.filter((tool) => tool.source !== 'platform');

describe('Tool.group', () => {
  it('每个 tool 都在 4 个合法 group 之一', () => {
    for (const t of tools) {
      expect(knownGroups).toContain(t.group);
    }
  });

  it('12 个内置 tool 全数', () => {
    expect(tools).toHaveLength(12);
    expect(tools.some((tool) => tool.id === 'codec')).toBe(false);
  });
});
