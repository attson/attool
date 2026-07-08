// Pure-JS time utilities: unix↔date, cross-timezone display, minimal
// cron parser + next-run enumerator, duration parse / format.

// ---------- unix ↔ date ----------

export function parseUnixInput(input: string): number | null {
  const s = input.trim();
  if (!s) return null;
  if (/^-?\d{1,19}$/.test(s)) {
    const n = Number(s);
    if (s.length <= 10) return n * 1000;
    if (s.length <= 13) return n;
    // 微秒或纳秒
    if (s.length <= 16) return Math.floor(n / 1000);
    return Math.floor(n / 1_000_000);
  }
  return null;
}

export interface TimezoneDisplay {
  zone: string;
  formatted: string;
  offset: string;
}

const DEFAULT_ZONES = [
  'Asia/Shanghai',
  'Asia/Tokyo',
  'Asia/Kolkata',
  'Europe/London',
  'Europe/Berlin',
  'America/New_York',
  'America/Los_Angeles',
  'UTC'
];

export function formatInZones(ms: number, zones: string[] = DEFAULT_ZONES): TimezoneDisplay[] {
  const out: TimezoneDisplay[] = [];
  for (const zone of zones) {
    try {
      const fmt = new Intl.DateTimeFormat('zh-CN', {
        timeZone: zone,
        year: 'numeric',
        month: '2-digit',
        day: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
        hour12: false,
        timeZoneName: 'short'
      });
      const parts = fmt.formatToParts(new Date(ms));
      const p = (t: string) => parts.find((x) => x.type === t)?.value ?? '';
      const y = p('year'), mo = p('month'), d = p('day');
      const h = p('hour'), mi = p('minute'), s = p('second');
      const zn = p('timeZoneName');
      out.push({
        zone,
        formatted: `${y}-${mo}-${d} ${h}:${mi}:${s}`,
        offset: zn
      });
    } catch {
      out.push({ zone, formatted: '<不支持的时区>', offset: '' });
    }
  }
  return out;
}

/** Compute Intl offset string like "GMT+08:00" for a given zone at a given instant. */
export function zoneOffset(ms: number, zone: string): string {
  try {
    const fmt = new Intl.DateTimeFormat('en-US', {
      timeZone: zone,
      timeZoneName: 'longOffset',
      hour: '2-digit'
    });
    const parts = fmt.formatToParts(new Date(ms));
    return parts.find((p) => p.type === 'timeZoneName')?.value ?? '';
  } catch {
    return '';
  }
}

// ---------- duration ----------

const DURATION_RE = /(-?\d+(?:\.\d+)?)\s*(ms|s|m|h|d|w|毫秒|秒|分钟?|小时|天|周)/gi;

const UNIT_MS: Record<string, number> = {
  ms: 1,
  毫秒: 1,
  s: 1000,
  秒: 1000,
  m: 60_000,
  分: 60_000,
  分钟: 60_000,
  h: 3_600_000,
  小时: 3_600_000,
  d: 86_400_000,
  天: 86_400_000,
  w: 604_800_000,
  周: 604_800_000
};

export function parseDuration(input: string): number | null {
  const s = input.trim();
  if (!s) return null;
  if (/^-?\d+$/.test(s)) return Number(s);
  let matched = false;
  let total = 0;
  s.replace(DURATION_RE, (_full, num: string, unit: string) => {
    matched = true;
    const key = unit.toLowerCase();
    const ms = UNIT_MS[key];
    if (ms !== undefined) total += Number(num) * ms;
    return '';
  });
  return matched ? total : null;
}

export function formatDurationHuman(ms: number): string {
  if (!Number.isFinite(ms)) return '';
  const sign = ms < 0 ? '-' : '';
  let rem = Math.abs(ms);
  const w = Math.floor(rem / 604_800_000); rem -= w * 604_800_000;
  const d = Math.floor(rem / 86_400_000); rem -= d * 86_400_000;
  const h = Math.floor(rem / 3_600_000); rem -= h * 3_600_000;
  const m = Math.floor(rem / 60_000); rem -= m * 60_000;
  const s = Math.floor(rem / 1000); rem -= s * 1000;
  const parts: string[] = [];
  if (w) parts.push(`${w} 周`);
  if (d) parts.push(`${d} 天`);
  if (h) parts.push(`${h} 小时`);
  if (m) parts.push(`${m} 分钟`);
  if (s) parts.push(`${s} 秒`);
  if (rem) parts.push(`${rem} 毫秒`);
  if (parts.length === 0) return '0 毫秒';
  return sign + parts.join(' ');
}

// ---------- cron ----------

export interface CronParts {
  minute: number[];
  hour: number[];
  dayOfMonth: number[];
  month: number[];
  dayOfWeek: number[];
}

function parseField(spec: string, min: number, max: number): number[] {
  const s = spec.trim();
  if (!s) throw new Error('空字段');
  const result = new Set<number>();
  for (const part of s.split(',')) {
    let step = 1;
    let range = part;
    if (range.includes('/')) {
      const [r, st] = range.split('/');
      step = Math.max(1, parseInt(st, 10));
      range = r;
    }
    let lo = min;
    let hi = max;
    if (range === '*') {
      // full range
    } else if (range.includes('-')) {
      const [a, b] = range.split('-');
      lo = parseInt(a, 10);
      hi = parseInt(b, 10);
    } else {
      lo = hi = parseInt(range, 10);
    }
    if (Number.isNaN(lo) || Number.isNaN(hi) || lo < min || hi > max || lo > hi) {
      throw new Error(`字段范围非法：${part}（应在 ${min}..${max}）`);
    }
    for (let v = lo; v <= hi; v += step) result.add(v);
  }
  return [...result].sort((a, b) => a - b);
}

export function parseCron(expr: string): CronParts {
  const fields = expr.trim().split(/\s+/);
  if (fields.length !== 5) {
    throw new Error(`Cron 应有 5 段（分 时 日 月 周），当前 ${fields.length}`);
  }
  const [mi, hr, dom, mo, dow] = fields;
  return {
    minute: parseField(mi, 0, 59),
    hour: parseField(hr, 0, 23),
    dayOfMonth: parseField(dom, 1, 31),
    month: parseField(mo, 1, 12),
    // Accept 0-6 (Sun=0) and 7 → 0
    dayOfWeek: parseField(dow.replace(/\b7\b/g, '0'), 0, 6)
  };
}

/**
 * Enumerate the next `count` firings of `parts` starting from `from`.
 * Brute-force scan, minute by minute, hard-capped at 366 days ahead.
 */
export function cronNextRuns(parts: CronParts, from: number, count: number): number[] {
  const out: number[] = [];
  const start = new Date(from);
  start.setSeconds(0, 0);
  start.setMinutes(start.getMinutes() + 1); // strictly next
  const maxMs = 366 * 24 * 60 * 60 * 1000;
  const end = from + maxMs;
  const cur = start;
  while (cur.getTime() <= end && out.length < count) {
    if (
      parts.minute.includes(cur.getMinutes()) &&
      parts.hour.includes(cur.getHours()) &&
      parts.dayOfMonth.includes(cur.getDate()) &&
      parts.month.includes(cur.getMonth() + 1) &&
      parts.dayOfWeek.includes(cur.getDay())
    ) {
      out.push(cur.getTime());
    }
    cur.setMinutes(cur.getMinutes() + 1);
  }
  return out;
}

export function describeCron(parts: CronParts): string {
  const desc = (arr: number[], min: number, max: number, label: string) => {
    if (arr.length === max - min + 1) return `每${label}`;
    if (arr.length === 1) return `第 ${arr[0]} ${label}`;
    return `${label} ${arr.slice(0, 8).join('/')}${arr.length > 8 ? '…' : ''}`;
  };
  return [
    desc(parts.minute, 0, 59, '分'),
    desc(parts.hour, 0, 23, '时'),
    desc(parts.dayOfMonth, 1, 31, '日'),
    desc(parts.month, 1, 12, '月'),
    desc(parts.dayOfWeek, 0, 6, '周')
  ].join('，');
}
