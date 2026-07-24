import { describe, expect, it } from 'vitest';
import {
  format, formatValue, minify, minifyValue, parseJson, sortKeys, sortValue,
} from './jsonFormat';

describe('parseJson', () => {
  it('parses valid JSON', () => {
    expect(parseJson('{"a":1}')).toEqual({ ok: true, value: { a: 1 } });
  });

  it('reports an error with line/column for invalid JSON', () => {
    // "{" reliably triggers V8 "position N" / "(line N column M)" — both message
    // shapes feed line/column into the error. Avoid inputs like '{"a":}' whose
    // V8 message varies across Node versions.
    const result = parseJson('{');
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.message).toMatch(/./);
      expect(result.error.line).toBe(1);
      expect(result.error.column).toBeGreaterThanOrEqual(1);
    }
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

describe('valueApi', () => {
  it('formatValue prints with 2 space indent by default', () => {
    expect(formatValue({ a: 1 })).toBe('{\n  "a": 1\n}');
  });
  it('minifyValue prints without whitespace', () => {
    expect(minifyValue({ a: 1, b: [2] })).toBe('{"a":1,"b":[2]}');
  });
  it('sortValue orders object keys deeply', () => {
    expect(sortValue({ b: 1, a: { d: 2, c: 3 } })).toEqual({ a: { c: 3, d: 2 }, b: 1 });
  });
});
