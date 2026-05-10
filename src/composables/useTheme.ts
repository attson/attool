import { ref } from 'vue';
import type { KVStorage } from './useSidebarState';

export type ThemeName = 'dark' | 'light';

export interface ThemeRoot {
  setAttribute(name: string, value: string): void;
}

const STORAGE_KEY = 'attool.theme';

export function useTheme(
  storage: KVStorage = localStorage,
  root: ThemeRoot = document.documentElement
) {
  const stored = storage.getItem(STORAGE_KEY);
  const initial: ThemeName = stored === 'light' ? 'light' : 'dark';
  const theme = ref<ThemeName>(initial);

  function apply(value: ThemeName) {
    root.setAttribute('data-theme', value);
  }
  apply(initial);

  function toggle() {
    theme.value = theme.value === 'dark' ? 'light' : 'dark';
    storage.setItem(STORAGE_KEY, theme.value);
    apply(theme.value);
  }

  return { theme, toggle };
}
