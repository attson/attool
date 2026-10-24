import { describe, expect, it, vi } from 'vitest';
import { watchMonacoTheme, type ThemeAttributeRoot, type ThemeObserverFactory } from './monacoTheme';

describe('watchMonacoTheme', () => {
  it('applies the app theme initially and after data-theme changes', () => {
    let theme = 'dark';
    let notify = () => {};
    const disconnect = vi.fn();
    const root: ThemeAttributeRoot = {
      getAttribute: () => theme
    };
    const observerFactory: ThemeObserverFactory = (onChange) => {
      notify = onChange;
      return { observe: vi.fn(), disconnect };
    };
    const applied: string[] = [];

    const stop = watchMonacoTheme((value) => applied.push(value), root, observerFactory);
    theme = 'light';
    notify();
    stop();

    expect(applied).toEqual(['vs-dark', 'vs']);
    expect(disconnect).toHaveBeenCalledOnce();
  });
});
