import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { useUpdater, type UpdaterClient, type UpdateInfo } from './useUpdater';

function makeClient(opts: {
  update?: { version: string; notes?: string };
  failCheck?: string;
  failInstall?: string;
}): UpdaterClient {
  const dl = vi.fn(async (cb: (e: { event: string; data?: any }) => void) => {
    if (opts.failInstall) throw new Error(opts.failInstall);
    cb({ event: 'Started', data: { contentLength: 100 } });
    cb({ event: 'Progress', data: { chunkLength: 50 } });
    cb({ event: 'Progress', data: { chunkLength: 50 } });
    cb({ event: 'Finished' });
  });
  return {
    check: vi.fn(async (): Promise<UpdateInfo | null> => {
      if (opts.failCheck) throw new Error(opts.failCheck);
      return opts.update
        ? { version: opts.update.version, notes: opts.update.notes, downloadAndInstall: dl }
        : null;
    }),
    relaunch: vi.fn(async () => {})
  };
}

describe('useUpdater', () => {
  beforeEach(() => { vi.useFakeTimers(); });
  afterEach(() => { vi.useRealTimers(); });

  it('idle by default', () => {
    const { state } = useUpdater(makeClient({}));
    expect(state.value.status).toBe('idle');
  });

  it('check transitions idle → checking → up-to-date when no update', async () => {
    const { state, check } = useUpdater(makeClient({}));
    const p = check();
    expect(state.value.status).toBe('checking');
    await p;
    expect(state.value.status).toBe('up-to-date');
  });

  it('up-to-date auto-reverts to idle after 3s', async () => {
    const { state, check } = useUpdater(makeClient({}));
    await check();
    expect(state.value.status).toBe('up-to-date');
    vi.advanceTimersByTime(3000);
    expect(state.value.status).toBe('idle');
  });

  it('check transitions to available when update found', async () => {
    const { state, check } = useUpdater(makeClient({ update: { version: '0.2.0', notes: 'fixes' } }));
    await check();
    expect(state.value.status).toBe('available');
    expect(state.value.available).toEqual({ version: '0.2.0', notes: 'fixes' });
  });

  it('check error transitions to error with message', async () => {
    const { state, check } = useUpdater(makeClient({ failCheck: 'network down' }));
    await check();
    expect(state.value.status).toBe('error');
    expect(state.value.error).toBe('network down');
  });

  it('records trigger from check argument', async () => {
    const { state, check } = useUpdater(makeClient({ update: { version: '0.2.0' } }));
    await check('manual');
    expect(state.value.trigger).toBe('manual');
  });

  it('install transitions available → downloading → ready', async () => {
    const { state, check, install } = useUpdater(makeClient({ update: { version: '0.2.0' } }));
    await check();
    const p = install();
    expect(state.value.status).toBe('downloading');
    await p;
    expect(state.value.status).toBe('ready');
  });

  it('install ends with 100% download percent', async () => {
    const { state, check, install } = useUpdater(makeClient({ update: { version: '0.2.0' } }));
    await check();
    await install();
    expect(state.value.downloadPercent).toBe(100);
  });

  it('install error transitions downloading → error', async () => {
    const { state, check, install } = useUpdater(makeClient({
      update: { version: '0.2.0' },
      failInstall: 'download interrupted'
    }));
    await check();
    await install();
    expect(state.value.status).toBe('error');
    expect(state.value.error).toBe('download interrupted');
  });

  it('relaunch calls client.relaunch', async () => {
    const client = makeClient({ update: { version: '0.2.0' } });
    const { relaunch } = useUpdater(client);
    await relaunch();
    expect(client.relaunch).toHaveBeenCalled();
  });

  it('dismiss resets to idle from any state', async () => {
    const { state, check, dismiss } = useUpdater(makeClient({ update: { version: '0.2.0' } }));
    await check();
    expect(state.value.status).toBe('available');
    dismiss();
    expect(state.value.status).toBe('idle');
    expect(state.value.available).toBeUndefined();
  });

  it('install without prior check is no-op (stays idle)', async () => {
    const { state, install } = useUpdater(makeClient({}));
    await install();
    expect(state.value.status).toBe('idle');
  });
});
