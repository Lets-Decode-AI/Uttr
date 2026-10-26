import { useCallback, useEffect, useState } from 'react';
import { api, type Snippet } from '../ipc';
import { useToast } from '../components/Toasts';
import Button from '../components/ui/Button';
import Group from '../components/ui/Group';
import Icon from '../components/ui/Icon';
import IconButton from '../components/ui/IconButton';
import { errMsg } from '../lib/format';

export default function Snippets() {
  const toast = useToast();
  const [items, setItems] = useState<Snippet[]>([]);
  const [editing, setEditing] = useState<number | null>(null);
  const load = useCallback(() => api.snippetList().then(setItems, (e) => toast(errMsg(e), 'error')), [toast]);
  useEffect(() => { load(); }, [load]);

  const run = async (fn: () => Promise<unknown>) => {
    try {
      await fn();
      await load();
      return true;
    } catch (e) {
      toast(errMsg(e), 'error');
      return false;
    }
  };

  return (
    <div className="page">
      <header className="page-head">
        <h1>Snippets</h1>
        <p className="muted">Say a short phrase and uttr types the saved text instead, such as an address, a meeting link or a sign-off. It works on its own or inside a sentence.</p>
      </header>

      {items.length === 0 && (
        <div className="empty">
          <span className="empty-icon" aria-hidden="true"><Icon name="snippet" size={28} /></span>
          <div className="empty-title">No snippets yet</div>
          <p className="muted">Try one for your email address: say “my email” and get the address.</p>
        </div>
      )}
      {items.length > 0 && (
        <Group>
          {items.map((s) => editing === s.id ? (
            <SnippetForm
              key={s.id} initial={s} submitLabel="Save" onCancel={() => setEditing(null)}
              onSubmit={async (t, x) => { if (await run(() => api.snippetUpdate(s.id, t, x))) setEditing(null); }}
            />
          ) : (
            <div key={s.id} className="ui-row snippet-row" data-testid="snippet-row">
              <span className="ui-row-text">
                <span className="ui-row-label">“{s.trigger}”</span>
                <span className="ui-row-detail snippet-expansion">{s.expansion}</span>
              </span>
              <span className="ui-row-accessory">
                <Button variant="plain" onClick={() => setEditing(s.id)}>Edit</Button>
                <IconButton icon="trash" label={`Remove ${s.trigger}`} tone="destructive" onClick={() => run(() => api.snippetDelete(s.id))} />
              </span>
            </div>
          ))}
        </Group>
      )}

      <Group title="New snippet">
        <SnippetForm submitLabel="Add" onSubmit={(t, x) => run(() => api.snippetAdd(t, x))} clearOnSubmit />
      </Group>
    </div>
  );
}

function SnippetForm({ initial, submitLabel, onSubmit, onCancel, clearOnSubmit }: {
  initial?: Snippet;
  submitLabel: string;
  onSubmit: (trigger: string, expansion: string) => Promise<unknown>;
  onCancel?: () => void;
  clearOnSubmit?: boolean;
}) {
  const [trigger, setTrigger] = useState(initial?.trigger ?? '');
  const [expansion, setExpansion] = useState(initial?.expansion ?? '');
  const ok = trigger.trim() !== '' && expansion.trim() !== '';
  return (
    <form
      className="ui-row snippet-form"
      onSubmit={async (e) => {
        e.preventDefault();
        if (!ok) return;
        const res = await onSubmit(trigger.trim(), expansion);
        if (clearOnSubmit && res !== false) { setTrigger(''); setExpansion(''); }
      }}
    >
      <input className="ui-field" aria-label="When I say" placeholder="When I say, for example: my email" value={trigger} onChange={(e) => setTrigger(e.target.value)} />
      <textarea className="ui-field ai-textarea" aria-label="Type this" placeholder="Type this" rows={3} value={expansion} onChange={(e) => setExpansion(e.target.value)} />
      <div className="ai-try-actions">
        <Button type="submit" variant="primary" disabled={!ok}>{submitLabel}</Button>
        {onCancel && <Button variant="plain" onClick={onCancel}>Cancel</Button>}
      </div>
    </form>
  );
}
