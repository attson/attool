export type CaptureOcrPhase = 'idle' | 'recognizing' | 'success' | 'empty' | 'error';

export interface CaptureOcrState {
  phase: CaptureOcrPhase;
  text: string;
  error: string;
  requestId: number;
}

export type CaptureOcrEvent =
  | { type: 'start' }
  | { type: 'resolve'; requestId: number; text: string }
  | { type: 'fail'; requestId: number; error: string }
  | { type: 'close' }
  | { type: 'reset' };

export function captureOcrAllowsGlobalShortcut(phase: CaptureOcrPhase, key: string): boolean {
  return phase === 'idle' || key === 'Escape';
}

export function initialCaptureOcrState(): CaptureOcrState {
  return { phase: 'idle', text: '', error: '', requestId: 0 };
}

export function nextCaptureOcrState(
  state: CaptureOcrState,
  event: CaptureOcrEvent
): CaptureOcrState {
  if (event.type === 'start') {
    return {
      phase: 'recognizing',
      text: state.text,
      error: '',
      requestId: state.requestId + 1
    };
  }
  if (event.type === 'resolve') {
    if (event.requestId !== state.requestId || state.phase !== 'recognizing') return state;
    if (!event.text.trim()) return { ...state, phase: 'empty', error: '' };
    return { ...state, phase: 'success', text: event.text, error: '' };
  }
  if (event.type === 'fail') {
    if (event.requestId !== state.requestId || state.phase !== 'recognizing') return state;
    return { ...state, phase: 'error', error: event.error };
  }
  if (event.type === 'reset') {
    return { phase: 'idle', text: '', error: '', requestId: state.requestId + 1 };
  }
  return { ...state, phase: 'idle', error: '' };
}
