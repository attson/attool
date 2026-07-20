import type { InjectionKey, Ref } from 'vue';

export type JsonExpandCommand = { action: 'expand' | 'collapse'; v: number };

export const expandCommandKey: InjectionKey<Ref<JsonExpandCommand | null>> =
  Symbol('jsonExpandCommand');
