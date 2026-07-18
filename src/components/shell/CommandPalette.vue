<script setup lang="ts">
import { NModal, NInput } from 'naive-ui';
import { computed, nextTick, ref, watch } from 'vue';
import type { CommandItem } from '../../composables/useCommandPalette';

const props = defineProps<{
  open: boolean;
  query: string;
  results: CommandItem[];
}>();

const emit = defineEmits<{
  (e: 'update:open', v: boolean): void;
  (e: 'update:query', v: string): void;
}>();

const inputRef = ref<InstanceType<typeof NInput> | null>(null);
const activeIdx = ref(0);
const listRef = ref<HTMLElement | null>(null);

interface Section { label: string; items: CommandItem[] }

const sections = computed<Section[]>(() => {
  const out: Section[] = [];
  for (const item of props.results) {
    const last = out[out.length - 1];
    if (last && last.label === item.groupLabel) {
      last.items.push(item);
    } else {
      out.push({ label: item.groupLabel, items: [item] });
    }
  }
  return out;
});

watch(
  () => props.open,
  (v) => {
    if (v) {
      activeIdx.value = 0;
      nextTick(() => {
        (inputRef.value as unknown as { focus?: () => void })?.focus?.();
      });
    }
  }
);

watch(
  () => props.query,
  () => { activeIdx.value = 0; }
);

function onQuery(v: string) { emit('update:query', v); }
function close() { emit('update:open', false); }

function moveDown() {
  if (props.results.length === 0) return;
  activeIdx.value = (activeIdx.value + 1) % props.results.length;
  scrollActiveIntoView();
}
function moveUp() {
  if (props.results.length === 0) return;
  activeIdx.value = (activeIdx.value - 1 + props.results.length) % props.results.length;
  scrollActiveIntoView();
}
function scrollActiveIntoView() {
  nextTick(() => {
    const el = listRef.value?.querySelector<HTMLElement>(`[data-idx="${activeIdx.value}"]`);
    el?.scrollIntoView({ block: 'nearest' });
  });
}
function activate(idx: number) {
  const item = props.results[idx];
  if (!item) return;
  item.onSelect();
  close();
}

function onKey(e: KeyboardEvent) {
  if (!props.open) return;
  if (e.key === 'ArrowDown') { e.preventDefault(); moveDown(); }
  else if (e.key === 'ArrowUp') { e.preventDefault(); moveUp(); }
  else if (e.key === 'Enter') { e.preventDefault(); activate(activeIdx.value); }
  else if (e.key === 'Escape') { e.preventDefault(); close(); }
}
</script>

<template>
  <n-modal
    :show="open"
    preset="card"
    :bordered="false"
    :mask-closable="true"
    :closable="false"
    style="width: 640px; max-width: 92vw;"
    :segmented="{ content: false }"
    @update:show="(v: boolean) => emit('update:open', v)"
    @keydown="onKey"
  >
    <div class="palette">
      <div class="search">
        <span class="ico" aria-hidden="true">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.8">
            <circle cx="11" cy="11" r="7" />
            <path d="M20 20l-4-4" />
          </svg>
        </span>
        <n-input
          ref="inputRef"
          :value="query"
          placeholder="搜索工具、查历史、切环境…"
          :bordered="false"
          size="large"
          @update:value="onQuery"
        />
      </div>

      <div ref="listRef" class="list">
        <template v-if="results.length === 0">
          <div class="empty">无匹配结果</div>
        </template>
        <template v-else>
          <div v-for="(section, si) in sections" :key="si + '-' + section.label" class="section">
            <div class="section-label">{{ section.label }}</div>
            <button
              v-for="item in section.items"
              :key="item.kind + ':' + item.id"
              :data-idx="results.indexOf(item)"
              :class="['row', { active: results.indexOf(item) === activeIdx }]"
              type="button"
              @mouseenter="activeIdx = results.indexOf(item)"
              @click="activate(results.indexOf(item))"
            >
              <span class="title">{{ item.title }}</span>
              <span v-if="item.subtitle" class="sub">{{ item.subtitle }}</span>
            </button>
          </div>
        </template>
      </div>

      <div class="foot">
        <span><span class="k">↑↓</span> 选择</span>
        <span><span class="k">↩</span> 打开</span>
        <span><span class="k">esc</span> 关闭</span>
      </div>
    </div>
  </n-modal>
</template>

<style scoped>
.palette { display: flex; flex-direction: column; max-height: 60vh; }

.search {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 16px;
  border-bottom: 1px solid var(--line);
}
.search .ico { color: var(--text-muted); display: grid; place-items: center; }
.search :deep(.n-input) { --n-height: 32px; font-size: var(--fs-md); }

.list { flex: 1; overflow-y: auto; padding: 6px 0; min-height: 100px; }

.empty {
  padding: 32px;
  text-align: center;
  color: var(--text-muted);
  font-size: var(--fs-xs);
}

.section + .section { margin-top: 4px; }
.section-label {
  padding: 8px 16px 4px;
  font-size: 10.5px;
  font-weight: 600;
  color: var(--text-faint);
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.row {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  width: 100%;
  padding: 8px 16px;
  border: 0;
  background: transparent;
  color: var(--text);
  cursor: pointer;
  text-align: left;
  border-left: 2px solid transparent;
}
.row:hover, .row.active {
  background: var(--accent-soft);
  border-left-color: var(--accent);
}
.row .title { font-size: var(--fs-md); font-weight: 500; }
.row .sub   { font-size: var(--fs-xxs); color: var(--text-muted); }

.foot {
  display: flex;
  gap: 16px;
  padding: 8px 16px;
  border-top: 1px solid var(--line);
  font-size: var(--fs-xxs);
  color: var(--text-muted);
}
.foot .k {
  font-family: var(--font-mono);
  padding: 1px 5px;
  border-radius: var(--radius-sm);
  background: var(--bg-elev-2);
  border: 1px solid var(--line-strong);
  margin-right: 4px;
}
</style>
