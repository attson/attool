import { describe, expect, it } from 'vitest';
import { computeDurationSeconds, formatClock, formatDurationSeconds } from './downloadFormat';

describe('formatClock', () => {
  it('returns HH:MM:SS from sqlite datetime string', () => {
    expect(formatClock('2026-07-07 14:23:05')).toBe('14:23:05');
  });

  it('returns empty string for null / undefined / empty', () => {
    expect(formatClock(null)).toBe('');
    expect(formatClock(undefined)).toBe('');
    expect(formatClock('')).toBe('');
  });

  it('returns the raw input when no HH:MM:SS pattern matches', () => {
    expect(formatClock('notatime')).toBe('notatime');
  });
});

describe('computeDurationSeconds', () => {
  it('returns positive second delta for valid sqlite timestamps', () => {
    expect(
      computeDurationSeconds('2026-07-07 14:23:05', '2026-07-07 14:23:47')
    ).toBe(42);
  });

  it('returns null when either side is missing', () => {
    expect(computeDurationSeconds(null, '2026-07-07 14:23:47')).toBeNull();
    expect(computeDurationSeconds('2026-07-07 14:23:05', undefined)).toBeNull();
  });

  it('returns null for negative diffs (clock skew)', () => {
    expect(
      computeDurationSeconds('2026-07-07 14:23:47', '2026-07-07 14:23:05')
    ).toBeNull();
  });

  it('returns null when timestamps cannot be parsed', () => {
    expect(computeDurationSeconds('garbage', '2026-07-07 14:23:47')).toBeNull();
  });
});

describe('formatDurationSeconds', () => {
  it('formats sub-minute values as Ns', () => {
    expect(formatDurationSeconds(0)).toBe('0s');
    expect(formatDurationSeconds(1)).toBe('1s');
    expect(formatDurationSeconds(59)).toBe('59s');
  });

  it('formats sub-hour values as Nm or Nm Ns', () => {
    expect(formatDurationSeconds(60)).toBe('1m');
    expect(formatDurationSeconds(125)).toBe('2m 5s');
    expect(formatDurationSeconds(3599)).toBe('59m 59s');
  });

  it('formats multi-hour values as Nh or Nh Nm', () => {
    expect(formatDurationSeconds(3600)).toBe('1h');
    expect(formatDurationSeconds(3900)).toBe('1h 5m');
    expect(formatDurationSeconds(7300)).toBe('2h 1m');
  });

  it('returns empty string for null / negative / NaN', () => {
    expect(formatDurationSeconds(null)).toBe('');
    expect(formatDurationSeconds(undefined)).toBe('');
    expect(formatDurationSeconds(-1)).toBe('');
    expect(formatDurationSeconds(Number.NaN)).toBe('');
  });
});
