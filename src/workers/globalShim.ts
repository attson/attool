if (typeof (globalThis as unknown as { global?: unknown }).global === 'undefined') {
  (globalThis as unknown as { global: typeof globalThis }).global = globalThis;
}
