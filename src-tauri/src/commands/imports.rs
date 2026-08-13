use crate::{
    import_folder_snapshot_into_database, operation_failed_payload,
    persist_and_ensure_folder_watcher, record_source_error, record_source_error_for_app,
    scan_and_persist_source_snapshot, scanner, spawn_tracked_background,
    validate_existing_image_file, validate_source_folder, AppState, CommandError, CommandResult,
    ImportResult, ReconciliationOutcome,
};
use std::collections::HashSet;
use std::path::Path;
use tauri::{Emitter, Manager};

#[tauri::command]
pub(crate) fn set_wallpaper_folder(
    state: tauri::State<AppState>,
    app: tauri::AppHandle,
    path: String,
) -> CommandResult<ImportResult> {
    let path = validate_source_folder(&path, "SOURCE_INVALID_PATH")?;

    // Start the file watcher BEFORE scanning so filesystem events during
    // the scan are buffered and not lost (MED-07).
    persist_and_ensure_folder_watcher(&app, &path, "mounted")?;

    let result = (|| {
        let images = scanner::scan_folder(&path).map_err(CommandError::from_display)?;
        let db = state.db.lock().map_err(|e| e.to_string())?;
        import_folder_snapshot_into_database(&db, &path, &images, "mounted")
    })();
    match result {
        Ok(ReconciliationOutcome::Applied(summary)) => Ok(summary),
        Ok(ReconciliationOutcome::SkippedStale) => Err(CommandError::new(
            "SOURCE_STALE",
            "The library source changed before its folder scan could be saved.",
        )),
        Err(error) => {
            record_source_error(&state, &path, &error.message);
            Err(error)
        }
    }
}

#[tauri::command]
pub(crate) fn import_wallpaper_folder(
    app: tauri::AppHandle,
    path: String,
) -> CommandResult<ImportResult> {
    let path = validate_source_folder(&path, "SOURCE_INVALID_PATH")?;

    // Fire-and-forget: return immediately so the frontend stays responsive.
    // The actual scan, DB import, and file-watcher setup happen on a
    // background thread. Completion is reported via the `import-complete` event.
    let import_path = path.clone();
    let worker_app = app.clone();
    spawn_tracked_background(&app, move || {
        let app = worker_app;
        let result = persist_and_ensure_folder_watcher(&app, &import_path, "imported-folder")
            .and_then(|_| {
                let state = app.try_state::<AppState>().ok_or_else(|| {
                    CommandError::new("app_still_starting", "PureWall is still starting.")
                })?;
                scan_and_persist_source_snapshot(&state, &import_path, "imported-folder")
            });

        match result {
            Ok(ReconciliationOutcome::Applied(summary)) => {
                let _ = app.emit("import-complete", summary);
            }
            Ok(ReconciliationOutcome::SkippedStale) => {
                let _ = app.emit(
                    "import-complete",
                    ImportResult {
                        scanned: 0,
                        imported: 0,
                    },
                );
            }
            Err(error) => {
                record_source_error_for_app(&app, &import_path, &error.message);
                eprintln!(
                    "[PureWall] Failed to import folder {import_path}: {}",
                    error.message
                );
                let _ = app.emit(
                    "operation-failed",
                    operation_failed_payload("Import failed", error.message),
                );
                let _ = app.emit(
                    "import-complete",
                    ImportResult {
                        scanned: 0,
                        imported: 0,
                    },
                );
            }
        }
    })?;

    Ok(ImportResult {
        scanned: 0,
        imported: 0,
    })
}

#[tauri::command]
pub(crate) fn import_wallpaper_files(
    state: tauri::State<AppState>,
    paths: Vec<String>,
) -> CommandResult<ImportResult> {
    let paths = paths
        .iter()
        .map(|path| validate_existing_image_file(path))
        .collect::<Result<Vec<_>, _>>()?;
    let images = scanner::scan_files(&paths).map_err(|e| e.to_string())?;
    let scanned = images.len();

    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        for img in &images {
            db.upsert_wallpaper(
                &img.path,
                &img.hash,
                "imported-file",
                img.width,
                img.height,
                img.file_size,
            )
            .map_err(|e| e.to_string())?;
        }
    }

    Ok(ImportResult {
        scanned,
        imported: scanned,
    })
}

#[tauri::command]
pub(crate) fn import_dropped_paths(
    state: tauri::State<AppState>,
    paths: Vec<String>,
) -> CommandResult<ImportResult> {
    if paths.is_empty() {
        return Ok(ImportResult {
            scanned: 0,
            imported: 0,
        });
    }

    let mut images = Vec::new();
    let mut file_paths = Vec::new();
    let mut seen_paths = HashSet::new();

    for raw_path in paths {
        let path = Path::new(&raw_path);
        scanner::ensure_local_path(path).map_err(CommandError::from_display)?;
        if path.is_dir() {
            for image in scanner::scan_folder(&raw_path).map_err(CommandError::from_display)? {
                if seen_paths.insert(image.path.clone()) {
                    if images.len() >= scanner::MAX_SCAN_IMAGES {
                        return Err(CommandError::new(
                            "drop_import_limit_exceeded",
                            format!(
                                "Too many images dropped. PureWall can import up to {} images at once.",
                                scanner::MAX_SCAN_IMAGES
                            ),
                        ));
                    }
                    images.push(image);
                }
            }
        } else if path.is_file() {
            file_paths.push(validate_existing_image_file(&raw_path)?);
        } else {
            return Err(CommandError::new(
                "invalid_drop_path",
                format!("Dropped path does not exist: {}", raw_path),
            ));
        }
    }

    for image in scanner::scan_files(&file_paths).map_err(CommandError::from_display)? {
        if seen_paths.insert(image.path.clone()) {
            if images.len() >= scanner::MAX_SCAN_IMAGES {
                return Err(CommandError::new(
                    "drop_import_limit_exceeded",
                    format!(
                        "Too many images dropped. PureWall can import up to {} images at once.",
                        scanner::MAX_SCAN_IMAGES
                    ),
                ));
            }
            images.push(image);
        }
    }

    let scanned = images.len();
    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        for img in &images {
            db.upsert_wallpaper(
                &img.path,
                &img.hash,
                "dropped",
                img.width,
                img.height,
                img.file_size,
            )
            .map_err(CommandError::from_display)?;
        }
    }

    Ok(ImportResult {
        scanned,
        imported: scanned,
    })
}
