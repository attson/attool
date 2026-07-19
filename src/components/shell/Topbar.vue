<script setup lang="ts">
defineProps<{
  crumb?: string;
}>();

const emit = defineEmits<{
  search: [];
}>();
</script>

<template>
  <header class="topbar">
    <div class="crumb">
      <span class="arrow">▸</span>
      <span class="root">工具</span>
      <span v-if="crumb" class="sep">/</span>
      <span v-if="crumb" class="here">{{ crumb }}</span>
    </div>

    <button class="searchbar" type="button" @click="emit('search')">
      <span class="ico" aria-hidden="true">
        <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.8">
          <circle cx="11" cy="11" r="7" />
          <path d="M20 20l-4-4" />
        </svg>
      </span>
      <span class="label">搜索工具、查历史、切环境…</span>
      <span class="kbd">⌘K</span>
    </button>

    <div class="right">
      <slot name="right" />
    </div>
  </header>
</template>

<style scoped>
.topbar {
  display: grid;
  grid-template-columns: 1fr auto 1fr;
  align-items: center;
  gap: 14px;
  height: var(--topbar-height);
  padding: 0 22px;
  background: rgba(10, 10, 11, 0.72);
  backdrop-filter: saturate(160%) blur(14px);
  -webkit-backdrop-filter: saturate(160%) blur(14px);
  border-bottom: 1px solid var(--line);
  font-size: var(--fs-xs);
  color: var(--text-muted);
}
[data-theme="light"] .topbar {
  background: rgba(250, 250, 249, 0.72);
}

.crumb { display: flex; align-items: center; gap: 6px; }
.crumb .arrow { color: var(--accent); font-size: 12px; line-height: 1; }
.crumb .root  { color: var(--text-muted); }
.crumb .sep   { color: var(--text-faint); }
.crumb .here  { color: var(--text); font-weight: 500; }

.searchbar {
  justify-self: center;
  width: 100%;
  max-width: 560px;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 14px;
  background: var(--bg-elev-2);
  border: 1px solid var(--line-strong);
  border-radius: var(--radius-md);
  color: var(--text-muted);
  font-size: var(--fs-xs);
  cursor: pointer;
  transition: border-color var(--motion-fast);
}
.searchbar:hover { border-color: var(--accent-line); color: var(--text); }
.searchbar .ico { display: grid; place-items: center; }
.searchbar .label { flex: 1; text-align: left; }
.searchbar .kbd {
  font-family: var(--font-mono);
  font-size: var(--fs-xxs);
  padding: 1px 5px;
  border-radius: var(--radius-sm);
  background: var(--bg-base);
  border: 1px solid var(--line-strong);
  color: var(--text-muted);
}

.right {
  justify-self: end;
  display: flex;
  align-items: center;
  gap: 8px;
}
</style>
