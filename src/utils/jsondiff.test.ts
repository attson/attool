import { describe, expect, it } from 'vitest';
import { diffJson, diffJsonHtml } from './jsondiff';

describe('diffJson', () => {
  it('returns null when inputs are deeply equal', () => {
    expect(diffJson('{"a":1}', '{"a":1}').delta).toBeNull();
  });

  it('ignores key order', () => {
    expect(diffJson('{"a":1,"b":2}', '{"b":2,"a":1}').delta).toBeNull();
  });

  it('detects changed values', () => {
    const r = diffJson('{"a":1}', '{"a":2}');
    expect(r.delta).not.toBeNull();
  });

  it('detects added keys', () => {
    const r = diffJson('{"a":1}', '{"a":1,"b":2}');
    expect(r.delta).not.toBeNull();
  });

  it('detects removed keys', () => {
    const r = diffJson('{"a":1,"b":2}', '{"a":1}');
    expect(r.delta).not.toBeNull();
  });

  it('reports parse error when left is invalid', () => {
    const r = diffJson('{', '{"a":1}');
    expect(r.leftError).toBeTruthy();
  });

  it('reports parse error when right is invalid', () => {
    const r = diffJson('{"a":1}', '{');
    expect(r.rightError).toBeTruthy();
  });
});

describe('diffJsonHtml', () => {
  it('returns non-empty HTML for changes', () => {
    const html = diffJsonHtml('{"a":1}', '{"a":2}');
    expect(html).toMatch(/<.+>/);
    expect(html.length).toBeGreaterThan(0);
  });

  it('returns empty string when delta is null', () => {
    expect(diffJsonHtml('{"a":1}', '{"a":1}')).toBe('');
  });
});
