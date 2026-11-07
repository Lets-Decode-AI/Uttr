//! Wayland shortcuts end to end over D-Bus: uttr's real portal backend talks to a stand-in
//! GlobalShortcuts portal (`scripts/fake_portal.py`) on a private session bus. Linux only and
//! ignored by default; CI runs it as:
//!
//!   dbus-run-session -- sh -c 'python3 scripts/fake_portal.py & sleep 2; \
//!     cargo test --test portal_integration -- --ignored'
#![cfg(target_os = "linux")]

use std::sync::mpsc::channel;
use std::time::Duration;

use uttr_lib::hotkey::portal;
use uttr_lib::hotkey::{Binding, HotkeyEvent, Key};

#[test]
#[ignore = "needs the stand-in portal on a session bus"]
fn portal_activations_drive_dictation_and_command_mode() {
    let log = std::env::var_os("FAKE_PORTAL_LOG")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("uttr-fake-portal.json"));
    let (tx, rx) = channel();
    portal::start(
        Binding {
            keys: vec![Key::ControlLeft, Key::MetaLeft],
        },
        tx,
    )
    .expect("the portal session opens");

    let events: Vec<HotkeyEvent> = (0..5)
        .map(|i| {
            rx.recv_timeout(Duration::from_secs(10))
                .unwrap_or_else(|_| panic!("event {i}"))
        })
        .collect();
    assert_eq!(
        events,
        [
            HotkeyEvent::Pressed,  // dictate held
            HotkeyEvent::Released, // let go after 600 ms
            HotkeyEvent::Pressed,  // command held
            HotkeyEvent::Command,
            HotkeyEvent::Released,
        ]
    );

    let triggers: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&log).expect("the portal logged the bind")).unwrap();
    assert_eq!(triggers["dictate"], "CTRL+LOGO");
    assert_eq!(triggers["command"], "CTRL+LOGO+ALT");
}
