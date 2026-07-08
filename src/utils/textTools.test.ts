import { describe, expect, it } from 'vitest';
import {
  BUILTIN_PATTERNS,
  changeCase,
  cleanText,
  computeStats,
  diffSummary,
  extractMatches,
  joinWith,
  lineDiff,
  sortLines,
  splitBy
} from './textTools';

describe('cleanText', () => {
  const base = 'a\n b \nc\na\n\n';
  it('dedups', () => {
    expect(cleanText(base, { dedup: true, dropEmpty: false, trimEachLine: false, collapseSpaces: false, keepOrder: true })).toBe(
      'a\n b \nc\n'
    );
  });
  it('drops empty lines', () => {
    expect(cleanText(base, { dedup: false, dropEmpty: true, trimEachLine: false, collapseSpaces: false, keepOrder: true })).toBe(
      'a\n b \nc\na'
    );
  });
  it('trims each line + dedups', () => {
    expect(cleanText('a\n a \nb\na  ', { dedup: true, dropEmpty: false, trimEachLine: true, collapseSpaces: false, keepOrder: true })).toBe(
      'a\nb'
    );
  });
  it('collapses spaces', () => {
    expect(
      cleanText('hi   world', { dedup: false, dropEmpty: false, trimEachLine: false, collapseSpaces: true, keepOrder: true })
    ).toBe('hi world');
  });
});

describe('sortLines', () => {
  it('asc / desc', () => {
    expect(sortLines('c\na\nb', 'asc')).toBe('a\nb\nc');
    expect(sortLines('c\na\nb', 'desc')).toBe('c\nb\na');
  });
  it('natural handles numbers', () => {
    expect(sortLines('item10\nitem2\nitem1', 'natural')).toBe('item1\nitem2\nitem10');
  });
  it('length asc / desc', () => {
    expect(sortLines('bbb\na\ncc', 'length-asc')).toBe('a\ncc\nbbb');
    expect(sortLines('bbb\na\ncc', 'length-desc')).toBe('bbb\ncc\na');
  });
  it('reverse', () => {
    expect(sortLines('a\nb\nc', 'reverse')).toBe('c\nb\na');
  });
});

describe('changeCase', () => {
  it('upper / lower', () => {
    expect(changeCase('Hello World', 'upper')).toBe('HELLO WORLD');
    expect(changeCase('Hello World', 'lower')).toBe('hello world');
  });
  it('title', () => {
    expect(changeCase('hello world foo', 'title')).toBe('Hello World Foo');
  });
  it('camel / pascal', () => {
    expect(changeCase('hello world', 'camel')).toBe('helloWorld');
    expect(changeCase('hello-world', 'pascal')).toBe('HelloWorld');
    expect(changeCase('fooBarBaz', 'snake')).toBe('foo_bar_baz');
  });
  it('snake / kebab / constant', () => {
    expect(changeCase('Hello World Foo', 'snake')).toBe('hello_world_foo');
    expect(changeCase('Hello World Foo', 'kebab')).toBe('hello-world-foo');
    expect(changeCase('helloWorldFoo', 'constant')).toBe('HELLO_WORLD_FOO');
  });
  it('swap', () => {
    expect(changeCase('Hello', 'swap')).toBe('hELLO');
  });
});

describe('split / join', () => {
  it('splitBy comma', () => {
    expect(splitBy('a,b,c', ',')).toBe('a\nb\nc');
  });
  it('joinWith comma', () => {
    expect(joinWith('a\nb\nc', ',')).toBe('a,b,c');
  });
  it('splitBy noop when delimiter empty', () => {
    expect(splitBy('a,b', '')).toBe('a,b');
  });
});

describe('extractMatches', () => {
  it('extracts URLs', () => {
    const text = '看看 https://example.com 还有 http://x.co/y?z=1';
    const urls = extractMatches(text, BUILTIN_PATTERNS.URL);
    expect(urls).toEqual(['https://example.com', 'http://x.co/y?z=1']);
  });
  it('dedups', () => {
    const emails = extractMatches('a@x.com b@x.com a@x.com', BUILTIN_PATTERNS.Email, true);
    expect(emails).toEqual(['a@x.com', 'b@x.com']);
  });
  it('extracts Chinese', () => {
    const zh = extractMatches('mix 中文 and english 又一段', BUILTIN_PATTERNS.中文);
    expect(zh).toEqual(['中文', '又一段']);
  });
  it('extracts IPv4', () => {
    const ips = extractMatches('192.168.1.1 and 999.999.999.999 and 8.8.8.8', BUILTIN_PATTERNS.IPv4);
    expect(ips).toEqual(['192.168.1.1', '8.8.8.8']);
  });
});

describe('lineDiff', () => {
  it('reports identical text', () => {
    const d = lineDiff('a\nb\nc', 'a\nb\nc');
    expect(d.every((l) => l.type === 'equal')).toBe(true);
    expect(diffSummary(d).identical).toBe(true);
  });
  it('reports pure addition', () => {
    const d = lineDiff('a\nb', 'a\nb\nc');
    const summary = diffSummary(d);
    expect(summary.added).toBe(1);
    expect(summary.removed).toBe(0);
    expect(d[d.length - 1]).toEqual({ type: 'add', text: 'c', lineNumB: 3 });
  });
  it('reports pure removal', () => {
    const d = lineDiff('a\nb\nc', 'a\nb');
    const summary = diffSummary(d);
    expect(summary.added).toBe(0);
    expect(summary.removed).toBe(1);
  });
  it('reports substitution as remove+add', () => {
    const d = lineDiff('a\nb\nc', 'a\nX\nc');
    const s = diffSummary(d);
    expect(s.added).toBe(1);
    expect(s.removed).toBe(1);
    expect(s.equal).toBe(2);
  });
  it('preserves line numbering', () => {
    const d = lineDiff('a\nb\nc', 'a\nX\nc');
    const cLine = d.find((l) => l.text === 'c');
    expect(cLine?.lineNumA).toBe(3);
    expect(cLine?.lineNumB).toBe(3);
  });
});

describe('computeStats', () => {
  it('counts basics', () => {
    const s = computeStats('hi world\nfoo');
    expect(s.chars).toBe(12);
    expect(s.words).toBe(3);
    expect(s.lines).toBe(2);
  });
  it('counts Chinese', () => {
    const s = computeStats('中文 hello');
    expect(s.chinese).toBe(2);
    expect(s.bytes).toBeGreaterThan(s.chars);
  });
  it('empty input', () => {
    const s = computeStats('');
    expect(s.chars).toBe(0);
    expect(s.lines).toBe(0);
    expect(s.words).toBe(0);
  });
});
