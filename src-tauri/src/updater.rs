//! In-app updates. Release builds check GitHub's latest *published, non-prerelease* release
//! (`latest.json`, signed with the project's updater key) shortly after launch and then every few
//! hours. When a newer version exists the UI shows an "Update" button and the tray item changes to
//! "Install update…". Nothing is downloaded until the user clicks.
//!
//! Installing replaces only the application files. Settings, history, snippets, downloaded models
//! and keychain entries live in the user's app-data folder and keychain, which the installers never
//! touch, so they carry over unchanged.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Serialize;
use tauri::menu::MenuItem;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::events;

const FIRST_CHECK_AFTER: Duration = Duration::from_secs(20);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

pub const TRAY_ITEM: &str = "update";
const TRAY_IDLE: &str = "Check for updates…";

/// Managed state: the update found by the last check, if any.
#[derive(Default)]
pub struct Updates {
    pending: Mutex<Option<Update>>,
    installing: AtomicBool,
    tray_item: Mutex<Option<MenuItem<tauri::Wry>>>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
    /// True once the download finished and the installer is running.
    pub installing: bool,
}

fn info_of(u: &Update) -> UpdateInfo {
    UpdateInfo {
        version: u.version.clone(),
        current: u.current_version.clone(),
        notes: u.body.clone().filter(|b| !b.trim().is_empty()),
    }
}

/// Tray label for the update item.
pub fn tray_label(version: Option<&str>) -> String {
    match version {
        Some(v) => format!("Install update {v}…"),
        None => TRAY_IDLE.to_string(),
    }
}

pub fn new_tray_item(app: &AppHandle) -> tauri::Result<MenuItem<tauri::Wry>> {
    let item = MenuItem::with_id(app, TRAY_ITEM, TRAY_IDLE, true, None::<&str>)?;
    if let Some(st) = app.try_state::<Updates>() {
        *st.tray_item.lock().unwrap_or_else(|p| p.into_inner()) = Some(item.clone());
    }
    Ok(item)
}

fn set_tray(app: &AppHandle, version: Option<&str>) {
    if let Some(st) = app.try_state::<Updates>()
        && let Some(item) = st
            .tray_item
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
    {
        let _ = item.set_text(tray_label(version));
    }
}

/// The update found by the last check.
pub fn pending(app: &AppHandle) -> Option<UpdateInfo> {
    let st = app.try_state::<Updates>()?;
    let guard = st.pending.lock().unwrap_or_else(|p| p.into_inner());
    guard.as_ref().map(info_of)
}

/// Asks the release server for a newer version. Emits `UPDATE_AVAILABLE` when one is found.
pub async fn check(app: &AppHandle) -> Result<Option<UpdateInfo>, String> {
    let update = app
        .updater()
        .map_err(|e| format!("Could not check for updates: {e}"))?
        .check()
        .await
        .map_err(|e| format!("Could not check for updates: {e}"))?;
    let info = update.as_ref().map(info_of);
    if let Some(st) = app.try_state::<Updates>() {
        *st.pending.lock().unwrap_or_else(|p| p.into_inner()) = update;
    }
    set_tray(app, info.as_ref().map(|i| i.version.as_str()));
    if let Some(i) = &info {
        tracing::info!("update available: {} -> {}", i.current, i.version);
        let _ = app.emit(events::UPDATE_AVAILABLE, i);
    }
    Ok(info)
}

/// Downloads, verifies (minisign signature) and installs the pending update, then restarts.
/// On Windows the installer closes the app itself and reopens it when done.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    let Some(st) = app.try_state::<Updates>() else {
        return Err("Updates are not available.".into());
    };
    let update = match st.pending.lock().unwrap_or_else(|p| p.into_inner()).clone() {
        Some(u) => u,
        None => return Err("There is no update to install. Check for updates first.".into()),
    };
    if st.installing.swap(true, Ordering::SeqCst) {
        return Err("The update is already being installed.".into());
    }
    let mut downloaded = 0u64;
    let progress_app = app.clone();
    let done_app = app.clone();
    let result = update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = progress_app.emit(
                    events::UPDATE_PROGRESS,
                    UpdateProgress {
                        downloaded,
                        total,
                        installing: false,
                    },
                );
            },
            move || {
                let _ = done_app.emit(
                    events::UPDATE_PROGRESS,
                    UpdateProgress {
                        downloaded: 0,
                        total: None,
                        installing: true,
                    },
                );
            },
        )
        .await;
    if let Err(e) = result {
        st.installing.store(false, Ordering::SeqCst);
        tracing::warn!("update failed: {e}");
        return Err(format!("The update could not be installed: {e}"));
    }
    tracing::info!("update {} installed; restarting", update.version);
    app.restart();
}

/// Release builds only: first check shortly after launch, then every few hours.
pub fn spawn_background_checks(app: &AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    let app = app.clone();
    std::thread::Builder::new()
        .name("update-check".into())
        .spawn(move || {
            std::thread::sleep(FIRST_CHECK_AFTER);
            loop {
                if let Err(e) = tauri::async_runtime::block_on(check(&app)) {
                    tracing::info!("{e}");
                }
                std::thread::sleep(CHECK_EVERY);
            }
        })
        .map(|_| ())
        .unwrap_or_else(|e| tracing::warn!("could not start update checks: {e}"));
}

#[cfg(test)]
mod tests {
    use super::tray_label;

    #[test]
    fn tray_label_names_the_new_version() {
        assert_eq!(tray_label(None), "Check for updates…");
        assert_eq!(tray_label(Some("0.5.0")), "Install update 0.5.0…");
    }
}
