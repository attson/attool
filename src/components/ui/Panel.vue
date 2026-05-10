<script setup lang="ts">
defineProps<{
  title?: string;
  flush?: boolean;
}>();
</script>

<template>
  <section class="panel">
    <header v-if="title || $slots.title || $slots.right" class="ph">
      <span class="pt"><slot name="title">{{ title }}</slot></span>
      <span class="pr"><slot name="right" /></span>
    </header>
    <div class="pb" :class="{ flush }">
      <slot />
    </div>
  </section>
</template>

<style scoped>
.panel {
  background: var(--bg-elevated);
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.ph {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 10px 14px;
  border-bottom: 1px solid var(--line);
  font-size: var(--fs-xs);
  color: var(--text-muted);
}
.pt { color: var(--text); font-weight: 500; font-size: var(--fs-md); }

.pb { padding: 14px; }
.pb.flush { padding: 0; }
</style>
