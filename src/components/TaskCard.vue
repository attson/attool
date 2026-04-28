<script setup lang="ts">
import { computed } from 'vue';
import {
  NButton,
  NCard,
  NEllipsis,
  NFlex,
  NProgress,
  NSpace,
  NTag,
  NText
} from 'naive-ui';
import type { DownloadStatus, DownloadTask } from '../types/download';

const statusText: Record<DownloadStatus, string> = {
  queued: '排队中',
  running: '下载中',
  completed: '已完成',
  failed: '失败',
  cancelled: '已取消'
};

const statusType: Record<DownloadStatus, 'default' | 'info' | 'success' | 'error' | 'warning'> = {
  queued: 'default',
  running: 'info',
  completed: 'success',
  failed: 'error',
  cancelled: 'warning'
};

const props = defineProps<{
  task: DownloadTask;
}>();

const emit = defineEmits<{
  cancel: [id: string];
}>();

const cancellable = computed(() => props.task.status === 'queued' || props.task.status === 'running');
const progressValue = computed(() => Math.round(Math.min(Math.max(props.task.progress, 0), 100)));
</script>

<template>
  <n-card class="task-card" size="small" :bordered="false">
    <n-space vertical :size="10">
      <n-flex justify="space-between" align="center" :wrap="false">
        <n-text depth="3">{{ task.createdAt }}</n-text>
        <n-tag round size="small" :type="statusType[task.status]">
          {{ statusText[task.status] }}
        </n-tag>
      </n-flex>

      <div>
        <n-ellipsis class="task-title" :tooltip="false">
          {{ task.fileName || task.url }}
        </n-ellipsis>
        <n-ellipsis v-if="task.fileName" class="task-url" :tooltip="false">
          {{ task.url }}
        </n-ellipsis>
      </div>

      <n-progress
        type="line"
        status="success"
        :percentage="progressValue"
        :height="8"
        :border-radius="8"
        :fill-border-radius="8"
      />

      <n-flex class="task-meta" :size="10">
        <n-text depth="3">{{ progressValue }}%</n-text>
        <n-text depth="3">{{ task.speed ? `速度 ${task.speed}` : '等待速度' }}</n-text>
        <n-text depth="3">{{ task.eta ? `ETA ${task.eta}` : 'ETA --' }}</n-text>
      </n-flex>

      <n-text v-if="task.message" class="task-message" depth="3">
        {{ task.message }}
      </n-text>

      <n-button v-if="cancellable" size="small" tertiary type="warning" @click="emit('cancel', task.id)">
        取消任务
      </n-button>
    </n-space>
  </n-card>
</template>
