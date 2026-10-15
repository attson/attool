export type CaptureRecordingState =
  | 'idle'
  | 'preparing-source'
  | 'selecting-region'
  | 'countdown'
  | 'recording'
  | 'paused'
  | 'stopping'
  | 'completed'
  | 'error';

export type CaptureRecordingEvent =
  | 'prepare'
  | 'source-ready'
  | 'source-cancelled'
  | 'start'
  | 'countdown-finished'
  | 'pause'
  | 'resume'
  | 'stop'
  | 'complete'
  | 'fail'
  | 'reset';

const transitions: Partial<Record<CaptureRecordingState, Partial<Record<CaptureRecordingEvent, CaptureRecordingState>>>> = {
  idle: { prepare: 'preparing-source', 'source-ready': 'selecting-region', fail: 'error' },
  'preparing-source': { 'source-ready': 'selecting-region', 'source-cancelled': 'idle', fail: 'error' },
  'selecting-region': { prepare: 'preparing-source', start: 'countdown', reset: 'idle', fail: 'error' },
  countdown: { 'countdown-finished': 'recording', reset: 'idle', fail: 'error' },
  recording: { pause: 'paused', stop: 'stopping', fail: 'error' },
  paused: { resume: 'recording', stop: 'stopping', fail: 'error' },
  stopping: { complete: 'completed', fail: 'error' },
  completed: { reset: 'idle', prepare: 'preparing-source' },
  error: { reset: 'idle', prepare: 'preparing-source' }
};

export function nextRecordingState(
  state: CaptureRecordingState,
  event: CaptureRecordingEvent
): CaptureRecordingState {
  const next = transitions[state]?.[event];
  if (!next) throw new Error(`${state} 状态不能执行 ${event}`);
  return next;
}

export function recordingShortcutAction(
  state: CaptureRecordingState
): 'open' | 'stop' | 'ignore' {
  if (state === 'recording' || state === 'paused') return 'stop';
  if (state === 'preparing-source' || state === 'countdown' || state === 'stopping') return 'ignore';
  return 'open';
}
