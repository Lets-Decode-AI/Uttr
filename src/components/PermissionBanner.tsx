import { useEffect, useState } from 'react';
import { api, type Perm, type PermissionsStatus } from '../ipc';
import { errMsg } from '../lib/format';
import { useToast } from './Toasts';
import Button from './ui/Button';

const ok = (p: Perm) => p === 'granted' || p === 'notRequired';

/** Shown above every page while uttr is missing a permission it needs to work. */
export default function PermissionBanner() {
  const toast = useToast();
  const [p, setP] = useState<PermissionsStatus | null>(null);
  useEffect(() => {
    let live = true;
    const poll = () => api.permissionsStatus().then((s) => live && setP(s), () => {});
    poll();
    const t = setInterval(poll, 2000);
    return () => { live = false; clearInterval(t); };
  }, []);
  if (!p || (ok(p.microphone) && ok(p.accessibility))) return null;
  const attempt = (f: () => Promise<void>) => f().catch((e) => toast(errMsg(e), 'error'));
  const micMissing = !ok(p.microphone);
  const accMissing = !ok(p.accessibility);
  const message = accMissing
    ? 'The shortcut is off because uttr does not have Accessibility access. Turn on uttr in Accessibility, then restart uttr.'
    : 'uttr cannot hear you because it does not have microphone access.';
  return (
    <div role="alert" className="banner-error" style={{ display: 'flex', alignItems: 'center', gap: 12, flexWrap: 'wrap', marginBottom: 16 }}>
      <span style={{ flex: '1 1 260px' }}>{message}</span>
      {micMissing && (
        <Button onClick={() => attempt(() => (p.microphone === 'denied' ? api.openPermissionSettings('microphone') : api.requestMicrophone()))}>
          Allow microphone
        </Button>
      )}
      {accMissing && (
        <>
          <Button onClick={() => attempt(() => api.openPermissionSettings('accessibility'))}>Open Accessibility settings</Button>
          <Button onClick={() => attempt(api.restartApp)}>Restart uttr</Button>
        </>
      )}
    </div>
  );
}
