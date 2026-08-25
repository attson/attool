import { ref } from 'vue';

export interface JsonWorkspaceTab {
  id: string;
  title: string;
}

export function useJsonWorkspaceTabs() {
  const tabs = ref<JsonWorkspaceTab[]>([
    { id: 'json-workspace-1', title: 'JSON 1' },
  ]);
  const activeTabId = ref('json-workspace-1');
  let nextSequence = 2;

  function newTab(): JsonWorkspaceTab {
    const sequence = nextSequence++;
    const tab = {
      id: `json-workspace-${sequence}`,
      title: `JSON ${sequence}`,
    };
    tabs.value.push(tab);
    activeTabId.value = tab.id;
    return tab;
  }

  function activateTab(id: string): void {
    if (tabs.value.some((tab) => tab.id === id)) {
      activeTabId.value = id;
    }
  }

  function closeTab(id: string): void {
    const index = tabs.value.findIndex((tab) => tab.id === id);
    if (index === -1) return;
    const wasActive = activeTabId.value === id;
    tabs.value.splice(index, 1);
    if (tabs.value.length === 0) {
      newTab();
      return;
    }
    if (wasActive && tabs.value.length > 0) {
      activeTabId.value = tabs.value[Math.min(index, tabs.value.length - 1)].id;
    }
  }

  return { tabs, activeTabId, newTab, activateTab, closeTab };
}
