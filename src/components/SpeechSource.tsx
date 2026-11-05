import { useCallback, useEffect, useState } from 'react';
import { api, type Settings, type SttProvider } from '../ipc';
import { useSettings } from '../lib/useSettings';
import { KeyForm, TextSetting } from './SettingFields';
import Group from './ui/Group';
import Row from './ui/Row';
import Segmented from './ui/Segmented';

const SOURCES: { value: SttProvider; label: string }[] = [
  { value: 'local', label: 'On this computer' },
  { value: 'cloud', label: 'Cloud (your key)' },
];

/** Ready-made settings for common OpenAI-compatible transcription services. */
export const SPEECH_PRESETS = [
  { id: 'openai', label: 'OpenAI', baseUrl: 'https://api.openai.com/v1', model: 'whisper-1' },
  { id: 'groq', label: 'Groq', baseUrl: 'https://api.groq.com/openai/v1', model: 'whisper-large-v3-turbo' },
] as const;

const presetOf = (s: Settings) => SPEECH_PRESETS.find((p) => p.baseUrl === s.sttCloudBaseUrl.replace(/\/$/, ''))?.id ?? 'custom';

/** Where speech is turned into text: on-device models, or a cloud service with your own key (BYOK). */
export default function SpeechSource({ onChange }: { onChange?: (p: SttProvider) => void }) {
  const { s, error, save } = useSettings();
  const [speechKey, setSpeechKey] = useState(false);
  const refreshKey = useCallback(() => api.aiKeysStatus().then((k) => setSpeechKey(k.speech), () => {}), []);
  useEffect(() => { refreshKey(); }, [refreshKey]);
  useEffect(() => { if (s) onChange?.(s.sttProvider); }, [s?.sttProvider]); // eslint-disable-line react-hooks/exhaustive-deps
  if (!s) return error ? <div className="banner-error" role="alert">{error}</div> : null;

  const preset = presetOf(s);
  const cloud = s.sttProvider === 'cloud';
  const host = preset === 'custom' ? 'your server' : SPEECH_PRESETS.find((p) => p.id === preset)?.label;
  return (
    <>
      {error && <div className="banner-error" role="alert">{error}</div>}
      <Group
        title="Speech recognition"
        footer={cloud
          ? `Bring your own key: your audio is sent to ${host} for transcription and billed to your account. AI cleanup, snippets and styles still apply.`
          : 'Speech is turned into text on this computer. Your audio never leaves it.'}
      >
        <div className="ui-row"><Segmented options={SOURCES} value={s.sttProvider} onChange={(sttProvider) => save({ sttProvider })} /></div>
        {cloud && (
          <>
            <Row
              label="Speech service"
              accessory={
                <select className="ui-select" aria-label="Speech service" value={preset} onChange={(e) => {
                  const p = SPEECH_PRESETS.find((x) => x.id === e.target.value);
                  if (p) save({ sttCloudBaseUrl: p.baseUrl, sttCloudModel: p.model });
                  else save({ sttCloudBaseUrl: 'http://localhost:8000/v1' });
                }}>
                  {SPEECH_PRESETS.map((p) => <option key={p.id} value={p.id}>{p.label}</option>)}
                  <option value="custom">Other (OpenAI-compatible)</option>
                </select>
              }
            />
            <KeyForm provider="speech" label="Speech API key" stored={speechKey} onChange={refreshKey} />
            {preset === 'custom' && <TextSetting label="Server address" value={s.sttCloudBaseUrl} onSave={(sttCloudBaseUrl) => save({ sttCloudBaseUrl })} />}
            <TextSetting label="Model" value={s.sttCloudModel} onSave={(sttCloudModel) => save({ sttCloudModel })} />
            <LanguageSetting value={s.sttCloudLanguage} onSave={(sttCloudLanguage) => save({ sttCloudLanguage })} />
          </>
        )}
      </Group>
    </>
  );
}

/** Optional language code; clearing it lets the service detect the language. */
function LanguageSetting({ value, onSave }: { value: string; onSave: (v: string) => void }) {
  const [v, setV] = useState(value);
  useEffect(() => setV(value), [value]);
  const commit = () => { if (v.trim() !== value) onSave(v.trim()); };
  return (
    <form className="ui-row ui-row-form" onSubmit={(e) => { e.preventDefault(); commit(); }}>
      <span className="ui-row-label ai-field-label">Language</span>
      <input className="ui-field" aria-label="Language" placeholder="Detect automatically, or a code such as en" value={v} spellCheck={false} onChange={(e) => setV(e.target.value)} onBlur={commit} />
    </form>
  );
}
