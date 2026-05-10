import { describe, it, expect } from 'vitest';
import { darkOverrides, lightOverrides, accentHex, accentLightHex } from './index';

describe('theme · dark', () => {
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

describe('theme · light', () => {
  it('exports emerald-500 for light accent', () => {
    expect(accentLightHex).toBe('#10B981');
  });

  it('common uses emerald-500 family + white surfaces', () => {
    expect(lightOverrides.common?.primaryColor).toBe('#10B981');
    expect(lightOverrides.common?.primaryColorHover).toBe('#059669');
    expect(lightOverrides.common?.primaryColorPressed).toBe('#047857');
    expect(lightOverrides.common?.bodyColor).toBe('#FAFAF9');
    expect(lightOverrides.common?.cardColor).toBe('#FFFFFF');
    expect(lightOverrides.common?.textColor1).toBe('#18181B');
    expect(lightOverrides.common?.textColor3).toBe('#71717A');
  });

  it('Card adapts to white bg + light line', () => {
    expect(lightOverrides.Card?.color).toBe('#FFFFFF');
    expect(lightOverrides.Card?.borderColor).toBe('#E4E4E7');
  });

  it('shares radii with dark overrides', () => {
    expect(lightOverrides.common?.borderRadius).toBe('6px');
    expect(lightOverrides.Button?.borderRadiusMedium).toBe('6px');
    expect(lightOverrides.Card?.borderRadius).toBe('8px');
  });
});
