import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

const configPath = resolve('src-tauri/tauri.conf.json');

describe('tauri.conf.json', () => {
  it('ad-hoc signs macOS app bundles so downloaded apps have sealed resources', () => {
    const config = JSON.parse(readFileSync(configPath, 'utf8'));

    expect(config.bundle?.macOS?.signingIdentity).toBe('-');
  });
});
