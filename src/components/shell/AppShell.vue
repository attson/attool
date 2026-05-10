<script setup lang="ts">
import Sidebar from './Sidebar.vue';
import Topbar from './Topbar.vue';
import type { Tool } from '../../types/tool';

defineProps<{
  tools: Tool[];
  activeId: string | null;
  collapsed: boolean;
  crumb?: string;
}>();

const emit = defineEmits<{
  select: [id: string];
  toggle: [];
  brand: [];
  search: [];
}>();
</script>

<template>
  <div class="app-shell">
    <Sidebar
      :tools="tools"
      :active-id="activeId"
      :collapsed="collapsed"
      @select="(id) => emit('select', id)"
      @toggle="emit('toggle')"
      @brand="emit('brand')"
      @search="emit('search')"
    />
    <main class="main">
      <Topbar :crumb="crumb">
        <template #right>
          <slot name="topbar-right" />
        </template>
      </Topbar>
      <div class="content">
        <slot />
      </div>
    </main>
  </div>
</template>

<style scoped>
.app-shell {
  display: flex;
  min-height: 100vh;
  background: var(--bg-base);
}

.main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.content {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 18px 22px;
}
</style>
