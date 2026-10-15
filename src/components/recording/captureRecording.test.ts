import { describe, expect, it } from 'vitest';
import { nextRecordingState, recordingShortcutAction } from './captureRecording';

describe('nextRecordingState', () => {
  it('walks through prepare, selection, countdown and recording controls', () => {
    expect(nextRecordingState('idle', 'prepare')).toBe('preparing-source');
    expect(nextRecordingState('preparing-source', 'source-ready')).toBe('selecting-region');
    expect(nextRecordingState('selecting-region', 'start')).toBe('countdown');
    expect(nextRecordingState('countdown', 'countdown-finished')).toBe('recording');
    expect(nextRecordingState('recording', 'pause')).toBe('paused');
    expect(nextRecordingState('paused', 'resume')).toBe('recording');
    expect(nextRecordingState('recording', 'stop')).toBe('stopping');
    expect(nextRecordingState('stopping', 'complete')).toBe('completed');
  });

  it('returns to idle after source cancellation and reset', () => {
    expect(nextRecordingState('preparing-source', 'source-cancelled')).toBe('idle');
    expect(nextRecordingState('error', 'reset')).toBe('idle');
  });

  it('supports selecting a region before requesting the system source', () => {
    expect(nextRecordingState('idle', 'source-ready')).toBe('selecting-region');
    expect(nextRecordingState('selecting-region', 'prepare')).toBe('preparing-source');
    expect(nextRecordingState('preparing-source', 'source-ready')).toBe('selecting-region');
  });

  it('rejects an event that is invalid for the current state', () => {
    expect(() => nextRecordingState('idle', 'pause')).toThrow('idle 状态不能执行 pause');
  });
});

describe('recordingShortcutAction', () => {
  it('stops an active recording and ignores transitional states', () => {
    expect(recordingShortcutAction('recording')).toBe('stop');
    expect(recordingShortcutAction('paused')).toBe('stop');
    expect(recordingShortcutAction('preparing-source')).toBe('ignore');
    expect(recordingShortcutAction('countdown')).toBe('ignore');
    expect(recordingShortcutAction('stopping')).toBe('ignore');
  });

  it('opens a new overlay from stable states', () => {
    expect(recordingShortcutAction('idle')).toBe('open');
    expect(recordingShortcutAction('completed')).toBe('open');
    expect(recordingShortcutAction('error')).toBe('open');
  });
});
