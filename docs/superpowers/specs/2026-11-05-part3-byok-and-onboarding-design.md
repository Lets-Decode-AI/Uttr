# Part 3, BYOK and onboarding: design

Date: 2026-11-05. Status: approved for implementation. Builds on Part 2 (AI cleanup, Command Mode,
snippets, styles).

## Goals

Finish the roadmap and close the gap with mature open-source dictation apps (Handy, `cjpais/Handy`,
was studied for reference: its provider presets with live model lists, its Welcome, Permissions,
Model, Demo, Try it and All set onboarding, and its history retention setting).

1. **OpenRouter** as an AI provider.
2. **BYOK speech-to-text**: transcription by a cloud service with the user's own API key.
3. **Part 3 dashboard**: a Home page with usage stats, plus fuller history.
4. **Linux Wayland shortcuts** through the XDG GlobalShortcuts portal.
5. **Tooltips**, a professional **Get started** flow and an in-app **walkthrough**.

Non-goals: accounts, sync, auto-update, file transcription, streaming partial results.

## A. OpenRouter

- `AiProvider::OpenRouter` (`"openRouter"`), key in the keychain as `openrouter_api_key`,
  setting `openrouterModel` (default `anthropic/claude-opus-5.5`).
- Requests use the OpenAI chat format at `https://openrouter.ai/api/v1` with the attribution
  headers `HTTP-Referer` and `X-Title: uttr`.
- The AI page fetches OpenRouter's public `/models` list and offers a searchable picker that shows
  each model's price per million tokens. If the list cannot be loaded, the model stays a text field.

Acceptance:
- A1 chat requests carry the key and both attribution headers (mock server test).
- A2 the model list keeps text-output models only, sorted by name, with prices per million tokens.
- A3 without a key, cleanup and Command Mode report "Add your OpenRouter API key on the AI page."
- A4 the AI page saves the key (never shows it) and the chosen model.

## B. BYOK speech-to-text

Settings: `sttProvider` (`"local"` default, or `"cloud"`), `sttCloudBaseUrl`
(default `https://api.openai.com/v1`), `sttCloudModel` (default `whisper-1`), `sttCloudLanguage`
(empty = auto). Key in the keychain as `stt_api_key`. Presets in the UI: OpenAI
(`whisper-1`), Groq (`https://api.groq.com/openai/v1`, `whisper-large-v3-turbo`), Custom (any
OpenAI-compatible `/audio/transcriptions` server, key optional).

Behavior:
- The same chunk planner runs. Each speech chunk is encoded as 16 kHz mono 16-bit PCM WAV and posted
  to `{base}/audio/transcriptions` as multipart (`file`, `model`, `response_format=json`, optional
  `language`) while the user keeps talking. Finish waits for every chunk in order and joins them
  with the existing `join_chunks`.
- The active "model" for a dictation is a virtual entry `cloud:<model>`; history stores that id.
- Not ready (cloud chosen, no key, and the base URL is OpenAI or Groq): pressing the shortcut does
  not record; uttr opens the Models page and shows "Add your speech API key on the Models page."
- HTTP errors become readable `SttError`s (rejected key, rate limit, server error).
- Cancel aborts uploads in flight.

Acceptance:
- B1 WAV encoding: RIFF header, 16 kHz, mono, 16 bit, clamped samples.
- B2 a session uploads each speech chunk once, skips silent chunks, and joins results in chunk order
  even when responses arrive out of order (mock server with delays).
- B3 the multipart body names the model and language; the key is sent as a bearer token.
- B4 a 401 becomes "The speech API key was rejected." and fails the dictation.
- B5 the router picks the local engine or the cloud by setting, and reports "not ready" for a
  missing key on key-requiring hosts.
- B6 the Models page switches providers, applies presets, and stores the key.

## C. Part 3: Home dashboard and history

Stats are computed in Rust from transcripts, in the user's local days (the UI passes its UTC
offset):
- total words, dictations, speaking time;
- average speed = total words / total speaking minutes;
- current streak = consecutive days with a dictation ending today, or yesterday if none yet today;
  longest streak;
- time saved = max(0, words / 40 WPM typing - speaking minutes);
- words per day for the last 30 days (zeros included), top 5 apps by words.

Home (new default page): greeting, four stat tiles (Words, Average speed, Day streak, Time saved),
a 30-day bar chart, top apps, recent dictations, and a **Get started** checklist (speech ready,
permissions, first dictation, try AI, add a snippet, set a style, take the tour) that can be
dismissed.

History: filter by app, "Show original" to see the raw transcript next to the cleaned text, and a
**Keep history** setting (forever, 90, 30 or 7 days) applied at start-up and after each dictation.

Acceptance:
- C1 stats math (streaks across a gap, today vs yesterday, local-day bucketing with an offset,
  time saved floor at 0, 30 zero-filled days, top apps order).
- C2 retention deletes only older transcripts; "forever" deletes nothing.
- C3 history filters by app.
- C4 Home renders tiles from the stats and the checklist ticks from real state; dismissing hides it.

## D. Wayland shortcuts

On Wayland, uttr opens a GlobalShortcuts portal session and binds `dictate` and `command` with
preferred triggers derived from the user's bindings (`CTRL+LOGO`, `CTRL+LOGO+ALT`, ...). Portal
`Activated`/`Deactivated` signals drive the same gesture logic as key chords (hold, double-tap
hands-free); `command` also emits `Command`. Esc cancel is not available on Wayland (the portal
cannot observe it) and the UI says so. Without the portal, the existing "not available" notice
stays.

Acceptance:
- D1 binding to portal trigger strings.
- D2 activation and deactivation of `dictate` and `command` produce the same events as chords.
- D3 Linux CI builds the portal backend.

## E. Tooltips

A `Tooltip` component: shows after 500 ms hover or on keyboard focus, hides on leave, blur or Esc,
`role="tooltip"` linked by `aria-describedby`, flips below when there is no room above. Icon
buttons show their label as a tooltip. An `InfoTip` (a small "i") explains technical settings
(unload when idle, load at startup, server address, model names, Command Mode, styles, keep history,
cloud speech).

Acceptance: E1 delay, focus, Esc and aria wiring; E2 icon buttons expose their label.

## F. Get started and walkthrough

Onboarding becomes: Welcome (what uttr does, three highlights), Shortcut, Permissions, Speech model
(local or BYOK), **How it works** (an animated demo of holding the shortcut, speaking and the text
appearing; a still frame under reduced motion), Try it, **All set** (a summary of shortcut, speech
and microphone with Change links, and "Start the tour").

The **walkthrough** is a coach-mark tour over the main window: Home, History, Snippets, Style, AI,
Models, Settings and the status indicator, each with a short explanation. Back, Next, Skip,
"3 of 8" progress, Esc to close, arrow keys to move. It starts once after onboarding, and can be
restarted from Home and from Settings > Help.

Acceptance:
- F1 the tour walks every step, supports keys, and records completion.
- F2 onboarding shows How it works and All set, and All set links back to earlier steps.

## Testing

Test first for every item above: Rust unit tests (mock HTTP with wiremock), Vitest + Testing
Library for UI. Then the full suites, clippy, fmt, tsc, a run of the dev app and the UI harness,
and CI on Windows, Linux and macOS.
