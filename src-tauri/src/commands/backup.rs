use crate::{
    backup_command_error, emit_focus_mode_status, ensure_runtime_folder_watcher, library_backup,
    load_restored_playback_state, parse_bool_setting, record_source_error, AppState, CommandError,
    CommandResult, RestoredPlaybackState, SETTING_FOCUS_MODE,
};
use std::path::Path;
use std::sync::atomic::Ordering;
use tauri::{Emitter, Manager};

pub(crate) fn apply_backup_runtime_state(
    state: &AppState,
    app: &tauri::AppHandle,
    restored: RestoredPlaybackState,
    focus_enabled: bool,
) -> Result<(), String> {
    state
        .rotation_secs
        .store(restored.rotation_secs, Ordering::Relaxed);
    {
        let mut display_mode = state.display_mode.lock().map_err(|e| e.to_string())?;
        *display_mode = restored.display_mode;
    }

    state.focus_mode.set_enabled(focus_enabled);
    if !focus_enabled {
        state.focus_mode.set_fullscreen_detected(false);
        state.focus_mode.set_auto_paused(false);
        state.focus_mode.set_auto_pause_suppressed(false);
    }

    let effective_paused = restored.manual_paused || state.focus_mode.auto_paused();
    {
        let mut paused = state.is_paused.lock().map_err(|e| e.to_string())?;
        *paused = effective_paused;
    }

    state.rotation_signal.notify();
    state.focus_signal.notify();
    emit_focus_mode_status(app, state);
    let _ = app.emit("pause-changed", effective_paused);
    Ok(())
}

#[tauri::command]
pub(crate) fn export_library_backup(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    destination: String,
    client_settings: library_backup::BackupClientSettings,
) -> CommandResult<library_backup::BackupExportResult> {
    let destination = destination.trim();
    if destination.is_empty() {
        return Err(CommandError::new(
            "BACKUP_WRITE_FAILED",
            "A backup destination is required.",
        ));
    }
    let document = {
        let db = state.db.lock().map_err(|_| {
            CommandError::new(
                "database_unavailable",
                "Could not access the wallpaper database for backup export.",
            )
        })?;
        db.export_backup_snapshot(
            client_settings,
            chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        )
        .map_err(CommandError::from_display)?
    };

    let app_data_dir = app.path().app_data_dir().map_err(|error| {
        CommandError::new(
            "BACKUP_UNSAFE_DESTINATION",
            format!("Could not resolve PureWall application data: {error}"),
        )
    })?;
    let bytes = library_backup::serialize_bounded(&document).map_err(backup_command_error)?;
    let destination_path = Path::new(destination);
    let protected_wallpapers = document
        .wallpapers
        .iter()
        .map(|wallpaper| wallpaper.path.clone())
        .collect::<Vec<_>>();
    library_backup::validate_export_destination(
        destination_path,
        &protected_wallpapers,
        Some(&app_data_dir),
    )
    .map_err(backup_command_error)?;
    library_backup::write_atomically(destination_path, &bytes).map_err(backup_command_error)?;
    Ok(library_backup::BackupExportResult {
        path: destination.to_string(),
        bytes: bytes.len() as u64,
    })
}

#[tauri::command]
pub(crate) fn preview_backup_import(
    state: tauri::State<AppState>,
    path: String,
) -> CommandResult<library_backup::BackupImportPreview> {
    let bytes =
        library_backup::read_bounded(Path::new(path.trim())).map_err(backup_command_error)?;
    let content_digest = library_backup::backup_content_digest(&bytes);
    let backup = library_backup::parse_and_normalize(&bytes).map_err(backup_command_error)?;
    let db = state.db.lock().map_err(|_| {
        CommandError::new(
            "database_unavailable",
            "Could not access the wallpaper database for backup preview.",
        )
    })?;
    db.preview_backup_import(&backup, content_digest)
        .map_err(CommandError::from_display)
}

#[tauri::command]
pub(crate) fn import_library_backup(
    state: tauri::State<AppState>,
    app: tauri::AppHandle,
    path: String,
    expected_digest: String,
) -> CommandResult<library_backup::BackupImportResult> {
    let bytes =
        library_backup::read_bounded(Path::new(path.trim())).map_err(backup_command_error)?;
    library_backup::verify_preview_digest(&bytes, expected_digest.trim())
        .map_err(backup_command_error)?;
    let backup = library_backup::parse_and_normalize(&bytes).map_err(backup_command_error)?;

    let (merge, runtime_state, persisted_sources) = {
        let mut db = state.db.lock().map_err(|_| {
            CommandError::new(
                "database_unavailable",
                "Could not access the wallpaper database for backup import.",
            )
        })?;
        let merge = db
            .merge_backup(&backup)
            .map_err(CommandError::from_display)?;
        let runtime_state = (|| -> Result<(RestoredPlaybackState, bool), String> {
            let restored = load_restored_playback_state(&db)?;
            let focus_enabled = parse_bool_setting(
                db.get_setting(SETTING_FOCUS_MODE)
                    .map_err(|error| error.to_string())?,
            );
            Ok((restored, focus_enabled))
        })();
        let persisted_sources = db.get_watched_folders().map_err(|error| error.to_string());
        (merge, runtime_state, persisted_sources)
    };

    let mut warnings = Vec::new();
    match runtime_state {
        Ok((restored, focus_enabled)) => {
            if let Err(error) =
                apply_backup_runtime_state(state.inner(), &app, restored, focus_enabled)
            {
                warnings.push(format!(
                    "Metadata was committed, but runtime settings could not be refreshed: {error}"
                ));
            }
        }
        Err(error) => warnings.push(format!(
            "Metadata was committed, but runtime settings could not be read back: {error}"
        )),
    }

    match persisted_sources {
        Ok(sources) => {
            let sources = sources
                .into_iter()
                .map(|source| library_backup::BackupSource {
                    path: source.path,
                    source: source.source,
                })
                .collect::<Vec<_>>();
            let reconciliation =
                library_backup::reconcile_persisted_sources(&sources, |path, source| {
                    ensure_runtime_folder_watcher(&app, path, source)
                        .map(|_| ())
                        .map_err(|error| error.message)
                });
            for failure in &reconciliation.failures {
                record_source_error(state.inner(), &failure.path, &failure.message);
            }
            warnings.extend(reconciliation.warnings);
        }
        Err(error) => warnings.push(format!(
            "Metadata was committed, but persisted sources could not be read for watcher restoration: {error}"
        )),
    }

    Ok(library_backup::BackupImportResult {
        committed: true,
        merge,
        warnings,
    })
}
