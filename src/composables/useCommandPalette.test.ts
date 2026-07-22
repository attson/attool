import { describe, it, expect, vi } from 'vitest';
import { useCommandPalette } from './useCommandPalette';
import type { Tool } from '../types/tool';

const fakeTools: Tool[] = [
  { id: 'http',  name: 'HTTP 请求', description: 'HTTP body/headers',   status: 'ready', icon: 'send',     group: 'network' },
  { id: 'json',  name: 'JSON 工具', description: '格式化 / JSONPath',   status: 'ready', icon: 'code',     group: 'edit' },
  { id: 'aria2', name: 'Aria2',    description: '多连接下载',           status: 'ready', icon: 'download', group: 'download' }
];

describe('useCommandPalette', () => {
  it('空 query 时按分区列出全部 tool', () => {
    const onOpenTool = vi.fn();
    const cp = useCommandPalette({ tools: () => fakeTools, onOpenTool });
    cp.show();
    cp.query.value = '';
    expect(cp.results.value.length).toBe(3);
    expect(cp.results.value.every((r) => r.kind === 'tool')).toBe(true);
  });

  it('substring 匹配 name', () => {
    const cp = useCommandPalette({ tools: () => fakeTools, onOpenTool: vi.fn() });
    cp.query.value = 'json';
    expect(cp.results.value.map((r) => r.id)).toEqual(['json']);
  });

  it('substring 匹配 description', () => {
    const cp = useCommandPalette({ tools: () => fakeTools, onOpenTool: vi.fn() });
    cp.query.value = 'headers';
    expect(cp.results.value.map((r) => r.id)).toEqual(['http']);
  });

  it('拼音首字母匹配（HTTP → h）', () => {
    const cp = useCommandPalette({ tools: () => fakeTools, onOpenTool: vi.fn() });
    cp.query.value = 'ht';
    expect(cp.results.value.map((r) => r.id)).toContain('http');
  });

  it('onSelect 触发 onOpenTool 回调', () => {
    const onOpenTool = vi.fn();
    const cp = useCommandPalette({ tools: () => fakeTools, onOpenTool });
    cp.query.value = 'json';
    cp.results.value[0].onSelect();
    expect(onOpenTool).toHaveBeenCalledWith('json');
  });

  it('提供 envs 时空 query 也列出环境', () => {
    const cp = useCommandPalette({
      tools: () => fakeTools,
      onOpenTool: vi.fn(),
      envs: () => [{ name: 'prod', active: true }, { name: 'dev', active: false }],
      onSwitchEnv: vi.fn()
    });
    cp.query.value = '';
    const envItems = cp.results.value.filter((r) => r.kind === 'env');
    expect(envItems.map((r) => r.id)).toEqual(['prod', 'dev']);
  });

  it('提供 httpHistory 时空 query 也列出前 5 条历史', () => {
    const history = Array.from({ length: 10 }, (_, i) => ({
      method: 'GET', url: `/x/${i}`, ts: i
    }));
    const cp = useCommandPalette({
      tools: () => fakeTools,
      onOpenTool: vi.fn(),
      httpHistory: () => history,
      onOpenHistory: vi.fn()
    });
    cp.query.value = '';
    const histItems = cp.results.value.filter((r) => r.kind === 'history');
    expect(histItems.length).toBe(5);
  });

  it('提供 collectionRequests 时可搜索集合请求', () => {
    const onOpenCollectionRequest = vi.fn();
    const cp = useCommandPalette({
      tools: () => fakeTools,
      onOpenTool: vi.fn(),
      collectionRequests: () => [
        { id: 'r1', name: 'GET users', method: 'GET', url: '{{baseUrl}}/users', collectionName: 'Admin API' }
      ],
      onOpenCollectionRequest
    });
    cp.query.value = 'users';
    const item = cp.results.value[0];
    expect(item.kind).toBe('collection-request');
    item.onSelect();
    expect(onOpenCollectionRequest).toHaveBeenCalledWith('r1');
  });

  it('提供 actions 时可触发普通动作', () => {
    const onSelect = vi.fn();
    const cp = useCommandPalette({
      tools: () => fakeTools,
      onOpenTool: vi.fn(),
      actions: () => [{ id: 'http-vars', title: 'HTTP 变量', subtitle: '打开变量管理', groupLabel: '动作', onSelect }]
    });
    cp.query.value = '变量';
    const item = cp.results.value[0];
    expect(item.kind).toBe('action');
    item.onSelect();
    expect(onSelect).toHaveBeenCalled();
  });

  it('每分区搜索命中最多 20 条', () => {
    const many = Array.from({ length: 50 }, (_, i) => ({
      id: `t${i}`, name: `Tool ${i}`, description: 'match',
      status: 'ready' as const, icon: 'send' as const, group: 'utility' as const
    }));
    const cp = useCommandPalette({ tools: () => many, onOpenTool: vi.fn() });
    cp.query.value = 'match';
    expect(cp.results.value.length).toBe(20);
  });

  it('show / hide / toggle 控制 open ref', () => {
    const cp = useCommandPalette({ tools: () => fakeTools, onOpenTool: vi.fn() });
    expect(cp.open.value).toBe(false);
    cp.show();
    expect(cp.open.value).toBe(true);
    cp.hide();
    expect(cp.open.value).toBe(false);
    cp.toggle();
    expect(cp.open.value).toBe(true);
  });

  it('hide 时清空 query', () => {
    const cp = useCommandPalette({ tools: () => fakeTools, onOpenTool: vi.fn() });
    cp.show();
    cp.query.value = 'foo';
    cp.hide();
    expect(cp.query.value).toBe('');
  });
});
