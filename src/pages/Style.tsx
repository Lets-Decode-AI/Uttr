import { useEffect, useState } from 'react';
import { api, type AppStyle, type Style as StyleT } from '../ipc';
import Button from '../components/ui/Button';
import Group from '../components/ui/Group';
import IconButton from '../components/ui/IconButton';
import Row from '../components/ui/Row';
import Segmented from '../components/ui/Segmented';
import { useSettings } from '../lib/useSettings';

const STYLES: { value: StyleT; label: string }[] = [
  { value: 'formal', label: 'Formal' },
  { value: 'casual', label: 'Casual' },
  { value: 'veryCasual', label: 'Very casual' },
];
const EXAMPLE: Record<StyleT, string> = {
  formal: 'Sounds good. I will send the notes tonight.',
  casual: 'Sounds good, I’ll send the notes tonight',
  veryCasual: 'sounds good, i’ll send the notes tonight',
};

export default function Style() {
  const { s, error, save } = useSettings();
  const [recent, setRecent] = useState<string[]>([]);
  const [app, setApp] = useState('');
  const [style, setStyle] = useState<StyleT>('casual');
  useEffect(() => { api.recentApps().then(setRecent, () => {}); }, []);

  if (!s) return <div className="page">{error && <div className="banner-error" role="alert">{error}</div>}</div>;

  const rules = s.appStyles;
  const setRules = (appStyles: AppStyle[]) => save({ appStyles });
  const add = () => {
    const name = app.trim();
    if (!name) return;
    setRules([...rules.filter((r) => r.app.toLowerCase() !== name.toLowerCase()), { app: name, style }]);
    setApp('');
  };
  const suggestions = recent.filter((a) => !rules.some((r) => r.app.toLowerCase() === a.toLowerCase()));

  return (
    <div className="page">
      <header className="page-head">
        <h1>Style</h1>
        <p className="muted">Choose how your writing should sound, and pick a different style for each app. With AI on, the AI rewrites in the style; without it, uttr adjusts capitals and the final period.</p>
      </header>
      {error && <div className="banner-error" role="alert">{error}</div>}

      <Group title="Default style" footer={<>Example: {EXAMPLE[s.defaultStyle]}</>}>
        <div className="ui-row">
          <Segmented options={STYLES} value={s.defaultStyle} onChange={(defaultStyle) => save({ defaultStyle })} />
        </div>
      </Group>

      <Group title="Apps" footer="uttr checks which app you are dictating into. Apps without a rule use the default style.">
        {rules.map((r) => (
          <Row
            key={r.app}
            label={r.app}
            accessory={
              <>
                <select className="ui-select" aria-label={`Style for ${r.app}`} value={r.style} onChange={(e) => setRules(rules.map((x) => (x.app === r.app ? { ...x, style: e.target.value as StyleT } : x)))}>
                  {STYLES.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
                </select>
                <IconButton icon="trash" label={`Remove the rule for ${r.app}`} tone="destructive" onClick={() => setRules(rules.filter((x) => x.app !== r.app))} />
              </>
            }
          />
        ))}
        <form className="ui-row ui-row-form" onSubmit={(e) => { e.preventDefault(); add(); }}>
          <input className="ui-field" aria-label="App name" placeholder="App name, for example Slack" list="recent-apps" value={app} onChange={(e) => setApp(e.target.value)} />
          <datalist id="recent-apps">{suggestions.map((a) => <option key={a} value={a} />)}</datalist>
          <select className="ui-select" aria-label="Style for the new app" value={style} onChange={(e) => setStyle(e.target.value as StyleT)}>
            {STYLES.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
          </select>
          <Button type="submit" disabled={!app.trim()}>Add</Button>
        </form>
        {suggestions.length > 0 && rules.length === 0 && (
          <Row label="Recently used" detail={suggestions.slice(0, 6).join(', ')} />
        )}
      </Group>
    </div>
  );
}
