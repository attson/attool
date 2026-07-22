import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

const iconPath = resolve('src-tauri/app-icon.svg');

describe('app icon', () => {
  it('keeps the visible artwork inside a Dock safe area', () => {
    const svg = readFileSync(iconPath, 'utf8');

    expect(svg).toContain('id="dock-safe-area"');
    expect(svg).toContain('transform="translate(92 92) scale(0.8203125)"');
  });
});
