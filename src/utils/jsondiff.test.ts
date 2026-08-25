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
  it('ignores value changes in key-only mode', () => {
    const view = prepareJsonDiffView(
      '{"event":"request_detail","level":"INFO"}',
      '{"event":"response_detail","level":"ERROR"}',
      { keyOnly: true },
    );

    expect(view.equal).toBe(true);
    expect(view.changeCount).toBe(0);
    expect(view.leftText).toBe('{}');
    expect(view.rightText).toBe('{}');
  });

  it('returns key skeletons for nested added and removed properties', () => {
    const view = prepareJsonDiffView(
      '{"shared":{"same":1,"leftOnly":{"deep":2}}}',
      '{"shared":{"same":9,"rightOnly":3}}',
      { keyOnly: true },
    );

    expect(view.equal).toBe(false);
    expect(view.changeCount).toBe(3);
    expect(view.leftText).toBe([
      '{',
      '  "shared": {',
      '    "leftOnly": {',
      '      "deep": null',
      '    }',
      '  }',
      '}',
    ].join('\n'));
    expect(view.rightText).toBe([
      '{',
      '  "shared": {',
      '    "rightOnly": null',
      '  }',
      '}',
    ].join('\n'));
  });

  it('expands nested JSON strings in regular diff mode', () => {
    const view = prepareJsonDiffView(
      '{"request_body":"{\\"messages\\":[{\\"role\\":\\"user\\",\\"content\\":\\"hello\\"}]}"}',
      '{"request_body":"{\\"messages\\":[{\\"role\\":\\"user\\",\\"content\\":\\"world\\"}]}"}',
      { parseNestedJsonStrings: true },
    );

    expect(view.equal).toBe(false);
    expect(view.changeCount).toBe(1);
    expect(JSON.parse(view.leftText)).toEqual({
      request_body: { messages: [{ content: 'hello', role: 'user' }] },
    });
    expect(JSON.parse(view.rightText)).toEqual({
      request_body: { messages: [{ content: 'world', role: 'user' }] },
    });
  });

  it('returns only nested string key differences when both options are enabled', () => {
    const view = prepareJsonDiffView(
      '{"request_body":"{\\"messages\\":[{\\"role\\":\\"user\\",\\"content\\":\\"hello\\",\\"leftOnly\\":1}]}"}',
      '{"request_body":"{\\"messages\\":[{\\"role\\":\\"user\\",\\"content\\":\\"world\\",\\"rightOnly\\":2}]}"}',
      { keyOnly: true, parseNestedJsonStrings: true },
    );

    expect(view.equal).toBe(false);
    expect(view.changeCount).toBe(2);
    expect(JSON.parse(view.leftText)).toEqual({
      request_body: { messages: [{ leftOnly: null }] },
    });
    expect(JSON.parse(view.rightText)).toEqual({
      request_body: { messages: [{ rightOnly: null }] },
    });
  });

  it('keeps nested strings unchanged when either side is invalid or scalar JSON', () => {
    const invalid = prepareJsonDiffView(
      '{"payload":"{oops"}',
      '{"payload":"{\\"a\\":1}"}',
      { parseNestedJsonStrings: true },
    );
    const scalar = prepareJsonDiffView(
      '{"payload":"1"}',
      '{"payload":"2"}',
      { parseNestedJsonStrings: true },
    );

    expect(JSON.parse(invalid.leftText)).toEqual({ payload: '{oops' });
    expect(JSON.parse(invalid.rightText)).toEqual({ payload: '{"a":1}' });
    expect(JSON.parse(scalar.leftText)).toEqual({ payload: '1' });
    expect(JSON.parse(scalar.rightText)).toEqual({ payload: '2' });
  });

  it('ignores array length and scalar changes in key-only mode', () => {
    const view = prepareJsonDiffView(
      '{"items":[{"same":1},{"leftOnly":2}],"values":[1,2]}',
      '{"items":[{"same":9}],"values":[3]}',
      { keyOnly: true },
    );

    expect(view.equal).toBe(true);
    expect(view.changeCount).toBe(0);
    expect(view.leftText).toBe('{}');
    expect(view.rightText).toBe('{}');
  });

  it('reports nested keys when a common property changes between object and scalar', () => {
    const view = prepareJsonDiffView(
      '{"payload":{"nested":1}}',
      '{"payload":"plain text"}',
      { keyOnly: true },
    );

    expect(view.equal).toBe(false);
    expect(view.changeCount).toBe(1);
    expect(JSON.parse(view.leftText)).toEqual({ payload: { nested: null } });
    expect(JSON.parse(view.rightText)).toEqual({ payload: null });
  });

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
