<script setup lang="ts">
// 无渲染组件：app 启动后查询后端快捷键注册结果，
// 若有失败(如快捷键被系统占用)则弹 toast 提示。
// 必须放在 NMessageProvider 内部才能拿到 useMessage()。
import { onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { useMessage } from 'naive-ui';

interface ShortcutRegisterErrors {
  clipboard: string | null;
  capture: string | null;
}

const message = useMessage();

onMounted(async () => {
  let errors: ShortcutRegisterErrors;
  try {
    errors = await invoke<ShortcutRegisterErrors>('get_shortcut_register_errors');
  } catch {
    // 非 tauri 环境(纯浏览器调试)或 command 缺失时静默跳过
    return;
  }
  const failed = [errors.clipboard, errors.capture].filter(
    (msg): msg is string => !!msg
  );
  for (const msg of failed) {
    message.warning(`${msg}（该快捷键已被占用，常见占用者：系统/桌面环境、输入法、远程桌面等；可在对应工具页更换）`, {
      duration: 8000,
      closable: true,
    });
  }
});
</script>

<template>
  <span style="display: none" />
</template>
