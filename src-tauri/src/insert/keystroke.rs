use std::sync::{Arc, mpsc};
use std::time::Duration;

use anyhow::{Result, anyhow};
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

pub trait Keystroke {
    fn paste(&self) -> Result<()>;
    /// Synthetic copy (Cmd+C / Ctrl+C), used by Command Mode to read the selection.
    fn copy(&self) -> Result<()> {
        Err(anyhow!("copy is not supported"))
    }
}

/// Synthetic paste via enigo: Cmd+V on macOS, Ctrl+V elsewhere. An `Enigo`
/// is created per call (cheap, avoids holding non-Send OS handles).
pub struct EnigoKeystroke;

#[cfg(target_os = "macos")]
const MODIFIER: Key = Key::Meta;
#[cfg(not(target_os = "macos"))]
const MODIFIER: Key = Key::Control;

impl EnigoKeystroke {
    fn chord(&self, key: char) -> Result<()> {
        // uttr shows its own permission banner; enigo must not open the system dialog on
        // every paste.
        let settings = Settings {
            open_prompt_to_get_permissions: false,
            ..Settings::default()
        };
        let mut enigo = Enigo::new(&settings).map_err(|e| anyhow!("enigo init: {e}"))?;
        // The hotkey hook sees synthetic keys like real ones; keep it from reacting to them.
        crate::hotkey::suppress_for(SYNTHETIC_WINDOW);
        enigo
            .key(MODIFIER, Direction::Press)
            .map_err(|e| anyhow!("modifier press: {e}"))?;
        let clicked = enigo.key(Key::Unicode(key), Direction::Click);
        // Always release the modifier, even if the key click failed.
        let released = enigo.key(MODIFIER, Direction::Release);
        clicked.map_err(|e| anyhow!("{key} click: {e}"))?;
        crate::hotkey::suppress_for(SYNTHETIC_WINDOW);
        released.map_err(|e| anyhow!("modifier release: {e}"))?;
        Ok(())
    }
}

/// How long the hotkey hook ignores keys around a synthetic keystroke.
const SYNTHETIC_WINDOW: Duration = Duration::from_millis(150);

/// Longest wait for the user to let go of the shortcut's modifiers before a synthetic paste.
const MODIFIERS_UP_TIMEOUT: Duration = Duration::from_millis(1500);

/// Waits (up to [`MODIFIERS_UP_TIMEOUT`]) until Win, Alt and Shift are physically up. The paste
/// can be ready while the user still holds part of the shortcut (Ctrl released first, or the
/// press that stops hands-free mode); Ctrl+V sent then arrives as Ctrl+Win+V or Ctrl+Alt+V,
/// which the app does not treat as paste. Windows and X11 only; elsewhere this returns at once.
fn wait_for_modifiers_up() {
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        use crate::hotkey::{Key as K, physical::is_down};
        #[cfg(target_os = "linux")]
        if crate::insert::is_wayland() {
            return;
        }
        const KEYS: [K; 6] = [
            K::MetaLeft,
            K::MetaRight,
            K::AltLeft,
            K::AltRight,
            K::ShiftLeft,
            K::ShiftRight,
        ];
        let deadline = std::time::Instant::now() + MODIFIERS_UP_TIMEOUT;
        while KEYS.iter().any(|k| is_down(*k)) {
            if std::time::Instant::now() >= deadline {
                tracing::debug!("modifiers still held; pasting anyway");
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Keystroke for EnigoKeystroke {
    fn paste(&self) -> Result<()> {
        self.chord('v')
    }
    fn copy(&self) -> Result<()> {
        self.chord('c')
    }
}

pub type Job = Box<dyn FnOnce() + Send>;
/// Schedules a job on the app's main thread (Tauri's `run_on_main_thread`).
pub type Runner = Arc<dyn Fn(Job) + Send + Sync>;

/// How long the caller waits for the main thread to perform the paste.
const MAIN_THREAD_TIMEOUT: Duration = Duration::from_secs(2);

/// Runs the inner keystroke on the main thread and waits for it.
///
/// On macOS, enigo resolves the "v" key through the Text Input Sources API,
/// which asserts that it runs on the main dispatch queue; calling it from a
/// worker thread aborts the process. Must not be called from the main thread
/// itself, or it waits on a job that can never run.
pub struct MainThreadKeystroke<K> {
    inner: Arc<K>,
    runner: Runner,
}

impl<K: Keystroke + Send + Sync + 'static> MainThreadKeystroke<K> {
    pub fn new(inner: K, runner: Runner) -> Self {
        Self {
            inner: Arc::new(inner),
            runner,
        }
    }
}

impl<K: Keystroke + Send + Sync + 'static> MainThreadKeystroke<K> {
    fn on_main(&self, f: fn(&K) -> Result<()>) -> Result<()> {
        // Off the main thread: the wait can take a moment.
        wait_for_modifiers_up();
        let (tx, rx) = mpsc::channel();
        let inner = self.inner.clone();
        (self.runner)(Box::new(move || {
            let _ = tx.send(f(&inner));
        }));
        rx.recv_timeout(MAIN_THREAD_TIMEOUT)
            .map_err(|_| anyhow!("the keystroke did not run on the main thread"))?
    }
}

impl<K: Keystroke + Send + Sync + 'static> Keystroke for MainThreadKeystroke<K> {
    fn paste(&self) -> Result<()> {
        self.on_main(K::paste)
    }
    fn copy(&self) -> Result<()> {
        self.on_main(K::copy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::thread::{self, ThreadId};

    /// Records the thread the inner keystroke ran on.
    struct Probe(Arc<Mutex<Option<ThreadId>>>);
    impl Keystroke for Probe {
        fn paste(&self) -> Result<()> {
            *self.0.lock().unwrap() = Some(thread::current().id());
            Ok(())
        }
    }

    #[test]
    fn paste_runs_on_the_runner_thread_not_the_caller() {
        // Stand-in for the main thread: a dedicated thread that runs jobs.
        let (tx, rx) = std::sync::mpsc::channel::<Job>();
        let main = thread::spawn(move || {
            let id = thread::current().id();
            if let Ok(job) = rx.recv() {
                job();
            }
            id
        });
        let runner: Runner = Arc::new(move |job| {
            let _ = tx.send(job);
        });
        let seen = Arc::new(Mutex::new(None));
        let ks = MainThreadKeystroke::new(Probe(seen.clone()), runner);

        ks.paste().unwrap();

        let main_id = main.join().unwrap();
        assert_eq!(*seen.lock().unwrap(), Some(main_id));
        assert_ne!(main_id, thread::current().id());
    }

    #[test]
    fn paste_errors_if_the_runner_drops_the_job() {
        let runner: Runner = Arc::new(|_job| {});
        let ks = MainThreadKeystroke::new(Probe(Arc::new(Mutex::new(None))), runner);
        assert!(ks.paste().is_err());
    }
}
