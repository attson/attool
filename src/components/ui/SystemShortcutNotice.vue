<script setup lang="ts">
defineProps<{
  title: string;
}>();
</script>

<template>
  <details class="system-shortcut-notice">
    <summary class="system-shortcut-notice__summary">
      <span class="system-shortcut-notice__icon" aria-hidden="true">!</span>
      <span class="system-shortcut-notice__title">{{ title }}</span>
      <svg class="system-shortcut-notice__chevron" viewBox="0 0 16 16" aria-hidden="true">
        <path d="m5 6 3 3 3-3" />
      </svg>
    </summary>
    <div class="system-shortcut-notice__content">
      <slot />
    </div>
  </details>
</template>

<style scoped>
.system-shortcut-notice {
  overflow: hidden;
  color: var(--text);
  background: var(--warning-soft);
  border: 1px solid color-mix(in srgb, var(--warning) 28%, transparent);
  border-radius: var(--radius);
}
.system-shortcut-notice__summary {
  display: grid;
  grid-template-columns: 20px minmax(0, 1fr) 20px;
  align-items: center;
  gap: 10px;
  min-height: 46px;
  padding: 0 14px;
  list-style: none;
  cursor: pointer;
  user-select: none;
}
.system-shortcut-notice__summary::-webkit-details-marker {
  display: none;
}
.system-shortcut-notice__summary:focus-visible {
  outline: 2px solid var(--warning);
  outline-offset: -2px;
}
.system-shortcut-notice__icon {
  display: grid;
  place-items: center;
  width: 20px;
  height: 20px;
  color: var(--accent-fg);
  background: var(--warning);
  border-radius: var(--radius-pill);
  font-size: var(--fs-xs);
  font-weight: 700;
  line-height: 1;
}
.system-shortcut-notice__title {
  min-width: 0;
  font-size: var(--fs-md);
  line-height: 1.5;
}
.system-shortcut-notice__chevron {
  width: 16px;
  height: 16px;
  color: var(--text-muted);
  transition: transform var(--motion-fast);
}
.system-shortcut-notice__chevron path {
  fill: none;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.5;
}
.system-shortcut-notice[open] .system-shortcut-notice__chevron {
  transform: rotate(180deg);
}
.system-shortcut-notice__content {
  padding: 12px 14px 14px 44px;
  border-top: 1px solid color-mix(in srgb, var(--warning) 18%, transparent);
}

@media (max-width: 640px) {
  .system-shortcut-notice__content {
    padding-left: 14px;
  }
}
</style>
