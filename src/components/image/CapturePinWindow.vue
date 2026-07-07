<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { save as saveDialog } from '@tauri-apps/plugin-dialog';
import { getCurrentWindow } from '@tauri-apps/api/window';

const currentWindow = getCurrentWindow();

const imagePath = ref('');
const menuOpen = ref(false);
const menuPos = ref({ x: 0, y: 0 });

const imageSrc = computed(() => (imagePath.value ? convertFileSrc(imagePath.value) : ''));

function readPinPathFromUrl(): string {
  const params = new URLSearchParams(window.location.search);
  const raw = params.get('pin');
  return raw ? decodeURIComponent(raw) : '';
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    close();
  }
}

async function close() {
  try {
    await currentWindow.close();
  } catch (err) {
    console.error('[pin] close failed', err);
  }
}

function openMenu(event: MouseEvent) {
  event.preventDefault();
  menuPos.value = { x: event.clientX, y: event.clientY };
  menuOpen.value = true;
}

function closeMenu() {
  menuOpen.value = false;
}

async function copyToClipboard() {
  closeMenu();
  if (!imagePath.value) return;
  await invoke('copy_pin_image', { path: imagePath.value }).catch((err) => {
    console.warn('[pin] copy failed', err);
  });
}

async function saveAs() {
  closeMenu();
  if (!imagePath.value) return;
  const ts = new Date().toISOString().replace(/[:.]/g, '-');
  const target = await saveDialog({
    defaultPath: `attool-pin-${ts}.png`,
    filters: [{ name: 'PNG', extensions: ['png'] }]
  }).catch(() => null);
  if (!target) return;
  await invoke('copy_pin_to', { source: imagePath.value, target }).catch((err) => {
    console.warn('[pin] save-as failed', err);
  });
}

onMounted(() => {
  imagePath.value = readPinPathFromUrl();
  window.addEventListener('keydown', onKeydown);
  window.addEventListener('click', closeMenu);
});

onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown);
  window.removeEventListener('click', closeMenu);
});
</script>

<template>
  <div
    class="pin-root"
    data-tauri-drag-region
    @contextmenu.prevent="openMenu"
    @dblclick="close"
  >
    <img
      v-if="imageSrc"
      :src="imageSrc"
      draggable="false"
      class="pin-image"
    />

    <div
      v-if="menuOpen"
      class="pin-menu"
      :style="{ left: menuPos.x + 'px', top: menuPos.y + 'px' }"
      @click.stop
    >
      <button @click="copyToClipboard">复制到剪贴板</button>
      <button @click="saveAs">另存为...</button>
      <div class="divider"></div>
      <button @click="close">关闭</button>
    </div>
  </div>
</template>

<style scoped>
.pin-root {
  position: fixed;
  inset: 0;
  overflow: hidden;
  background: transparent;
  cursor: grab;
  user-select: none;
}
.pin-root:active { cursor: grabbing; }
.pin-image {
  display: block;
  width: 100%;
  height: 100%;
  -webkit-user-drag: none;
  border-radius: 4px;
  box-shadow: 0 4px 24px rgba(0, 0, 0, 0.35);
  /* Events fall through to .pin-root so data-tauri-drag-region + right-click work over the image */
  pointer-events: none;
}

.pin-menu {
  position: fixed;
  min-width: 160px;
  padding: 4px;
  background: rgba(30, 30, 30, 0.98);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 6px;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4);
  z-index: 100;
  display: grid;
  gap: 2px;
}
.pin-menu button {
  padding: 6px 10px;
  border: none;
  background: transparent;
  color: #fff;
  text-align: left;
  font-size: 13px;
  cursor: pointer;
  border-radius: 4px;
}
.pin-menu button:hover { background: rgba(255, 255, 255, 0.12); }
.pin-menu .divider {
  height: 1px;
  background: rgba(255, 255, 255, 0.1);
  margin: 4px 6px;
}
</style>
