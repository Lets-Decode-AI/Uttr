# Manual end-to-end checklist

Automated tests cannot cover permissions, global shortcuts, synthetic paste or real apps. Run this checklist on a **release
build** before you tag a release. Use a fresh app data directory (on macOS, move
`~/Library/Application Support/ai.letsdecode.uttr` aside) and install the `.dmg` into `/Applications`.

Record the OS, version, date and results in the release PR.

## 1. First run

- [ ] The app launches from `/Applications` and appears in the tray. It does not crash within 20 seconds.
- [ ] Setup opens. Grant Microphone and Accessibility. Both show as allowed without a restart.
- [ ] The recommended model (Parakeet TDT 0.6B v2 on an English machine) downloads with progress, speed and time left. Pause and resume work.
- [ ] After the download, the model is in use and the Try it step accepts dictation.

## 2. Dictation in real apps

For each of **TextEdit, a Safari textarea, VS Code and Slack**:

- [ ] Hold the shortcut (`Fn` on macOS), say a sentence with natural punctuation, then release. Punctuated, capitalized text appears at the cursor.
- [ ] The Flow Bar shows the waveform, then the transcribing dots, then a check mark. It never takes focus.

## 3. Behaviors

- [ ] Clipboard restore: copy some text, dictate, then paste. The original text comes back. Repeat with an image on the clipboard.
- [ ] Pressing `Esc` while recording cancels it, and nothing is inserted.
- [ ] Double-press the shortcut to start hands-free mode. Press it again to finish and insert.
- [ ] A tap shorter than 250 ms and a silent recording insert nothing and show no error.
- [ ] A dictation longer than 30 s (speak for about a minute with a few pauses) is inserted soon after key-up.
- [ ] The dictation appears in History with the right app name and word count. Copy and delete work.
- [ ] A personal dictionary replacement is applied.
- [ ] With no model installed, or with Accessibility revoked, using the shortcut shows a clear Flow Bar message.

## 3b. Text intelligence (Part 2)

- [ ] AI page, On this computer: download Qwen2.5 1.5B, then "Try it" > Clean up turns the sample into one clean sentence with "Wednesday".
- [ ] With on-device AI on, dictate "um so lets meet at two no three" into Notes: the inserted text keeps only "three" and has no "um".
- [ ] With an Anthropic or OpenAI key saved, the same dictation is cleaned up; a wrong key makes the Flow Bar say "AI cleanup didn't work" and the plain transcript is inserted.
- [ ] Command Mode: select a sentence in TextEdit, hold `Fn+Ctrl` (`Ctrl+Win+Alt` on Windows), say "make this more formal". The Flow Bar shows Command and the selection is replaced. Your clipboard is unchanged afterwards.
- [ ] Command Mode with nothing selected writes new text at the cursor ("write a two-line haiku about tea").
- [ ] Command Mode with AI off shows "Command Mode needs AI" and records nothing.
- [ ] Snippets: add "my email" and say "my email." on its own: the address is inserted exactly. Inside a sentence it is expanded too.
- [ ] Style: add Slack as Very casual, dictate "Sounds good." into Slack: "sounds good" is inserted. Mail with Formal keeps "Sounds good."

## 4. Performance

- [ ] A 5 s utterance is inserted in under 500 ms after key-up on Apple Silicon. Check the timing in the logs, or use the numbers from the ignored integration test.
- [ ] After 10 minutes idle with the model unloaded, all uttr processes together use under 60 MB in Activity Monitor. Include the WebKit content process for the Flow Bar.
- [ ] The installer is under 25 MB.

## 5. Other platforms (best effort)

- [ ] Windows: install with the NSIS installer, hold `Ctrl+Win` and dictate into Notepad.
- [ ] Windows: releasing `Ctrl+Win` must not open the Start menu. Part 1 does not mask the Win key, so note what happens and whether the Start menu takes focus before the paste.
- [ ] Windows and X11: press `Win+L` (lock) or `Alt+Tab` while the chord is partly held, return to the session, then press only `Ctrl`. No recording may start (stale-modifier check).
- [ ] Linux (X11): install the `.deb` or run the `.AppImage`, hold `Ctrl+Super` and dictate into a text editor.
- [ ] Linux (Wayland): confirm the clipboard fallback notice appears when paste is blocked.
