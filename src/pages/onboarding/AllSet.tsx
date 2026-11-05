import { useEffect, useState } from 'react';
import { api, type Settings } from '../../ipc';
import { useToast } from '../../components/Toasts';
import Button from '../../components/ui/Button';
import Icon from '../../components/ui/Icon';
import { keycapsFor } from '../../components/ui/Keycap';
import { errMsg } from '../../lib/format';
import { navigate } from '../../lib/hooks';
import { markTourDone, startTour } from '../../lib/tour';
import { STEP_KEY } from './Shell';

/** Final step: a summary of the setup with links back, then into the app (and the tour). */
export default function AllSet({ onBack, goTo }: { onBack: () => void; goTo: (step: number) => void }) {
  const toast = useToast();
  const [s, setS] = useState<Settings | null>(null);
  const [model, setModel] = useState<string | null>(null);
  useEffect(() => {
    api.settingsGet().then((v) => {
      setS(v);
      if (v.sttProvider === 'cloud') setModel(`${v.sttCloudModel} (cloud, your key)`);
      else api.installed().then((m) => setModel((m.find((x) => x.active) ?? m[0])?.name ?? 'Not chosen yet'), () => {});
    }, () => {});
  }, []);

  const finish = async (tour: boolean) => {
    try {
      const cur = await api.settingsGet();
      if (!cur.onboarded) await api.settingsSet({ ...cur, onboarded: true });
      try { window.localStorage.removeItem(STEP_KEY); } catch { /* storage unavailable */ }
    } catch (e) {
      toast(errMsg(e), 'error');
    }
    if (!tour) markTourDone();
    navigate('/home');
    // Let the main window mount its tour before asking for it.
    if (tour) setTimeout(startTour, 300);
  };

  return (
    <>
      <div className="onb-body">
        <div className="onb-success" aria-hidden="true"><Icon name="check" size={28} /></div>
        <h1>You're all set</h1>
        <p className="onb-lead">Here is how uttr is set up. You can change any of it later in Settings.</p>
        <ul className="ui-group-body onb-summary" aria-label="Your setup">
          <li className="ui-row"><span className="ui-row-text"><span className="ui-row-label">Shortcut</span></span><span className="ui-row-accessory">{s && keycapsFor(s.hotkey)}<Button variant="plain" aria-label="Change shortcut" onClick={() => goTo(1)}>Change</Button></span></li>
          <li className="ui-row"><span className="ui-row-text"><span className="ui-row-label">Speech</span><span className="ui-row-detail">{model ?? '…'}</span></span><span className="ui-row-accessory"><Button variant="plain" aria-label="Change speech model" onClick={() => goTo(3)}>Change</Button></span></li>
          <li className="ui-row"><span className="ui-row-text"><span className="ui-row-label">Microphone</span><span className="ui-row-detail">{s ? s.mic ?? 'System default' : '…'}</span></span><span className="ui-row-accessory"><Button variant="plain" aria-label="Change microphone" onClick={() => goTo(5)}>Change</Button></span></li>
        </ul>
      </div>
      <footer className="onb-footer">
        <div className="onb-actions">
          <Button variant="plain" size="large" onClick={onBack}>Back</Button>
          <span className="onb-actions-end">
            <Button size="large" onClick={() => finish(false)}>Start using uttr</Button>
            <Button variant="primary" size="large" onClick={() => finish(true)}>Start the tour</Button>
          </span>
        </div>
      </footer>
    </>
  );
}
