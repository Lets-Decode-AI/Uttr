import { useEffect, useRef, useState } from 'react';
import type { Binding, Key } from '../ipc';
import Button from './ui/Button';
import { currentPlatform } from './ui/Keycap';

const CODE_TO_KEY: Record<string, Key> = {
  ControlLeft: 'ControlLeft', ControlRight: 'ControlRight', MetaLeft: 'MetaLeft', MetaRight: 'MetaRight',
  AltLeft: 'AltLeft', AltRight: 'AltRight', ShiftLeft: 'ShiftLeft', ShiftRight: 'ShiftRight', Space: 'Space', Fn: 'Fn',
};

export interface HotkeyPreset { label: string; keys: Key[] }

const PRESETS: HotkeyPreset[] = [
  { label: 'Fn', keys: ['Fn'] },
  { label: 'Right ⌥', keys: ['AltRight'] },
  { label: 'Ctrl+Win/⌘', keys: ['ControlLeft', 'MetaLeft'] },
];

/** The Fn key only reaches apps on macOS, so presets that use it are hidden elsewhere. Linux
 *  calls the Win key Super. */
export function presetsFor(presets: HotkeyPreset[], platform = currentPlatform()): HotkeyPreset[] {
  return presets
    .filter((p) => platform === 'mac' || !p.keys.includes('Fn'))
    .map((p) => ({ ...p, label: localLabel(p.label, platform) }));
}

/** "Ctrl+Win/⌘" becomes "Ctrl+⌘" on a Mac, "Ctrl+Win" on Windows and "Ctrl+Super" on Linux. */
function localLabel(label: string, platform: ReturnType<typeof currentPlatform>): string {
  if (platform === 'mac') return label.replace('Win/⌘', '⌘');
  const named = label.replace('Win/⌘', 'Win').replace(/⌥/g, 'Alt').replace(/⇧/g, 'Shift');
  return platform === 'linux' ? named.replace('Win', 'Super') : named;
}

const same = (a: Binding, b: Key[]) => a.keys.length === b.length && b.every((k) => a.keys.includes(k));

export default function HotkeyInput({ value, onChange, presets = PRESETS }: { value: Binding; onChange: (b: Binding) => void; presets?: HotkeyPreset[] }) {
  const [recording, setRecording] = useState(false);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  useEffect(() => {
    if (!recording) return;
    const held = new Set<Key>();
    const chord: Key[] = [];
    const down = (e: KeyboardEvent) => {
      if (e.code === 'Escape' || e.key === 'Escape') {
        e.preventDefault();
        setRecording(false); // Escape cancels and is never a binding
        return;
      }
      const k = CODE_TO_KEY[e.code] ?? (e.key === 'Fn' ? 'Fn' : undefined);
      if (!k) return;
      e.preventDefault();
      held.add(k);
      if (!chord.includes(k)) chord.push(k);
    };
    const up = (e: KeyboardEvent) => {
      if (e.code === 'Escape') return;
      if (chord.length) {
        e.preventDefault();
        setRecording(false);
        onChangeRef.current({ keys: [...chord] });
      }
    };
    window.addEventListener('keydown', down, true);
    window.addEventListener('keyup', up, true);
    return () => {
      window.removeEventListener('keydown', down, true);
      window.removeEventListener('keyup', up, true);
    };
  }, [recording]);

  return (
    <div className="hotkey">
      <div className="hotkey-choices" role="group" aria-label="Shortcut choices">
        {presetsFor(presets).map((p) => (
          <Button key={p.label} aria-pressed={same(value, p.keys)} onClick={() => { setRecording(false); onChange({ keys: p.keys }); }}>
            {p.label}
          </Button>
        ))}
        <Button onClick={() => setRecording((r) => !r)}>{recording ? 'Cancel' : 'Record shortcut'}</Button>
      </div>
      <div className="hotkey-status muted" aria-live="polite">{recording ? 'Press the keys. Press Esc to cancel.' : ''}</div>
    </div>
  );
}
