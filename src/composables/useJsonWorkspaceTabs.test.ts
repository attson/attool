import { describe, expect, it } from 'vitest';
import { useJsonWorkspaceTabs } from './useJsonWorkspaceTabs';

describe('useJsonWorkspaceTabs', () => {
  it('starts with JSON 1 active', () => {
    const workspaces = useJsonWorkspaceTabs();

    expect(workspaces.tabs.value).toEqual([
      { id: 'json-workspace-1', title: 'JSON 1' },
    ]);
    expect(workspaces.activeTabId.value).toBe('json-workspace-1');
  });

  it('creates and activates sequentially numbered workspaces', () => {
    const workspaces = useJsonWorkspaceTabs();

    workspaces.newTab();
    workspaces.newTab();

    expect(workspaces.tabs.value.map((tab) => tab.title)).toEqual([
      'JSON 1',
      'JSON 2',
      'JSON 3',
    ]);
    expect(workspaces.activeTabId.value).toBe('json-workspace-3');
  });

  it('activates only existing workspaces', () => {
    const workspaces = useJsonWorkspaceTabs();
    workspaces.newTab();

    workspaces.activateTab('json-workspace-1');
    expect(workspaces.activeTabId.value).toBe('json-workspace-1');

    workspaces.activateTab('missing');
    expect(workspaces.activeTabId.value).toBe('json-workspace-1');
  });

  it('closes an inactive workspace without changing the active one', () => {
    const workspaces = useJsonWorkspaceTabs();
    workspaces.newTab();

    workspaces.closeTab('json-workspace-1');

    expect(workspaces.tabs.value.map((tab) => tab.id)).toEqual(['json-workspace-2']);
    expect(workspaces.activeTabId.value).toBe('json-workspace-2');
  });

  it('selects the right neighbor then the left when closing active workspaces', () => {
    const workspaces = useJsonWorkspaceTabs();
    workspaces.newTab();
    workspaces.newTab();
    workspaces.activateTab('json-workspace-2');

    workspaces.closeTab('json-workspace-2');
    expect(workspaces.activeTabId.value).toBe('json-workspace-3');

    workspaces.closeTab('json-workspace-3');
    expect(workspaces.activeTabId.value).toBe('json-workspace-1');
  });

  it('replaces the last workspace with a newly numbered blank workspace', () => {
    const workspaces = useJsonWorkspaceTabs();

    workspaces.closeTab('json-workspace-1');

    expect(workspaces.tabs.value).toEqual([
      { id: 'json-workspace-2', title: 'JSON 2' },
    ]);
    expect(workspaces.activeTabId.value).toBe('json-workspace-2');
  });

  it('ignores requests to close a missing workspace', () => {
    const workspaces = useJsonWorkspaceTabs();

    workspaces.closeTab('missing');

    expect(workspaces.tabs.value).toEqual([
      { id: 'json-workspace-1', title: 'JSON 1' },
    ]);
    expect(workspaces.activeTabId.value).toBe('json-workspace-1');
  });
});
