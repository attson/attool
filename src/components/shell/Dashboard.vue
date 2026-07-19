<script setup lang="ts">
import { computed } from 'vue';
import ToolIcon from './ToolIcon.vue';
import type { Tool, ToolGroup } from '../../types/tool';

const props = defineProps<{
  tools: Tool[];
  lastToolId: string | null;
}>();

const emit = defineEmits<{
  open: [id: string];
}>();

const GROUP_ORDER: ToolGroup[] = ['download', 'edit', 'network', 'utility'];
const GROUP_LABEL: Record<ToolGroup, string> = {
  download: '下载',
  edit: '编辑',
  network: '网络',
  utility: '实用'
};

const ready = computed(() => props.tools.filter((t) => t.status === 'ready'));
const soonCount = computed(() => props.tools.filter((t) => t.status === 'soon').length);

const readyByGroup = computed(() =>
  GROUP_ORDER.map((g) => ({
    key: g,
    label: GROUP_LABEL[g],
    items: props.tools.filter((t) => t.status === 'ready' && t.group === g)
  })).filter((s) => s.items.length > 0)
);

const lastTool = computed(() =>
  props.lastToolId ? ready.value.find((t) => t.id === props.lastToolId) ?? null : null
);
</script>

<template>
  <section class="dashboard">
    <div class="hero">
      <div class="title">欢迎回来</div>
      <div class="meta">{{ ready.length }} 个工具就绪 · {{ soonCount }} 个规划中</div>
    </div>

    <div v-if="lastTool" class="block">
      <div class="block-title">上次使用</div>
      <button class="last" type="button" @click="emit('open', lastTool.id)">
        <ToolIcon :name="lastTool.icon" :size="16" class="last-icon" />
        <span class="text">
          <strong>{{ lastTool.name }}</strong>
          <span class="desc">{{ lastTool.description }}</span>
        </span>
        <span class="arrow">→</span>
      </button>
    </div>

    <div v-for="section in readyByGroup" :key="section.key" class="block">
      <div class="block-title">{{ section.label }}</div>
      <div class="grid">
        <button
          v-for="tool in section.items"
          :key="tool.id"
          class="tile"
          type="button"
          @click="emit('open', tool.id)"
        >
          <ToolIcon :name="tool.icon" :size="16" class="tile-icon" />
          <span class="tile-name">{{ tool.name }}</span>
          <span class="tile-desc">{{ tool.description }}</span>
        </button>
      </div>
    </div>
  </section>
</template>

<style scoped>
.dashboard {
  max-width: 720px;
  margin: 36px auto 0;
  padding: 0 22px 36px;
  display: grid;
  gap: 28px;
}

.hero .title {
  font-size: var(--fs-2xl);
  font-weight: 600;
  letter-spacing: -0.012em;
  margin-bottom: 4px;
}
.hero .meta {
  color: var(--text-muted);
  font-size: var(--fs-xs);
}

.block-title {
  font-size: var(--fs-xxs);
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--text-muted);
  font-weight: 600;
  margin-bottom: 8px;
}

.last {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  background: var(--bg-elevated);
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  padding: 12px 14px;
  color: var(--text);
  cursor: pointer;
  text-align: left;
}
.last:hover { border-color: var(--line-strong); }
.last :deep(.last-icon) { color: var(--accent); }
.last .text { flex: 1; display: flex; flex-direction: column; gap: 2px; }
.last .text strong { font-size: var(--fs-md); font-weight: 600; }
.last .text .desc { color: var(--text-muted); font-size: var(--fs-xs); }
.last .arrow { color: var(--text-muted); font-size: 14px; }

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 10px;
}

.tile {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 12px 14px;
  background: var(--bg-elevated);
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  cursor: pointer;
  color: var(--text);
  text-align: left;
}
.tile:hover { border-color: var(--line-strong); }
.tile :deep(.tile-icon) { color: var(--text-muted); }
.tile:hover :deep(.tile-icon) { color: var(--accent); }
.tile-name { font-size: var(--fs-md); font-weight: 600; }
.tile-desc { color: var(--text-muted); font-size: var(--fs-xs); }
</style>
