//! Wayland backend. Compositors do not let clients observe global key state; the sanctioned
//! route is the XDG GlobalShortcuts portal, which reports when user-approved shortcuts are
//! activated and deactivated. uttr binds two: `dictate` and `command`. Their activations
//! drive the same hold / double-tap gestures as key chords. The portal cannot report Esc, so
//! Esc does not cancel a dictation on Wayland.
use super::gesture::GestureDetector;
use super::{Binding, HotkeyError, HotkeyEvent, Key};
use std::sync::mpsc::Sender;
use std::time::Instant;

pub const DICTATE_ID: &str = "dictate";
pub const COMMAND_ID: &str = "command";

/// The portal's preferred-trigger string for a binding, such as `CTRL+LOGO`. `Fn` has no
/// portal name and is left out.
pub fn trigger(b: &Binding) -> String {
    let has = |ks: &[Key]| ks.iter().any(|k| b.keys.contains(k));
    let parts: Vec<&str> = [
        (has(&[Key::ControlLeft, Key::ControlRight]), "CTRL"),
        (has(&[Key::ShiftLeft, Key::ShiftRight]), "SHIFT"),
        (has(&[Key::MetaLeft, Key::MetaRight]), "LOGO"),
        (has(&[Key::AltLeft, Key::AltRight]), "ALT"),
        (has(&[Key::Space]), "space"),
    ]
    .into_iter()
    .filter_map(|(on, name)| on.then_some(name))
    .collect();
    parts.join("+")
}

/// Turns portal activation signals into hotkey events, like `Core` does for key chords.
#[derive(Default)]
pub struct PortalChords {
    dictate: bool,
    command: bool,
    gesture: GestureDetector,
}

impl PortalChords {
    pub fn on_signal(&mut self, id: &str, active: bool, at: Instant) -> Vec<HotkeyEvent> {
        let was_held = self.dictate || self.command;
        let flag = match id {
            DICTATE_ID => &mut self.dictate,
            COMMAND_ID => &mut self.command,
            _ => return vec![],
        };
        if std::mem::replace(flag, active) == active {
            return vec![]; // a repeat
        }
        let held = self.dictate || self.command;
        let mut events = if held != was_held {
            self.gesture.on_key(held, at)
        } else {
            vec![]
        };
        if id == COMMAND_ID && active {
            events.push(HotkeyEvent::Command);
        }
        events
    }
}

#[cfg(not(target_os = "linux"))]
pub fn start(_binding: Binding, _tx: Sender<HotkeyEvent>) -> Result<(), HotkeyError> {
    Err(HotkeyError::Unsupported)
}

/// Opens a portal session on its own thread and binds the shortcuts. Returns once the portal
/// answered (binding may still wait for the user to approve it in a system dialog).
#[cfg(target_os = "linux")]
pub fn start(binding: Binding, tx: Sender<HotkeyEvent>) -> Result<(), HotkeyError> {
    use std::time::Duration;
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("uttr-portal".into())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = ready_tx.send(Err(HotkeyError::Backend(e.to_string())));
                    return;
                }
            };
            if let Err(e) = rt.block_on(linux::run(binding, tx, ready_tx.clone())) {
                tracing::warn!("global shortcuts portal: {e}");
                let _ = ready_tx.send(Err(HotkeyError::Unsupported));
            }
        })
        .map_err(|e| HotkeyError::Backend(e.to_string()))?;
    ready_rx
        .recv_timeout(Duration::from_secs(5))
        .unwrap_or(Err(HotkeyError::Unsupported))
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use ashpd::desktop::CreateSessionOptions;
    use ashpd::desktop::global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut};
    use futures_util::StreamExt;

    pub async fn run(
        binding: Binding,
        tx: Sender<HotkeyEvent>,
        ready: Sender<Result<(), HotkeyError>>,
    ) -> ashpd::Result<()> {
        let portal = GlobalShortcuts::new().await?;
        let session = portal
            .create_session(CreateSessionOptions::default())
            .await?;
        let _ = ready.send(Ok(()));
        let mut command = binding.clone();
        command
            .keys
            .extend(super::super::default_command_binding().keys);
        let shortcuts = [
            NewShortcut::new(DICTATE_ID, "Hold to dictate")
                .preferred_trigger(Some(trigger(&binding).as_str()).filter(|t| !t.is_empty())),
            NewShortcut::new(COMMAND_ID, "Hold for Command Mode")
                .preferred_trigger(Some(trigger(&command).as_str()).filter(|t| !t.is_empty())),
        ];
        let bound = portal
            .bind_shortcuts(&session, &shortcuts, None, BindShortcutsOptions::default())
            .await?
            .response()?;
        for s in bound.shortcuts() {
            tracing::info!("portal shortcut {}: {}", s.id(), s.trigger_description());
        }
        let mut on = portal.receive_activated().await?;
        let mut off = portal.receive_deactivated().await?;
        let mut chords = PortalChords::default();
        loop {
            let (id, active) = tokio::select! {
                Some(a) = on.next() => (a.shortcut_id().to_string(), true),
                Some(d) = off.next() => (d.shortcut_id().to_string(), false),
                else => break,
            };
            for ev in chords.on_signal(&id, active, Instant::now()) {
                if tx.send(ev).is_err() {
                    return Ok(());
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(keys: &[Key]) -> Binding {
        Binding {
            keys: keys.to_vec(),
        }
    }

    #[test]
    fn triggers_use_portal_modifier_names() {
        assert_eq!(trigger(&b(&[Key::ControlLeft, Key::MetaLeft])), "CTRL+LOGO");
        assert_eq!(
            trigger(&b(&[Key::ControlLeft, Key::MetaLeft, Key::AltLeft])),
            "CTRL+LOGO+ALT"
        );
        assert_eq!(trigger(&b(&[Key::AltRight, Key::ShiftRight])), "SHIFT+ALT");
        assert_eq!(trigger(&b(&[Key::ControlRight, Key::Space])), "CTRL+space");
        assert_eq!(trigger(&b(&[Key::Fn, Key::ControlLeft])), "CTRL");
        assert_eq!(trigger(&b(&[Key::ControlLeft, Key::ControlRight])), "CTRL");
    }

    #[test]
    fn hold_and_release_of_dictate_report_press_and_release() {
        let mut c = PortalChords::default();
        let t = Instant::now();
        assert_eq!(c.on_signal(DICTATE_ID, true, t), [HotkeyEvent::Pressed]);
        let later = t + std::time::Duration::from_secs(2);
        assert_eq!(
            c.on_signal(DICTATE_ID, false, later),
            [HotkeyEvent::Released]
        );
    }

    #[test]
    fn command_reports_press_then_command() {
        let mut c = PortalChords::default();
        let t = Instant::now();
        assert_eq!(
            c.on_signal(COMMAND_ID, true, t),
            [HotkeyEvent::Pressed, HotkeyEvent::Command]
        );
        assert_eq!(
            c.on_signal(COMMAND_ID, false, t + std::time::Duration::from_secs(1)),
            [HotkeyEvent::Released]
        );
    }

    #[test]
    fn command_while_dictating_switches_without_a_new_press() {
        let mut c = PortalChords::default();
        let t = Instant::now();
        c.on_signal(DICTATE_ID, true, t);
        assert_eq!(c.on_signal(COMMAND_ID, true, t), [HotkeyEvent::Command]);
        assert!(
            c.on_signal(COMMAND_ID, false, t).is_empty(),
            "dictate is still held"
        );
        assert_eq!(
            c.on_signal(DICTATE_ID, false, t + std::time::Duration::from_secs(1)),
            [HotkeyEvent::Released]
        );
    }

    #[test]
    fn double_press_is_hands_free() {
        let mut c = PortalChords::default();
        let t = Instant::now();
        let ms = |n| t + std::time::Duration::from_millis(n);
        c.on_signal(DICTATE_ID, true, t);
        c.on_signal(DICTATE_ID, false, ms(100));
        assert_eq!(
            c.on_signal(DICTATE_ID, true, ms(250)),
            [HotkeyEvent::DoubleTap]
        );
    }

    #[test]
    fn repeated_and_unknown_signals_are_ignored() {
        let mut c = PortalChords::default();
        let t = Instant::now();
        c.on_signal(DICTATE_ID, true, t);
        assert!(c.on_signal(DICTATE_ID, true, t).is_empty());
        assert!(c.on_signal("other", true, t).is_empty());
    }
}
