<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useJsonWorkspaceTabs } from '../../composables/useJsonWorkspaceTabs';
import JsonWorkspace from './JsonWorkspace.vue';

const props = defineProps<{ active: boolean }>();
const workspaces = useJsonWorkspaceTabs();
const root = ref<HTMLElement | null>(null);

watch(workspaces.activeTabId, async (id) => {
  await nextTick();
  root.value
    ?.querySelector<HTMLElement>(`.workspace-tab[data-workspace-id="${id}"]`)
    ?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
});

function onTabMouseDown(id: string, event: MouseEvent) {
  if (event.button === 1) {
    event.preventDefault();
    workspaces.closeTab(id);
  } else if (event.button === 0) {
    workspaces.activateTab(id);
  }
}

function onKeydown(event: KeyboardEvent) {
  if (!props.active || !(event.metaKey || event.ctrlKey)) return;
  const key = event.key.toLowerCase();
  if (key === 't') {
    event.preventDefault();
    workspaces.newTab();
  } else if (key === 'w') {
    event.preventDefault();
    workspaces.closeTab(workspaces.activeTabId.value);
  }
}

onMounted(() => window.addEventListener('keydown', onKeydown));
onBeforeUnmount(() => window.removeEventListener('keydown', onKeydown));
</script>

<template>
  <div ref="root" class="json-tool">
    <div class="workspace-tabs" role="tablist" aria-label="JSON 工作页">
      <div
        v-for="tab in workspaces.tabs.value"
        :key="tab.id"
        :data-workspace-id="tab.id"
        :class="['workspace-tab', { active: tab.id === workspaces.activeTabId.value }]"
        role="tab"
        :aria-selected="tab.id === workspaces.activeTabId.value"
        :tabindex="tab.id === workspaces.activeTabId.value ? 0 : -1"
        :title="tab.title"
        @mousedown="onTabMouseDown(tab.id, $event)"
        @keydown.enter="workspaces.activateTab(tab.id)"
        @keydown.space.prevent="workspaces.activateTab(tab.id)"
      >
        <span class="workspace-title">{{ tab.title }}</span>
        <button
          type="button"
          class="workspace-close"
          :aria-label="`关闭 ${tab.title}`"
          @mousedown.stop
          @click.stop="workspaces.closeTab(tab.id)"
        >×</button>
      </div>
      <button
        type="button"
        class="workspace-new"
        title="新建 JSON 工作页 (⌘T)"
        aria-label="新建 JSON 工作页"
        @click="workspaces.newTab"
      >+</button>
    </div>

    <div class="workspace-content">
      <JsonWorkspace
        v-for="tab in workspaces.tabs.value"
        v-show="tab.id === workspaces.activeTabId.value"
        :key="tab.id"
      />
    </div>
  </div>
</template>

<style scoped>
.json-tool { height: 100%; display: flex; flex-direction: column; }
.workspace-tabs {
  display: flex;
  flex: 0 0 auto;
  align-items: stretch;
  min-width: 0;
  overflow-x: auto;
  border-bottom: 1px solid var(--line);
  background: var(--bg-base);
  scrollbar-width: thin;
}
.workspace-tab {
  position: relative;
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 8px;
  min-width: 112px;
  max-width: 180px;
  padding: 7px 9px 7px 12px;
  border-right: 1px solid var(--line);
  color: var(--text-muted);
  cursor: pointer;
  font-size: var(--fs-xs);
  user-select: none;
}
.workspace-tab:hover { background: var(--bg-elev); }
.workspace-tab.active {
  background: var(--bg-elev);
  color: var(--text);
  box-shadow: inset 0 -2px 0 var(--accent);
}
.workspace-tab:focus-visible { outline: 1px solid var(--accent); outline-offset: -2px; }
.workspace-title {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.workspace-close,
.workspace-new {
  border: 0;
  background: none;
  color: var(--text-muted);
  cursor: pointer;
}
.workspace-close {
  display: grid;
  width: 20px;
  height: 20px;
  padding: 0;
  place-items: center;
  border-radius: var(--radius-sm);
  font-size: var(--fs-md);
}
.workspace-close:hover { background: var(--bg-base); color: var(--error); }
.workspace-new {
  flex: 0 0 auto;
  min-width: 42px;
  padding: 0 14px;
  font-size: var(--fs-lg);
}
.workspace-new:hover { color: var(--text); background: var(--bg-elev); }
.workspace-content { flex: 1; min-height: 0; padding-top: 8px; }
</style>
