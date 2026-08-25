<script setup lang="ts">
import { computed, ref } from 'vue';
import { NSwitch } from 'naive-ui';
import CodeEditor from './CodeEditor.vue';
import JsonDiffViewer from './JsonDiffViewer.vue';
import { useJsonWorker } from '../../composables/useJsonWorker';

const worker = useJsonWorker();

const left = ref('');
const right = ref('');
const leftError = ref<string | null>(null);
const rightError = ref<string | null>(null);
const viewMode = ref<'input' | 'result'>('input');
const comparing = ref(false);
const equal = ref(false);
const changeCount = ref(0);
const elapsedMs = ref(0);
const resultLeft = ref('');
const resultRight = ref('');
const keyOnly = ref(false);
const parseNestedJsonStrings = ref(false);
let compareRun = 0;

async function compare() {
  if (!left.value.trim() && !right.value.trim()) {
    return;
  }
  comparing.value = true;
  const run = ++compareRun;
  leftError.value = null;
  rightError.value = null;
  const res = await worker.diff(
    left.value || '{}',
    right.value || '{}',
    false,
    {
      keyOnly: keyOnly.value,
      parseNestedJsonStrings: parseNestedJsonStrings.value,
    },
    'diff:compare',
  );
  if (run !== compareRun) return;
  comparing.value = false;
  if (res === null) return;
  leftError.value = res.leftError ?? null;
  rightError.value = res.rightError ?? null;
  if (res.leftError || res.rightError) {
    equal.value = false;
    return;
  }
  equal.value = res.equal;
  changeCount.value = res.changeCount;
  elapsedMs.value = res.elapsedMs;
  resultLeft.value = res.leftText;
  resultRight.value = res.rightText;
  viewMode.value = 'result';
}

function backToInput() {
  compareRun++;
  comparing.value = false;
  viewMode.value = 'input';
}

function setKeyOnly(value: boolean) {
  keyOnly.value = value;
  refreshResult();
}

function setParseNestedJsonStrings(value: boolean) {
  parseNestedJsonStrings.value = value;
  refreshResult();
}

function refreshResult() {
  if (viewMode.value === 'result') void compare();
}

const status = computed(() => {
  if (leftError.value) return `左侧 JSON 解析失败：${leftError.value}`;
  if (rightError.value) return `右侧 JSON 解析失败：${rightError.value}`;
  if (!left.value.trim() && !right.value.trim()) return '粘贴左右两份 JSON 进行对比';
  return '点开始对比生成只读 diff 视图';
});

const resultStatus = computed(() => {
  if (comparing.value) return '重新对比中...';
  if (keyOnly.value) {
    if (equal.value) return `两侧 Key 结构等价 · ${elapsedMs.value} ms`;
    return `发现 ${changeCount.value} 处 Key 差异 · ${elapsedMs.value} ms`;
  }
  if (equal.value) return `两侧内容等价 · ${elapsedMs.value} ms`;
  return `发现 ${changeCount.value} 处差异 · ${elapsedMs.value} ms`;
});
</script>

<template>
  <div class="diff-pane">
    <div v-if="viewMode === 'input'" class="input-mode">
      <div class="split">
        <CodeEditor :model-value="left" language="json" @update:model-value="(v) => left = v" height="100%" />
        <CodeEditor :model-value="right" language="json" @update:model-value="(v) => right = v" height="100%" />
      </div>
      <div class="footer">
        <span class="status">{{ status }}</span>
        <div class="footer-actions">
          <div class="diff-options">
            <label class="diff-option">
              <span>只看 Key</span>
              <n-switch size="small" :value="keyOnly" @update:value="setKeyOnly" />
            </label>
            <label class="diff-option">
              <span>解析嵌套 JSON 字符串</span>
              <n-switch
                size="small"
                :value="parseNestedJsonStrings"
                @update:value="setParseNestedJsonStrings"
              />
            </label>
          </div>
          <button
            type="button"
            class="primary-btn"
            :disabled="comparing || (!left.trim() && !right.trim())"
            @click="compare"
          >
            {{ comparing ? '对比中...' : '开始对比' }}
          </button>
        </div>
      </div>
    </div>

    <div v-else class="result-mode">
      <div class="toolbar">
        <div class="result-title">
          <strong>对比结果</strong>
          <span>{{ resultStatus }}</span>
        </div>
        <div class="diff-options">
          <label class="diff-option">
            <span>只看 Key</span>
            <n-switch size="small" :value="keyOnly" @update:value="setKeyOnly" />
          </label>
          <label class="diff-option">
            <span>解析嵌套 JSON 字符串</span>
            <n-switch
              size="small"
              :value="parseNestedJsonStrings"
              @update:value="setParseNestedJsonStrings"
            />
          </label>
        </div>
        <button type="button" class="secondary-btn" @click="backToInput">重新编辑</button>
      </div>
      <JsonDiffViewer :original="resultLeft" :modified="resultRight" height="100%" />
    </div>
  </div>
</template>

<style scoped>
.diff-pane { height: 100%; min-height: 0; }
.input-mode,
.result-mode { height: 100%; min-height: 0; display: grid; gap: 8px; }
.input-mode { grid-template-rows: 1fr auto; }
.result-mode { grid-template-rows: auto 1fr; }
.split { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; min-height: 0; }
.footer,
.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}
.footer { min-height: 34px; }
.toolbar {
  min-height: 36px;
  padding: 0 2px;
}
.status,
.result-title span { color: var(--text-muted); font-size: var(--fs-xs); }
.status { flex: 1 1 180px; }
.footer-actions,
.diff-options,
.diff-option {
  display: flex;
  align-items: center;
}
.footer-actions {
  gap: 16px;
  flex-wrap: wrap;
  justify-content: flex-end;
}
.diff-options {
  gap: 14px;
  flex-wrap: wrap;
}
.diff-option {
  gap: 6px;
  color: var(--text-muted);
  font-size: var(--fs-xs);
  white-space: nowrap;
}
.result-title {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}
.result-title strong {
  color: var(--text);
  font-size: var(--fs-md);
}
.primary-btn,
.secondary-btn {
  height: 30px;
  border-radius: var(--radius-sm);
  padding: 0 12px;
  cursor: pointer;
  font-size: var(--fs-xs);
}
.primary-btn {
  color: var(--accent-fg);
  background: var(--accent);
  border: 1px solid var(--accent);
}
.primary-btn:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}
.secondary-btn {
  background: var(--bg-elev-2);
  border: 1px solid var(--line);
  color: var(--text);
}
</style>
