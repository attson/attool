const DEFAULT_MAX_BYTES = 10 * 1024 * 1024;

export interface FileDropOptions {
  accept?: string[];
  maxBytes?: number;
  onError?: (message: string) => void;
}

export interface FileDropHandlers {
  onDrop: (event: DragEvent) => Promise<void>;
  onDragOver: (event: DragEvent) => void;
  openFile: () => Promise<void>;
}

export function useFileDrop(
  onContent: (text: string, filename: string) => void,
  options: FileDropOptions = {},
): FileDropHandlers {
  const { accept, maxBytes = DEFAULT_MAX_BYTES, onError } = options;

  async function handleFile(file: File) {
    if (!acceptsExtension(file.name, accept)) {
      onError?.(`不支持的文件类型：${file.name}`);
      return;
    }
    try {
      const text = await decodeFile(file, { maxBytes });
      onContent(text, file.name);
    } catch (error) {
      onError?.(error instanceof Error ? error.message : String(error));
    }
  }

  async function onDrop(event: DragEvent) {
    event.preventDefault();
    const file = event.dataTransfer?.files?.[0];
    if (file) await handleFile(file);
  }

  function onDragOver(event: DragEvent) {
    event.preventDefault();
  }

  async function openFile() {
    const input = document.createElement('input');
    input.type = 'file';
    if (accept && accept.length > 0) {
      input.accept = accept.map((ext) => `.${ext}`).join(',');
    }
    const picked = await new Promise<File | null>((resolve) => {
      input.onchange = () => resolve(input.files?.[0] ?? null);
      input.click();
    });
    if (picked) await handleFile(picked);
  }

  return { onDrop, onDragOver, openFile };
}

export function acceptsExtension(filename: string, accept: string[] | undefined): boolean {
  if (!accept || accept.length === 0) return true;
  const dot = filename.lastIndexOf('.');
  if (dot < 0) return false;
  const ext = filename.slice(dot + 1).toLowerCase();
  return accept.map((e) => e.toLowerCase()).includes(ext);
}

export async function decodeFile(file: File, options: { maxBytes?: number } = {}): Promise<string> {
  const maxBytes = options.maxBytes ?? DEFAULT_MAX_BYTES;
  if (file.size > maxBytes) {
    throw new Error(`文件超出 ${(maxBytes / 1024 / 1024).toFixed(0)} MB 上限`);
  }
  return file.text();
}
