import { ref, type Ref } from 'vue';

const pending = ref<string[]>([]);

export function useAria2Handoff() {
  return {
    push(url: string) {
      pending.value.push(url);
    },
    drainInto(target: Ref<string>) {
      if (!pending.value.length) return;
      const joined = pending.value.join('\n');
      target.value = target.value ? target.value + '\n' + joined : joined;
      pending.value = [];
    },
    // exported for tests only — resets the singleton
    _reset() {
      pending.value = [];
    },
    _size() {
      return pending.value.length;
    },
  };
}
