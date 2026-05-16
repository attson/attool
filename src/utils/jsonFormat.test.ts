import { describe, expect, it } from 'vitest';
import { format, minify, parseJson, sortKeys } from './jsonFormat';

describe('parseJson', () => {
  it('parses valid JSON', () => {
    expect(parseJson('{"a":1}')).toEqual({ ok: true, value: { a: 1 } });
  });

  it('reports an error with line/column for invalid JSON', () => {
    const result = parseJson('{ "a": }');
    expect(result.ok).toBe(false);
    expect(result.error?.message).toMatch(/./);
  });

  it('treats empty input as an error', () => {
    expect(parseJson('').ok).toBe(false);
    expect(parseJson('   ').ok).toBe(false);
  });
});

describe('format', () => {
  it('pretty-prints with 2-space indent', () => {
    expect(format('{"a":1,"b":[2,3]}')).toBe('{\n  "a": 1,\n  "b": [\n    2,\n    3\n  ]\n}');
  });

  it('throws on invalid input', () => {
    expect(() => format('{')).toThrow();
  });
});

describe('minify', () => {
  it('removes whitespace', () => {
    expect(minify('{\n  "a": 1\n}')).toBe('{"a":1}');
  });

  it('throws on invalid input', () => {
    expect(() => minify('not json')).toThrow();
  });
});

describe('sortKeys', () => {
  it('sorts object keys at every depth', () => {
    expect(sortKeys('{"b":1,"a":{"d":4,"c":3}}')).toBe(
      '{\n  "a": {\n    "c": 3,\n    "d": 4\n  },\n  "b": 1\n}',
    );
  });

  it('leaves array order alone', () => {
    expect(sortKeys('[3,1,2]')).toBe('[\n  3,\n  1,\n  2\n]');
  });

  it('throws on invalid input', () => {
    expect(() => sortKeys('{')).toThrow();
  });
});
