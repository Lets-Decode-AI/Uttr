import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import PermissionBanner from '../components/PermissionBanner';
import { api } from '../ipc';

vi.mock('../ipc', () => ({
  api: {
    permissionsStatus: vi.fn(),
    openPermissionSettings: vi.fn(async () => {}),
    requestMicrophone: vi.fn(async () => {}),
    restartApp: vi.fn(async () => {}),
  },
}));
vi.mock('../components/ui/Keycap', async (orig) => ({
  ...(await orig<typeof import('../components/ui/Keycap')>()),
  currentPlatform: () => 'mac',
}));
vi.mock('../components/Toasts', () => ({ useToast: () => () => {} }));

const a = api as unknown as Record<string, ReturnType<typeof vi.fn>>;
const flush = () => act(async () => { await Promise.resolve(); });

beforeEach(() => vi.useFakeTimers());
afterEach(() => { cleanup(); vi.useRealTimers(); vi.clearAllMocks(); });

describe('PermissionBanner', () => {
  it('hidden_when_everything_is_allowed', async () => {
    a.permissionsStatus.mockResolvedValue({ microphone: 'granted', accessibility: 'granted' });
    render(<PermissionBanner />);
    await flush();
    expect(screen.queryByRole('alert')).toBeNull();
  });

  it('explains_a_missing_accessibility_permission_and_offers_fixes', async () => {
    a.permissionsStatus.mockResolvedValue({ microphone: 'granted', accessibility: 'denied' });
    render(<PermissionBanner />);
    await flush();
    expect(screen.getByRole('alert').textContent).toMatch(/shortcut/i);
    fireEvent.click(screen.getByRole('button', { name: /open accessibility settings/i }));
    expect(a.openPermissionSettings).toHaveBeenCalledWith('accessibility');
    fireEvent.click(screen.getByRole('button', { name: /restart uttr/i }));
    expect(a.restartApp).toHaveBeenCalled();
  });

  it('asks_for_the_microphone_when_it_is_not_allowed', async () => {
    a.permissionsStatus.mockResolvedValue({ microphone: 'unknown', accessibility: 'granted' });
    render(<PermissionBanner />);
    await flush();
    fireEvent.click(screen.getByRole('button', { name: /allow microphone/i }));
    expect(a.requestMicrophone).toHaveBeenCalled();
  });

  it('disappears_once_the_permission_is_granted', async () => {
    a.permissionsStatus.mockResolvedValue({ microphone: 'granted', accessibility: 'denied' });
    render(<PermissionBanner />);
    await flush();
    expect(screen.getByRole('alert')).toBeTruthy();
    a.permissionsStatus.mockResolvedValue({ microphone: 'granted', accessibility: 'granted' });
    await act(async () => { vi.advanceTimersByTime(2000); });
    await flush();
    await flush();
    expect(screen.queryByRole('alert')).toBeNull();
  });
});
