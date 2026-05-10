import { describe, it, expect } from 'vitest';
import { darkOverrides, accentHex } from './index';

describe('theme', () => {
  it('exports emerald accent hex', () => {
    expect(accentHex).toBe('#34D399');
  });

  it('common.primaryColor uses emerald', () => {
    expect(darkOverrides.common?.primaryColor).toBe('#34D399');
    expect(darkOverrides.common?.primaryColorHover).toBe('#4ADE80');
    expect(darkOverrides.common?.primaryColorPressed).toBe('#16A34A');
  });

  it('common.borderRadius is 6px (no chunky corners)', () => {
    expect(darkOverrides.common?.borderRadius).toBe('6px');
    expect(darkOverrides.common?.borderRadiusSmall).toBe('4px');
  });

  it('Card uses 8px radius and elevated bg', () => {
    expect(darkOverrides.Card?.borderRadius).toBe('8px');
    expect(darkOverrides.Card?.color).toBe('#131316');
    expect(darkOverrides.Card?.borderColor).toBe('#1F1F23');
  });

  it('Button radii are tight', () => {
    expect(darkOverrides.Button?.borderRadiusMedium).toBe('6px');
    expect(darkOverrides.Button?.borderRadiusSmall).toBe('4px');
  });
});
