<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { NButton, NInput, NPopconfirm, NPopover } from 'naive-ui';
import { useAiChat } from './useAiChat';
import type { AiSession } from '../../types/ai';

const chat = useAiChat();

const query = ref('');
let debounceTimer: ReturnType<typeof setTimeout> | null = null;
function onSearchInput() {
  if (debounceTimer) clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => { chat.loadSessions(query.value.trim() || undefined); }, 200);
}

onMounted(() => { chat.loadSessions(); });

function ymd(ts: number): string {
  const d = new Date(ts);
  return `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;
}

function formatTime(ts: number): string {
  const d = new Date(ts);
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
}

interface Group { label: string; sessions: AiSession[] }

// Group by local calendar day relative to "now" so the boundary matches what
// a user visually expects from "今天/昨天", not a fixed 24h window.
const groups = computed<Group[]>(() => {
  const now = Date.now();
  const todayKey = ymd(now);
  const yesterdayKey = ymd(now - 86_400_000);
  const today: AiSession[] = [];
  const yesterday: AiSession[] = [];
  const older: AiSession[] = [];
  for (const s of chat.sessions.value) {
    const key = ymd(s.updatedAt);
    if (key === todayKey) today.push(s);
    else if (key === yesterdayKey) yesterday.push(s);
    else older.push(s);
  }
  const out: Group[] = [];
  if (today.length) out.push({ label: '今天', sessions: today });
  if (yesterday.length) out.push({ label: '昨天', sessions: yesterday });
  if (older.length) out.push({ label: '更早', sessions: older });
  return out;
});

const menuFor = ref<string | null>(null);
const renamingId = ref<string | null>(null);
const renameValue = ref('');

function startRename(s: AiSession) {
  renamingId.value = s.id;
  renameValue.value = s.title;
  menuFor.value = null;
}

async function commitRename() {
  const id = renamingId.value;
  if (!id) return;
  const title = renameValue.value.trim();
  renamingId.value = null;
  if (!title) return;
  await chat.renameSession(id, title);
}

async function removeSession(id: string) {
  menuFor.value = null;
  await chat.deleteSession(id);
}
</script>

<template>
  <div class="ai-session-list">
    <div class="head">
      <n-button size="tiny" secondary block @click="chat.newSession()">+ 新建会话</n-button>
      <n-input
        v-model:value="query"
        size="tiny"
        placeholder="搜索会话……"
        clearable
        @update:value="onSearchInput"
      />
    </div>

    <div class="list">
      <div v-if="chat.sessions.value.length === 0" class="empty">还没有会话 —— 点上方新建会话</div>
      <template v-for="group in groups" :key="group.label">
        <div class="group-label">{{ group.label }}</div>
        <div
          v-for="s in group.sessions"
          :key="s.id"
          class="row"
          :class="{ active: chat.currentSessionId.value === s.id }"
          @click="chat.openSession(s.id)"
        >
          <template v-if="renamingId === s.id">
            <n-input
              v-model:value="renameValue"
              size="tiny"
              autofocus
              @click.stop
              @keyup.enter="commitRename"
              @blur="commitRename"
            />
          </template>
          <template v-else>
            <span class="title">{{ s.title }}</span>
            <span class="time">{{ formatTime(s.updatedAt) }}</span>
            <n-popover :show="menuFor === s.id" trigger="manual" placement="bottom-end" @clickoutside="menuFor = null">
              <template #trigger>
                <button class="menu-btn" @click.stop="menuFor = menuFor === s.id ? null : s.id">⋯</button>
              </template>
              <div class="menu">
                <button @click.stop="startRename(s)">重命名</button>
                <n-popconfirm @positive-click="removeSession(s.id)">
                  <template #trigger>
                    <button class="danger" @click.stop>删除</button>
                  </template>
                  确认删除该会话？
                </n-popconfirm>
              </div>
            </n-popover>
          </template>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.ai-session-list {
  display: flex;
  flex-direction: column;
  height: 100%;
}
.head {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 8px;
  border-bottom: 1px solid var(--line);
}
.list { flex: 1; overflow-y: auto; padding: 4px 0; }
.empty { padding: 16px; text-align: center; color: var(--text-muted); font-size: var(--fs-xs); }
.group-label {
  padding: 6px 10px 2px;
  color: var(--text-muted);
  font-size: var(--fs-xxs);
}
.row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 10px;
  cursor: pointer;
  border-radius: var(--radius-sm);
  position: relative;
}
.row:hover { background: var(--bg-elev); }
.row.active { background: var(--accent-line); }
.title {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--fs-xs);
  color: var(--text);
}
.time { color: var(--text-faint); font-size: var(--fs-xxs); flex: none; }
.menu-btn {
  border: 0;
  background: transparent;
  color: var(--text-faint);
  cursor: pointer;
  padding: 2px 4px;
  border-radius: var(--radius-sm);
  flex: none;
  visibility: hidden;
}
.row:hover .menu-btn { visibility: visible; }
.menu-btn:hover { color: var(--text); background: var(--bg-elev-2); }
.menu {
  display: grid;
  gap: 2px;
  min-width: 100px;
}
.menu button {
  background: none;
  border: none;
  color: var(--text);
  padding: 4px 8px;
  text-align: left;
  font-size: var(--fs-xs);
  cursor: pointer;
  border-radius: var(--radius-sm);
}
.menu button:hover { background: var(--bg-elev-2); }
.menu button.danger { color: var(--error); }
</style>
