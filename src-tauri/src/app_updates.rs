use std::sync::Mutex;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri_plugin_updater::{Update, UpdaterExt};

const NO_PENDING_UPDATE: &str = "NO_PENDING_UPDATE";

/// The only mutable updater state kept by the application. The update payload,
/// URL, and signature never cross the command boundary.
pub struct PendingUpdateState<T> {
    pending: Option<T>,
}

impl<T> Default for PendingUpdateState<T> {
    fn default() -> Self {
        Self { pending: None }
    }
}

impl<T> PendingUpdateState<T> {
    pub fn replace(&mut self, value: Option<T>) {
        self.pending = value;
    }

    pub fn take(&mut self) -> Option<T> {
        self.pending.take()
    }

    pub fn take_for_install(&mut self) -> Result<T, &'static str> {
        self.take().ok_or(NO_PENDING_UPDATE)
    }

    /// Restores a pending value after a failed install so Retry can reuse the same
    /// verified update instead of re-fetching. The value is only restored when no
    /// newer fetch replaced it in the meantime.
    pub fn restore_after_failed_install(&mut self, value: T) {
        if self.pending.is_none() {
            self.pending = Some(value);
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMetadata {
    pub version: String,
    pub current_version: String,
    pub date: Option<String>,
    pub body: Option<String>,
}

impl From<&Update> for UpdateMetadata {
    fn from(update: &Update) -> Self {
        Self {
            version: update.version.clone(),
            current_version: update.current_version.clone(),
            date: update.date.map(|date| date.to_string()),
            body: update.body.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "data")]
pub enum DownloadEvent {
    #[serde(rename = "started", rename_all = "camelCase")]
    Started { content_length: Option<u64> },
    #[serde(rename = "progress", rename_all = "camelCase")]
    Progress {
        downloaded: u64,
        content_length: Option<u64>,
    },
    #[serde(rename = "finished")]
    Finished,
}

const CHECK_FAILED: &str = "Could not check for updates. Please try again later.";
const INSTALL_FAILED: &str = "Could not install the update. Please try again later.";
const STATE_UNAVAILABLE: &str = "The update service is temporarily unavailable.";

fn state_error() -> String {
    STATE_UNAVAILABLE.to_string()
}

#[tauri::command]
pub async fn fetch_update(
    app: tauri::AppHandle,
    state: tauri::State<'_, Mutex<PendingUpdateState<Update>>>,
) -> Result<Option<UpdateMetadata>, String> {
    let update = app
        .updater()
        .map_err(|_| CHECK_FAILED.to_string())?
        .check()
        .await
        .map_err(|_| CHECK_FAILED.to_string())?;

    let metadata = update.as_ref().map(UpdateMetadata::from);
    state.lock().map_err(|_| state_error())?.replace(update);
    Ok(metadata)
}

#[tauri::command]
pub async fn install_update(
    on_event: Channel<DownloadEvent>,
    state: tauri::State<'_, Mutex<PendingUpdateState<Update>>>,
) -> Result<(), String> {
    let update = state
        .lock()
        .map_err(|_| state_error())?
        .take_for_install()
        .map_err(str::to_string)?;

    let _ = on_event.send(DownloadEvent::Started {
        content_length: None,
    });
    let mut downloaded = 0u64;
    let progress_channel = on_event.clone();
    let result = update
        .download_and_install(
            move |chunk_length, content_length| {
                downloaded = downloaded.saturating_add(chunk_length as u64);
                let _ = progress_channel.send(DownloadEvent::Progress {
                    downloaded,
                    content_length,
                });
            },
            || {
                let _ = on_event.send(DownloadEvent::Finished);
            },
        )
        .await;

    if result.is_ok() {
        return Ok(());
    }

    // A failed install must not consume the pending update permanently. Restore it
    // so a user-visible Retry can re-download from the same verified metadata
    // instead of surfacing NO_PENDING_UPDATE. The frontend maps this failure to the
    // stable INSTALL_FAILED message before any retry is offered.
    if let Ok(mut guard) = state.lock() {
        guard.restore_after_failed_install(update);
    }
    Err(INSTALL_FAILED.to_string())
}

#[cfg(test)]
mod tests {
    use super::PendingUpdateState;

    #[test]
    fn successful_fetch_replaces_previous_pending_update() {
        let mut state = PendingUpdateState::default();
        state.replace(Some("old"));
        state.replace(Some("new"));

        assert_eq!(state.take(), Some("new"));
    }

    #[test]
    fn no_update_clears_previous_pending_update() {
        let mut state = PendingUpdateState::default();
        state.replace(Some("old"));
        state.replace(None);

        assert_eq!(state.take(), None);
    }

    #[test]
    fn install_takes_pending_update_exactly_once() {
        let mut state = PendingUpdateState::default();
        state.replace(Some("update"));

        assert_eq!(state.take_for_install(), Ok("update"));
        assert_eq!(state.take_for_install(), Err("NO_PENDING_UPDATE"));
    }

    #[test]
    fn failed_install_restores_pending_update_for_retry() {
        let mut state = PendingUpdateState::default();
        state.replace(Some("update"));

        // Install fails -> backend restores the pending value so Retry can reuse it.
        let taken = state.take_for_install().unwrap();
        state.restore_after_failed_install(taken);

        assert_eq!(state.take_for_install(), Ok("update"));
        assert_eq!(state.take_for_install(), Err("NO_PENDING_UPDATE"));
    }

    #[test]
    fn failed_install_does_not_restore_over_a_newer_fetch() {
        let mut state = PendingUpdateState::default();
        state.replace(Some("first"));

        let taken = state.take_for_install().unwrap();
        // A concurrent/newer fetch replaced the pending value while install ran.
        state.replace(Some("newer"));
        state.restore_after_failed_install(taken);

        assert_eq!(state.take_for_install(), Ok("newer"));
    }
}
