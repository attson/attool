import { ref } from 'vue';

// Image path handed off between tabs — e.g. Capture → Annotate.
const pendingAnnotate = ref<string | null>(null);
// A tab that ImageTool should switch to when it mounts / receives the signal.
const requestedTab = ref<string | null>(null);

export function setPendingAnnotateImage(path: string) {
  pendingAnnotate.value = path;
}

export function requestImageTab(name: string) {
  requestedTab.value = name;
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

export function useRequestedTab() {
  return {
    requested: requestedTab,
    consume(): string | null {
      const v = requestedTab.value;
      requestedTab.value = null;
      return v;
    }
  };
}
