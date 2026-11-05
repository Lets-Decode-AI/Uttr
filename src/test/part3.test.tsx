import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const handlers: Record<string, Array<(p: any) => void>> = {};
const base = {
  hotkey: { keys: ['Fn'] }, mic: null, showFlowbar: true, idleUnloadMin: 10, preload: false, launchAtLogin: false, onboarded: true,
  commandHotkey: { keys: ['Fn', 'ControlLeft'] }, aiProvider: 'off', openaiModel: 'gpt-5-mini', openaiBaseUrl: 'https://api.openai.com/v1',
  anthropicModel: 'claude-opus-5-5', openrouterModel: 'anthropic/claude-opus-5.5', defaultStyle: 'formal', appStyles: [],
  sttProvider: 'local', sttCloudBaseUrl: 'https://api.openai.com/v1', sttCloudModel: 'whisper-1', sttCloudLanguage: '',
  historyRetentionDays: null,
};

vi.mock('../ipc', () => ({
  api: {
    settingsGet: vi.fn(), settingsSet: vi.fn(async () => {}),
    llmCatalog: vi.fn(async () => []), llmInstalled: vi.fn(async () => []), aiKeysStatus: vi.fn(), aiKeySet: vi.fn(async () => {}),
    aiTry: vi.fn(), openrouterModels: vi.fn(),
    catalog: vi.fn(async () => []), installed: vi.fn(async () => []), hfSearch: vi.fn(async () => []),
    modelInstall: vi.fn(async () => {}), modelPause: vi.fn(async () => {}), modelResume: vi.fn(async () => {}),
    modelCancel: vi.fn(async () => {}), modelDelete: vi.fn(async () => {}), modelSetActive: vi.fn(async () => {}),
  },
  on: vi.fn(async (name: string, cb: (p: any) => void) => {
    (handlers[name] ||= []).push(cb);
    return () => {};
  }),
}));

import { api } from '../ipc';
import Ai from '../pages/Ai';
import Models from '../pages/Models';

const a = api as any;
const flush = () => act(async () => {});
const lastSaved = () => a.settingsSet.mock.calls.at(-1)[0];

beforeEach(() => {
  for (const k of Object.keys(handlers)) delete handlers[k];
  vi.clearAllMocks();
  a.settingsGet.mockResolvedValue({ ...base });
  a.aiKeysStatus.mockResolvedValue({ openAi: false, anthropic: false, openRouter: false, speech: false });
  a.openrouterModels.mockResolvedValue([
    { id: 'anthropic/claude-opus-5.5', name: 'Anthropic: Claude Opus 5.5', contextLength: 1000000, inputPerM: 4, outputPerM: 20 },
    { id: 'openai/gpt-5-mini', name: 'OpenAI: GPT-5 Mini', contextLength: 400000, inputPerM: 0.25, outputPerM: 2 },
    { id: 'meta/llama-free', name: 'Meta: Llama (free)', contextLength: 8000, inputPerM: 0, outputPerM: 0 },
  ]);
});
afterEach(cleanup);

describe('OpenRouter on the AI page', () => {
  it('saves_the_provider_and_the_key', async () => {
    render(<Ai />);
    await flush();
    fireEvent.click(screen.getByRole('tab', { name: 'OpenRouter' }));
    await flush();
    expect(lastSaved().aiProvider).toBe('openRouter');
    const field = screen.getByLabelText('OpenRouter API key') as HTMLInputElement;
    expect(field.type).toBe('password');
    fireEvent.change(field, { target: { value: 'sk-or-1' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await flush();
    expect(a.aiKeySet).toHaveBeenCalledWith('openRouter', 'sk-or-1');
  });

  it('picks_a_model_from_the_live_list_with_prices', async () => {
    a.settingsGet.mockResolvedValue({ ...base, aiProvider: 'openRouter' });
    render(<Ai />);
    await flush();
    expect(a.openrouterModels).toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText('Search OpenRouter models'), { target: { value: 'gpt' } });
    const list = screen.getByRole('listbox', { name: 'OpenRouter models' });
    const options = within(list).getAllByRole('option');
    expect(options).toHaveLength(1);
    expect(options[0].textContent).toContain('$0.25 / $2 per 1M tokens');
    fireEvent.click(options[0]);
    await flush();
    expect(lastSaved().openrouterModel).toBe('openai/gpt-5-mini');
    expect(screen.getByText('openai/gpt-5-mini')).toBeTruthy();
  });

  it('marks_free_models', async () => {
    a.settingsGet.mockResolvedValue({ ...base, aiProvider: 'openRouter' });
    render(<Ai />);
    await flush();
    fireEvent.change(screen.getByLabelText('Search OpenRouter models'), { target: { value: 'llama' } });
    expect(screen.getByRole('option').textContent).toContain('Free');
  });

  it('falls_back_to_a_text_field_when_the_list_fails', async () => {
    a.openrouterModels.mockRejectedValue('offline');
    a.settingsGet.mockResolvedValue({ ...base, aiProvider: 'openRouter' });
    render(<Ai />);
    await flush();
    const field = screen.getByLabelText('Model');
    fireEvent.change(field, { target: { value: 'mistral/large' } });
    fireEvent.blur(field);
    await flush();
    expect(lastSaved().openrouterModel).toBe('mistral/large');
  });
});

describe('BYOK speech on the Models page', () => {
  it('switches_to_cloud_speech_and_applies_the_groq_preset', async () => {
    render(<Models />);
    await flush();
    fireEvent.click(screen.getByRole('tab', { name: 'Cloud (your key)' }));
    await flush();
    expect(lastSaved().sttProvider).toBe('cloud');
    expect(screen.getByText(/audio is sent to/i)).toBeTruthy();
    fireEvent.change(screen.getByLabelText('Speech service'), { target: { value: 'groq' } });
    await flush();
    expect(lastSaved().sttCloudBaseUrl).toBe('https://api.groq.com/openai/v1');
    expect(lastSaved().sttCloudModel).toBe('whisper-large-v3-turbo');
  });

  it('stores_the_speech_key_and_custom_server', async () => {
    a.settingsGet.mockResolvedValue({ ...base, sttProvider: 'cloud' });
    render(<Models />);
    await flush();
    fireEvent.change(screen.getByLabelText('Speech API key'), { target: { value: 'gsk-1' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await flush();
    expect(a.aiKeySet).toHaveBeenCalledWith('speech', 'gsk-1');
    fireEvent.change(screen.getByLabelText('Speech service'), { target: { value: 'custom' } });
    await flush();
    const url = screen.getByLabelText('Server address');
    fireEvent.change(url, { target: { value: 'http://localhost:8000/v1' } });
    fireEvent.blur(url);
    await flush();
    expect(lastSaved().sttCloudBaseUrl).toBe('http://localhost:8000/v1');
  });

  it('local_speech_shows_the_model_tabs', async () => {
    render(<Models />);
    await flush();
    expect(screen.getByRole('tab', { name: 'Installed' })).toBeTruthy();
    expect(screen.queryByLabelText('Speech API key')).toBeNull();
  });
});
