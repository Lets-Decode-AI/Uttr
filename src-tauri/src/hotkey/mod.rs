//! Global push-to-talk hotkeys: raw key transitions from a platform backend
//! -> chord tracking -> `GestureDetector` -> `HotkeyEvent` on an mpsc channel.
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::{
    Arc, Mutex, MutexGuard,
    atomic::{AtomicBool, Ordering},
    mpsc::Sender,
};
use std::time::Instant;

pub mod gesture;
#[cfg(target_os = "macos")]
pub mod macos_fn;
#[cfg_attr(not(any(target_os = "windows", target_os = "linux")), allow(dead_code))]
pub mod physical;
pub mod portal;
#[cfg(not(target_os = "macos"))]
pub mod rdev_backend;

use gesture::GestureDetector;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyEvent {
    Pressed,
    Released,
    DoubleTap,
    Cancel,
    /// The Command Mode chord is held: the recording in progress is an instruction for the AI.
    Command,
}

#[derive(Debug, thiserror::Error)]
pub enum HotkeyError {
    /// The macOS event tap could not be created (Accessibility / Input
    /// Monitoring not granted). Retry once the permission is granted.
    #[error("The keyboard shortcut needs Accessibility permission.")]
    TapUnavailable,
    #[error("Global shortcuts are not supported in this session.")]
    Unsupported,
    #[error("The keyboard listener could not start. Details: {0}")]
    ListenFailed(String),
    #[error("The keyboard shortcut failed. Details: {0}")]
    Backend(String),
}

/// Keys that can take part in a binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Key {
    Fn,
    ControlLeft,
    ControlRight,
    MetaLeft,
    MetaRight,
    AltLeft,
    AltRight,
    ShiftLeft,
    ShiftRight,
    Space,
    Escape,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub keys: Vec<Key>,
}

/// Hold `Fn` on macOS, `Ctrl+Super` elsewhere.
pub fn default_binding() -> Binding {
    #[cfg(target_os = "macos")]
    let keys = vec![Key::Fn];
    #[cfg(not(target_os = "macos"))]
    let keys = vec![Key::ControlLeft, Key::MetaLeft];
    Binding { keys }
}

/// Hold `Fn+Ctrl` on macOS, `Ctrl+Super+Alt` elsewhere: the dictation chord plus one key, so
/// adding that key while dictating switches the recording to Command Mode.
pub fn default_command_binding() -> Binding {
    let mut keys = default_binding().keys;
    #[cfg(target_os = "macos")]
    keys.push(Key::ControlLeft);
    #[cfg(not(target_os = "macos"))]
    keys.push(Key::AltLeft);
    Binding { keys }
}

/// Tracks which keys are down and reports chord transitions.
pub struct ChordTracker {
    binding: Binding,
    down: HashSet<Key>,
    active: bool,
}

impl ChordTracker {
    pub fn new(binding: Binding) -> Self {
        Self {
            binding,
            down: HashSet::new(),
            active: false,
        }
    }

    /// `Some(true)` when the chord becomes fully held, `Some(false)` when it
    /// stops being held, `None` otherwise (including key repeat).
    pub fn on_key(&mut self, key: Key, down: bool) -> Option<bool> {
        if down {
            self.down.insert(key);
        } else {
            self.down.remove(&key);
        }
        let now = !self.binding.keys.is_empty()
            && self.binding.keys.iter().all(|k| self.down.contains(k));
        (now != std::mem::replace(&mut self.active, now)).then_some(now)
    }
}

/// Asks the OS whether a key is physically down right now. Used to discard keys the tracker
/// believes are held but whose release was never seen (e.g. Win after Win+L).
pub type KeyVerifier = Box<dyn Fn(Key) -> bool + Send>;

/// Whether a stopped/failed hotkey service should be started again. Pure so the policy is
/// testable: retry when the slot is empty and either Accessibility was just granted (once per
/// grant, tracked in `retried_for_grant`) or onboarding just completed.
pub fn retry_hotkey_start(
    slot_empty: bool,
    accessibility_granted: bool,
    retried_for_grant: &mut bool,
    onboarded_now: bool,
) -> bool {
    if !accessibility_granted {
        *retried_for_grant = false;
    }
    if !slot_empty {
        return false;
    }
    let fresh_grant = accessibility_granted && !*retried_for_grant;
    if fresh_grant {
        *retried_for_grant = true;
    }
    fresh_grant || onboarded_now
}

/// State shared between the service handle and the backend thread.
pub struct Core {
    chord: ChordTracker,
    /// Command Mode chord. Either chord counts as "held" for the gesture; completing this one
    /// also reports `Command`.
    command: Option<ChordTracker>,
    gesture: GestureDetector,
    pub armed: bool,
    esc_down: bool,
    tx: Sender<HotkeyEvent>,
    verifier: Option<KeyVerifier>,
}

impl Core {
    pub fn new(binding: Binding, tx: Sender<HotkeyEvent>) -> Self {
        Self {
            chord: ChordTracker::new(binding),
            command: None,
            gesture: GestureDetector::new(),
            armed: false,
            esc_down: false,
            tx,
            verifier: None,
        }
    }

    /// Install the physical-key check (Windows / X11 backends).
    pub fn set_verifier(&mut self, v: KeyVerifier) {
        self.verifier = Some(v);
    }

    fn emit(&self, events: impl IntoIterator<Item = HotkeyEvent>) {
        for e in events {
            // Unbounded channel: never blocks. A dropped receiver is fine.
            let _ = self.tx.send(e);
        }
    }

    pub fn handle(&mut self, key: Key, down: bool) {
        if key == Key::Escape && !self.chord.binding.keys.contains(&Key::Escape) {
            let first_down = down && !std::mem::replace(&mut self.esc_down, true);
            if !down {
                self.esc_down = false;
            }
            if first_down && self.armed {
                self.emit([HotkeyEvent::Cancel]);
            }
            return;
        }
        let was_held = self.held();
        let dictation = self.chord.on_key(key, down);
        let command = self.command.as_mut().and_then(|c| c.on_key(key, down));
        if dictation == Some(true) && !Self::keys_really_down(&mut self.chord, &self.verifier, key)
        {
            return;
        }
        if command == Some(true)
            && let Some(c) = self.command.as_mut()
            && !Self::keys_really_down(c, &self.verifier, key)
        {
            return;
        }
        let held = self.held();
        if held != was_held {
            let events = self.gesture.on_key(held, Instant::now());
            self.emit(events);
        }
        if command == Some(true) {
            self.emit([HotkeyEvent::Command]);
        }
    }

    fn held(&self) -> bool {
        self.chord.active || self.command.as_ref().is_some_and(|c| c.active)
    }

    /// `chord` just completed with `latest`. The other keys must be physically down: any the
    /// OS says are up are stale (their release was missed) and are forgotten, and the chord is
    /// not reported. `latest` itself is skipped: a low-level hook runs before the OS updates it.
    fn keys_really_down(
        chord: &mut ChordTracker,
        verifier: &Option<KeyVerifier>,
        latest: Key,
    ) -> bool {
        let Some(verify) = verifier.as_ref() else {
            return true;
        };
        let stale: Vec<Key> = chord
            .binding
            .keys
            .iter()
            .copied()
            .filter(|k| *k != latest && !verify(*k))
            .collect();
        for k in &stale {
            chord.down.remove(k);
        }
        if !stale.is_empty() {
            tracing::debug!("ignoring stale held keys: {stale:?}");
            chord.active = false;
        }
        stale.is_empty()
    }

    /// Forget all held keys, releasing the chord if it was held.
    pub fn reset(&mut self) {
        let was_held = self.held();
        self.chord.down.clear();
        self.chord.active = false;
        if let Some(c) = self.command.as_mut() {
            c.down.clear();
            c.active = false;
        }
        self.esc_down = false;
        if was_held {
            let events = self.gesture.on_key(false, Instant::now());
            self.emit(events);
        }
    }

    pub fn set_binding(&mut self, binding: Binding) {
        self.reset();
        self.chord = ChordTracker::new(binding);
    }

    /// `None` turns Command Mode's shortcut off.
    pub fn set_command_binding(&mut self, binding: Option<Binding>) {
        self.reset();
        self.command = binding.map(ChordTracker::new);
    }
}

fn lock(core: &Mutex<Core>) -> MutexGuard<'_, Core> {
    core.lock().unwrap_or_else(|p| p.into_inner())
}

pub struct HotkeyService {
    core: Arc<Mutex<Core>>,
    stopped: Arc<AtomicBool>,
}

impl HotkeyService {
    pub fn start(binding: Binding, tx: Sender<HotkeyEvent>) -> Result<HotkeyService, HotkeyError> {
        let core = Arc::new(Mutex::new(Core::new(binding.clone(), tx.clone())));
        let stopped = Arc::new(AtomicBool::new(false));
        Self::start_backend(binding, tx, core.clone(), stopped.clone())?;
        Ok(HotkeyService { core, stopped })
    }

    #[cfg(target_os = "macos")]
    fn start_backend(
        _binding: Binding,
        _tx: Sender<HotkeyEvent>,
        core: Arc<Mutex<Core>>,
        stopped: Arc<AtomicBool>,
    ) -> Result<(), HotkeyError> {
        macos_fn::start(core, stopped)
    }

    #[cfg(not(target_os = "macos"))]
    fn start_backend(
        binding: Binding,
        tx: Sender<HotkeyEvent>,
        core: Arc<Mutex<Core>>,
        stopped: Arc<AtomicBool>,
    ) -> Result<(), HotkeyError> {
        if crate::insert::is_wayland() {
            return portal::start(binding, tx);
        }
        rdev_backend::start(core, stopped)
    }

    pub fn set_binding(&self, binding: Binding) {
        lock(&self.core).set_binding(binding);
    }

    pub fn set_command_binding(&self, binding: Option<Binding>) {
        lock(&self.core).set_command_binding(binding);
    }

    /// Esc emits `Cancel` only while armed (set by the dictation controller
    /// for the duration of a recording).
    pub fn set_armed(&self, armed: bool) {
        lock(&self.core).armed = armed;
    }

    /// Stop delivering events. Backend threads exit on their next wake-up
    /// (macOS) or stay parked inside `rdev::listen` ignoring events (which
    /// cannot be cancelled).
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Relaxed);
    }
}

impl Drop for HotkeyService {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chord_requires_all_keys() {
        let mut c = ChordTracker::new(Binding {
            keys: vec![Key::ControlLeft, Key::MetaLeft],
        });
        assert_eq!(c.on_key(Key::ControlLeft, true), None);
        assert_eq!(c.on_key(Key::MetaLeft, true), Some(true));
        assert_eq!(c.on_key(Key::MetaLeft, true), None); // key repeat
        assert_eq!(c.on_key(Key::ControlLeft, false), Some(false));
        assert_eq!(c.on_key(Key::MetaLeft, false), None);
    }

    #[test]
    fn binding_serde_roundtrip() {
        let b = Binding {
            keys: vec![Key::ControlLeft, Key::MetaLeft],
        };
        let s = serde_json::to_string(&b).unwrap();
        assert_eq!(s, r#"{"keys":["ControlLeft","MetaLeft"]}"#);
        assert_eq!(serde_json::from_str::<Binding>(&s).unwrap(), b);
        assert_eq!(
            serde_json::from_str::<Binding>(r#"{"keys":["Fn"]}"#)
                .unwrap()
                .keys,
            vec![Key::Fn]
        );
    }

    #[test]
    fn escape_cancels_only_when_armed() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut core = Core::new(
            Binding {
                keys: vec![Key::Fn],
            },
            tx,
        );
        core.handle(Key::Escape, true);
        assert!(rx.try_recv().is_err());
        core.handle(Key::Escape, false);
        core.armed = true;
        core.handle(Key::Escape, true);
        assert_eq!(rx.try_recv().unwrap(), HotkeyEvent::Cancel);
    }

    #[test]
    fn set_binding_mid_hold_releases() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut core = Core::new(
            Binding {
                keys: vec![Key::Fn],
            },
            tx,
        );
        core.handle(Key::Fn, true);
        assert_eq!(rx.try_recv().unwrap(), HotkeyEvent::Pressed);
        core.set_binding(Binding {
            keys: vec![Key::AltRight],
        });
        assert_eq!(rx.try_recv().unwrap(), HotkeyEvent::Released);
    }

    #[test]
    fn held_esc_cancels_once() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut core = Core::new(
            Binding {
                keys: vec![Key::Fn],
            },
            tx,
        );
        core.armed = true;
        core.handle(Key::Escape, true);
        core.handle(Key::Escape, true);
        core.handle(Key::Escape, true);
        assert_eq!(rx.try_recv().unwrap(), HotkeyEvent::Cancel);
        assert!(rx.try_recv().is_err());
        core.handle(Key::Escape, false);
        core.handle(Key::Escape, true);
        assert_eq!(rx.try_recv().unwrap(), HotkeyEvent::Cancel);
    }

    #[test]
    fn wayland_with_xwayland_display_is_wayland() {
        // XWayland sessions also set DISPLAY; only the Wayland vars matter.
        assert!(crate::insert::detect_wayland(Some("wayland-0"), None));
        assert!(crate::insert::detect_wayland(None, Some("wayland")));
        assert!(!crate::insert::detect_wayland(None, Some("x11")));
        assert!(!crate::insert::detect_wayland(None, None));
    }

    #[test]
    fn reset_clears_held_escape_so_the_next_press_cancels() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut core = Core::new(
            Binding {
                keys: vec![Key::Fn],
            },
            tx,
        );
        core.armed = true;
        core.handle(Key::Escape, true);
        assert_eq!(rx.try_recv().unwrap(), HotkeyEvent::Cancel);
        core.reset(); // e.g. the macOS tap was disabled and the Esc key-up was missed
        core.handle(Key::Escape, true);
        assert_eq!(rx.try_recv().unwrap(), HotkeyEvent::Cancel);
    }

    #[test]
    fn stuck_modifier_is_not_reported_as_a_held_chord() {
        // Win+L: the Win key-up goes to the lock screen, so the tracker thinks Win is still down.
        let (tx, rx) = std::sync::mpsc::channel();
        let mut core = Core::new(
            Binding {
                keys: vec![Key::ControlLeft, Key::MetaLeft],
            },
            tx,
        );
        let win_physically_down = Arc::new(AtomicBool::new(false));
        let flag = win_physically_down.clone();
        core.set_verifier(Box::new(move |k| {
            k != Key::MetaLeft || flag.load(Ordering::SeqCst)
        }));
        core.handle(Key::MetaLeft, true); // stale
        core.handle(Key::ControlLeft, true); // Ctrl alone must not start dictation
        assert!(
            rx.try_recv().is_err(),
            "stuck Win must not complete the chord"
        );
        // The user now really presses Win while Ctrl is held: the chord works again.
        win_physically_down.store(true, Ordering::SeqCst);
        core.handle(Key::MetaLeft, true);
        assert_eq!(rx.try_recv().unwrap(), HotkeyEvent::Pressed);
        core.handle(Key::MetaLeft, false);
        assert_eq!(rx.try_recv().unwrap(), HotkeyEvent::Released);
    }

    #[test]
    fn verified_chord_is_reported_normally() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut core = Core::new(
            Binding {
                keys: vec![Key::ControlLeft, Key::MetaLeft],
            },
            tx,
        );
        core.set_verifier(Box::new(|_| true));
        core.handle(Key::ControlLeft, true);
        core.handle(Key::MetaLeft, true);
        assert_eq!(rx.try_recv().unwrap(), HotkeyEvent::Pressed);
    }

    fn fn_ctrl_core() -> (Core, std::sync::mpsc::Receiver<HotkeyEvent>) {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut core = Core::new(
            Binding {
                keys: vec![Key::Fn],
            },
            tx,
        );
        core.set_command_binding(Some(Binding {
            keys: vec![Key::Fn, Key::ControlLeft],
        }));
        (core, rx)
    }

    #[test]
    fn adding_the_command_key_while_dictating_switches_to_command_mode() {
        let (mut core, rx) = fn_ctrl_core();
        core.handle(Key::Fn, true);
        assert_eq!(rx.try_recv().unwrap(), HotkeyEvent::Pressed);
        core.handle(Key::ControlLeft, true);
        assert_eq!(rx.try_recv().unwrap(), HotkeyEvent::Command);
        // Letting go of Ctrl first keeps the recording going until Fn is released.
        core.handle(Key::ControlLeft, false);
        assert!(rx.try_recv().is_err());
        core.handle(Key::Fn, false);
        assert_eq!(rx.try_recv().unwrap(), HotkeyEvent::Released);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn command_chord_pressed_together_reports_press_then_command() {
        let (mut core, rx) = fn_ctrl_core();
        core.handle(Key::ControlLeft, true);
        assert!(rx.try_recv().is_err(), "Ctrl alone does nothing");
        core.handle(Key::Fn, true);
        assert_eq!(rx.try_recv().unwrap(), HotkeyEvent::Pressed);
        assert_eq!(rx.try_recv().unwrap(), HotkeyEvent::Command);
        core.handle(Key::Fn, false);
        assert_eq!(rx.try_recv().unwrap(), HotkeyEvent::Released);
    }

    #[test]
    fn separate_command_chord_starts_its_own_recording() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut core = Core::new(
            Binding {
                keys: vec![Key::Fn],
            },
            tx,
        );
        core.set_command_binding(Some(Binding {
            keys: vec![Key::AltRight],
        }));
        core.handle(Key::AltRight, true);
        assert_eq!(rx.try_recv().unwrap(), HotkeyEvent::Pressed);
        assert_eq!(rx.try_recv().unwrap(), HotkeyEvent::Command);
        core.handle(Key::AltRight, false);
        assert_eq!(rx.try_recv().unwrap(), HotkeyEvent::Released);
    }

    #[test]
    fn without_a_command_binding_extra_keys_change_nothing() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut core = Core::new(
            Binding {
                keys: vec![Key::Fn],
            },
            tx,
        );
        core.handle(Key::Fn, true);
        core.handle(Key::ControlLeft, true);
        core.handle(Key::Fn, false);
        let got: Vec<_> = rx.try_iter().collect();
        assert_eq!(got, vec![HotkeyEvent::Pressed, HotkeyEvent::Released]);
    }

    #[test]
    fn default_command_binding_extends_the_dictation_chord() {
        let d = default_binding().keys;
        let c = default_command_binding().keys;
        assert_eq!(c.len(), d.len() + 1);
        assert!(d.iter().all(|k| c.contains(k)));
    }

    #[test]
    fn retry_trigger_fires_once_per_accessibility_grant() {
        let mut retried = false;
        // Not granted yet: nothing, and the memory stays clear.
        assert!(!retry_hotkey_start(true, false, &mut retried, false));
        // Granted while the slot is empty: retry once.
        assert!(retry_hotkey_start(true, true, &mut retried, false));
        // Still granted and still empty (the retry failed): do not hammer.
        assert!(!retry_hotkey_start(true, true, &mut retried, false));
        // Revoked then granted again: a new grant retries.
        assert!(!retry_hotkey_start(true, false, &mut retried, false));
        assert!(retry_hotkey_start(true, true, &mut retried, false));
        // A running service never needs a retry.
        let mut retried = false;
        assert!(!retry_hotkey_start(false, true, &mut retried, true));
    }

    #[test]
    fn retry_trigger_fires_when_onboarding_completes() {
        let mut retried = true;
        assert!(retry_hotkey_start(true, false, &mut retried, true));
    }
}
