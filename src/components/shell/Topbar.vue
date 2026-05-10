<script setup lang="ts">
defineProps<{
  crumb?: string;
}>();
</script>

<template>
  <header class="topbar">
    <div class="crumb">
      <span class="root">工具</span>
      <span v-if="crumb" class="sep">/</span>
      <span v-if="crumb" class="here">{{ crumb }}</span>
    </div>
    <div class="right">
      <slot name="right" />
    </div>
  </header>
</template>

<style scoped>
.topbar {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 22px;
  background: var(--bg-base);
  border-bottom: 1px solid var(--line);
  font-size: var(--fs-xs);
  color: var(--text-muted);
}

.crumb {
  display: flex;
  align-items: center;
  gap: 6px;
}
.crumb .sep { color: var(--text-faint); }
.crumb .here {
  color: var(--text);
  font-weight: 500;
}

.right {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 8px;
}
</style>
