import { describe, expect, it } from 'vitest';
import { convert } from './jsonConvert';

describe('convert JSON → YAML', () => {
  it('produces YAML', () => {
    const result = convert('{"a":1,"b":[2,3]}', 'json', 'yaml');
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.value).toContain('a: 1');
    expect(result.value).toContain('- 2');
  });
});

describe('convert YAML → JSON', () => {
  it('round-trips object', () => {
    const result = convert('a: 1\nb:\n  - 2\n  - 3\n', 'yaml', 'json');
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(JSON.parse(result.value)).toEqual({ a: 1, b: [2, 3] });
  });
});

describe('convert JSON → TOML', () => {
  it('produces TOML', () => {
    const result = convert('{"a":1,"section":{"key":"v"}}', 'json', 'toml');
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.value).toContain('a = 1');
    expect(result.value).toContain('[section]');
  });
});

describe('convert TOML → JSON', () => {
  it('round-trips object', () => {
    const result = convert('a = 1\n[section]\nkey = "v"\n', 'toml', 'json');
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(JSON.parse(result.value)).toEqual({ a: 1, section: { key: 'v' } });
  });
});

describe('convert JSON → CSV', () => {
  it('emits header + rows for flat arrays', () => {
    const result = convert('[{"a":1,"b":"x"},{"a":2,"b":"y"}]', 'json', 'csv');
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.value).toBe('a,b\n1,x\n2,y');
  });

  it('quotes fields containing commas / quotes / newlines', () => {
    const result = convert('[{"a":"hi, there","b":"\\"q\\""}]', 'json', 'csv');
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.value).toBe('a,b\n"hi, there","""q"""');
  });

  it('rejects non-array input', () => {
    const result = convert('{"a":1}', 'json', 'csv');
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.error).toMatch(/CSV/);
  });

  it('rejects arrays with nested structures', () => {
    const result = convert('[{"a":1,"b":{"c":2}}]', 'json', 'csv');
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.error).toMatch(/嵌套/);
  });
});

describe('convert CSV → JSON', () => {
  it('parses header + rows into array of objects', () => {
    const result = convert('a,b\n1,x\n2,y', 'csv', 'json');
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(JSON.parse(result.value)).toEqual([
      { a: '1', b: 'x' },
      { a: '2', b: 'y' },
    ]);
  });

  it('handles quoted fields', () => {
    const result = convert('a,b\n"hi, there","""q"""', 'csv', 'json');
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(JSON.parse(result.value)).toEqual([{ a: 'hi, there', b: '"q"' }]);
  });

  it('handles CRLF line endings', () => {
    const result = convert('a,b\r\n1,x\r\n2,y\r\n', 'csv', 'json');
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(JSON.parse(result.value)).toEqual([
      { a: '1', b: 'x' },
      { a: '2', b: 'y' },
    ]);
  });
});

describe('convert error handling', () => {
  it('returns error for invalid YAML', () => {
    const result = convert(': : :', 'yaml', 'json');
    expect(result.ok).toBe(false);
  });
});
