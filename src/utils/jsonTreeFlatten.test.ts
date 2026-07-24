import { describe, it, expect } from 'vitest';
import { flatten, allExpandableKeys, primitiveDisplay } from './jsonTreeFlatten';

const sample = {
  info: { title: 'X', version: '1' },
  tags: [{ name: 'a' }, { name: 'b' }],
  count: 3,
  nick: 'hi',
};

describe('flatten', () => {
  it('root closed shows only root row', () => {
    const rows = flatten(sample, new Set());
    expect(rows).toHaveLength(1);
    expect(rows[0].kind).toBe('object');
    expect(rows[0].depth).toBe(0);
    expect(rows[0].size).toBe(4);
  });

  it('root open shows top-level children in source order', () => {
    const rows = flatten(sample, new Set(['$']));
    expect(rows.map((r) => r.label)).toEqual(['', 'info', 'tags', 'count', 'nick']);
    expect(rows[2].kind).toBe('array');
    expect(rows[2].size).toBe(2);
  });

  it('nested open expands sub containers', () => {
    const rows = flatten(sample, new Set(['$', '$.tags', '$.tags[0]']));
    const labels = rows.map((r) => r.label);
    expect(labels).toEqual(['', 'info', 'tags', '0', 'name', '1', 'count', 'nick']);
  });

  it('primitive display picks class', () => {
    expect(primitiveDisplay('hi').klass).toBe('string');
    expect(primitiveDisplay(3).klass).toBe('number');
    expect(primitiveDisplay(true).klass).toBe('boolean');
    expect(primitiveDisplay(null).klass).toBe('null');
  });

  it('primitive display truncates long strings', () => {
    const long = 'a'.repeat(600);
    const { text } = primitiveDisplay(long, 512);
    expect(text).toContain('600');
    expect(text.length).toBeLessThan(long.length);
  });
});

describe('allExpandableKeys', () => {
  it('collects every container key iteratively', () => {
    const keys = allExpandableKeys(sample);
    expect(keys.has('$')).toBe(true);
    expect(keys.has('$.info')).toBe(true);
    expect(keys.has('$.tags')).toBe(true);
    expect(keys.has('$.tags[0]')).toBe(true);
    expect(keys.has('$.tags[1]')).toBe(true);
    expect(keys.has('$.count')).toBe(false);
    expect(keys.has('$.nick')).toBe(false);
  });
});
