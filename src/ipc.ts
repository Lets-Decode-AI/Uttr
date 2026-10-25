// Typed bridge to the Rust backend. Command names are snake_case; Tauri converts the
// camelCase argument keys used here to the snake_case Rust parameter names.
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

// ---- DTOs ----------------------------------------------------------------------------------

export type Perm = 'granted' | 'denied' | 'unknown' | 'notRequired';
export interface PermissionsStatus {
  microphone: Perm;
  accessibility: Perm;
}
export type PermKind = 'microphone' | 'accessibility' | 'keyboard';

export type DictationState =
  | { kind: 'idle' }
  | { kind: 'recording'; handsFree: boolean; command?: boolean }
  | { kind: 'transcribing' }
  | { kind: 'inserting' }
  | { kind: 'error'; message: string };

export type Key =
  | 'Fn' | 'ControlLeft' | 'ControlRight' | 'MetaLeft' | 'MetaRight'
  | 'AltLeft' | 'AltRight' | 'ShiftLeft' | 'ShiftRight' | 'Space' | 'Escape';
export interface Binding {
  keys: Key[];
}

export type AiProvider = 'off' | 'local' | 'openAi' | 'anthropic';
export type Style = 'formal' | 'casual' | 'veryCasual';
export interface AppStyle {
  app: string;
  style: Style;
}

export interface Settings {
  hotkey: Binding;
  /** Input device name; null = system default. */
  mic: string | null;
  showFlowbar: boolean;
  /** Minutes before the idle model unloads; null = never. */
  idleUnloadMin: number | null;
  preload: boolean;
  launchAtLogin: boolean;
  onboarded: boolean;
  /** Hold to speak an instruction for the AI (Command Mode). */
  commandHotkey: Binding;
  aiProvider: AiProvider;
  openaiModel: string;
  /** OpenAI or a compatible server (Ollama, LM Studio), including `/v1`. */
  openaiBaseUrl: string;
  anthropicModel: string;
  defaultStyle: Style;
  appStyles: AppStyle[];
}

export type EngineKind = 'transducer' | 'whisper' | 'moonshine' | 'sensevoice' | 'llm';
export interface ModelFile {
  path: string;
  size: number;
  sha256: string;
}
/** Serialized as-is from the catalog manifest (snake_case keys). */
export interface CatalogEntry {
  id: string;
  name: string;
  hf_repo: string;
  revision: string;
  files: ModelFile[];
  engine: EngineKind;
  languages: string[];
  min_ram_gb: number;
  notes: string;
}
export type InstalledModelDto = CatalogEntry & { diskBytes: number; active: boolean };

export interface Hardware {
  totalRamGb: number;
  cores: number;
  arch: string;
  appleSilicon: boolean;
  gpuVendor: string | null;
}
export interface Recommendation {
  id: string;
  hardware: Hardware;
}

export interface HfRepoSummary {
  repo: string;
  downloads: number;
  likes: number;
  lastModified: string;
}
export type Inspection =
  | ({ kind: 'compatible' } & CatalogEntry)
  | { kind: 'incompatible'; reason: string };

/** Rust enum `DlStatus`, serde default encoding. */
export type DlStatus = 'Queued' | 'Downloading' | 'Paused' | 'Verifying' | 'Installed' | { Failed: string };
export interface DownloadProgress {
  modelId: string;
  bytesDone: number;
  total: number;
  bytesPerSec: number;
  etaSecs: number | null;
  status: DlStatus;
  error: string | null;
}

export interface Transcript {
  id: number;
  createdAt: number;
  text: string;
  rawText: string;
  appName: string | null;
  durationMs: number;
  wordCount: number;
  wpm: number;
  modelId: string;
}
export interface DictEntry {
  id: number;
  term: string;
  replacement: string | null;
}
export interface Snippet {
  id: number;
  trigger: string;
  expansion: string;
}
/** Which API keys are stored in the keychain. */
export interface AiKeysStatus {
  openAi: boolean;
  anthropic: boolean;
}
export interface Toast {
  level: 'info' | 'warning' | 'error' | string;
  message: string;
}

// ---- commands ------------------------------------------------------------------------------

export const api = {
  getState: () => invoke<DictationState>('get_state'),
  setPaused: (paused: boolean) => invoke<void>('set_paused', { paused }),

  permissionsStatus: () => invoke<PermissionsStatus>('permissions_status'),
  openPermissionSettings: (kind: PermKind) => invoke<void>('open_permission_settings', { kind }),
  requestMicrophone: () => invoke<void>('request_microphone'),
  restartApp: () => invoke<void>('restart_app'),

  catalog: () => invoke<CatalogEntry[]>('catalog'),
  installed: () => invoke<InstalledModelDto[]>('installed'),
  recommend: () => invoke<Recommendation>('recommend'),
  hfSearch: (q: string) => invoke<HfRepoSummary[]>('hf_search', { q }),
  hfInspect: (repo: string) => invoke<Inspection>('hf_inspect', { repo }),
  /** Install a bundled catalog model by id. */
  modelInstall: (id: string) => invoke<void>('model_install', { id, entry: undefined }),
  /** Install a custom entry (e.g. from `hfInspect`). */
  modelInstallEntry: (entry: CatalogEntry) => invoke<void>('model_install', { id: undefined, entry }),
  modelPause: (id: string) => invoke<void>('model_pause', { id }),
  modelResume: (id: string) => invoke<void>('model_resume', { id }),
  modelCancel: (id: string) => invoke<void>('model_cancel', { id }),
  modelDelete: (id: string) => invoke<void>('model_delete', { id }),
  modelSetActive: (id: string) => invoke<void>('model_set_active', { id }),

  historyList: (page: number, query?: string) => invoke<Transcript[]>('history_list', { page, query }),
  historyDelete: (id: number) => invoke<void>('history_delete', { id }),

  settingsGet: () => invoke<Settings>('settings_get'),
  settingsSet: (settings: Settings) => invoke<void>('settings_set', { settings }),
  hfTokenSet: (token: string | null) => invoke<void>('hf_token_set', { token }),

  dictionaryList: () => invoke<DictEntry[]>('dictionary_list'),
  dictionaryAdd: (term: string, replacement?: string) => invoke<number>('dictionary_add', { term, replacement }),
  dictionaryDelete: (id: number) => invoke<void>('dictionary_delete', { id }),

  snippetList: () => invoke<Snippet[]>('snippet_list'),
  snippetAdd: (trigger: string, expansion: string) => invoke<number>('snippet_add', { trigger, expansion }),
  snippetUpdate: (id: number, trigger: string, expansion: string) => invoke<void>('snippet_update', { id, trigger, expansion }),
  snippetDelete: (id: number) => invoke<void>('snippet_delete', { id }),
  recentApps: () => invoke<string[]>('recent_apps'),

  /** On-device AI models; install, pause, delete and activate them with the model commands. */
  llmCatalog: () => invoke<CatalogEntry[]>('llm_catalog'),
  llmInstalled: () => invoke<InstalledModelDto[]>('llm_installed'),
  aiKeysStatus: () => invoke<AiKeysStatus>('ai_keys_status'),
  aiKeySet: (provider: 'openAi' | 'anthropic', key: string | null) => invoke<void>('ai_key_set', { provider, key }),
  /** Runs text through the current AI settings: cleanup, or a Command Mode instruction. */
  aiTry: (text: string, command: boolean) => invoke<string>('ai_try', { text, command }),

  micList: () => invoke<string[]>('mic_list'),
  openLogFolder: () => invoke<void>('open_log_folder'),
};

// ---- events (names mirror src-tauri/src/events.rs) -----------------------------------------

export const EVENTS = {
  state: 'dictation://state',
  level: 'dictation://level',
  progress: 'models://progress',
  toast: 'app://toast',
  transcriptAdded: 'history://added',
  settingsChanged: 'settings://changed',
  modelsChanged: 'models://changed',
} as const;

export interface EventPayloads {
  state: DictationState;
  /** Mic level, roughly 0..1. */
  level: number;
  progress: DownloadProgress;
  toast: Toast;
  transcriptAdded: { id: number };
  settingsChanged: Settings;
  /** The active model changed outside the Models page (tray). */
  modelsChanged: null;
}

/** Subscribe to a backend event; resolves to the unlisten function. */
export function on<K extends keyof EventPayloads>(
  event: K,
  cb: (payload: EventPayloads[K]) => void,
): Promise<UnlistenFn> {
  return listen<EventPayloads[K]>(EVENTS[event], (e) => cb(e.payload));
}
