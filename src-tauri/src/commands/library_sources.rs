use crate::{
    begin_source_operation, db, ensure_runtime_folder_watcher, library_source_entry,
    library_sources, paths, record_source_error, scan_and_persist_source_snapshot, scanner,
    source_database_error, source_path_error, take_runtime_folder_watcher, validate_source_folder,
    AppState, CommandError, CommandResult, ImportResult, ReconciliationOutcome,
};
use std::path::Path;
use tauri::Emitter;

pub(crate) fn rescan_source(
    state: &AppState,
    app: &tauri::AppHandle,
    requested_path: &str,
) -> CommandResult<ImportResult> {
    let folder = library_source_entry(state, requested_path)?;
    let _operation = begin_source_operation(state, &folder.path)?;
    let result = (|| {
        let canonical = validate_source_folder(&folder.path, "SOURCE_OFFLINE")?;
        if paths::path_identity_key(Path::new(&canonical))
            != paths::path_identity_key(Path::new(&folder.path))
        {
            return Err(CommandError::new(
                "SOURCE_INVALID_PATH",
                "The source now resolves to a different folder. Use Relocate to preserve metadata.",
            ));
        }
        ensure_runtime_folder_watcher(app, &folder.path, &folder.source)
            .map_err(|error| source_path_error(&folder.path, error.message, "SOURCE_OFFLINE"))?;
        scan_and_persist_source_snapshot(state, &folder.path, &folder.source)
    })();

    match result {
        Ok(ReconciliationOutcome::Applied(summary)) => {
            if let Err(error) = app.emit("folder-changed", summary.clone()) {
                eprintln!("[PureWall] Failed to emit source rescan completion: {error}");
            }
            Ok(summary)
        }
        Ok(ReconciliationOutcome::SkippedStale) => Err(CommandError::new(
            "SOURCE_STALE",
            "The library source changed before its rescan could be saved.",
        )),
        Err(error) => {
            record_source_error(state, &folder.path, &error.message);
            Err(error)
        }
    }
}

#[tauri::command]
pub(crate) fn list_library_sources(
    state: tauri::State<AppState>,
) -> CommandResult<Vec<library_sources::LibrarySource>> {
    let summaries = {
        let db = state.db.lock().map_err(|_| {
            CommandError::new(
                "database_unavailable",
                "Could not access the wallpaper database.",
            )
        })?;
        db.get_watched_folder_summaries()
            .map_err(CommandError::from_display)?
    };
    let scanning_paths = state
        .source_operations
        .lock()
        .map_err(|_| {
            CommandError::new(
                "SOURCE_OPERATIONS_UNAVAILABLE",
                "Could not read current library source operations.",
            )
        })?
        .clone();

    Ok(summaries
        .into_iter()
        .map(|summary| {
            let entry = summary.entry;
            let status = library_sources::status_for(
                &entry.path,
                &scanning_paths,
                entry.last_error.as_deref(),
            );
            library_sources::LibrarySource {
                path: entry.path,
                source: entry.source,
                status,
                available_count: summary.available_count,
                unavailable_count: summary.unavailable_count,
                last_scan_at: entry.last_scan_at,
                last_error: entry.last_error,
            }
        })
        .collect())
}

#[tauri::command]
pub(crate) fn rescan_library_source(
    state: tauri::State<AppState>,
    app: tauri::AppHandle,
    path: String,
) -> CommandResult<ImportResult> {
    rescan_source(&state, &app, &path)
}

#[tauri::command]
pub(crate) fn retry_library_source(
    state: tauri::State<AppState>,
    app: tauri::AppHandle,
    path: String,
) -> CommandResult<ImportResult> {
    rescan_source(&state, &app, &path)
}

#[tauri::command]
pub(crate) fn preview_remove_library_source(
    state: tauri::State<AppState>,
    path: String,
) -> CommandResult<library_sources::RemoveLibrarySourceImpact> {
    let folder = library_source_entry(&state, &path)?;
    let db = state.db.lock().map_err(|_| {
        CommandError::new(
            "database_unavailable",
            "Could not access the wallpaper database.",
        )
    })?;
    let affected_wallpapers = db
        .source_removal_impact(&folder.path)
        .map_err(source_database_error)?;
    Ok(library_sources::RemoveLibrarySourceImpact {
        affected_wallpapers,
    })
}

#[tauri::command]
pub(crate) fn remove_library_source(
    state: tauri::State<AppState>,
    app: tauri::AppHandle,
    path: String,
    mode: library_sources::RemoveLibrarySourceMode,
) -> CommandResult<library_sources::LibrarySourceMutation> {
    let folder = library_source_entry(&state, &path)?;
    let _operation = begin_source_operation(&state, &folder.path)?;
    let summary = {
        let db = state.db.lock().map_err(|_| {
            CommandError::new(
                "database_unavailable",
                "Could not access the wallpaper database.",
            )
        })?;
        db.remove_watched_folder(&folder.path, mode.into())
            .map_err(source_database_error)?
    };

    let mut watcher_warning = None;
    let removed_watcher = match state.folder_watchers.lock() {
        Ok(mut watchers) => take_runtime_folder_watcher(&mut watchers, &folder.path),
        Err(_) => {
            watcher_warning = Some(
                "The source was removed, but its watcher could not be stopped until PureWall restarts."
                    .to_string(),
            );
            None
        }
    };
    drop(removed_watcher);
    state
        .watcher_queue
        .remove_root(&folder.path, &folder.source);

    let unavailable_wallpapers = match mode {
        library_sources::RemoveLibrarySourceMode::KeepMetadata => summary.affected_wallpapers,
        library_sources::RemoveLibrarySourceMode::ClearMetadata => 0,
    };
    let mutation = library_sources::LibrarySourceMutation {
        affected_wallpapers: summary.affected_wallpapers,
        matched_wallpapers: 0,
        imported_wallpapers: 0,
        unavailable_wallpapers,
        watcher_warning,
    };
    if let Err(error) = app.emit(
        "folder-changed",
        ImportResult {
            scanned: 0,
            imported: 0,
        },
    ) {
        eprintln!("[PureWall] Failed to emit source removal: {error}");
    }
    Ok(mutation)
}

#[tauri::command]
pub(crate) fn relocate_library_source(
    state: tauri::State<AppState>,
    app: tauri::AppHandle,
    path: String,
    new_path: String,
) -> CommandResult<library_sources::LibrarySourceMutation> {
    let folder = library_source_entry(&state, &path)?;
    let _operation = begin_source_operation(&state, &folder.path)?;
    let target = match validate_source_folder(&new_path, "SOURCE_INVALID_PATH") {
        Ok(target) => target,
        Err(error) => {
            record_source_error(&state, &folder.path, &error.message);
            return Err(error);
        }
    };
    if paths::path_identity_key(Path::new(&folder.path))
        == paths::path_identity_key(Path::new(&target))
    {
        let error = CommandError::new(
            "SOURCE_PATH_CONFLICT",
            "Choose a different folder for relocation.",
        );
        record_source_error(&state, &folder.path, &error.message);
        return Err(error);
    }

    let images = match scanner::scan_folder(&target) {
        Ok(images) => images,
        Err(error) => {
            let error = source_path_error(&target, error.to_string(), "SOURCE_OFFLINE");
            record_source_error(&state, &folder.path, &error.message);
            return Err(error);
        }
    };
    let scanned = images
        .iter()
        .map(|image| db::ScannedWallpaperRecord {
            path: image.path.clone(),
            hash: image.hash.clone(),
            width: image.width,
            height: image.height,
            file_size: image.file_size,
        })
        .collect::<Vec<_>>();
    let relocation = {
        let db = state.db.lock().map_err(|_| {
            CommandError::new(
                "database_unavailable",
                "Could not access the wallpaper database.",
            )
        })?;
        match db.relocate_watched_folder(&folder.path, &target, &folder.source, &scanned) {
            Ok(summary) => summary,
            Err(error) => {
                let error = source_database_error(error);
                drop(db);
                record_source_error(&state, &folder.path, &error.message);
                return Err(error);
            }
        }
    };

    let mut warnings = Vec::new();
    let old_watcher = match state.folder_watchers.lock() {
        Ok(mut watchers) => take_runtime_folder_watcher(&mut watchers, &folder.path),
        Err(_) => {
            warnings
                .push("The old watcher could not be stopped until PureWall restarts.".to_string());
            None
        }
    };
    drop(old_watcher);
    state
        .watcher_queue
        .remove_root(&folder.path, &folder.source);

    match ensure_runtime_folder_watcher(&app, &target, &folder.source) {
        Ok(_) => {
            match scan_and_persist_source_snapshot(&state, &target, &folder.source) {
                Ok(ReconciliationOutcome::Applied(_)) => {}
                Ok(ReconciliationOutcome::SkippedStale) => warnings.push(
                    "Relocation committed, but the source changed before the handoff rescan could be saved."
                        .to_string(),
                ),
                Err(error) => {
                    record_source_error(&state, &target, &error.message);
                    warnings.push(format!(
                        "Relocation committed, but the handoff rescan failed: {}",
                        error.message
                    ));
                }
            }
        }
        Err(error) => {
            let error = source_path_error(&target, error.message, "SOURCE_OFFLINE");
            record_source_error(&state, &target, &error.message);
            warnings.push(format!(
                "Relocation committed, but the watcher could not start: {}",
                error.message
            ));
        }
    }

    let mutation = library_sources::LibrarySourceMutation {
        affected_wallpapers: relocation.matched + relocation.unavailable,
        matched_wallpapers: relocation.matched,
        imported_wallpapers: relocation.imported,
        unavailable_wallpapers: relocation.unavailable,
        watcher_warning: (!warnings.is_empty()).then(|| warnings.join(" ")),
    };
    if let Err(error) = app.emit(
        "folder-changed",
        ImportResult {
            scanned: images.len(),
            imported: relocation.imported,
        },
    ) {
        eprintln!("[PureWall] Failed to emit source relocation: {error}");
    }
    Ok(mutation)
}
