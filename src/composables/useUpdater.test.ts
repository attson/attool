import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { useUpdater, type UpdaterClient, type UpdateStateSnapshot, type ReleaseInfo } from './useUpdater';

function snap(phase: UpdateStateSnapshot['phase']): UpdateStateSnapshot {
  return { phase, current: '0.8.4', autoCheck: true, lastCheckAt: null, error: null };
}

function info(v: string, notes = ''): ReleaseInfo {
  return {
    version: v,
    notes,
    publishedAt: '',
    assetName: `AT_Tool_${v}_amd64.tar.gz`,
    assetUrl: 'https://example/x.tar.gz',
    assetSize: 100
  };
}

interface Opts {
  update?: { version: string; notes?: string };
  failCheck?: string;
  failInstall?: string;
}

function makeClient(opts: Opts): UpdaterClient {
  const state = { current: snap({ kind: 'idle' }) };
  return {
    getState: vi.fn(async () => state.current),
    check: vi.fn(async () => {
      if (opts.failCheck) {
        state.current = snap({ kind: 'error', message: opts.failCheck });
        throw new Error(opts.failCheck);
      }
      state.current = opts.update
        ? snap({ kind: 'available', info: info(opts.update.version, opts.update.notes) })
        : snap({ kind: 'up-to-date' });
      return state.current;
    }),
    download: vi.fn(async () => {
      if (opts.failInstall) throw new Error(opts.failInstall);
      state.current = snap({ kind: 'ready', version: opts.update!.version });
      return state.current;
    }),
    apply: vi.fn(async () => {}),
    cancel: vi.fn(async () => {}),
    onState: vi.fn(async () => () => {})
  };
}

describe('useUpdater', () => {
  beforeEach(() => { vi.useFakeTimers(); });
  afterEach(() => { vi.useRealTimers(); });

  it('idle by default', () => {
    const { state } = useUpdater(makeClient({}));
    expect(state.value.status).toBe('idle');
  });

  it('check transitions to up-to-date when no update', async () => {
    const { state, check } = useUpdater(makeClient({}));
    await check();
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

  it('check error transitions to error', async () => {
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

  it('install error transitions to error', async () => {
    const { state, check, install } = useUpdater(makeClient({
      update: { version: '0.2.0' },
      failInstall: 'download interrupted'
    }));
    await check();
    await install();
    expect(state.value.status).toBe('error');
    expect(state.value.error).toBe('download interrupted');
  });

  it('relaunch calls client.apply', async () => {
    const client = makeClient({ update: { version: '0.2.0' } });
    const { relaunch } = useUpdater(client);
    await relaunch();
    expect(client.apply).toHaveBeenCalled();
  });

  it('dismiss resets to idle', async () => {
    const { state, check, dismiss } = useUpdater(makeClient({ update: { version: '0.2.0' } }));
    await check();
    expect(state.value.status).toBe('available');
    dismiss();
    expect(state.value.status).toBe('idle');
    expect(state.value.available).toBeUndefined();
  });
});
