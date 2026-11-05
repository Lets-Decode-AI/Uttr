import { useCallback, useEffect, useState } from 'react';
import { api, type PermissionsStatus, type Settings, type Stats, type Transcript } from '../ipc';
import Button from '../components/ui/Button';
import Group from '../components/ui/Group';
import Icon, { type IconName } from '../components/ui/Icon';
import { keycapsFor } from '../components/ui/Keycap';
import InfoTip from '../components/ui/InfoTip';
import { fmtClock } from '../lib/format';
import { navigate, useEvent } from '../lib/hooks';
import { startTour, tourDone } from '../lib/tour';

const HIDE_KEY = 'uttr.getStarted.hidden';

/** "3 h 39 min", "12 min", "under a minute". */
export function fmtMinutes(min: number): string {
  const m = Math.round(min);
  if (m < 1) return min > 0 ? 'under a minute' : '0 min';
  const h = Math.floor(m / 60);
  return h ? `${h} h ${m % 60} min` : `${m} min`;
}

function greeting(d = new Date()): string {
  const h = d.getHours();
  return h < 12 ? 'Good morning' : h < 18 ? 'Good afternoon' : 'Good evening';
}

function Tile({ label, value, hint, icon }: { label: string; value: string; hint: string; icon: IconName }) {
  return (
    <div className="home-tile" role="group" aria-label={label}>
      <span className="home-tile-head"><Icon name={icon} size={14} />{label}<InfoTip text={hint} /></span>
      <span className="home-tile-value">{value}</span>
    </div>
  );
}

function Chart({ days }: { days: Stats['days'] }) {
  const max = Math.max(1, ...days.map((d) => d.words));
  const total = days.reduce((n, d) => n + d.words, 0);
  return (
    <div className="home-chart" role="img" aria-label={`Words per day, last 30 days: ${total.toLocaleString()} words in total`}>
      {days.map((d) => (
        <span key={d.date} className="home-bar-slot" title={`${new Date(`${d.date}T12:00`).toLocaleDateString(undefined, { month: 'short', day: 'numeric' })}: ${d.words.toLocaleString()} words`}>
          <span data-testid="bar" className={`home-bar${d.words ? '' : ' home-bar-empty'}`} style={{ height: `${Math.max(4, (d.words / max) * 100)}%` }} />
        </span>
      ))}
    </div>
  );
}

interface Step { id: string; label: string; done: boolean; action: string; go: () => void }

function GetStarted({ steps, onHide }: { steps: Step[]; onHide: () => void }) {
  const done = steps.filter((s) => s.done).length;
  return (
    <section className="ui-group home-start">
      <div className="home-start-head">
        <div>
          <h2 className="ui-group-title home-start-title">Get started</h2>
          <p className="muted">{done} of {steps.length} done</p>
        </div>
        <Button variant="plain" onClick={onHide}>Hide</Button>
      </div>
      <div className="home-progress" aria-hidden="true"><span style={{ width: `${(done / steps.length) * 100}%` }} /></div>
      <ul className="ui-group-body home-steps" aria-label="Get started">
        {steps.map((s) => (
          <li key={s.id} className="ui-row home-step" data-done={String(s.done)}>
            <span className={`home-check${s.done ? ' home-check-on' : ''}`} aria-hidden="true">{s.done && <Icon name="check" size={12} />}</span>
            <span className="ui-row-text"><span className="ui-row-label">{s.label}</span></span>
            {!s.done && <span className="ui-row-accessory"><Button onClick={s.go}>{s.action}</Button></span>}
          </li>
        ))}
      </ul>
    </section>
  );
}

export default function Home() {
  const [stats, setStats] = useState<Stats | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [recent, setRecent] = useState<Transcript[]>([]);
  const [speechReady, setSpeechReady] = useState(false);
  const [perms, setPerms] = useState<PermissionsStatus | null>(null);
  const [snippets, setSnippets] = useState(0);
  const [hidden, setHidden] = useState(() => { try { return localStorage.getItem(HIDE_KEY) === '1'; } catch { return false; } });

  const load = useCallback(() => {
    api.stats(new Date().getTimezoneOffset()).then(setStats, () => {});
    api.historyList(0).then((r) => setRecent(r.slice(0, 3)), () => {});
    api.settingsGet().then((s) => {
      setSettings(s);
      if (s.sttProvider === 'cloud') setSpeechReady(true);
      else api.installed().then((m) => setSpeechReady(m.length > 0), () => {});
    }, () => {});
    api.permissionsStatus().then(setPerms, () => {});
    api.snippetList().then((l) => setSnippets(l.length), () => {});
  }, []);
  useEffect(() => { load(); }, [load]);
  useEvent('transcriptAdded', load);

  const ok = (p?: string) => p === 'granted' || p === 'notRequired';
  const steps: Step[] = [
    { id: 'model', label: 'Set up a speech model', done: speechReady, action: 'Choose', go: () => navigate('/models') },
    { id: 'perms', label: 'Allow the microphone and Accessibility', done: !!perms && ok(perms.microphone) && ok(perms.accessibility), action: 'Fix', go: () => navigate('/settings') },
    { id: 'first', label: 'Make your first dictation', done: (stats?.dictations ?? 0) > 0, action: 'How', go: () => navigate('/onboarding') },
    { id: 'ai', label: 'Turn on AI cleanup', done: !!settings && settings.aiProvider !== 'off', action: 'Open AI', go: () => navigate('/ai') },
    { id: 'snippet', label: 'Add a snippet', done: snippets > 0, action: 'Add', go: () => navigate('/snippets') },
    { id: 'style', label: 'Pick a style for an app', done: !!settings && (settings.appStyles.length > 0 || settings.defaultStyle !== 'formal'), action: 'Open Style', go: () => navigate('/style') },
    { id: 'tour', label: 'Take the tour', done: tourDone(), action: 'Start', go: startTour },
  ];
  const hide = () => {
    setHidden(true);
    try { localStorage.setItem(HIDE_KEY, '1'); } catch { /* convenience only */ }
  };
  const empty = stats !== null && stats.dictations === 0;

  return (
    <div className="page">
      <header className="page-head page-head-row">
        <div>
          <h1>{greeting()}</h1>
          {settings && <p className="muted home-sub">Hold {keycapsFor(settings.hotkey)} in any app and start talking.</p>}
        </div>
        <Button onClick={startTour}><Icon name="sparkle" size={14} />Take the tour</Button>
      </header>

      {!hidden && steps.some((s) => !s.done) && <GetStarted steps={steps} onHide={hide} />}

      {empty && (
        <div className="empty home-empty">
          <span className="empty-icon" aria-hidden="true"><Icon name="mic" size={28} /></span>
          <div className="empty-title">Your stats appear after your first dictation</div>
          <p className="muted">Words, speed, streaks and the time you save show up here.</p>
        </div>
      )}

      {stats && !empty && (
        <>
          <div className="home-tiles">
            <Tile label="Words dictated" icon="mic" value={stats.totalWords.toLocaleString()} hint={`Across ${stats.dictations.toLocaleString()} dictations.`} />
            <Tile label="Average speed" icon="sparkle" value={`${Math.round(stats.avgWpm)} WPM`} hint="Words per minute while you speak. Typing is about 40." />
            <Tile label="Day streak" icon="history" value={`${stats.currentStreak} ${stats.currentStreak === 1 ? 'day' : 'days'}`} hint={`Days in a row with a dictation. Your longest: ${stats.longestStreak}.`} />
            <Tile label="Time saved" icon="check" value={fmtMinutes(stats.timeSavedMin)} hint="Compared with typing the same words at 40 words per minute." />
          </div>
          <Group title="Last 30 days"><div className="ui-row home-chart-row"><Chart days={stats.days} /></div></Group>
          <div className="home-columns">
            <Group title="Top apps">
              {stats.topApps.length === 0 && <div className="ui-row muted">No app names yet.</div>}
              {stats.topApps.map((t) => (
                <div key={t.app} className="ui-row"><span className="ui-row-text"><span className="ui-row-label">{t.app}</span></span><span className="ui-row-accessory muted">{t.words.toLocaleString()} words</span></div>
              ))}
            </Group>
            <Group title="Recent" footer={<a href="#/history">See all history</a>}>
              {recent.map((t) => (
                <div key={t.id} className="ui-row home-recent"><span className="ui-row-text"><span className="ui-row-label home-recent-text">{t.text}</span><span className="ui-row-detail">{[t.appName, fmtClock(t.createdAt)].filter(Boolean).join(' · ')}</span></span></div>
              ))}
            </Group>
          </div>
        </>
      )}
    </div>
  );
}
