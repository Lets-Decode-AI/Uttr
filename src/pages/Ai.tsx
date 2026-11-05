import { useCallback, useEffect, useState } from 'react';
import { api, type AiKeysStatus, type AiProvider, type CatalogEntry, type DownloadProgress, type InstalledModelDto, type RouterModel, type Settings } from '../ipc';
import HotkeyInput, { type HotkeyPreset } from '../components/HotkeyInput';
import { KeyForm, TextSetting } from '../components/SettingFields';
import ModelRow from '../components/ModelRow';
import { useToast } from '../components/Toasts';
import Button from '../components/ui/Button';
import Group from '../components/ui/Group';
import { keycapsFor } from '../components/ui/Keycap';
import Row from '../components/ui/Row';
import Segmented from '../components/ui/Segmented';
import { errMsg, fmtBytes } from '../lib/format';
import { useEvent } from '../lib/hooks';
import { useSettings } from '../lib/useSettings';

const PROVIDERS: { value: AiProvider; label: string }[] = [
  { value: 'off', label: 'Off' },
  { value: 'local', label: 'On this computer' },
  { value: 'openAi', label: 'OpenAI' },
  { value: 'anthropic', label: 'Anthropic' },
  { value: 'openRouter', label: 'OpenRouter' },
];

const COMMAND_PRESETS: HotkeyPreset[] = [
  { label: 'Fn+Ctrl', keys: ['Fn', 'ControlLeft'] },
  { label: 'Right ⌥+Right ⇧', keys: ['AltRight', 'ShiftRight'] },
  { label: 'Ctrl+Win/⌘+Alt', keys: ['ControlLeft', 'MetaLeft', 'AltLeft'] },
];

const FOOTERS: Record<AiProvider, string> = {
  off: 'Without AI, uttr applies your dictionary, snippets and basic fixes. Command Mode needs AI.',
  local: 'The model runs on this computer, so your text never leaves it. Larger models write better and need more memory.',
  openAi: 'Your dictated text is sent to OpenAI, or to the server you set below. Audio always stays on this computer.',
  anthropic: 'Your dictated text is sent to Anthropic. Audio always stays on this computer.',
  openRouter: 'Your dictated text is sent to OpenRouter, which passes it to the model you pick. Audio always stays on this computer.',
};

export default function Ai() {
  const { s, error, save } = useSettings();
  const [keys, setKeys] = useState<AiKeysStatus>({ openAi: false, anthropic: false, openRouter: false, speech: false });
  const refreshKeys = useCallback(() => api.aiKeysStatus().then(setKeys, () => {}), []);
  useEffect(() => { refreshKeys(); }, [refreshKeys]);

  if (!s) return <div className="page">{error && <div className="banner-error" role="alert">{error}</div>}</div>;

  return (
    <div className="page">
      <header className="page-head">
        <h1>AI</h1>
        <p className="muted">AI cleans up what you dictate: it removes filler words, applies your corrections and follows your style. It also powers Command Mode.</p>
      </header>
      {error && <div className="banner-error" role="alert">{error}</div>}

      <Group title="Provider" footer={FOOTERS[s.aiProvider]}>
        <div className="ui-row">
          <Segmented options={PROVIDERS} value={s.aiProvider} onChange={(aiProvider) => save({ aiProvider })} />
        </div>
      </Group>

      {s.aiProvider === 'local' && <LocalModels />}
      {s.aiProvider === 'openAi' && (
        <Group title="OpenAI" footer="Leave the server address as it is for OpenAI. For Ollama or LM Studio, use its address, for example http://localhost:11434/v1. Those need no key.">
          <KeyForm provider="openAi" label="OpenAI API key" stored={keys.openAi} onChange={refreshKeys} />
          <TextSetting label="Model" value={s.openaiModel} onSave={(openaiModel) => save({ openaiModel })} />
          <TextSetting label="Server address" value={s.openaiBaseUrl} onSave={(openaiBaseUrl) => save({ openaiBaseUrl })} />
        </Group>
      )}
      {s.aiProvider === 'anthropic' && (
        <Group title="Anthropic" footer="Keys are stored in your system keychain.">
          <KeyForm provider="anthropic" label="Anthropic API key" stored={keys.anthropic} onChange={refreshKeys} />
          <TextSetting label="Model" value={s.anthropicModel} onSave={(anthropicModel) => save({ anthropicModel })} />
        </Group>
      )}

      {s.aiProvider === 'openRouter' && (
        <Group title="OpenRouter" footer="One key for models from Anthropic, OpenAI, Google, Meta, Mistral and more. Prices are per million tokens; a dictation uses a few hundred.">
          <KeyForm provider="openRouter" label="OpenRouter API key" stored={keys.openRouter} onChange={refreshKeys} />
          <RouterModelPicker value={s.openrouterModel} onSave={(openrouterModel) => save({ openrouterModel })} />
        </Group>
      )}

      <CommandMode s={s} save={save} />
      {s.aiProvider !== 'off' && <TryIt />}
    </div>
  );
}

function CommandMode({ s, save }: { s: Settings; save: (p: Partial<Settings>) => void }) {
  const [changing, setChanging] = useState(false);
  return (
    <Group title="Command Mode" footer="Select text, hold this shortcut and say what to do, for example “make this more friendly” or “translate to Spanish”. With nothing selected, uttr writes new text at the cursor. You can also start dictating and add the extra key.">
      <Row
        label="Shortcut"
        accessory={
          <>
            <span role="group" aria-label="Current Command Mode shortcut">{keycapsFor(s.commandHotkey)}</span>
            <Button aria-expanded={changing} onClick={() => setChanging((c) => !c)}>Change</Button>
          </>
        }
      />
      {changing && (
        <div className="ui-row ui-row-panel">
          <HotkeyInput value={s.commandHotkey} presets={COMMAND_PRESETS} onChange={(commandHotkey) => save({ commandHotkey })} />
        </div>
      )}
    </Group>
  );
}

const price = (n: number) => `$${Number.isInteger(n) ? n : n.toFixed(n < 1 ? 2 : 1).replace(/0$/, '')}`;
/** "$0.25 / $2 per 1M tokens", "Free", or nothing when OpenRouter lists no price. */
export function priceLabel(m: RouterModel): string {
  if (m.inputPerM === null || m.outputPerM === null) return '';
  if (m.inputPerM === 0 && m.outputPerM === 0) return 'Free';
  return `${price(m.inputPerM)} / ${price(m.outputPerM)} per 1M tokens`;
}

/** Searchable OpenRouter model list; a plain text field when the list cannot be loaded. */
function RouterModelPicker({ value, onSave }: { value: string; onSave: (id: string) => void }) {
  const [models, setModels] = useState<RouterModel[] | null>(null);
  const [failed, setFailed] = useState(false);
  const [q, setQ] = useState('');
  useEffect(() => {
    let live = true;
    api.openrouterModels().then((m) => live && setModels(m), () => live && setFailed(true));
    return () => { live = false; };
  }, []);
  if (failed) return <TextSetting label="Model" value={value} onSave={onSave} />;
  const term = q.trim().toLowerCase();
  const hits = term ? (models ?? []).filter((m) => m.id.toLowerCase().includes(term) || m.name.toLowerCase().includes(term)).slice(0, 8) : [];
  const current = models?.find((m) => m.id === value);
  return (
    <>
      <Row label="Model" detail={current ? `${current.name}${priceLabel(current) ? ` · ${priceLabel(current)}` : ''}` : undefined} accessory={<code className="ai-model-id">{value}</code>} />
      <div className="ui-row ai-picker">
        <input className="ui-field" aria-label="Search OpenRouter models" placeholder={models ? `Search ${models.length} models` : 'Loading models…'} value={q} onChange={(e) => setQ(e.target.value)} disabled={!models} />
        {hits.length > 0 && (
          <ul className="ai-picker-list" role="listbox" aria-label="OpenRouter models">
            {hits.map((m) => (
              <li key={m.id} role="option" aria-selected={m.id === value} tabIndex={0} className="ai-picker-item"
                onClick={() => { onSave(m.id); setQ(''); }}
                onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onSave(m.id); setQ(''); } }}>
                <span className="ai-picker-name">{m.name}</span>
                <span className="ai-picker-meta">{[m.id, priceLabel(m)].filter(Boolean).join(' · ')}</span>
              </li>
            ))}
          </ul>
        )}
      </div>
    </>
  );
}

function LocalModels() {
  const toast = useToast();
  const [catalog, setCatalog] = useState<CatalogEntry[]>([]);
  const [installed, setInstalled] = useState<InstalledModelDto[]>([]);
  const [progress, setProgress] = useState<Record<string, DownloadProgress>>({});

  const refresh = useCallback(async () => {
    try {
      const [c, i] = await Promise.all([api.llmCatalog(), api.llmInstalled()]);
      setCatalog(c);
      setInstalled(i);
    } catch (e) {
      toast(errMsg(e), 'error');
    }
  }, [toast]);
  useEffect(() => { refresh(); }, [refresh]);
  useEvent('progress', (p) => {
    if (!catalog.some((c) => c.id === p.modelId)) return;
    setProgress((m) => ({ ...m, [p.modelId]: p }));
    if (p.status === 'Installed') refresh();
  });

  const run = async (fn: () => Promise<unknown>, after?: () => void) => {
    try {
      await fn();
      after?.();
    } catch (e) {
      toast(errMsg(e), 'error');
    }
    refresh();
  };
  const drop = (id: string) => () => setProgress(({ [id]: _, ...rest }) => rest);

  return (
    <Group title="On-device model" footer={installed.length === 0 ? 'Download a model to turn on AI cleanup. It is used whenever AI is set to On this computer.' : undefined}>
      {catalog.map((e) => {
        const inst = installed.find((m) => m.id === e.id);
        const bytes = inst ? inst.diskBytes : e.files.reduce((n, f) => n + f.size, 0);
        return (
          <ModelRow
            key={e.id} id={e.id} name={e.name} meta={`${fmtBytes(bytes)} · needs ${e.min_ram_gb} GB of memory`} note={e.notes || undefined}
            installed={!!inst} active={inst?.active ?? false} progress={progress[e.id]}
            onInstall={() => run(() => api.modelInstall(e.id))}
            onPause={() => run(() => api.modelPause(e.id))}
            onResume={() => run(() => api.modelResume(e.id))}
            onCancel={() => run(() => api.modelCancel(e.id), drop(e.id))}
            onActivate={() => run(() => api.modelSetActive(e.id))}
            onDelete={() => { if (window.confirm('Delete this model? You can download it again later.')) run(() => api.modelDelete(e.id), drop(e.id)); }}
          />
        );
      })}
    </Group>
  );
}

function TryIt() {
  const [text, setText] = useState('um so I think we should uh meet on Tuesday no wait Wednesday at ten');
  const [out, setOut] = useState<{ ok: boolean; text: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const go = async (command: boolean) => {
    setBusy(true);
    setOut(null);
    try {
      setOut({ ok: true, text: await api.aiTry(text, command) });
    } catch (e) {
      setOut({ ok: false, text: errMsg(e) });
    } finally {
      setBusy(false);
    }
  };
  return (
    <Group title="Try it" footer="Type what you might say and see what the AI makes of it. Clean up uses your default style.">
      <div className="ui-row ai-try">
        <textarea className="ui-field ai-textarea" aria-label="Text to try" rows={3} value={text} onChange={(e) => setText(e.target.value)} />
        <div className="ai-try-actions">
          <Button variant="primary" disabled={busy || !text.trim()} onClick={() => go(false)}>Clean up</Button>
          <Button disabled={busy || !text.trim()} onClick={() => go(true)}>Run as a command</Button>
          {busy && <span className="muted" aria-live="polite">Working…</span>}
        </div>
        {out && <div className={`ai-try-out${out.ok ? '' : ' ui-row-error'}`} role="status" data-testid="ai-try-result">{out.text}</div>}
      </div>
    </Group>
  );
}
