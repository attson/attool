import { describe, it, expect } from 'vitest';
import {
  handleParse, handleSerialize, handleJsonpath, handleDiff, handleConvert,
} from './jsonWorkerHandlers';

describe('handleParse', () => {
  it('parses valid JSON', () => {
    const r = handleParse('{"a":1}');
    expect(r.ok).toBe(true);
    if (r.ok) expect(r.value).toEqual({ a: 1 });
  });
  it('reports parse error', () => {
    const r = handleParse('{oops');
    expect(r.ok).toBe(false);
  });
  it('reports empty input error', () => {
    const r = handleParse('   ');
    expect(r.ok).toBe(false);
  });
});

describe('handleSerialize', () => {
  it('formats with indent', () => {
    const r = handleSerialize({ a: 1 }, 'format');
    expect(r.ok).toBe(true);
    if (r.ok) expect(r.text).toContain('\n');
  });
  it('minifies without whitespace', () => {
    const r = handleSerialize({ a: 1, b: 2 }, 'minify');
    if (r.ok) expect(r.text).toBe('{"a":1,"b":2}');
  });
  it('sort orders keys', () => {
    const r = handleSerialize({ b: 1, a: 2 }, 'sort');
    if (r.ok) expect(r.text).toBe('{\n  "a": 2,\n  "b": 1\n}');
  });
});

describe('handleJsonpath', () => {
  it('extracts matches and returns pretty text', () => {
    const r = handleJsonpath({ a: [1, 2, 3] }, '$.a[*]');
    expect(r.ok).toBe(true);
    if (r.ok) {
      expect(r.matches).toEqual([1, 2, 3]);
      expect(r.text).toContain('1');
    }
  });
  it('reports bad expression', () => {
    const r = handleJsonpath({}, '][[[');
    expect(r.ok).toBe(false);
  });
});

describe('handleDiff', () => {
  it('reports equal', () => {
    const r = handleDiff('{"a":1}', '{"a":1}', true);
    expect(r.equal).toBe(true);
    expect(r.delta).toBeNull();
    expect(r.html).toBeNull();
  });
  it('emits html when different and requested', () => {
    const r = handleDiff('{"a":1}', '{"a":2}', true);
    expect(r.equal).toBe(false);
    expect(r.html && r.html.length > 0).toBe(true);
  });
  it('skips html when not requested', () => {
    const r = handleDiff('{"a":1}', '{"a":2}', false);
    expect(r.html).toBeNull();
  });
  it('surfaces left parse error', () => {
    const r = handleDiff('{oops', '{}', true);
    expect(r.leftError).toBeDefined();
  });
});

describe('handleConvert', () => {
  it('converts json to yaml', () => {
    const r = handleConvert('{"a":1}', 'json', 'yaml');
    expect(r.ok).toBe(true);
    if (r.ok) expect(r.text).toContain('a: 1');
  });
  it('reports convert error', () => {
    const r = handleConvert('{oops', 'json', 'yaml');
    expect(r.ok).toBe(false);
  });
});
