import { describe, expect, it, beforeEach } from 'vitest';
import { ref } from 'vue';
import { useAria2Handoff } from './useAria2Handoff';

describe('useAria2Handoff', () => {
  beforeEach(() => {
    useAria2Handoff()._reset();
  });

  it('drainInto does nothing when nothing is pending', () => {
    const target = ref('existing');
    useAria2Handoff().drainInto(target);
    expect(target.value).toBe('existing');
  });

  it('drainInto sets target when pending has one url and target is empty', () => {
    const handoff = useAria2Handoff();
    handoff.push('https://a.mp4');
    const target = ref('');
    handoff.drainInto(target);
    expect(target.value).toBe('https://a.mp4');
    expect(handoff._size()).toBe(0);
  });

  it('drainInto appends with newline when target is not empty', () => {
    const handoff = useAria2Handoff();
    handoff.push('https://a.mp4');
    handoff.push('https://b.mp4');
    const target = ref('https://kept.com/file.zip');
    handoff.drainInto(target);
    expect(target.value).toBe('https://kept.com/file.zip\nhttps://a.mp4\nhttps://b.mp4');
    expect(handoff._size()).toBe(0);
  });

  it('drain clears pending so a subsequent drain is a noop', () => {
    const handoff = useAria2Handoff();
    handoff.push('https://a.mp4');
    const target1 = ref('');
    handoff.drainInto(target1);
    expect(target1.value).toBe('https://a.mp4');

    const target2 = ref('other');
    handoff.drainInto(target2);
    expect(target2.value).toBe('other');
  });

  it('push after drain queues a fresh batch', () => {
    const handoff = useAria2Handoff();
    handoff.push('https://a.mp4');
    handoff.drainInto(ref(''));
    handoff.push('https://c.mp4');
    const target = ref('');
    handoff.drainInto(target);
    expect(target.value).toBe('https://c.mp4');
  });

  it('handoff is a shared singleton across multiple calls', () => {
    const a = useAria2Handoff();
    const b = useAria2Handoff();
    a.push('https://x.mp4');
    expect(b._size()).toBe(1);
  });
});
