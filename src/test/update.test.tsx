import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import UpdateCard from '../components/UpdateCard';
import { api } from '../ipc';
import { resetUpdateState } from '../lib/useUpdate';

const handlers: Record<string, (p: unknown) => void> = {};
vi.mock('../ipc', () => ({
  api: {
    updatePending: vi.fn(),
    updateCheck: vi.fn(),
    updateInstall: vi.fn(),
  },
  on: vi.fn(async (name: string, cb: (p: unknown) => void) => { handlers[name] = cb; return () => {}; }),
}));

const a = api as unknown as Record<string, ReturnType<typeof vi.fn>>;
const flush = () => act(async () => { await Promise.resolve(); await Promise.resolve(); });

afterEach(() => { cleanup(); vi.clearAllMocks(); resetUpdateState(); });

describe('UpdateCard', () => {
  it('hidden_when_no_update_is_known', async () => {
    a.updatePending.mockResolvedValue(null);
    render(<UpdateCard />);
    await flush();
    expect(screen.queryByLabelText('Update available')).toBeNull();
  });

  it('shows_the_new_version_and_installs_on_click', async () => {
    a.updatePending.mockResolvedValue({ version: '0.5.0', current: '0.4.0' });
    a.updateInstall.mockReturnValue(new Promise(() => {}));
    render(<UpdateCard />);
    await flush();
    expect(screen.getByText('Version 0.5.0')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Update and restart' }));
    await flush();
    expect(a.updateInstall).toHaveBeenCalledTimes(1);
    expect(screen.getByRole('progressbar')).toBeTruthy();
  });

  it('appears_when_the_backend_announces_an_update_and_shows_progress', async () => {
    a.updatePending.mockResolvedValue(null);
    render(<UpdateCard />);
    await flush();
    act(() => handlers.updateAvailable({ version: '0.6.0', current: '0.4.0' }));
    expect(screen.getByText('Version 0.6.0')).toBeTruthy();
    act(() => handlers.updateProgress({ downloaded: 50, total: 100, installing: false }));
    expect(screen.getByRole('progressbar').getAttribute('aria-valuenow')).toBe('50');
    act(() => handlers.updateProgress({ downloaded: 0, total: null, installing: true }));
    expect(screen.getByText('Installing…')).toBeTruthy();
  });

  it('a_failed_install_shows_the_error_and_offers_the_button_again', async () => {
    a.updatePending.mockResolvedValue({ version: '0.5.0', current: '0.4.0' });
    a.updateInstall.mockRejectedValue('signature mismatch');
    render(<UpdateCard />);
    await flush();
    fireEvent.click(screen.getByRole('button', { name: 'Update and restart' }));
    await flush();
    expect(screen.getByRole('alert').textContent).toContain('signature mismatch');
    expect(screen.getByRole('button', { name: 'Update and restart' })).toBeTruthy();
  });
});
