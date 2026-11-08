import type { ReactNode } from 'react';
import type { Binding, Key } from '../../ipc';

export type Platform = 'mac' | 'linux' | 'other';

export const currentPlatform = (): Platform => {
  if (typeof navigator === 'undefined') return 'other';
  if (/Mac/i.test(navigator.platform)) return 'mac';
  if (/Linux/i.test(navigator.platform)) return 'linux';
  return 'other';
};

export function Keycap({ children }: { children: ReactNode }) {
  return <kbd className="ui-keycap">{children}</kbd>;
}

const GLYPHS: Record<Platform, Record<Key, string>> = {
  mac: {
    Fn: 'fn', ControlLeft: '⌃', ControlRight: '⌃', AltLeft: '⌥', AltRight: '⌥',
    ShiftLeft: '⇧', ShiftRight: '⇧', MetaLeft: '⌘', MetaRight: '⌘', Space: 'Space', Escape: 'Esc',
  },
  other: {
    Fn: 'Fn', ControlLeft: 'Ctrl', ControlRight: 'Ctrl', AltLeft: 'Alt', AltRight: 'Alt',
    ShiftLeft: 'Shift', ShiftRight: 'Shift', MetaLeft: 'Win', MetaRight: 'Win', Space: 'Space', Escape: 'Esc',
  },
  linux: {
    Fn: 'Fn', ControlLeft: 'Ctrl', ControlRight: 'Ctrl', AltLeft: 'Alt', AltRight: 'AltGr',
    ShiftLeft: 'Shift', ShiftRight: 'Shift', MetaLeft: 'Super', MetaRight: 'Super', Space: 'Space', Escape: 'Esc',
  },
};

/** Text label for a chord, such as "Ctrl+Win" or "fn+⌃". */
export function chordLabel(keys: Key[], platform: Platform = currentPlatform()): string {
  return keys.map((k) => GLYPHS[platform][k]).join('+');
}

/** Render a hotkey binding as a row of keycaps. */
export function keycapsFor(binding: Binding, platform: Platform = currentPlatform()): ReactNode {
  return (
    <span className="ui-keycaps">
      {binding.keys.map((k) => <Keycap key={k}>{GLYPHS[platform][k]}</Keycap>)}
    </span>
  );
}

export default Keycap;
