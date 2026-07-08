import { describe, expect, it } from 'vitest';
import {
  cronNextRuns,
  describeCron,
  formatDurationHuman,
  formatInZones,
  parseCron,
  parseDuration,
  parseUnixInput
} from './timeTools';

describe('parseUnixInput', () => {
  it('parses 10-digit seconds', () => {
    expect(parseUnixInput('1516239022')).toBe(1516239022_000);
  });
  it('parses 13-digit millis', () => {
    expect(parseUnixInput('1516239022123')).toBe(1516239022123);
  });
  it('parses 16-digit micros', () => {
    expect(parseUnixInput('1516239022123456')).toBe(1516239022123);
  });
  it('rejects non-numeric', () => {
    expect(parseUnixInput('abc')).toBeNull();
  });
});

describe('formatInZones', () => {
  it('formats UTC', () => {
    const [row] = formatInZones(1516239022_000, ['UTC']);
    expect(row.formatted).toContain('2018-01-18');
  });
});

describe('parseDuration', () => {
  it('parses hours and minutes', () => {
    expect(parseDuration('1h30m')).toBe(90 * 60_000);
  });
  it('parses complex mix', () => {
    expect(parseDuration('1天2小时30分钟')).toBe(86_400_000 + 2 * 3_600_000 + 30 * 60_000);
  });
  it('accepts bare numbers as ms', () => {
    expect(parseDuration('1500')).toBe(1500);
  });
  it('returns null for garbage', () => {
    expect(parseDuration('abc')).toBeNull();
  });
});

describe('formatDurationHuman', () => {
  it('formats compound', () => {
    expect(formatDurationHuman(90 * 60_000)).toContain('小时');
    expect(formatDurationHuman(90 * 60_000)).toContain('分钟');
  });
  it('zero', () => {
    expect(formatDurationHuman(0)).toBe('0 毫秒');
  });
  it('negative preserves sign', () => {
    expect(formatDurationHuman(-60_000).startsWith('-')).toBe(true);
  });
});

describe('parseCron', () => {
  it('parses "*/5 * * * *"', () => {
    const p = parseCron('*/5 * * * *');
    expect(p.minute).toEqual([0, 5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55]);
    expect(p.hour.length).toBe(24);
  });
  it('parses ranges + lists', () => {
    const p = parseCron('0,15,30,45 9-17 * * 1-5');
    expect(p.minute).toEqual([0, 15, 30, 45]);
    expect(p.hour).toEqual([9, 10, 11, 12, 13, 14, 15, 16, 17]);
    expect(p.dayOfWeek).toEqual([1, 2, 3, 4, 5]);
  });
  it('accepts 7 as Sunday', () => {
    const p = parseCron('0 9 * * 7');
    expect(p.dayOfWeek).toEqual([0]);
  });
  it('rejects wrong field count', () => {
    expect(() => parseCron('* * *')).toThrow();
  });
});

describe('cronNextRuns', () => {
  it('every minute enumerates 5 in a row', () => {
    const p = parseCron('* * * * *');
    const from = new Date('2026-01-01T00:00:00').getTime();
    const runs = cronNextRuns(p, from, 5);
    expect(runs.length).toBe(5);
    // Difference between consecutive runs is exactly 1 minute
    for (let i = 1; i < runs.length; i++) {
      expect(runs[i] - runs[i - 1]).toBe(60_000);
    }
  });
  it('daily 9am gives 3 firings 24h apart', () => {
    const p = parseCron('0 9 * * *');
    const from = new Date('2026-01-01T10:00:00').getTime();
    const runs = cronNextRuns(p, from, 3);
    expect(runs.length).toBe(3);
    for (let i = 1; i < runs.length; i++) {
      expect(runs[i] - runs[i - 1]).toBe(86_400_000);
    }
  });
});

describe('describeCron', () => {
  it('says every minute for full-range', () => {
    expect(describeCron(parseCron('* * * * *'))).toContain('每分');
  });
});
