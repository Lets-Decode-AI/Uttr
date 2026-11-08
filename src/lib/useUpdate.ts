import { useCallback, useEffect, useState } from 'react';
import { api, type UpdateInfo, type UpdateProgress } from '../ipc';
import { errMsg } from './format';
import { useEvent } from './hooks';

export type UpdatePhase = 'idle' | 'checking' | 'upToDate' | 'available' | 'downloading' | 'installing' | 'error';

export interface UpdateState {
  phase: UpdatePhase;
  info: UpdateInfo | null;
  /** 0..1 while downloading, when the size is known. */
  progress: number | null;
  error: string | null;
}

// One shared state for the sidebar card and the Settings section.
let shared: UpdateState = { phase: 'idle', info: null, progress: null, error: null };
const listeners = new Set<(s: UpdateState) => void>();
function set(patch: Partial<UpdateState>) {
  shared = { ...shared, ...patch };
  listeners.forEach((l) => l(shared));
}

/** Runs `f`, tolerating an api that is missing (older backend, tests). */
const attempt = <T>(f: () => Promise<T>) => Promise.resolve().then(f);

export function useUpdate() {
  const [s, setS] = useState(shared);
  useEffect(() => {
    listeners.add(setS);
    setS(shared);
    if (shared.phase === 'idle') {
      attempt(api.updatePending).then((info) => { if (info && shared.phase === 'idle') set({ phase: 'available', info }); }, () => {});
    }
    return () => { listeners.delete(setS); };
  }, []);

  useEvent('updateAvailable', (info) => {
    if (shared.phase !== 'downloading' && shared.phase !== 'installing') set({ phase: 'available', info, error: null });
  });
  useEvent('updateProgress', (p: UpdateProgress) => {
    if (p.installing) set({ phase: 'installing', progress: 1 });
    else set({ phase: 'downloading', progress: p.total ? Math.min(1, p.downloaded / p.total) : null });
  });

  const check = useCallback(() => {
    set({ phase: 'checking', error: null });
    attempt(api.updateCheck).then(
      (info) => set(info ? { phase: 'available', info } : { phase: 'upToDate', info: null }),
      (e) => set({ phase: 'error', error: errMsg(e) }),
    );
  }, []);

  const install = useCallback(() => {
    set({ phase: 'downloading', progress: null, error: null });
    // On success the app restarts, so only failures come back here.
    attempt(api.updateInstall).catch((e) => set({ phase: shared.info ? 'available' : 'error', error: errMsg(e), progress: null }));
  }, []);

  return { ...s, check, install };
}

/** Resets the shared state; for tests. */
export function resetUpdateState() {
  shared = { phase: 'idle', info: null, progress: null, error: null };
}
