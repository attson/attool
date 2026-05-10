<script setup lang="ts">
defineProps<{ tone?: 'default' | 'accent' | 'warning' | 'error' }>();
</script>

<template>
  <span class="stat-pill" :data-tone="tone ?? 'default'">
    <slot />
  </span>
</template>

<style scoped>
.stat-pill {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 9px;
  border-radius: var(--radius-pill);
  background: var(--bg-elevated);
  border: 1px solid var(--line);
  color: var(--text-muted);
  font-size: var(--fs-xs);
  font-feature-settings: "tnum";
  white-space: nowrap;
}

.stat-pill[data-tone="accent"] {
  background: var(--accent-soft);
  color: var(--accent);
  border-color: transparent;
}

.stat-pill[data-tone="warning"] {
  background: color-mix(in srgb, var(--warning) 18%, transparent);
  color: var(--warning);
  border-color: transparent;
}

.stat-pill[data-tone="error"] {
  background: color-mix(in srgb, var(--error) 18%, transparent);
  color: var(--error);
  border-color: transparent;
}
</style>
