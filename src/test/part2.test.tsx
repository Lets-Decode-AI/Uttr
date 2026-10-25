import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const handlers: Record<string, Array<(p: any) => void>> = {};
const llm = (id: string, name: string) => ({
  id, name, hf_repo: `o/${id}`, revision: 'r', files: [{ path: `${id}.gguf`, size: 1_000_000_000, sha256: 'x' }],
  engine: 'llm', languages: ['multi'], min_ram_gb: 4, notes: '',
});
const base = {
  hotkey: { keys: ['Fn'] }, mic: null, showFlowbar: true, idleUnloadMin: 10, preload: false, launchAtLogin: false, onboarded: true,
  commandHotkey: { keys: ['Fn', 'ControlLeft'] }, aiProvider: 'off', openaiModel: 'gpt-5-mini', openaiBaseUrl: 'https://api.openai.com/v1',
  anthropicModel: 'claude-opus-5-5', defaultStyle: 'formal', appStyles: [] as { app: string; style: string }[],
};

vi.mock('../ipc', () => ({
  api: {
    settingsGet: vi.fn(), settingsSet: vi.fn(async () => {}),
    llmCatalog: vi.fn(), llmInstalled: vi.fn(), aiKeysStatus: vi.fn(), aiKeySet: vi.fn(async () => {}), aiTry: vi.fn(),
    modelInstall: vi.fn(async () => {}), modelPause: vi.fn(async () => {}), modelResume: vi.fn(async () => {}),
    modelCancel: vi.fn(async () => {}), modelDelete: vi.fn(async () => {}), modelSetActive: vi.fn(async () => {}),
    snippetList: vi.fn(), snippetAdd: vi.fn(async () => 1), snippetUpdate: vi.fn(async () => {}), snippetDelete: vi.fn(async () => {}),
    recentApps: vi.fn(),
  },
  on: vi.fn(async (name: string, cb: (p: any) => void) => {
    (handlers[name] ||= []).push(cb);
    return () => {};
  }),
}));

import { api } from '../ipc';
import Ai from '../pages/Ai';
import Snippets from '../pages/Snippets';
import Style from '../pages/Style';

const a = api as any;
const flush = () => act(async () => {});
const emit = (name: string, p: unknown) => act(() => handlers[name]?.forEach((h) => h(p)));
const lastSaved = () => a.settingsSet.mock.calls.at(-1)[0];

beforeEach(() => {
  for (const k of Object.keys(handlers)) delete handlers[k];
  vi.clearAllMocks();
  a.settingsGet.mockResolvedValue({ ...base });
  a.llmCatalog.mockResolvedValue([llm('q', 'Qwen Small'), llm('l', 'Llama')]);
  a.llmInstalled.mockResolvedValue([]);
  a.aiKeysStatus.mockResolvedValue({ openAi: false, anthropic: false });
  a.snippetList.mockResolvedValue([]);
  a.recentApps.mockResolvedValue(['Slack', 'Mail']);
});
afterEach(cleanup);

describe('AI page', () => {
  it('choosing_a_provider_saves_it', async () => {
    render(<Ai />);
    await flush();
    fireEvent.click(screen.getByRole('tab', { name: 'Anthropic' }));
    await flush();
    expect(lastSaved().aiProvider).toBe('anthropic');
    expect(screen.getByLabelText('Anthropic API key')).toBeTruthy();
    expect(screen.getByLabelText('Model')).toHaveProperty('value', 'claude-opus-5-5');
  });

  it('saves_an_api_key_to_the_keychain_and_never_shows_it', async () => {
    a.settingsGet.mockResolvedValue({ ...base, aiProvider: 'openAi' });
    render(<Ai />);
    await flush();
    const field = screen.getByLabelText('OpenAI API key') as HTMLInputElement;
    expect(field.type).toBe('password');
    fireEvent.change(field, { target: { value: ' sk-test ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await flush();
    expect(a.aiKeySet).toHaveBeenCalledWith('openAi', 'sk-test');
    expect(field.value).toBe('');
    expect(a.aiKeysStatus).toHaveBeenCalledTimes(2);
  });

  it('model_name_saves_on_blur', async () => {
    a.settingsGet.mockResolvedValue({ ...base, aiProvider: 'openAi' });
    render(<Ai />);
    await flush();
    const field = screen.getByLabelText('Model');
    fireEvent.change(field, { target: { value: 'llama3.2' } });
    fireEvent.blur(field);
    await flush();
    expect(lastSaved().openaiModel).toBe('llama3.2');
  });

  it('local_models_download_through_the_model_commands', async () => {
    a.settingsGet.mockResolvedValue({ ...base, aiProvider: 'local' });
    render(<Ai />);
    await flush();
    const row = screen.getByText('Llama').closest('[data-testid="model-row"]') as HTMLElement;
    fireEvent.click(within(row).getByRole('button', { name: /download/i }));
    await flush();
    expect(a.modelInstall).toHaveBeenCalledWith('l');
    emit('progress', { modelId: 'l', bytesDone: 5, total: 10, bytesPerSec: 1, etaSecs: 5, status: 'Downloading', error: null });
    expect(within(row).getByRole('button', { name: 'Pause' })).toBeTruthy();
  });

  it('try_it_shows_the_result_or_the_error', async () => {
    a.settingsGet.mockResolvedValue({ ...base, aiProvider: 'anthropic' });
    a.aiTry.mockResolvedValueOnce('We should meet on Wednesday at ten.');
    render(<Ai />);
    await flush();
    fireEvent.click(screen.getByRole('button', { name: 'Clean up' }));
    await flush();
    expect(a.aiTry).toHaveBeenCalledWith(expect.stringContaining('Tuesday'), false);
    expect(screen.getByTestId('ai-try-result').textContent).toBe('We should meet on Wednesday at ten.');
    a.aiTry.mockRejectedValueOnce('Add your Anthropic API key on the AI page.');
    fireEvent.click(screen.getByRole('button', { name: 'Run as a command' }));
    await flush();
    expect(screen.getByTestId('ai-try-result').textContent).toContain('Add your Anthropic API key');
  });

  it('command_mode_shortcut_can_be_changed', async () => {
    render(<Ai />);
    await flush();
    expect(screen.queryByRole('button', { name: 'Clean up' })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Change' }));
    fireEvent.click(screen.getByRole('button', { name: 'Right ⌥+Right ⇧' }));
    await flush();
    expect(lastSaved().commandHotkey).toEqual({ keys: ['AltRight', 'ShiftRight'] });
  });
});

describe('Snippets page', () => {
  it('adds_a_snippet_with_a_multiline_expansion', async () => {
    render(<Snippets />);
    await flush();
    fireEvent.change(screen.getByLabelText('When I say'), { target: { value: ' my address ' } });
    fireEvent.change(screen.getByLabelText('Type this'), { target: { value: '1 Main St\nSpringfield' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add' }));
    await flush();
    expect(a.snippetAdd).toHaveBeenCalledWith('my address', '1 Main St\nSpringfield');
    expect(screen.getByLabelText('When I say')).toHaveProperty('value', '');
  });

  it('edits_and_deletes_snippets', async () => {
    a.snippetList.mockResolvedValue([{ id: 7, trigger: 'my email', expansion: 'a@x.io' }]);
    render(<Snippets />);
    await flush();
    const row = screen.getByTestId('snippet-row');
    fireEvent.click(within(row).getByRole('button', { name: 'Edit' }));
    const fields = screen.getAllByLabelText('Type this');
    fireEvent.change(fields[0], { target: { value: 'b@x.io' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await flush();
    expect(a.snippetUpdate).toHaveBeenCalledWith(7, 'my email', 'b@x.io');
    fireEvent.click(screen.getByRole('button', { name: 'Remove my email' }));
    await flush();
    expect(a.snippetDelete).toHaveBeenCalledWith(7);
  });
});

describe('Style page', () => {
  it('sets_the_default_style', async () => {
    render(<Style />);
    await flush();
    fireEvent.click(screen.getByRole('tab', { name: 'Very casual' }));
    await flush();
    expect(lastSaved().defaultStyle).toBe('veryCasual');
  });

  it('adds_changes_and_removes_an_app_rule', async () => {
    render(<Style />);
    await flush();
    fireEvent.change(screen.getByLabelText('App name'), { target: { value: 'Slack' } });
    fireEvent.change(screen.getByLabelText('Style for the new app'), { target: { value: 'veryCasual' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add' }));
    await flush();
    expect(lastSaved().appStyles).toEqual([{ app: 'Slack', style: 'veryCasual' }]);
    fireEvent.change(screen.getByLabelText('Style for Slack'), { target: { value: 'formal' } });
    await flush();
    expect(lastSaved().appStyles).toEqual([{ app: 'Slack', style: 'formal' }]);
    fireEvent.click(screen.getByRole('button', { name: 'Remove the rule for Slack' }));
    await flush();
    expect(lastSaved().appStyles).toEqual([]);
  });
});
