import { useEffect, useState } from 'react';
import { api, type KeyFor } from '../ipc';
import { errMsg } from '../lib/format';
import { useToast } from './Toasts';
import Button from './ui/Button';

/** A text setting saved when the field loses focus or Return is pressed. */
export function TextSetting({ label, value, onSave }: { label: string; value: string; onSave: (v: string) => void }) {
  const [v, setV] = useState(value);
  useEffect(() => setV(value), [value]);
  const commit = () => { if (v.trim() && v.trim() !== value) onSave(v.trim()); else setV(value); };
  return (
    <form className="ui-row ui-row-form" onSubmit={(e) => { e.preventDefault(); commit(); }}>
      <span className="ui-row-label ai-field-label">{label}</span>
      <input className="ui-field" aria-label={label} value={v} spellCheck={false} onChange={(e) => setV(e.target.value)} onBlur={commit} />
    </form>
  );
}

export function KeyForm({ provider, label, stored, onChange }: { provider: KeyFor; label: string; stored: boolean; onChange: () => void }) {
  const toast = useToast();
  const [key, setKey] = useState('');
  const store = async (k: string | null) => {
    try {
      await api.aiKeySet(provider, k);
      setKey('');
      toast(k ? 'Key saved to your keychain' : 'Key removed');
      onChange();
    } catch (e) {
      toast(errMsg(e), 'error');
    }
  };
  return (
    <form className="ui-row ui-row-form" onSubmit={(e) => { e.preventDefault(); if (key.trim()) store(key.trim()); }}>
      <input className="ui-field" type="password" aria-label={label} placeholder={stored ? 'Key saved. Enter a new one to replace it.' : 'API key'} autoComplete="off" value={key} onChange={(e) => setKey(e.target.value)} />
      <Button type="submit" disabled={!key.trim()}>Save</Button>
      {stored && <Button variant="plain" onClick={() => store(null)}>Remove</Button>}
    </form>
  );
}

