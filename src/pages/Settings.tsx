import { useEffect, useState } from 'react';
import { api, type DictEntry } from '../ipc';
import HotkeyInput from '../components/HotkeyInput';
import MicPicker from '../components/MicPicker';
import { useToast } from '../components/Toasts';
import Button from '../components/ui/Button';
import Group from '../components/ui/Group';
import IconButton from '../components/ui/IconButton';
import InfoTip from '../components/ui/InfoTip';
import { keycapsFor } from '../components/ui/Keycap';
import Row from '../components/ui/Row';
import Segmented from '../components/ui/Segmented';
import Switch from '../components/ui/Switch';
import { errMsg } from '../lib/format';
import { isWindows } from '../lib/platform';
import { useTheme, type ThemePref } from '../lib/theme';
import { useSettings } from '../lib/useSettings';
import { useUpdate } from '../lib/useUpdate';
import { navigate } from '../lib/hooks';
import { startTour } from '../lib/tour';

const THEMES: { value: ThemePref; label: string }[] = [{ value: 'system', label: 'System' }, { value: 'light', label: 'Light' }, { value: 'dark', label: 'Dark' }];
const IDLE: [string, number | null][] = [['After 5 minutes', 5], ['After 10 minutes', 10], ['After 30 minutes', 30], ['Never', null]];
const KEEP: [string, number | null][] = [['Forever', null], ['90 days', 90], ['30 days', 30], ['7 days', 7]];
/** A row label with an explanation behind an "i". */
const tip = (label: string, text: string) => <span className="label-tip">{label}<InfoTip text={text} /></span>;

export default function Settings() {
  const { s, error, save } = useSettings();
  const [changing, setChanging] = useState(false);

  if (!s) return <div className="page">{error && <div className="banner-error" role="alert">{error}</div>}</div>;

  return (
    <div className="page">
      <header className="page-head">
        <h1>Settings</h1>
      </header>
      {error && <div className="banner-error" role="alert">{error}</div>}

      <Group title="Dictation" footer="Hold the shortcut to talk, or double-press it to keep listening hands-free. The Command Mode shortcut is on the AI page.">
        <Row
          label="Shortcut"
          accessory={
            <>
              <span role="group" aria-label="Current shortcut">{keycapsFor(s.hotkey)}</span>
              <Button aria-expanded={changing} onClick={() => setChanging((c) => !c)}>Change</Button>
            </>
          }
        />
        {changing && (
          <div className="ui-row ui-row-panel">
            <HotkeyInput value={s.hotkey} onChange={(hotkey) => save({ hotkey })} />
          </div>
        )}
        <Row
          label="Microphone"
          detail="Your computer's built-in mic, or a headset or USB mic."
          accessory={<MicPicker value={s.mic} onChange={(mic) => save({ mic })} />}
        />
        <Row
          label={tip('Show Flow Bar', 'The Flow Bar shows when uttr is listening, transcribing or done, and shows errors. It never takes focus or clicks.')} detail="The small pill at the bottom of the screen."
          accessory={<Switch label="Show Flow Bar" checked={s.showFlowbar} onChange={(showFlowbar) => save({ showFlowbar })} />}
        />
      </Group>

      <Group title="Model" footer="Unloading the model frees memory. The next dictation loads it again.">
        <Row
          label={tip('Unload when idle', 'After this long without dictation, the speech model is removed from memory. The next dictation loads it again, which takes a second or two.')}
          accessory={
            <select className="ui-select" aria-label="Unload when idle" value={String(s.idleUnloadMin)} onChange={(e) => save({ idleUnloadMin: e.target.value === 'null' ? null : Number(e.target.value) })}>
              {IDLE.map(([l, v]) => <option key={l} value={String(v)}>{l}</option>)}
            </select>
          }
        />
        <Row
          label={tip('Load at startup', 'Loads the speech model as soon as uttr opens, so your first dictation is instant. It then uses memory until the idle timeout unloads it.')} detail="Makes the first dictation faster and uses more memory."
          accessory={<Switch label="Load at startup" checked={s.preload} onChange={(preload) => save({ preload })} />}
        />
      </Group>

      <Group title="General">
        <Row label="Open at login" accessory={<Switch label="Open at login" checked={s.launchAtLogin} onChange={(launchAtLogin) => save({ launchAtLogin })} />} />
        <Appearance />
      </Group>

      <Group title="History" footer="Older dictations are deleted automatically. Your stats on Home only count what is kept.">
        <Row
          label={tip('Keep history', 'Dictations are stored only on this computer. Choose a limit if you would rather not keep them.')}
          accessory={
            <select className="ui-select" aria-label="Keep history" value={String(s.historyRetentionDays)} onChange={(e) => save({ historyRetentionDays: e.target.value === 'null' ? null : Number(e.target.value) })}>
              {KEEP.map(([l, v]) => <option key={l} value={String(v)}>{l}</option>)}
            </select>
          }
        />
      </Group>

      <Dictionary />
      <HfToken />
      <Group title="Help">
        <Row label="Tour" detail="A quick walk through every page of uttr." accessory={<Button onClick={startTour}>Take the tour</Button>} />
        <Row label="Setup" detail="Go through the first-run setup again." accessory={<Button onClick={() => navigate('/onboarding')}>Run setup</Button>} />
      </Group>
      <About />
      <Troubleshooting />
    </div>
  );
}

function About() {
  const u = useUpdate();
  const [version, setVersion] = useState('');
  useEffect(() => { Promise.resolve().then(api.appVersion).then(setVersion, () => {}); }, []);
  const status =
    u.phase === 'checking' ? 'Checking for updates…'
      : u.phase === 'upToDate' ? 'You have the latest version.'
        : u.phase === 'available' && u.info ? `Version ${u.info.version} is available.`
          : u.phase === 'downloading' ? `Downloading version ${u.info?.version ?? ''}…`
            : u.phase === 'installing' ? 'Installing. uttr restarts when it is done.'
              : 'uttr checks for updates automatically.';
  return (
    <Group title="About and updates" footer="Updates replace only the app. Your settings, history, snippets, models and keys stay on this computer and are kept.">
      <Row
        label={version ? `uttr ${version}` : 'uttr'}
        detail={u.error ? <span className="ui-row-error">{u.error}</span> : status}
        accessory={
          u.phase === 'available'
            ? <Button variant="primary" onClick={u.install}>Update and restart</Button>
            : <Button disabled={['checking', 'downloading', 'installing'].includes(u.phase)} onClick={u.check}>Check for updates</Button>
        }
      />
      {u.phase === 'available' && u.info?.notes && (
        <div className="ui-row update-notes"><span className="ui-row-text"><span className="ui-row-detail selectable">{u.info.notes}</span></span></div>
      )}
    </Group>
  );
}

function Troubleshooting() {
  const toast = useToast();
  const run = (f: () => Promise<void>) => f().catch((e) => toast(errMsg(e), 'error'));
  return (
    <Group title="Troubleshooting">
      {isWindows() && (
        <Row
          label="Microphone privacy" detail="Windows must allow desktop apps to use the microphone."
          accessory={<Button onClick={() => run(() => api.openPermissionSettings('microphone'))}>Open</Button>}
        />
      )}
      <Row
        label="Log file" detail="Attach it when you report a problem."
        accessory={<Button onClick={() => run(api.openLogFolder)}>Show</Button>}
      />
    </Group>
  );
}

function Appearance() {
  const { pref, setPref } = useTheme();
  return <Row label="Appearance" detail="System follows your computer's light or dark mode." accessory={<Segmented options={THEMES} value={pref} onChange={setPref} />} />;
}

function HfToken() {
  const toast = useToast();
  const [token, setToken] = useState('');
  const store = async (t: string | null) => {
    try {
      await api.hfTokenSet(t);
      setToken('');
      toast(t ? 'Token saved to your keychain' : 'Token removed');
    } catch (e) {
      toast(errMsg(e), 'error');
    }
  };
  return (
    <Group title="Hugging Face" footer="Only needed to download gated or private models. The token is stored in your system keychain.">
      <form className="ui-row ui-row-form" onSubmit={(e) => { e.preventDefault(); if (token.trim()) store(token.trim()); }}>
        <input className="ui-field" type="password" aria-label="Hugging Face token" placeholder="Access token" autoComplete="off" value={token} onChange={(e) => setToken(e.target.value)} />
        <Button type="submit" disabled={!token.trim()}>Save</Button>
        <Button variant="plain" onClick={() => store(null)}>Remove</Button>
      </form>
    </Group>
  );
}

function Dictionary() {
  const toast = useToast();
  const [items, setItems] = useState<DictEntry[]>([]);
  const [term, setTerm] = useState('');
  const [rep, setRep] = useState('');
  const load = () => api.dictionaryList().then(setItems, (e) => toast(errMsg(e), 'error'));
  useEffect(() => { load(); }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const add = async () => {
    if (!term.trim()) return;
    try {
      await api.dictionaryAdd(term.trim(), rep.trim() || undefined);
      setTerm('');
      setRep('');
      load();
    } catch (e) {
      toast(errMsg(e), 'error');
    }
  };
  const del = async (id: number) => {
    try {
      await api.dictionaryDelete(id);
      load();
    } catch (e) {
      toast(errMsg(e), 'error');
    }
  };

  return (
    <Group title="Personal dictionary" footer="Add names and jargon so they are spelled correctly. A replacement rewrites the term as you dictate.">
      {items.map((d) => (
        <Row
          key={d.id}
          label={d.term}
          detail={d.replacement ? `Replaced with ${d.replacement}` : undefined}
          accessory={<IconButton icon="trash" label={`Remove ${d.term}`} tone="destructive" onClick={() => del(d.id)} />}
        />
      ))}
      <form className="ui-row ui-row-form" onSubmit={(e) => { e.preventDefault(); add(); }}>
        <input className="ui-field" aria-label="Term" placeholder="Term" value={term} onChange={(e) => setTerm(e.target.value)} />
        <input className="ui-field" aria-label="Replacement" placeholder="Replacement (optional)" value={rep} onChange={(e) => setRep(e.target.value)} />
        <Button type="submit" disabled={!term.trim()}>Add</Button>
      </form>
    </Group>
  );
}
