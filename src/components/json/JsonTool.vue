<script setup lang="ts"></script>

<template>
  <div class="json-tool">
    <header class="json-tool__header">
      <h2>JSON 工具</h2>
      <p>格式化 / 查询 / 对比 / 转换 — 建设中</p>
    </header>
  </div>
</template>

<style scoped>
.json-tool { padding: 0; }
.json-tool__header h2 {
  margin: 0;
  font-size: var(--fs-xl);
  font-weight: 600;
  letter-spacing: -0.012em;
}
.json-tool__header p {
  margin: 4px 0 0;
  color: var(--text-muted);
  font-size: var(--fs-xs);
}
</style>
