import { describe, expect, it } from 'vitest';
import { applyVarsToSpec, collectUnknownVars, collectVars, makeVarContext, resolveVars } from './variables';
import { makeEmptySpec } from './types';

const ctx = makeVarContext(
  [
    { key: 'baseUrl', value: 'https://api.prod', enabled: true },
    { key: 'token', value: 'abc', enabled: true },
    { key: 'disabled', value: 'x', enabled: false }
  ],
  [
    { key: 'baseUrl', value: 'https://api.global', enabled: true },
    { key: 'timeout', value: '5000', enabled: true }
  ]
);

describe('resolveVars', () => {
  it('replaces {{name}}', () => {
    expect(resolveVars('{{baseUrl}}/users', ctx)).toBe('https://api.prod/users');
  });

  it('active takes priority over global', () => {
    expect(resolveVars('{{baseUrl}}', ctx)).toBe('https://api.prod');
  });

  it('falls back to global when active has no key', () => {
    expect(resolveVars('t={{timeout}}', ctx)).toBe('t=5000');
  });

  it('keeps unknown vars literal', () => {
    expect(resolveVars('{{missing}}/x', ctx)).toBe('{{missing}}/x');
  });

  it('skips disabled entries', () => {
    expect(resolveVars('{{disabled}}', ctx)).toBe('{{disabled}}');
  });

  it('tolerates whitespace inside braces', () => {
    expect(resolveVars('{{ token }}', ctx)).toBe('abc');
  });

  it('replaces multiple occurrences', () => {
    expect(resolveVars('{{token}}-{{token}}', ctx)).toBe('abc-abc');
  });

  it('leaves single braces alone', () => {
    expect(resolveVars('{token}', ctx)).toBe('{token}');
  });

  it('returns empty string as-is', () => {
    expect(resolveVars('', ctx)).toBe('');
  });
});

describe('collectVars / collectUnknownVars', () => {
  it('lists unique variable names', () => {
    expect(collectVars('{{a}}/{{b}}/{{a}}')).toEqual(['a', 'b']);
  });

  it('returns [] for input without vars', () => {
    expect(collectVars('no vars here')).toEqual([]);
  });

  it('reports undefined vars only', () => {
    expect(collectUnknownVars('{{baseUrl}}/{{nope}}', ctx)).toEqual(['nope']);
  });
});

describe('applyVarsToSpec', () => {
  it('replaces URL / headers / query / body / auth', () => {
    const spec = makeEmptySpec();
    spec.url = '{{baseUrl}}/u';
    spec.headers = [{ key: 'X-{{token}}', value: 'v-{{token}}', enabled: true }];
    spec.queryParams = [{ key: 'q', value: '{{token}}', enabled: true }];
    spec.bodyType = 'json';
    spec.body = '{"t":"{{token}}"}';
    spec.auth = { type: 'bearer', bearerToken: '{{token}}' };
    const out = applyVarsToSpec(spec, ctx);
    expect(out.url).toBe('https://api.prod/u');
    expect(out.headers[0].key).toBe('X-abc');
    expect(out.headers[0].value).toBe('v-abc');
    expect(out.queryParams[0].value).toBe('abc');
    expect(out.body).toBe('{"t":"abc"}');
    expect(out.auth.bearerToken).toBe('abc');
  });

  it('does not replace multipart file paths', () => {
    const spec = makeEmptySpec();
    spec.bodyType = 'multipart';
    spec.multipartFields = [
      { key: 'file', kind: 'file', value: '/abs/{{token}}.png', enabled: true },
      { key: 'note', kind: 'text', value: 'hi {{token}}', enabled: true }
    ];
    const out = applyVarsToSpec(spec, ctx);
    expect(out.multipartFields[0].value).toBe('/abs/{{token}}.png');
    expect(out.multipartFields[1].value).toBe('hi abc');
  });

  it('does not mutate the input spec', () => {
    const spec = makeEmptySpec();
    spec.url = '{{baseUrl}}';
    applyVarsToSpec(spec, ctx);
    expect(spec.url).toBe('{{baseUrl}}');
  });
});
