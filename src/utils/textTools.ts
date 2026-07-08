// Pure text-processing helpers. No DOM / no filesystem — safe to unit test.

// ---------- clean ----------

export interface CleanOptions {
  dedup: boolean;
  dropEmpty: boolean;
  trimEachLine: boolean;
  collapseSpaces: boolean;
  keepOrder: boolean;
}

export function cleanText(input: string, opts: CleanOptions): string {
  let lines = input.split(/\r?\n/);
  if (opts.trimEachLine) lines = lines.map((l) => l.trim());
  if (opts.collapseSpaces) lines = lines.map((l) => l.replace(/[ \t]+/g, ' '));
  if (opts.dropEmpty) lines = lines.filter((l) => l.length > 0);
  if (opts.dedup) {
    const seen = new Set<string>();
    const out: string[] = [];
    for (const l of lines) {
      if (!seen.has(l)) {
        seen.add(l);
        out.push(l);
      }
    }
    lines = out;
  }
  if (!opts.keepOrder) {
    // Legacy path — currently unused but reserved for future "sort while cleaning"
  }
  return lines.join('\n');
}

// ---------- sort ----------

export type SortMode = 'asc' | 'desc' | 'natural' | 'length-asc' | 'length-desc' | 'reverse' | 'shuffle';

export function sortLines(input: string, mode: SortMode): string {
  const lines = input.split(/\r?\n/);
  const sorted = [...lines];
  switch (mode) {
    case 'asc':
      sorted.sort((a, b) => a.localeCompare(b));
      break;
    case 'desc':
      sorted.sort((a, b) => b.localeCompare(a));
      break;
    case 'natural':
      sorted.sort((a, b) => a.localeCompare(b, undefined, { numeric: true, sensitivity: 'base' }));
      break;
    case 'length-asc':
      sorted.sort((a, b) => a.length - b.length);
      break;
    case 'length-desc':
      sorted.sort((a, b) => b.length - a.length);
      break;
    case 'reverse':
      sorted.reverse();
      break;
    case 'shuffle':
      for (let i = sorted.length - 1; i > 0; i--) {
        const j = Math.floor(Math.random() * (i + 1));
        [sorted[i], sorted[j]] = [sorted[j], sorted[i]];
      }
      break;
  }
  return sorted.join('\n');
}

// ---------- case ----------

export type CaseMode =
  | 'upper'
  | 'lower'
  | 'title'
  | 'sentence'
  | 'camel'
  | 'pascal'
  | 'snake'
  | 'kebab'
  | 'constant'
  | 'swap';

export function changeCase(input: string, mode: CaseMode): string {
  switch (mode) {
    case 'upper':
      return input.toUpperCase();
    case 'lower':
      return input.toLowerCase();
    case 'title':
      return input.replace(/\b([a-z])/g, (m) => m.toUpperCase());
    case 'sentence':
      return input.replace(/(^|[.!?]\s+)([a-z])/g, (_, p, c) => p + c.toUpperCase());
    case 'swap':
      return input.replace(/[a-zA-Z]/g, (ch) =>
        ch === ch.toLowerCase() ? ch.toUpperCase() : ch.toLowerCase()
      );
    case 'camel':
      return toCamel(input, false);
    case 'pascal':
      return toCamel(input, true);
    case 'snake':
      return splitWords(input).join('_').toLowerCase();
    case 'kebab':
      return splitWords(input).join('-').toLowerCase();
    case 'constant':
      return splitWords(input).join('_').toUpperCase();
  }
}

function splitWords(input: string): string[] {
  return input
    // insert boundary between lowercase→UPPER (fooBar → foo Bar)
    .replace(/([a-z])([A-Z])/g, '$1 $2')
    // insert boundary between consecutive UPPER + Lower (FOOBar → FOO Bar)
    .replace(/([A-Z]+)([A-Z][a-z])/g, '$1 $2')
    // split on non-alphanumeric
    .split(/[^A-Za-z0-9]+/)
    .filter(Boolean);
}

function toCamel(input: string, pascal: boolean): string {
  const words = splitWords(input).map((w) => w.toLowerCase());
  const parts = words.map((w, i) => {
    if (i === 0 && !pascal) return w;
    return w.charAt(0).toUpperCase() + w.slice(1);
  });
  return parts.join('');
}

// ---------- split / join ----------

export function splitBy(input: string, delimiter: string): string {
  if (!delimiter) return input;
  return input.split(delimiter).join('\n');
}

export function joinWith(input: string, delimiter: string): string {
  return input.split(/\r?\n/).join(delimiter);
}

// ---------- extract ----------

export const BUILTIN_PATTERNS: Record<string, RegExp> = {
  URL: /https?:\/\/[^\s"'<>]+/gi,
  Email: /[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}/g,
  数字: /-?\d+(?:\.\d+)?/g,
  中文: /[一-龥]+/g,
  手机号: /1[3-9]\d{9}/g,
  IPv4: /\b(?:(?:25[0-5]|2[0-4]\d|[01]?\d?\d)\.){3}(?:25[0-5]|2[0-4]\d|[01]?\d?\d)\b/g,
  Hex颜色: /#[0-9a-fA-F]{6}\b|#[0-9a-fA-F]{3}\b/g,
  日期: /\d{4}[-/年]\d{1,2}[-/月]\d{1,2}日?/g
};

export function extractMatches(input: string, pattern: RegExp, dedup = false): string[] {
  const matches = input.match(pattern) ?? [];
  if (!dedup) return matches;
  return [...new Set(matches)];
}

// ---------- stats ----------

export interface TextStats {
  chars: number;
  charsNoSpace: number;
  words: number;
  lines: number;
  bytes: number;
  chinese: number;
  ascii: number;
}

// ---------- line diff (LCS backtrack) ----------

export type DiffLineType = 'equal' | 'add' | 'remove';

export interface DiffLine {
  type: DiffLineType;
  text: string;
  /** 1-based line number in the left (original) side, when applicable. */
  lineNumA?: number;
  /** 1-based line number in the right (new) side, when applicable. */
  lineNumB?: number;
}

export function lineDiff(a: string, b: string): DiffLine[] {
  const aLines = a.split(/\r?\n/);
  const bLines = b.split(/\r?\n/);
  const m = aLines.length;
  const n = bLines.length;

  // dp[i][j] = LCS length of aLines[0..i) and bLines[0..j)
  const dp: number[][] = Array.from({ length: m + 1 }, () => new Array(n + 1).fill(0));
  for (let i = 1; i <= m; i++) {
    for (let j = 1; j <= n; j++) {
      if (aLines[i - 1] === bLines[j - 1]) dp[i][j] = dp[i - 1][j - 1] + 1;
      else dp[i][j] = Math.max(dp[i - 1][j], dp[i][j - 1]);
    }
  }

  const result: DiffLine[] = [];
  let i = m;
  let j = n;
  while (i > 0 || j > 0) {
    if (i > 0 && j > 0 && aLines[i - 1] === bLines[j - 1]) {
      result.push({ type: 'equal', text: aLines[i - 1], lineNumA: i, lineNumB: j });
      i--;
      j--;
    } else if (j > 0 && (i === 0 || dp[i][j - 1] >= dp[i - 1][j])) {
      result.push({ type: 'add', text: bLines[j - 1], lineNumB: j });
      j--;
    } else if (i > 0) {
      result.push({ type: 'remove', text: aLines[i - 1], lineNumA: i });
      i--;
    }
  }
  result.reverse();
  return result;
}

export interface DiffSummary {
  added: number;
  removed: number;
  equal: number;
  identical: boolean;
}

export function diffSummary(lines: DiffLine[]): DiffSummary {
  let added = 0;
  let removed = 0;
  let equal = 0;
  for (const l of lines) {
    if (l.type === 'add') added++;
    else if (l.type === 'remove') removed++;
    else equal++;
  }
  return { added, removed, equal, identical: added === 0 && removed === 0 };
}

export function computeStats(input: string): TextStats {
  const bytes = new TextEncoder().encode(input).length;
  const lines = input === '' ? 0 : input.split(/\r?\n/).length;
  const chineseMatches = input.match(/[一-龥]/g);
  const asciiMatches = input.match(/[\x21-\x7e]/g); // printable ASCII
  const wordMatches = input.trim() === '' ? [] : input.trim().split(/\s+/);
  return {
    chars: [...input].length, // codepoint count (emojis = 1)
    charsNoSpace: [...input.replace(/\s+/g, '')].length,
    words: wordMatches.length,
    lines,
    bytes,
    chinese: chineseMatches ? chineseMatches.length : 0,
    ascii: asciiMatches ? asciiMatches.length : 0
  };
}
