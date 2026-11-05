import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const handlers: Record<string, Array<(p: any) => void>> = {};
const t = (id: number, text: string, raw: string, app: string) => ({
  id, createdAt: Date.now() - id * 1000, text, rawText: raw, appName: app, durationMs: 1000, wordCount: 2, wpm: 120, modelId: 'm',
});

vi.mock('../ipc', () => ({
  api: {
    historyList: vi.fn(), historyDelete: vi.fn(async () => {}), recentApps: vi.fn(), settingsGet: vi.fn(async () => ({ hotkey: { keys: ['Fn'] } })),
  },
  on: vi.fn(async (name: string, cb: (p: any) => void) => {
    (handlers[name] ||= []).push(cb);
    return () => {};
  }),
}));

import { api } from '../ipc';
import History from '../pages/History';
import IconButton from '../components/ui/IconButton';
import Tooltip, { TOOLTIP_DELAY } from '../components/ui/Tooltip';

const a = api as any;
const flush = () => act(async () => {});

beforeEach(() => {
  vi.clearAllMocks();
  a.historyList.mockResolvedValue([t(1, 'Meet at three.', 'um meet at two no three', 'Mail'), t(2, 'Same text', 'Same text', 'Slack')]);
  a.recentApps.mockResolvedValue(['Mail', 'Slack']);
});
afterEach(() => { cleanup(); vi.useRealTimers(); });

describe('History', () => {
  it('filters_by_app', async () => {
    render(<History />);
    await flush();
    fireEvent.change(screen.getByLabelText('Filter by app'), { target: { value: 'Mail' } });
    await act(async () => { await new Promise((r) => setTimeout(r, 350)); });
    expect(a.historyList).toHaveBeenLastCalledWith(0, undefined, 'Mail');
  });

  it('shows_the_original_transcript_when_asked', async () => {
    render(<History />);
    await flush();
    expect(screen.queryByText('um meet at two no three')).toBeNull();
    fireEvent.click(screen.getByRole('switch', { name: 'Show original' }));
    expect(screen.getByText('um meet at two no three')).toBeTruthy();
    // Unchanged dictations do not repeat their text.
    expect(screen.getAllByText('Same text')).toHaveLength(1);
  });
});

describe('Tooltips', () => {
  it('appear_after_a_hover_delay_and_hide_on_escape', async () => {
    vi.useFakeTimers();
    render(<Tooltip content="Explains it"><button type="button">Thing</button></Tooltip>);
    const btn = screen.getByRole('button', { name: 'Thing' });
    fireEvent.mouseEnter(btn);
    expect(screen.queryByRole('tooltip')).toBeNull();
    act(() => { vi.advanceTimersByTime(TOOLTIP_DELAY); });
    const tip = screen.getByRole('tooltip');
    expect(tip.textContent).toBe('Explains it');
    expect(btn.getAttribute('aria-describedby')).toBe(tip.id);
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.queryByRole('tooltip')).toBeNull();
  });

  it('appear_at_once_on_keyboard_focus', () => {
    render(<Tooltip content="Now"><button type="button">F</button></Tooltip>);
    fireEvent.focus(screen.getByRole('button', { name: 'F' }));
    expect(screen.getByRole('tooltip').textContent).toBe('Now');
    fireEvent.blur(screen.getByRole('button', { name: 'F' }));
    expect(screen.queryByRole('tooltip')).toBeNull();
  });

  it('icon_buttons_show_their_label', () => {
    render(<IconButton icon="copy" label="Copy" />);
    fireEvent.focus(screen.getByRole('button', { name: 'Copy' }));
    expect(screen.getByRole('tooltip').textContent).toBe('Copy');
  });
});
