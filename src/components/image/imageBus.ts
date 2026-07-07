import { ref } from 'vue';

// Image path handed off between tabs — e.g. Capture → Annotate.
// Producer sets it; consumer reads then calls `consume()`.
const pendingAnnotate = ref<string | null>(null);

export function setPendingAnnotateImage(path: string) {
  pendingAnnotate.value = path;
}

export function usePendingAnnotateImage() {
  return {
    pending: pendingAnnotate,
    consume(): string | null {
      const v = pendingAnnotate.value;
      pendingAnnotate.value = null;
      return v;
    }
  };
}
