import { describe, expect, it } from 'vitest';
import { acceptsExtension, decodeFile } from './useFileDrop';

describe('acceptsExtension', () => {
  it('accepts when no filter provided', () => {
    expect(acceptsExtension('foo.bin', undefined)).toBe(true);
  });

  it('matches case-insensitively', () => {
    expect(acceptsExtension('data.JSON', ['json'])).toBe(true);
  });

  it('rejects unmatched extensions', () => {
    expect(acceptsExtension('image.png', ['json', 'yaml'])).toBe(false);
  });
});

describe('decodeFile', () => {
  it('reads UTF-8 text from a File', async () => {
    const file = new File(['{"hello":"世界"}'], 'data.json', { type: 'application/json' });
    expect(await decodeFile(file)).toBe('{"hello":"世界"}');
  });

  it('rejects files exceeding the max bytes', async () => {
    const file = new File(['x'.repeat(20)], 'big.json', { type: 'application/json' });
    await expect(decodeFile(file, { maxBytes: 10 })).rejects.toThrow(/超出/);
  });
});
