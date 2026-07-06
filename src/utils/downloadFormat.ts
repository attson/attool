export function formatClock(dt: string | null | undefined): string {
  if (!dt) return '';
  // sqlite datetime('now','localtime') 返回 'YYYY-MM-DD HH:MM:SS'
  const m = dt.match(/(\d{2}:\d{2}:\d{2})/);
  return m ? m[1] : dt;
}

export function computeDurationSeconds(
  started: string | null | undefined,
  finished: string | null | undefined
): number | null {
  if (!started || !finished) return null;
  const s = Date.parse(started.replace(' ', 'T'));
  const f = Date.parse(finished.replace(' ', 'T'));
  if (Number.isNaN(s) || Number.isNaN(f)) return null;
  const diff = Math.round((f - s) / 1000);
  return diff >= 0 ? diff : null;
}

export function formatDurationSeconds(n: number | null | undefined): string {
  if (n == null || !Number.isFinite(n) || n < 0) return '';
  const total = Math.floor(n);
  if (total < 60) return `${total}s`;
  if (total < 3600) {
    const m = Math.floor(total / 60);
    const s = total % 60;
    return s === 0 ? `${m}m` : `${m}m ${s}s`;
  }
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  return m === 0 ? `${h}h` : `${h}h ${m}m`;
}
