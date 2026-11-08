// Sample backend for README screenshots: answers the app's commands with fixed, made-up data.
import { mockIPC } from '@tauri-apps/api/mocks';

const now = Date.now();
const min = 60_000;
const llm = (id: string, name: string, size: number, ram: number, notes: string) => ({
  id, name, hf_repo: '', revision: '', files: [{ path: `${id}.gguf`, size, sha256: '' }],
  engine: 'llm', languages: ['multi'], min_ram_gb: ram, notes,
});
const stt = (id: string, name: string, size: number, languages: string[], notes: string) => ({
  id, name, hf_repo: '', revision: '', files: [{ path: 'model.onnx', size, sha256: '' }],
  engine: 'transducer', languages, min_ram_gb: 4, notes,
});
const llms = [
  llm('qwen2.5-1.5b-instruct', 'Qwen2.5 1.5B Instruct (Q4_K_M)', 1_117_320_736, 4, 'Recommended. Small and fast, good for cleanup.'),
  llm('llama-3.2-3b-instruct', 'Llama 3.2 3B Instruct (Q4_K_M)', 2_019_377_696, 8, 'Better rewrites for Command Mode. Llama 3.2 Community License.'),
  llm('qwen3-4b-instruct-2507', 'Qwen3 4B Instruct 2507 (Q4_K_M)', 2_497_281_120, 8, 'Best quality. Slower on computers without a GPU.'),
];
const stts = [
  stt('parakeet-v2', 'Parakeet TDT 0.6B v2 (int8)', 661_190_513, ['en'], 'Default. Best English accuracy and speed.'),
  stt('parakeet-v3', 'Parakeet TDT 0.6B v3 (int8)', 670_000_000, ['en', 'de', 'fr', 'es'], '25 European languages.'),
  stt('whisper-turbo', 'Whisper large-v3 turbo (int8)', 1_000_000_000, ['multi'], 'Many languages. Slower.'),
];

const settings = {
  hotkey: { keys: ['Fn'] }, mic: null, showFlowbar: true, idleUnloadMin: 10, preload: false, launchAtLogin: true,
  onboarded: true, commandHotkey: { keys: ['Fn', 'ControlLeft'] }, aiProvider: 'local', openaiModel: 'gpt-5-mini',
  openaiBaseUrl: 'https://api.openai.com/v1', anthropicModel: 'claude-opus-5-5', defaultStyle: 'formal',
  appStyles: [{ app: 'Slack', style: 'veryCasual' }, { app: 'Messages', style: 'casual' }, { app: 'Mail', style: 'formal' }],
  openrouterModel: 'anthropic/claude-opus-5.5', sttProvider: 'local', sttCloudBaseUrl: 'https://api.groq.com/openai/v1',
  sttCloudModel: 'whisper-large-v3-turbo', sttCloudLanguage: '', historyRetentionDays: null,
};
const day = (i: number) => new Date(now - (29 - i) * 86_400_000).toISOString().slice(0, 10);
const words = [120, 340, 0, 410, 280, 520, 610, 0, 0, 380, 450, 700, 640, 300, 520, 810, 760, 0, 430, 590, 680, 720, 910, 0, 560, 840, 990, 760, 880, 640];
const stats = {
  totalWords: 48210, dictations: 1284, speakingMs: 21_300_000, avgWpm: 135.8, currentStreak: 6, longestStreak: 14,
  timeSavedMin: 850.3, days: words.map((w, i) => ({ date: day(i), words: w })),
  topApps: [{ app: 'Slack', words: 17840 }, { app: 'Mail', words: 11230 }, { app: 'Visual Studio Code', words: 8120 }, { app: 'Notes', words: 6010 }, { app: 'Messages', words: 3420 }],
};
const t = (id: number, ago: number, text: string, app: string, words: number) => ({
  id, createdAt: now - ago * min, text, rawText: text, appName: app, durationMs: words * 400, wordCount: words, wpm: 150, modelId: 'parakeet-v2',
});
const history = [
  t(6, 2, 'Can we move the design review to Thursday at 3? I want Priya to see the new onboarding flow first.', 'Slack', 19),
  t(5, 14, 'Thanks for the quick turnaround on the contract. I have two small changes, both in section four, and then we are ready to sign.', 'Mail', 25),
  t(4, 41, 'Reminder: pick up the dry cleaning and book a table for Saturday.', 'Notes', 12),
  t(3, 95, 'The flaky test was a race between the recorder thread and the shutdown signal. Fixed by joining the pump before cancelling the session.', 'Visual Studio Code', 24),
  t(2, 180, 'sounds good, see you at the standup', 'Slack', 7),
  t(1, 1440, 'Draft the release notes for 0.3: AI cleanup, Command Mode, snippets and per-app styles.', 'Notes', 15),
];
const snippets = [
  { id: 1, trigger: 'my email', expansion: 'sam.rivera@example.com' },
  { id: 2, trigger: 'my calendar link', expansion: 'https://cal.example.com/sam/30min' },
  { id: 3, trigger: 'my sign off', expansion: 'Best,\nSam Rivera\nProduct Lead, Example Co.' },
];

const replies: Record<string, unknown> = {
  get_state: { kind: 'idle' },
  permissions_status: { microphone: 'granted', accessibility: 'granted' },
  settings_get: settings,
  catalog: stts,
  installed: [{ ...stts[0], diskBytes: 661_190_513, active: true }],
  recommend: { id: 'parakeet-v2', hardware: {} },
  llm_catalog: llms,
  llm_installed: [{ ...llms[0], diskBytes: 1_117_320_736, active: true }],
  ai_keys_status: { openAi: false, anthropic: true, openRouter: true, speech: false },
  stats,
  openrouter_models: [
    { id: 'anthropic/claude-opus-5.5', name: 'Anthropic: Claude Opus 5.5', contextLength: 1000000, inputPerM: 4, outputPerM: 20 },
    { id: 'anthropic/claude-haiku-5.5', name: 'Anthropic: Claude Haiku 5.5', contextLength: 1000000, inputPerM: 0.1, outputPerM: 0.5 },
    { id: 'openai/gpt-5-mini', name: 'OpenAI: GPT-5 Mini', contextLength: 400000, inputPerM: 0.25, outputPerM: 2 },
  ],
  ai_try: 'I think we should meet on Wednesday at ten.',
  history_list: history,
  dictionary_list: [{ id: 1, term: 'Kubernetes', replacement: null }, { id: 2, term: 'uttr', replacement: null }],
  snippet_list: snippets,
  recent_apps: ['Slack', 'Mail', 'Notes', 'Visual Studio Code'],
  mic_list: ['MacBook Pro Microphone', 'AirPods Pro'],
};

const params = new URLSearchParams(location.search);
// ?provider=openRouter, ?stt=cloud and ?tour=1 show those states.
if (params.get('provider')) settings.aiProvider = params.get('provider')!;
if (params.get('stt')) settings.sttProvider = params.get('stt')!;
try {
  localStorage.setItem('uttr.tour.done', params.get('tour') ? '' : '1');
  if (!params.get('tour')) localStorage.removeItem('uttr.getStarted.hidden');
  // ?step=4 opens onboarding on that step (0-based).
  if (params.get('step')) localStorage.setItem('uttr.onboardingStep', params.get('step')!);
} catch { /* storage unavailable */ }

mockIPC((cmd, args) => {
  if (cmd === 'app_version') return '0.4.0';
  if (cmd === 'history_list') return (args as { page: number }).page === 0 ? history : [];
  return replies[cmd] ?? null;
}, { shouldMockEvents: true });

const theme = params.get('theme');
if (theme) localStorage.setItem('uttr.theme', theme);

await import('../../src/main.tsx');
// A browser has no window vibrancy: paint the sidebar with the plain theme color instead.
document.documentElement.classList.remove('platform-mac');
