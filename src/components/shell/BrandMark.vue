<script setup lang="ts">
defineProps<{ size?: number }>();
</script>

<template>
  <span class="brand-mark" :style="size ? { width: `${size}px`, height: `${size}px` } : undefined">A</span>
</template>

<style scoped>
.brand-mark {
  display: grid;
  place-items: center;
  width: 26px;
  height: 26px;
  border-radius: 7px;
  background: var(--accent);
  color: var(--accent-fg);
  font-weight: 700;
  font-size: 12px;
  letter-spacing: 0.02em;
  flex-shrink: 0;
}
</style>
