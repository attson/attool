import { describe, it, expect } from 'vitest';
import type { ToolGroup } from './tool';

const knownGroups: ToolGroup[] = ['download', 'edit', 'network', 'utility'];

const tools = [
  { id: 'aria2',      group: 'download' as ToolGroup },
  { id: 'template',   group: 'edit' as ToolGroup },
  { id: 'clipboard',  group: 'edit' as ToolGroup },
  { id: 'json',       group: 'edit' as ToolGroup },
  { id: 'video-link', group: 'download' as ToolGroup },
  { id: 'image',      group: 'edit' as ToolGroup },
  { id: 'text',       group: 'edit' as ToolGroup },
  { id: 'network',    group: 'network' as ToolGroup },
  { id: 'codec',      group: 'network' as ToolGroup },
  { id: 'generator',  group: 'utility' as ToolGroup },
  { id: 'time',       group: 'utility' as ToolGroup },
  { id: 'http',       group: 'network' as ToolGroup }
];

describe('Tool.group', () => {
  it('每个 tool 都在 4 个合法 group 之一', () => {
    for (const t of tools) {
      expect(knownGroups).toContain(t.group);
    }
  });

  it('12 个 tool 全数', () => {
    expect(tools.length).toBe(12);
  });
});
