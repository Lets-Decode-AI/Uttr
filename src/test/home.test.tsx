import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const handlers: Record<string, Array<(p: any) => void>> = {};
const days = Array.from({ length: 30 }, (_, i) => ({ date: `2026-10-${String(i + 1).padStart(2, '0')}`, words: i === 29 ? 120 : i * 2 }));
const stats = {
  totalWords: 12345, dictations: 210, speakingMs: 5_400_000, avgWpm: 137.2, currentStreak: 3, longestStreak: 9,
  timeSavedMin: 218.6, days, topApps: [{ app: 'Slack', words: 5000 }, { app: 'Mail', words: 2500 }],
};
const settings = {
  hotkey: { keys: ['Fn'] }, aiProvider: 'off', appStyles: [], defaultStyle: 'formal', sttProvider: 'local', historyRetentionDays: null,
};

vi.mock('../ipc', () => ({
  api: {
    stats: vi.fn(), settingsGet: vi.fn(), installed: vi.fn(), permissionsStatus: vi.fn(), snippetList: vi.fn(), historyList: vi.fn(),
  },
  on: vi.fn(async (name: string, cb: (p: any) => void) => {
    (handlers[name] ||= []).push(cb);
    return () => {};
  }),
}));

import { api } from '../ipc';
import Home from '../pages/Home';
import { TOUR_EVENT } from '../lib/tour';

const a = api as any;
const flush = () => act(async () => {});

beforeEach(() => {
  for (const k of Object.keys(handlers)) delete handlers[k];
  vi.clearAllMocks();
  localStorage.clear();
  a.stats.mockResolvedValue(stats);
  a.settingsGet.mockResolvedValue({ ...settings });
  a.installed.mockResolvedValue([{ id: 'parakeet-v2', active: true }]);
  a.permissionsStatus.mockResolvedValue({ microphone: 'granted', accessibility: 'granted' });
  a.snippetList.mockResolvedValue([]);
  a.historyList.mockResolvedValue([
    { id: 1, createdAt: Date.now(), text: 'Latest dictation here', rawText: 'latest', appName: 'Notes', durationMs: 1, wordCount: 3, wpm: 1, modelId: 'm' },
  ]);
});
afterEach(cleanup);

describe('Home dashboard', () => {
  it('shows_the_four_stat_tiles', async () => {
    render(<Home />);
    await flush();
    expect(a.stats).toHaveBeenCalledWith(new Date().getTimezoneOffset());
    const tile = (name: string) => screen.getByRole('group', { name });
    expect(within(tile('Words dictated')).getByText('12,345')).toBeTruthy();
    expect(within(tile('Average speed')).getByText('137 WPM')).toBeTruthy();
    expect(within(tile('Day streak')).getByText('3 days')).toBeTruthy();
    expect(within(tile('Time saved')).getByText('3 h 39 min')).toBeTruthy();
  });

  it('charts_thirty_days_and_lists_top_apps', async () => {
    render(<Home />);
    await flush();
    const chart = screen.getByRole('img', { name: /words per day, last 30 days/i });
    expect(chart.querySelectorAll('[data-testid="bar"]').length).toBe(30);
    expect(screen.getByText('Slack')).toBeTruthy();
    expect(screen.getByText('Latest dictation here')).toBeTruthy();
  });

  it('get_started_ticks_from_real_state_and_can_be_dismissed', async () => {
    render(<Home />);
    await flush();
    const list = screen.getByRole('list', { name: 'Get started' });
    const item = (n: RegExp) => within(list).getByText(n).closest('li') as HTMLElement;
    expect(item(/speech model/i).dataset.done).toBe('true');
    expect(item(/first dictation/i).dataset.done).toBe('true');
    expect(item(/turn on ai/i).dataset.done).toBe('false');
    expect(item(/add a snippet/i).dataset.done).toBe('false');
    fireEvent.click(screen.getByRole('button', { name: 'Hide' }));
    expect(screen.queryByRole('list', { name: 'Get started' })).toBeNull();
    expect(localStorage.getItem('uttr.getStarted.hidden')).toBe('1');
  });

  it('take_the_tour_starts_the_walkthrough', async () => {
    const seen = vi.fn();
    window.addEventListener(TOUR_EVENT, seen);
    render(<Home />);
    await flush();
    fireEvent.click(screen.getByRole('button', { name: 'Take the tour' }));
    expect(seen).toHaveBeenCalled();
    window.removeEventListener(TOUR_EVENT, seen);
  });

  it('first_run_shows_an_empty_state_instead_of_zeros', async () => {
    a.stats.mockResolvedValue({ ...stats, totalWords: 0, dictations: 0, speakingMs: 0, avgWpm: 0, currentStreak: 0, timeSavedMin: 0, topApps: [], days: days.map((d) => ({ ...d, words: 0 })) });
    a.historyList.mockResolvedValue([]);
    render(<Home />);
    await flush();
    expect(screen.getByText(/your stats appear after your first dictation/i)).toBeTruthy();
  });

  it('refreshes_after_a_new_dictation', async () => {
    render(<Home />);
    await flush();
    const n = a.stats.mock.calls.length;
    await act(async () => handlers['transcriptAdded']?.forEach((h) => h({ id: 2 })));
    expect(a.stats.mock.calls.length).toBeGreaterThan(n);
  });
});
