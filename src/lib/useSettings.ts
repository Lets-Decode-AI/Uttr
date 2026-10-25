import { useEffect, useRef, useState } from 'react';
import { api, type Settings } from '../ipc';
import { useToast } from '../components/Toasts';
import { errMsg } from './format';

/** Loads the settings and saves patches to them. Saves run one at a time, each applied on top
 * of the last state the backend accepted, so a rejected save can never wipe a later one. */
export function useSettings() {
  const toast = useToast();
  const [s, setS] = useState<Settings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const confirmed = useRef<Settings | null>(null); // last state the backend accepted
  const queue = useRef<Promise<void>>(Promise.resolve());
  const pending = useRef(0);

  useEffect(() => {
    api.settingsGet().then((v) => { confirmed.current = v; setS(v); }, (e) => setError(errMsg(e)));
  }, []);

  const save = (patch: Partial<Settings>) => {
    setS((x) => (x ? { ...x, ...patch } : x));
    setError(null);
    pending.current++;
    queue.current = queue.current.then(async () => {
      try {
        const base = confirmed.current;
        if (!base) return;
        const next = { ...base, ...patch };
        await api.settingsSet(next);
        confirmed.current = next;
      } catch (e) {
        setError(errMsg(e)); // the backend rejected it, for example an invalid shortcut
        toast(errMsg(e), 'error');
      } finally {
        if (--pending.current === 0) setS(confirmed.current);
      }
    });
  };

  return { s, error, save };
}
