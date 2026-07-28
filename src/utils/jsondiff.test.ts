import { describe, expect, it } from 'vitest';
import { countJsonDiffChanges, diffJson, diffJsonHtml, prepareJsonDiffView } from './jsondiff';

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

describe('prepareJsonDiffView', () => {
  it('returns sorted formatted texts for the read-only diff editor', () => {
    const view = prepareJsonDiffView('{"b":2,"a":1}', '{"b":3,"a":1,"c":4}');

    expect(view.leftText).toBe('{\n  "a": 1,\n  "b": 2\n}');
    expect(view.rightText).toBe('{\n  "a": 1,\n  "b": 3,\n  "c": 4\n}');
    expect(view.equal).toBe(false);
    expect(view.changeCount).toBe(2);
  });

  it('treats semantically equal objects as equal even when key order differs', () => {
    const view = prepareJsonDiffView('{"b":2,"a":1}', '{"a":1,"b":2}');

    expect(view.equal).toBe(true);
    expect(view.changeCount).toBe(0);
  });

  it('surfaces parse errors without formatted diff texts', () => {
    const view = prepareJsonDiffView('{', '{"a":1}');

    expect(view.leftError).toBeTruthy();
    expect(view.leftText).toBe('');
    expect(view.rightText).toBe('');
  });
});

describe('countJsonDiffChanges', () => {
  it('counts changed, added, and deleted leaf changes', () => {
    const r = diffJson('{"a":1,"b":2,"c":3}', '{"a":2,"b":2,"d":4}');

    expect(countJsonDiffChanges(r.delta)).toBe(3);
  });
});
