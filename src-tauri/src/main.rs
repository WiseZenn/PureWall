// In release builds, hide the console window (prevents flash when invoked from right-click menu)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;
use tauri::{Emitter, Manager};

use playback_action::{dispatch_with, PlaybackAction, PlaybackActionOutcome};
mod active_preview;
#[cfg(test)]
mod active_preview_prewarm_tests;
#[cfg(test)]
mod active_preview_tests;
mod app_updates;
mod autostart;
mod batch_operations;
mod commands;
mod context_menu;
mod db;
mod diagnostics;
mod focus;
mod image_decoder;
#[cfg(test)]
mod image_decoder_tests;
mod library_backup;
#[cfg(test)]
mod library_backup_tests;
#[cfg(test)]
mod library_source_tests;
mod library_sources;
mod media_queue;
#[cfg(test)]
mod media_queue_lane_tests;
#[cfg(test)]
mod media_queue_upgrade_tests;
mod paths;
mod playback_action;
#[cfg(test)]
mod playback_entrypoint_tests;
#[cfg(test)]
mod playback_state_tests;
#[cfg(all(test, windows))]
mod preview_performance_tests;
mod scanner;
mod shell_metadata;
#[cfg(test)]
mod thumbnail_cache_fallback_tests;
#[cfg(test)]
mod thumbnail_derivative_tests;
mod thumbnails;
mod tray;
mod wallpaper;
mod widget;
#[cfg(windows)]
mod windows_image_decoder;

#[cfg(all(test, windows))]
mod windows_image_decoder_tests;

pub struct AppState {
    pub db: Mutex<db::Database>,
    pub folder_watchers: Mutex<HashMap<String, (String, scanner::FolderWatcher)>>,
    pub source_operations: Mutex<HashSet<String>>,
    pub is_paused: Mutex<bool>,
    pub thumb_cache: thumbnails::ThumbnailCache,
    pub media_queue: media_queue::MediaJobQueue,
    pub rotation_secs: AtomicU64,
    pub display_mode: Mutex<String>,
    pub focus_mode: focus::FocusMode,
    pub shutdown_requested: std::sync::atomic::AtomicBool,
    pub background_threads: Mutex<Vec<JoinHandle<()>>>,
    rotation_signal: RotationSignal,
    focus_signal: RotationSignal,
}

struct RotationSignal {
    generation: Mutex<u64>,
    condvar: Condvar,
}

enum RotationWait {
    Signaled(u64),
    TimedOut(u64),
}

impl RotationSignal {
    fn new() -> Self {
        Self {
            generation: Mutex::new(0),
            condvar: Condvar::new(),
        }
    }

    fn notify(&self) {
        if let Ok(mut generation) = self.generation.lock() {
            *generation = generation.wrapping_add(1);
        }
        self.condvar.notify_all();
    }

    fn wait_for_change(&self, last_seen: u64) -> u64 {
        let mut generation = self
            .generation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        while *generation == last_seen {
            generation = self
                .condvar
                .wait(generation)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        *generation
    }

    fn wait_for_change_or_timeout(&self, last_seen: u64, timeout: Duration) -> RotationWait {
        let generation = self
            .generation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *generation != last_seen {
            return RotationWait::Signaled(*generation);
        }

        match self.condvar.wait_timeout(generation, timeout) {
            Ok((generation, wait_result)) => {
                if wait_result.timed_out() && *generation == last_seen {
                    RotationWait::TimedOut(*generation)
                } else {
                    RotationWait::Signaled(*generation)
                }
            }
            Err(poisoned) => {
                let (generation, _) = poisoned.into_inner();
                RotationWait::Signaled(*generation)
            }
        }
    }
}

#[derive(Clone, serde::Serialize)]
struct RatingChangedPayload {
    path: String,
    rating: i32,
}

#[derive(Clone, serde::Serialize)]
pub(crate) struct ImportResult {
    scanned: usize,
    imported: usize,
}

#[derive(Clone, serde::Serialize)]
pub(crate) struct DeleteResult {
    path: String,
    deleted: bool,
    message: Option<String>,
}

#[derive(Clone, serde::Serialize)]
pub(crate) struct ImageMetadata {
    width: u32,
    height: u32,
    file_size: u64,
}

#[derive(Clone, serde::Serialize)]
struct OperationFailedPayload {
    title: String,
    message: String,
}

#[derive(Clone, serde::Serialize)]
struct ThumbnailGeneratedPayload {
    path: String,
    cache_path: String,
}

#[derive(Clone, serde::Serialize)]
struct ThumbnailGenerationFailedPayload {
    path: String,
    message: String,
}

pub(crate) fn emit_thumbnail_generation_failed(
    app: &tauri::AppHandle,
    path: &str,
    message: impl Into<String>,
) {
    let message = message.into();
    eprintln!("[PureWall] Thumbnail failed for {path}: {message}");
    let _ = app.emit(
        "thumbnail-generation-failed",
        ThumbnailGenerationFailedPayload {
            path: path.to_string(),
            message,
        },
    );
}
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct CommandError {
    code: &'static str,
    message: String,
}

pub(crate) type CommandResult<T> = std::result::Result<T, CommandError>;

impl CommandError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    fn from_display(error: impl std::fmt::Display) -> Self {
        Self::new("operation_failed", error.to_string())
    }
}

pub(crate) fn backup_command_error(error: library_backup::BackupError) -> CommandError {
    let code = error.code();
    CommandError::new(code, error.to_string())
}

impl From<String> for CommandError {
    fn from(message: String) -> Self {
        Self::new("operation_failed", message)
    }
}

impl From<&str> for CommandError {
    fn from(message: &str) -> Self {
        Self::new("operation_failed", message)
    }
}

pub(crate) const SETTING_ROTATION_SECS: &str = "rotation_secs";
pub(crate) const SETTING_DISPLAY_MODE: &str = "display_mode";
pub(crate) const SETTING_FOCUS_MODE: &str = "focus_mode_enabled";
const SETTING_PAUSED: &str = "paused";
const SETTING_CURRENT_WALLPAPER: &str = "current_wallpaper";
const DEFAULT_ROTATION_SECS: u64 = 600;
pub(crate) const MAX_ROTATION_SECS: u64 = 86_400;
fn validate_existing_folder(path: &str) -> Result<String, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("Folder path cannot be empty".to_string());
    }

    let folder = Path::new(path);
    scanner::ensure_local_path(folder).map_err(|e| e.to_string())?;
    if !folder.is_dir() {
        return Err(format!(
            "Folder does not exist or is not a directory: {}",
            path
        ));
    }

    // Canonicalize to resolve `..`, symlinks, and other indirections (HIG-06).
    folder
        .canonicalize()
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| format!("Failed to resolve folder path: {}", e))
}

pub(crate) fn validate_existing_image_file(path: &str) -> Result<String, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("Wallpaper path cannot be empty".to_string());
    }

    let file = Path::new(path);
    scanner::ensure_local_path(file).map_err(|e| e.to_string())?;
    if !file.is_file() {
        return Err(format!("Wallpaper file does not exist: {}", path));
    }

    if !scanner::is_supported_image(file) {
        return Err(format!("Unsupported wallpaper file type: {}", path));
    }

    Ok(path.to_string())
}

pub(crate) fn require_registered_wallpaper_file(
    db: &db::Database,
    path: &str,
) -> Result<String, String> {
    let path = validate_existing_image_file(path)?;
    if db
        .get_wallpaper_by_path(&path)
        .map_err(|e| e.to_string())?
        .is_none()
    {
        return Err(format!("Wallpaper is not registered: {}", path));
    }
    Ok(path)
}

pub(crate) fn require_registered_wallpaper_files(
    db: &db::Database,
    paths: &[String],
) -> Result<Vec<String>, String> {
    paths
        .iter()
        .map(|path| require_registered_wallpaper_file(db, path))
        .collect()
}

pub(crate) fn require_batch_wallpaper_files(
    db: &db::Database,
    paths: &[String],
) -> CommandResult<Vec<String>> {
    let paths = batch_operations::normalize_batch_paths(paths)?;
    Ok(require_registered_wallpaper_files(db, &paths)?)
}

pub(crate) fn require_existing_tag(db: &db::Database, tag_id: i64) -> CommandResult<i64> {
    let tag_id = batch_operations::validate_relation_id(tag_id, "tag")?;
    if !db.tag_exists(tag_id).map_err(CommandError::from_display)? {
        return Err(CommandError::new(
            "tag_not_found",
            "The selected tag no longer exists.",
        ));
    }
    Ok(tag_id)
}

pub(crate) fn require_existing_collection(
    db: &db::Database,
    collection_id: i64,
) -> CommandResult<i64> {
    let collection_id = batch_operations::validate_relation_id(collection_id, "collection")?;
    if !db
        .collection_exists(collection_id)
        .map_err(CommandError::from_display)?
    {
        return Err(CommandError::new(
            "collection_not_found",
            "The selected collection no longer exists.",
        ));
    }
    Ok(collection_id)
}

#[tauri::command]
fn get_wallpapers(state: tauri::State<AppState>) -> CommandResult<Vec<db::WallpaperEntry>> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_all_wallpapers().map_err(CommandError::from_display)
}

#[tauri::command]
fn get_wallpapers_filtered(
    state: tauri::State<AppState>,
    filter: String,
    sort: String,
) -> CommandResult<Vec<db::WallpaperEntry>> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_wallpapers_filtered(&filter, &sort)
        .map_err(CommandError::from_display)
}

#[tauri::command]
fn get_wallpapers_page(
    state: tauri::State<AppState>,
    filter: String,
    sort: String,
    search: String,
    offset: i64,
    limit: i64,
) -> CommandResult<db::WallpaperPage> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_wallpapers_page(&filter, &sort, &search, offset, limit)
        .map_err(CommandError::from_display)
}
#[tauri::command]
fn get_wallpaper_by_path(
    state: tauri::State<AppState>,
    path: String,
) -> CommandResult<db::WallpaperEntry> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let path = require_registered_wallpaper_file(&db, &path)?;
    db.get_wallpaper_by_path(&path)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Wallpaper not found".to_string())
        .map_err(Into::into)
}

#[tauri::command]
fn get_wallpapers_by_filter(
    state: tauri::State<AppState>,
    filter: String,
) -> CommandResult<Vec<db::WallpaperEntry>> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    match filter.as_str() {
        "liked" => db
            .get_liked_wallpapers()
            .map_err(CommandError::from_display),
        "disliked" => db
            .get_disliked_wallpapers()
            .map_err(CommandError::from_display),
        _ => db.get_all_wallpapers().map_err(CommandError::from_display),
    }
}

fn import_images_into_database(
    db: &db::Database,
    images: &[scanner::ImageInfo],
    source: &str,
) -> CommandResult<ImportResult> {
    let mut imported = 0;
    for img in images {
        if !Path::new(&img.path).is_file() {
            continue;
        }
        db.upsert_wallpaper(
            &img.path,
            &img.hash,
            source,
            img.width,
            img.height,
            img.file_size,
        )
        .map_err(CommandError::from_display)?;
        imported += 1;
    }

    Ok(ImportResult {
        scanned: images.len(),
        imported,
    })
}

pub(crate) fn import_folder_snapshot_into_database(
    db: &db::Database,
    folder_path: &str,
    images: &[scanner::ImageInfo],
    source: &str,
) -> CommandResult<ImportResult> {
    let existing_paths = images
        .iter()
        .map(|image| image.path.clone())
        .collect::<Vec<_>>();
    db.reconcile_watched_folder_availability(folder_path, &existing_paths)
        .map_err(CommandError::from_display)?;
    let result = import_images_into_database(db, images, source)?;
    db.record_watched_folder_scan_success(folder_path)
        .map_err(CommandError::from_display)?;
    Ok(result)
}

pub(crate) fn record_source_error(state: &AppState, path: &str, message: &str) {
    match state.db.lock() {
        Ok(db) => {
            if let Err(error) = db.record_watched_folder_error(path, message) {
                eprintln!("[PureWall] Failed to persist source error for {path}: {error}");
            }
        }
        Err(_) => {
            eprintln!(
                "[PureWall] Could not access the database to persist source error for {path}"
            );
        }
    }
}

pub(crate) fn record_source_error_for_app(app: &tauri::AppHandle, path: &str, message: &str) {
    if let Some(state) = app.try_state::<AppState>() {
        record_source_error(&state, path, message);
    }
}

struct WatchedPathInspection {
    images: Vec<scanner::ImageInfo>,
    missing_paths: Vec<String>,
}

fn inspect_watched_paths(paths: &[PathBuf]) -> CommandResult<WatchedPathInspection> {
    let mut images = Vec::new();
    let mut missing_paths = Vec::new();
    let mut seen_images = HashSet::new();
    let mut seen_missing = HashSet::new();

    for path in paths {
        if path.is_dir() {
            let folder = path.to_string_lossy();
            for image in scanner::scan_folder(&folder).map_err(CommandError::from_display)? {
                if seen_images.insert(image.path.clone()) {
                    images.push(image);
                }
            }
        } else if let Some(image) = scanner::image_info(&path.to_string_lossy()) {
            if seen_images.insert(image.path.clone()) {
                images.push(image);
            }
        } else if !path.exists() {
            let missing_path = path.to_string_lossy().to_string();
            if seen_missing.insert(missing_path.clone()) {
                missing_paths.push(missing_path);
            }
        }

        if images.len() > scanner::MAX_SCAN_IMAGES {
            return Err(CommandError::new(
                "watcher_sync_limit_exceeded",
                format!(
                    "A folder update contained more than {} images. Refresh the library explicitly.",
                    scanner::MAX_SCAN_IMAGES
                ),
            ));
        }
    }

    images.sort_by(|left, right| left.path.cmp(&right.path));
    missing_paths.sort();
    Ok(WatchedPathInspection {
        images,
        missing_paths,
    })
}

fn reconcile_watched_paths<F>(paths: &[PathBuf], persist: F) -> CommandResult<ImportResult>
where
    F: FnOnce(&WatchedPathInspection) -> CommandResult<ImportResult>,
{
    let inspection = inspect_watched_paths(paths)?;
    persist(&inspection)
}

fn synchronize_watched_paths(
    app: &tauri::AppHandle,
    source_root: &str,
    paths: &[PathBuf],
    source: &str,
) {
    if app_shutdown_requested(app) {
        return;
    }
    if app.try_state::<AppState>().is_some_and(|state| {
        state
            .source_operations
            .lock()
            .map(|operations| operations.contains(source_root))
            .unwrap_or(false)
    }) {
        return;
    }
    let result = reconcile_watched_paths(paths, |inspection| {
        let state = app.try_state::<AppState>().ok_or_else(|| {
            CommandError::new(
                "app_still_starting",
                "PureWall is still starting. The folder update will be picked up on the next change.",
            )
        })?;
        let db = state.db.lock().map_err(|_| {
            CommandError::new(
                "database_unavailable",
                "Could not access the wallpaper database during folder synchronization.",
            )
        })?;
        let registrations = db
            .get_watched_folders()
            .map_err(CommandError::from_display)?;
        if !watched_folder_registration_is_current(&registrations, source_root, source) {
            return Ok(ImportResult {
                scanned: inspection.images.len(),
                imported: 0,
            });
        }
        db.mark_paths_unavailable(&inspection.missing_paths)
            .map_err(CommandError::from_display)?;
        import_images_into_database(&db, &inspection.images, source)
    });

    match result {
        Ok(summary) => {
            if let Err(error) = app.emit("folder-changed", summary) {
                eprintln!("[PureWall] Failed to emit synchronized folder update: {error}");
            }
        }
        Err(error) => {
            eprintln!(
                "[PureWall] Failed to synchronize watched folder paths: {}",
                error.message
            );
            record_source_error_for_app(app, source_root, &error.message);
            let _ = app.emit(
                "operation-failed",
                OperationFailedPayload {
                    title: "Folder update failed".to_string(),
                    message: error.message,
                },
            );
        }
    }
}

fn start_folder_watcher(
    app: tauri::AppHandle,
    folder_path: String,
    source: &str,
) -> anyhow::Result<scanner::FolderWatcher> {
    let callback_app = app.clone();
    let source = source.to_string();
    let source_root = folder_path.clone();
    scanner::start_watcher(folder_path, move |paths| {
        synchronize_watched_paths(&callback_app, &source_root, &paths, &source);
    })
}

#[cfg(test)]
fn ensure_watcher_registered<T, E, F>(
    watchers: &mut HashMap<String, (String, T)>,
    folder_path: &str,
    source: &str,
    start: F,
) -> std::result::Result<bool, E>
where
    F: FnOnce() -> std::result::Result<T, E>,
{
    if watchers
        .get(folder_path)
        .is_some_and(|(registered_source, _)| registered_source == source)
    {
        return Ok(false);
    }

    let watcher = start()?;
    watchers.insert(folder_path.to_string(), (source.to_string(), watcher));
    Ok(true)
}

pub(crate) fn take_runtime_folder_watcher<T>(
    watchers: &mut HashMap<String, (String, T)>,
    folder_path: &str,
) -> Option<(String, T)> {
    watchers.remove(folder_path)
}

pub(crate) fn ensure_runtime_folder_watcher(
    app: &tauri::AppHandle,
    folder_path: &str,
    source: &str,
) -> CommandResult<bool> {
    let state = app.try_state::<AppState>().ok_or_else(|| {
        CommandError::new(
            "app_still_starting",
            "PureWall is still starting and cannot register the folder watcher yet.",
        )
    })?;
    let source_is_current = {
        let watchers = state.folder_watchers.lock().map_err(|_| {
            CommandError::new(
                "watcher_registry_unavailable",
                "Could not access the folder watcher registry.",
            )
        })?;
        watchers
            .get(folder_path)
            .is_some_and(|(registered_source, _)| registered_source == source)
    };
    if source_is_current {
        return Ok(false);
    }

    let mut new_watcher = Some(
        start_folder_watcher(app.clone(), folder_path.to_string(), source)
            .map_err(CommandError::from_display)?,
    );
    let replaced_watcher = {
        let mut watchers = state.folder_watchers.lock().map_err(|_| {
            CommandError::new(
                "watcher_registry_unavailable",
                "Could not access the folder watcher registry.",
            )
        })?;
        if watchers
            .get(folder_path)
            .is_some_and(|(registered_source, _)| registered_source == source)
        {
            None
        } else {
            Some(
                watchers.insert(
                    folder_path.to_string(),
                    (
                        source.to_string(),
                        new_watcher
                            .take()
                            .expect("new watcher must be available for insertion"),
                    ),
                ),
            )
        }
    };

    match replaced_watcher {
        Some(replaced) => {
            drop(replaced);
            Ok(true)
        }
        None => {
            drop(new_watcher);
            Ok(false)
        }
    }
}

pub(crate) fn persist_and_ensure_folder_watcher(
    app: &tauri::AppHandle,
    folder_path: &str,
    source: &str,
) -> CommandResult<bool> {
    let state = app
        .try_state::<AppState>()
        .ok_or_else(|| CommandError::new("app_still_starting", "PureWall is still starting."))?;
    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.upsert_watched_folder(folder_path, source)
            .map_err(CommandError::from_display)?;
    }

    match ensure_runtime_folder_watcher(app, folder_path, source) {
        Ok(started) => Ok(started),
        Err(error) => {
            let error = source_path_error(folder_path, error.message, "SOURCE_OFFLINE");
            record_source_error(&state, folder_path, &error.message);
            Err(error)
        }
    }
}

fn report_folder_sync_failure(app: &tauri::AppHandle, message: String) {
    eprintln!("[PureWall] Folder synchronization failed: {message}");
    let _ = app.emit(
        "operation-failed",
        OperationFailedPayload {
            title: "Folder synchronization failed".to_string(),
            message,
        },
    );
}

fn register_restored_folder_watcher(
    app: &tauri::AppHandle,
    folder: &db::WatchedFolderEntry,
) -> bool {
    match ensure_runtime_folder_watcher(app, &folder.path, &folder.source) {
        Ok(_) => true,
        Err(error) => {
            if !Path::new(&folder.path).is_dir() {
                if let Some(state) = app.try_state::<AppState>() {
                    if let Ok(db) = state.db.lock() {
                        let _ = db.mark_paths_unavailable(std::slice::from_ref(&folder.path));
                    }
                }
            }
            eprintln!(
                "[PureWall] Could not restore watcher for {}: {}",
                folder.path, error.message
            );
            record_source_error_for_app(app, &folder.path, &error.message);
            false
        }
    }
}

fn synchronize_persisted_folder(
    app: &tauri::AppHandle,
    folder: &db::WatchedFolderEntry,
) -> CommandResult<ImportResult> {
    let result = (|| {
        let images = scanner::scan_folder(&folder.path).map_err(CommandError::from_display)?;
        let state = app.try_state::<AppState>().ok_or_else(|| {
            CommandError::new("app_still_starting", "PureWall is still starting.")
        })?;
        let db = state.db.lock().map_err(|e| e.to_string())?;
        import_folder_snapshot_into_database(&db, &folder.path, &images, &folder.source)
    })();
    if let Err(error) = &result {
        record_source_error_for_app(app, &folder.path, &error.message);
    }
    result
}

fn app_shutdown_requested(app: &tauri::AppHandle) -> bool {
    app.try_state::<AppState>()
        .is_some_and(|state| state.shutdown_requested.load(Ordering::Relaxed))
}

pub(crate) fn spawn_tracked_background<F>(app: &tauri::AppHandle, worker: F) -> CommandResult<()>
where
    F: FnOnce() + Send + 'static,
{
    let state = app
        .try_state::<AppState>()
        .ok_or_else(|| CommandError::new("app_still_starting", "PureWall is still starting."))?;
    let mut handles = state.background_threads.lock().map_err(|_| {
        CommandError::new(
            "background_threads_unavailable",
            "Could not access the background worker registry.",
        )
    })?;
    if state.shutdown_requested.load(Ordering::Relaxed) {
        return Err(CommandError::new(
            "app_shutting_down",
            "PureWall is shutting down and cannot start new background work.",
        ));
    }
    handles.push(std::thread::spawn(worker));
    Ok(())
}

fn synchronize_persisted_folders(app: tauri::AppHandle, folders: Vec<db::WatchedFolderEntry>) {
    let mut summary = ImportResult {
        scanned: 0,
        imported: 0,
    };
    let mut synchronized_any = false;
    for folder in folders {
        if app_shutdown_requested(&app) {
            break;
        }
        match synchronize_persisted_folder(&app, &folder) {
            Ok(result) => {
                summary.scanned += result.scanned;
                summary.imported += result.imported;
                synchronized_any = true;
            }
            Err(error) => report_folder_sync_failure(&app, error.message),
        }
    }
    if synchronized_any && !app_shutdown_requested(&app) {
        let _ = app.emit("folder-changed", summary);
    }
}

fn restore_persisted_folder_watchers(app: &tauri::AppHandle, folders: Vec<db::WatchedFolderEntry>) {
    let active_folders = folders
        .into_iter()
        .filter(|folder| register_restored_folder_watcher(app, folder))
        .collect::<Vec<_>>();
    if !active_folders.is_empty() {
        let worker_app = app.clone();
        if let Err(error) = spawn_tracked_background(app, move || {
            synchronize_persisted_folders(worker_app, active_folders)
        }) {
            eprintln!(
                "[PureWall] Could not register startup folder synchronization: {}",
                error.message
            );
        }
    }
}

fn watched_folder_registration_is_current(
    folders: &[db::WatchedFolderEntry],
    root: &str,
    source: &str,
) -> bool {
    let root_key = paths::path_identity_key(Path::new(root));
    folders.iter().any(|entry| {
        paths::path_identity_key(Path::new(&entry.path)) == root_key && entry.source == source
    })
}

pub(crate) fn library_source_entry(
    state: &AppState,
    path: &str,
) -> CommandResult<db::WatchedFolderEntry> {
    let requested_key = paths::path_identity_key(Path::new(path));
    let db = state.db.lock().map_err(|_| {
        CommandError::new(
            "database_unavailable",
            "Could not access the wallpaper database.",
        )
    })?;
    db.get_watched_folders()
        .map_err(CommandError::from_display)?
        .into_iter()
        .find(|entry| paths::path_identity_key(Path::new(&entry.path)) == requested_key)
        .ok_or_else(|| {
            CommandError::new(
                "SOURCE_INVALID_PATH",
                "The selected library source is not registered.",
            )
        })
}

pub(crate) fn source_path_error(
    path: &str,
    message: impl Into<String>,
    missing_code: &'static str,
) -> CommandError {
    let message = message.into();
    let lower = message.to_ascii_lowercase();
    if lower.contains("permission")
        || lower.contains("access is denied")
        || lower.contains("os error 5")
    {
        CommandError::new("SOURCE_PERMISSION_DENIED", message)
    } else if !Path::new(path).is_dir()
        || lower.contains("does not exist")
        || lower.contains("not a directory")
    {
        CommandError::new(missing_code, message)
    } else {
        CommandError::new("SOURCE_INVALID_PATH", message)
    }
}

pub(crate) fn source_database_error(error: impl std::fmt::Display) -> CommandError {
    let message = error.to_string();
    let lower = message.to_ascii_lowercase();
    if lower.contains("relocation collision") {
        CommandError::new("SOURCE_RELOCATE_COLLISION", message)
    } else if lower.contains("source path conflict") {
        CommandError::new("SOURCE_PATH_CONFLICT", message)
    } else if lower.contains("unknown watched folder") {
        CommandError::new("SOURCE_INVALID_PATH", message)
    } else {
        CommandError::from_display(message)
    }
}

pub(crate) fn validate_source_folder(
    path: &str,
    missing_code: &'static str,
) -> CommandResult<String> {
    validate_existing_folder(path).map_err(|message| source_path_error(path, message, missing_code))
}

pub(crate) fn begin_source_operation<'a>(
    state: &'a AppState,
    path: &str,
) -> CommandResult<library_sources::SourceOperationGuard<'a>> {
    library_sources::SourceOperationGuard::begin(&state.source_operations, path)
        .map_err(|message| CommandError::new("SOURCE_OPERATION_BUSY", message))
}

pub(crate) fn scan_and_persist_source_snapshot(
    state: &AppState,
    path: &str,
    source: &str,
) -> CommandResult<ImportResult> {
    let images = scanner::scan_folder(path)
        .map_err(|error| source_path_error(path, error.to_string(), "SOURCE_OFFLINE"))?;
    let db = state.db.lock().map_err(|_| {
        CommandError::new(
            "database_unavailable",
            "Could not access the wallpaper database.",
        )
    })?;
    import_folder_snapshot_into_database(&db, path, &images, source)
}

#[tauri::command]
fn get_image_metadata(state: tauri::State<AppState>, path: String) -> CommandResult<ImageMetadata> {
    let path = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        require_registered_wallpaper_file(&db, &path)?
    };

    if let Some(info) = scanner::image_info(&path) {
        let metadata = ImageMetadata {
            width: info.width,
            height: info.height,
            file_size: info.file_size,
        };
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.update_wallpaper_metadata(&path, metadata.width, metadata.height, metadata.file_size)
            .map_err(|e| e.to_string())?;
        return Ok(metadata);
    }

    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_wallpaper_metadata(&path)
        .map_err(|e| e.to_string())?
        .filter(|(width, height, file_size)| *width > 0 || *height > 0 || *file_size > 0)
        .map(|(width, height, file_size)| ImageMetadata {
            width,
            height,
            file_size,
        })
        .ok_or_else(|| format!("Failed to read image metadata: {path}"))
        .map_err(Into::into)
}

#[tauri::command]
fn get_shell_metadata(path: String) -> CommandResult<shell_metadata::ShellMetadata> {
    let path = validate_existing_image_file(&path)?;
    shell_metadata::read(&path).map_err(CommandError::from_display)
}

fn legacy_pause_file() -> std::path::PathBuf {
    paths::app_data_dir().join("paused.txt")
}

/// Parse a boolean setting from the settings table.
/// Accepts `"true"`, `"1"`, and the legacy `"paused"` value
/// from the old pause-file migration path.
fn parse_bool_setting(value: Option<String>) -> bool {
    matches!(value.as_deref(), Some("true" | "1" | "paused"))
}

fn read_manual_paused(db: &db::Database) -> Result<bool, String> {
    db.get_setting(SETTING_PAUSED)
        .map(parse_bool_setting)
        .map_err(|e| e.to_string())
}

fn write_manual_paused(db: &db::Database, paused: bool) -> Result<(), String> {
    db.set_setting(SETTING_PAUSED, if paused { "true" } else { "false" })
        .map_err(|e| e.to_string())
}

pub(crate) fn sync_pause_state_from_settings(
    state: &AppState,
    app: Option<&tauri::AppHandle>,
) -> Result<bool, String> {
    let manual_paused = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        read_manual_paused(&db)?
    };
    let effective_paused = manual_paused || state.focus_mode.auto_paused();
    let changed = {
        let mut paused = state.is_paused.lock().map_err(|e| e.to_string())?;
        let changed = *paused != effective_paused;
        *paused = effective_paused;
        changed
    };

    if changed {
        state.rotation_signal.notify();
        if let Some(app) = app {
            let _ = app.emit("pause-changed", effective_paused);
        }
    }

    Ok(effective_paused)
}

fn set_manual_pause_state(
    state: &AppState,
    app: &tauri::AppHandle,
    manual_paused: bool,
) -> Result<bool, String> {
    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        write_manual_paused(&db, manual_paused)?;
    }

    if !manual_paused {
        if state.focus_mode.auto_paused() && state.focus_mode.fullscreen_detected() {
            state.focus_mode.set_auto_pause_suppressed(true);
        }
        state.focus_mode.set_auto_paused(false);
    } else {
        state.focus_mode.set_auto_pause_suppressed(false);
    }

    let effective_paused = manual_paused || state.focus_mode.auto_paused();
    {
        let mut paused = state.is_paused.lock().map_err(|e| e.to_string())?;
        *paused = effective_paused;
    }

    state.rotation_signal.notify();
    emit_focus_mode_status(app, state);
    Ok(effective_paused)
}

fn current_wallpaper_file() -> std::path::PathBuf {
    paths::app_data_dir().join("current_wallpaper.txt")
}

fn write_current_wallpaper_file(path: &str) -> Result<(), String> {
    let file = current_wallpaper_file();
    let temp_file = file.with_extension("txt.tmp");
    std::fs::write(&temp_file, path).map_err(|e| e.to_string())?;
    if std::fs::rename(&temp_file, &file).is_err() {
        let _ = std::fs::remove_file(&file);
        std::fs::rename(&temp_file, &file).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub(crate) fn persist_current_wallpaper(db: &db::Database, path: &str) -> Result<(), String> {
    db.set_setting(SETTING_CURRENT_WALLPAPER, path)
        .map_err(|e| e.to_string())?;
    if let Err(e) = write_current_wallpaper_file(path) {
        eprintln!(
            "Failed to update current wallpaper compatibility file: {}",
            e
        );
    }
    Ok(())
}

fn read_current_wallpaper_file() -> Result<String, String> {
    std::fs::read_to_string(current_wallpaper_file())
        .map(|s| s.trim().to_string())
        .map_err(|e| format!("No current wallpaper path: {}", e))
}

pub(crate) fn read_current_wallpaper_path(db: &db::Database) -> Result<String, String> {
    if let Some(path) = db
        .get_setting(SETTING_CURRENT_WALLPAPER)
        .map_err(|e| e.to_string())?
        .filter(|path| !path.trim().is_empty())
    {
        return Ok(path.trim().to_string());
    }

    let path = read_current_wallpaper_file()?;
    if !path.is_empty() {
        persist_current_wallpaper(db, &path)?;
        return Ok(path);
    }

    Err("No current wallpaper path".to_string())
}

pub(crate) fn is_valid_display_mode(mode: &str) -> bool {
    matches!(mode, "all" | "span" | "independent")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RestoredPlaybackState {
    pub current_wallpaper_path: Option<String>,
    pub manual_paused: bool,
    pub rotation_secs: u64,
    pub display_mode: String,
}

pub(crate) fn load_restored_playback_state(
    db: &db::Database,
) -> Result<RestoredPlaybackState, String> {
    let current_wallpaper_path = db
        .get_setting(SETTING_CURRENT_WALLPAPER)
        .map_err(|error| error.to_string())?
        .and_then(|path| {
            let path = path.trim();
            (!path.is_empty()).then(|| path.to_string())
        });
    let manual_paused = parse_bool_setting(
        db.get_setting(SETTING_PAUSED)
            .map_err(|error| error.to_string())?,
    );
    let rotation_secs = db
        .get_setting(SETTING_ROTATION_SECS)
        .map_err(|error| error.to_string())?
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_ROTATION_SECS)
        .min(MAX_ROTATION_SECS);
    let display_mode = db
        .get_setting(SETTING_DISPLAY_MODE)
        .map_err(|error| error.to_string())?
        .filter(|mode| is_valid_display_mode(mode))
        .unwrap_or_else(|| "all".to_string());

    Ok(RestoredPlaybackState {
        current_wallpaper_path,
        manual_paused,
        rotation_secs,
        display_mode,
    })
}

pub(crate) fn current_display_mode(state: &AppState) -> String {
    state
        .display_mode
        .lock()
        .map(|mode| {
            if is_valid_display_mode(&mode) {
                mode.clone()
            } else {
                "all".to_string()
            }
        })
        .unwrap_or_else(|_| "all".to_string())
}

pub(crate) fn advance_shared_wallpaper_with_db(
    db: &db::Database,
    placement: wallpaper::WallpaperPlacement,
) -> Result<String, String> {
    let path = db.get_next_wallpaper().map_err(|e| e.to_string())?;
    wallpaper::set_wallpaper_all(&path, placement).map_err(|e| e.to_string())?;
    db.record_play(&path).map_err(|e| e.to_string())?;
    persist_current_wallpaper(db, &path)?;
    Ok(path)
}

pub(crate) fn apply_independent_display_assignments_with<Apply>(
    db: &db::Database,
    assignments: &[(String, String)],
    mut apply: Apply,
) -> Result<String, String>
where
    Apply: FnMut(&str, &str) -> Result<(), String>,
{
    let primary_path = assignments
        .first()
        .map(|(_, path)| path.clone())
        .ok_or_else(|| "No wallpapers available".to_string())?;

    for (index, (display_id, path)) in assignments.iter().enumerate() {
        apply(display_id, path)?;
        db.record_play_for_display(path, Some(display_id))
            .map_err(|error| error.to_string())?;
        if index == 0 {
            persist_current_wallpaper(db, path)?;
        }
    }

    Ok(primary_path)
}

pub(crate) fn advance_independent_wallpapers_with_db(db: &db::Database) -> Result<String, String> {
    let displays = wallpaper::get_displays().map_err(|e| e.to_string())?;
    if displays.is_empty() {
        return advance_shared_wallpaper_with_db(db, wallpaper::WallpaperPlacement::Fill);
    }

    let paths = db
        .get_next_wallpapers(displays.len())
        .map_err(|e| e.to_string())?;
    let assignments = displays
        .iter()
        .zip(paths)
        .map(|(display, path)| (display.id.clone(), path))
        .collect::<Vec<_>>();

    apply_independent_display_assignments_with(db, &assignments, |display_id, path| {
        wallpaper::set_wallpaper_on_monitor(display_id, path).map_err(|error| error.to_string())
    })
}

pub(crate) fn advance_wallpaper(state: &AppState) -> Result<String, String> {
    let path = match current_display_mode(state).as_str() {
        "span" => advance_shared_wallpaper(state, wallpaper::WallpaperPlacement::Span),
        "independent" => advance_independent_wallpapers(state),
        _ => advance_shared_wallpaper(state, wallpaper::WallpaperPlacement::Fill),
    }?;
    active_preview::prewarm_active_preview(&path, &state.thumb_cache, &state.media_queue);
    Ok(path)
}

pub(crate) fn execute_playback_action(
    state: &AppState,
    app: &tauri::AppHandle,
    action: PlaybackAction,
) -> Result<PlaybackActionOutcome, String> {
    execute_playback_action_for_caller(state, app, action, None)
}

pub(crate) fn finish_playback_action_with_best_effort_completion<Commit, Emit, Observe>(
    commit: Commit,
    emit: Emit,
    observe_emit_failure: Observe,
) -> Result<PlaybackActionOutcome, String>
where
    Commit: FnOnce() -> Result<PlaybackActionOutcome, String>,
    Emit: FnOnce(&PlaybackActionOutcome) -> Result<(), String>,
    Observe: FnOnce(&str),
{
    let outcome = commit()?;
    if let Err(error) = emit(&outcome) {
        observe_emit_failure(&error);
    }
    Ok(outcome)
}

pub(crate) fn execute_playback_action_for_caller(
    state: &AppState,
    app: &tauri::AppHandle,
    action: PlaybackAction,
    excluded_caller_label: Option<&str>,
) -> Result<PlaybackActionOutcome, String> {
    finish_playback_action_with_best_effort_completion(
        || {
            dispatch_with(
                action,
                || advance_wallpaper(state),
                |rating| {
                    let path = set_current_wallpaper_rating(state, rating)?;
                    Ok((path, rating))
                },
                || {
                    let currently_paused = sync_pause_state_from_settings(state, None)?;
                    set_manual_pause_state(state, app, !currently_paused)
                },
            )
        },
        |outcome| emit_playback_action_outcome(app, outcome, excluded_caller_label),
        |error| {
            eprintln!(
                "[PureWall] {} completion emit failed after commit: {error}",
                action.label()
            );
        },
    )
}

pub(crate) fn should_deliver_playback_completion(
    target: &tauri::EventTarget,
    excluded_caller_label: Option<&str>,
) -> bool {
    let Some(excluded_label) = excluded_caller_label else {
        return true;
    };

    match target {
        tauri::EventTarget::AnyLabel { label }
        | tauri::EventTarget::Window { label }
        | tauri::EventTarget::Webview { label }
        | tauri::EventTarget::WebviewWindow { label } => label != excluded_label,
        tauri::EventTarget::Any | tauri::EventTarget::App => true,
        _ => true,
    }
}

fn emit_playback_completion<S: serde::Serialize + Clone>(
    app: &tauri::AppHandle,
    event: &str,
    payload: S,
    excluded_caller_label: Option<&str>,
) -> Result<(), String> {
    match excluded_caller_label {
        Some(label) => app.emit_filter(event, payload, |target| {
            should_deliver_playback_completion(target, Some(label))
        }),
        None => app.emit(event, payload),
    }
    .map_err(|error| error.to_string())
}

fn emit_playback_action_outcome(
    app: &tauri::AppHandle,
    outcome: &PlaybackActionOutcome,
    excluded_caller_label: Option<&str>,
) -> Result<(), String> {
    let event = playback_completion_events(outcome.action, true)[0];
    match outcome.action {
        PlaybackAction::Next => {
            let path = outcome.current_wallpaper_path.clone().ok_or_else(|| {
                "Committed Next outcome did not contain a wallpaper path".to_string()
            })?;
            emit_playback_completion(app, event, path, excluded_caller_label)
        }
        PlaybackAction::Like | PlaybackAction::Dislike => {
            let path = outcome.current_wallpaper_path.clone().ok_or_else(|| {
                "Committed rating outcome did not contain a target path".to_string()
            })?;
            let rating = outcome
                .rating
                .ok_or_else(|| "Committed rating outcome did not contain a rating".to_string())?;
            emit_playback_completion(
                app,
                event,
                RatingChangedPayload { path, rating },
                excluded_caller_label,
            )
        }
        PlaybackAction::TogglePause => {
            let paused = outcome.paused.ok_or_else(|| {
                "Committed pause outcome did not contain a pause state".to_string()
            })?;
            emit_playback_completion(app, event, paused, excluded_caller_label)
        }
    }
}

pub(crate) fn playback_error_code(message: &str) -> &'static str {
    let unavailable = [
        "No wallpapers available",
        "No existing wallpaper files found",
        "No current wallpaper path",
        "Wallpaper path cannot be empty",
        "Wallpaper file does not exist:",
        "Unsupported wallpaper file type:",
        "Wallpaper is not registered:",
    ];
    if unavailable.iter().any(|prefix| message.starts_with(prefix)) {
        "playback_unavailable"
    } else {
        "operation_failed"
    }
}

pub(crate) fn playback_command_error(message: String) -> CommandError {
    CommandError::new(playback_error_code(&message), message)
}

pub(crate) fn legacy_wallpaper_path(outcome: PlaybackActionOutcome) -> CommandResult<String> {
    outcome.current_wallpaper_path.ok_or_else(|| {
        playback_command_error(format!(
            "{} action did not return a wallpaper path",
            outcome.action.label()
        ))
    })
}

pub(crate) fn legacy_pause_state(outcome: PlaybackActionOutcome) -> CommandResult<bool> {
    outcome.paused.ok_or_else(|| {
        playback_command_error(format!(
            "{} action did not return a pause state",
            outcome.action.label()
        ))
    })
}

pub(crate) fn playback_completion_events(
    action: PlaybackAction,
    succeeded: bool,
) -> [&'static str; 1] {
    let event = if succeeded {
        match action {
            PlaybackAction::Next => "auto-rotated",
            PlaybackAction::Like | PlaybackAction::Dislike => "wallpaper-rating-changed",
            PlaybackAction::TogglePause => "pause-changed",
        }
    } else {
        "operation-failed"
    };
    [event]
}

pub(crate) fn report_playback_failure(
    app: &tauri::AppHandle,
    source: &str,
    action: PlaybackAction,
    message: impl Into<String>,
) {
    let message = message.into();
    eprintln!("[PureWall] {source} {} failed: {message}", action.label());
    let _ = app.emit(
        playback_completion_events(action, false)[0],
        OperationFailedPayload {
            title: format!("{source}: {} failed", action.label()),
            message,
        },
    );
}

fn advance_shared_wallpaper(
    state: &AppState,
    placement: wallpaper::WallpaperPlacement,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    advance_shared_wallpaper_with_db(&db, placement)
}

fn advance_independent_wallpapers(state: &AppState) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    advance_independent_wallpapers_with_db(&db)
}

pub(crate) fn save_display_title_with_writer<F>(
    database: &Mutex<db::Database>,
    path: &str,
    title: &str,
    write_title: F,
) -> CommandResult<db::WallpaperEntry>
where
    F: FnOnce(&str, &str) -> Result<(), String>,
{
    write_title(path, title).map_err(|error| {
        CommandError::new(
            "file_metadata_write_failed",
            format!("Failed to write title to file: {error}"),
        )
    })?;

    let db = database.lock().map_err(|e| e.to_string())?;
    db.set_wallpaper_display_title(path, title)
        .map_err(CommandError::from_display)?;
    db.get_wallpaper_by_path(path)
        .map_err(CommandError::from_display)?
        .ok_or_else(|| CommandError::new("not_found", "Wallpaper not found"))
}

const GENERAL_MEDIA_WORKER_COUNT: usize = 2;

#[derive(Clone, Copy)]
enum MediaWorkerLane {
    PreviewReserved,
    General,
}

fn start_media_workers(app: tauri::AppHandle) -> Vec<JoinHandle<()>> {
    let mut workers = Vec::with_capacity(GENERAL_MEDIA_WORKER_COUNT + 1);
    {
        let app = app.clone();
        workers.push(std::thread::spawn(move || {
            media_worker_loop(app, MediaWorkerLane::PreviewReserved)
        }));
    }
    for _ in 0..GENERAL_MEDIA_WORKER_COUNT {
        workers.push({
            let app = app.clone();
            std::thread::spawn(move || media_worker_loop(app, MediaWorkerLane::General))
        });
    }
    workers
}

fn media_worker_loop(app: tauri::AppHandle, lane: MediaWorkerLane) {
    loop {
        let (queue, cache) = {
            let Some(state) = app.try_state::<AppState>() else {
                return;
            };
            if state.shutdown_requested.load(Ordering::Relaxed) {
                return;
            }

            (state.media_queue.clone(), state.thumb_cache.clone())
        };

        let job = match lane {
            MediaWorkerLane::PreviewReserved => queue.pop_preview(),
            MediaWorkerLane::General => queue.pop(),
        };
        let Some(job) = job else {
            return;
        };

        if let Some(state) = app.try_state::<AppState>() {
            if state.shutdown_requested.load(Ordering::Relaxed) {
                queue.finish(&job);
                return;
            }
        }

        let derivatives = cache.generate_derivatives(&job.path, job.needs);
        if job.needs.preview {
            match derivatives.preview {
                Some(cp) => {
                    let _ = app.emit(
                        "preview-generated",
                        ThumbnailGeneratedPayload {
                            path: job.path.clone(),
                            cache_path: commands::media::normalize_cache_path(&cp),
                        },
                    );
                }
                None => eprintln!("[PureWall] Preview FAILED: {}", job.path),
            }
        }
        if job.needs.thumbnail {
            match derivatives.thumbnail {
                Some(cp) => {
                    let _ = app.emit(
                        "thumbnail-generated",
                        ThumbnailGeneratedPayload {
                            path: job.path.clone(),
                            cache_path: commands::media::normalize_cache_path(&cp),
                        },
                    );
                }
                None => {
                    emit_thumbnail_generation_failed(&app, &job.path, "Thumbnail generation failed")
                }
            }
        }

        queue.finish(&job);
    }
}

fn start_rotation_timer(app_handle: tauri::AppHandle) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let mut signal_generation = 0u64;

        loop {
            let state = app_handle.state::<AppState>();
            if state.shutdown_requested.load(Ordering::Relaxed) {
                break;
            }

            // Read the cached boolean first — no DB I/O needed (MED-20).
            // The Mutex is kept in sync by every path that changes pause state.
            let paused = state.is_paused.lock().map(|g| *g).unwrap_or_else(|_| {
                // Mutex poisoned: fall back to DB query.
                sync_pause_state_from_settings(state.inner(), Some(&app_handle)).unwrap_or(true)
            });
            if paused {
                signal_generation = state.rotation_signal.wait_for_change(signal_generation);
                continue;
            }

            let interval = state.rotation_secs.load(Ordering::Relaxed);
            if interval == 0 {
                signal_generation = state.rotation_signal.wait_for_change(signal_generation);
                continue;
            }

            match state
                .rotation_signal
                .wait_for_change_or_timeout(signal_generation, Duration::from_secs(interval))
            {
                RotationWait::Signaled(generation) => {
                    signal_generation = generation;
                }
                RotationWait::TimedOut(generation) => {
                    signal_generation = generation;
                    let state = app_handle.state::<AppState>();
                    if state.shutdown_requested.load(Ordering::Relaxed) {
                        break;
                    }
                    let paused = state.is_paused.lock().map(|g| *g).unwrap_or_else(|_| {
                        sync_pause_state_from_settings(state.inner(), Some(&app_handle))
                            .unwrap_or(true)
                    });
                    if !paused && state.rotation_secs.load(Ordering::Relaxed) > 0 {
                        if let Err(message) = execute_playback_action(
                            state.inner(),
                            &app_handle,
                            PlaybackAction::Next,
                        ) {
                            report_playback_failure(
                                &app_handle,
                                "Automatic rotation",
                                PlaybackAction::Next,
                                message,
                            );
                        }
                    }
                }
            }
        }
    })
}

pub(crate) fn emit_focus_mode_status(app: &tauri::AppHandle, state: &AppState) {
    let _ = app.emit("focus-mode-changed", state.focus_mode.snapshot());
}

fn start_focus_monitor(app_handle: tauri::AppHandle) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let mut last_fullscreen = false;
        let mut signal_generation = 0u64;

        loop {
            let state = app_handle.state::<AppState>();
            if state.shutdown_requested.load(Ordering::Relaxed) {
                break;
            }

            if !state.focus_mode.enabled() {
                signal_generation = state.focus_signal.wait_for_change(signal_generation);
                continue;
            }

            match state
                .focus_signal
                .wait_for_change_or_timeout(signal_generation, Duration::from_secs(2))
            {
                RotationWait::Signaled(generation) => {
                    signal_generation = generation;
                    continue;
                }
                RotationWait::TimedOut(generation) => {
                    signal_generation = generation;
                }
            }

            let state = app_handle.state::<AppState>();
            if state.shutdown_requested.load(Ordering::Relaxed) {
                break;
            }

            let enabled = state.focus_mode.enabled();
            let fullscreen = if enabled {
                focus::is_fullscreen_foreground()
            } else {
                false
            };

            let fullscreen_changed = fullscreen != last_fullscreen;
            last_fullscreen = fullscreen;
            state.focus_mode.set_fullscreen_detected(fullscreen);

            let mut status_changed = fullscreen_changed;

            if (!enabled || !fullscreen) && state.focus_mode.auto_pause_suppressed() {
                state.focus_mode.set_auto_pause_suppressed(false);
            }

            if enabled
                && fullscreen
                && !state.focus_mode.auto_paused()
                && !state.focus_mode.auto_pause_suppressed()
            {
                let mut pause_changed = false;
                if let Ok(mut paused) = state.is_paused.lock() {
                    if !*paused {
                        *paused = true;
                        state.focus_mode.set_auto_paused(true);
                        let _ = app_handle.emit("pause-changed", true);
                        pause_changed = true;
                        status_changed = true;
                    }
                }
                if pause_changed {
                    state.rotation_signal.notify();
                }
            } else if (!enabled || !fullscreen) && state.focus_mode.auto_paused() {
                state.focus_mode.set_auto_paused(false);
                state.focus_mode.set_auto_pause_suppressed(false);
                let _ = sync_pause_state_from_settings(state.inner(), Some(&app_handle));
                status_changed = true;
            }

            if status_changed {
                emit_focus_mode_status(&app_handle, state.inner());
            }
        }
    })
}

pub(crate) fn shutdown_background_threads(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };

    state.shutdown_requested.store(true, Ordering::Relaxed);
    state.rotation_signal.notify();
    state.focus_signal.notify();
    state.media_queue.shutdown();
    let watchers = state
        .folder_watchers
        .lock()
        .map(|mut watchers| std::mem::take(&mut *watchers))
        .unwrap_or_default();
    drop(watchers);
    let handles = state
        .background_threads
        .lock()
        .map(|mut handles| handles.drain(..).collect::<Vec<_>>())
        .unwrap_or_default();

    for handle in handles {
        let _ = handle.join();
    }

    // Checkpoint only after every database-using watcher/worker has stopped.
    // Otherwise a late write can recreate WAL pages after the checkpoint.
    if let Ok(db) = state.db.lock() {
        let _ = db.checkpoint_wal();
    };
}
/// Handle CLI actions from right-click menu (no Tauri window)
fn cli_action_from_args(args: &[String]) -> Option<PlaybackAction> {
    args.iter()
        .position(|arg| arg == "--action")
        .and_then(|idx| args.get(idx + 1))
        .and_then(|value| PlaybackAction::parse(value))
}

fn run_app_cli_action(
    app: &tauri::AppHandle,
    action: PlaybackAction,
) -> Result<PlaybackActionOutcome, String> {
    let state = app
        .try_state::<AppState>()
        .ok_or_else(|| "PureWall is still starting; retry this action in a moment".to_string())?;
    execute_playback_action(state.inner(), app, action)
}
fn cli_action_argument(action: PlaybackAction) -> &'static str {
    match action {
        PlaybackAction::Next => "next",
        PlaybackAction::Like => "like",
        PlaybackAction::Dislike => "dislike",
        PlaybackAction::TogglePause => "pause",
    }
}

fn handle_forwarded_cli_action(app: &tauri::AppHandle, action: PlaybackAction) {
    if let Err(message) = run_app_cli_action(app, action) {
        diagnostics::append_cli_log(cli_action_argument(action), &message);
        report_playback_failure(app, "CLI action", action, message);
    }
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

// === Context Menu Commands ===

#[tauri::command]
fn register_context_menu() -> CommandResult<()> {
    context_menu::register().map_err(CommandError::from_display)
}

#[tauri::command]
fn unregister_context_menu() -> CommandResult<()> {
    context_menu::unregister().map_err(CommandError::from_display)
}

#[tauri::command]
fn is_context_menu_registered() -> bool {
    context_menu::is_registered()
}

pub(crate) fn set_current_wallpaper_rating(
    state: &AppState,
    rating: i32,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let current_path = read_current_wallpaper_path(&db)?;
    let path = require_registered_wallpaper_file(&db, &current_path)?;
    db.set_rating(&path, rating).map_err(|e| e.to_string())?;
    drop(db);

    Ok(path)
}

// === Autostart Commands ===

#[tauri::command]
fn set_autostart(enable: bool) -> CommandResult<()> {
    if enable {
        autostart::enable().map_err(CommandError::from_display)
    } else {
        autostart::disable().map_err(CommandError::from_display)
    }
}

#[tauri::command]
fn is_autostart_enabled() -> bool {
    autostart::is_enabled()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let initial_cli_action = cli_action_from_args(&args);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if let Some(action) = cli_action_from_args(&args) {
                handle_forwarded_cli_action(app, action);
            } else {
                show_main_window(app);
            }
        }))
        .setup(move |app| {
            let db_path = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| dirs::data_dir().unwrap().join("PureWall"));
            std::fs::create_dir_all(&db_path)?;

            let db = db::Database::new(db_path.join("purewall.db"))?;
            let watched_folders = db.get_watched_folders()?;
            let thumb_cache = thumbnails::ThumbnailCache::new(&db_path)?;
            let mut restored_playback =
                load_restored_playback_state(&db).map_err(std::io::Error::other)?;
            if !restored_playback.manual_paused && legacy_pause_file().exists() {
                restored_playback.manual_paused = true;
                db.set_setting(SETTING_PAUSED, "true")?;
                let _ = std::fs::remove_file(legacy_pause_file());
            }

            // Prune stale cache files so the thumbnail directory doesn't grow
            // unboundedly. Keeps a generous recent working set; cache hits touch
            // files so frequently viewed derivatives survive later cleanups.
            let protected_cache_sources = restored_playback
                .current_wallpaper_path
                .clone()
                .into_iter()
                .collect::<Vec<_>>();

            const MAX_CACHE_FILES: usize = 5000;
            match thumb_cache.cleanup_with_protected(MAX_CACHE_FILES, &protected_cache_sources) {
                Ok(removed) if removed > 0 => {
                    eprintln!(
                        "[PureWall] Cache cleanup: removed {} stale derivative files",
                        removed
                    );
                }
                Err(e) => {
                    eprintln!("[PureWall] Cache cleanup skipped: {}", e);
                }
                _ => {}
            }

            let focus_enabled = db
                .get_setting(SETTING_FOCUS_MODE)?
                .map(|value| value == "true")
                .unwrap_or(false);

            app.manage(Mutex::new(app_updates::PendingUpdateState::<
                tauri_plugin_updater::Update,
            >::default()));
            app.manage(AppState {
                db: Mutex::new(db),
                folder_watchers: Mutex::new(HashMap::new()),
                source_operations: Mutex::new(HashSet::new()),
                is_paused: Mutex::new(restored_playback.manual_paused),
                thumb_cache,
                media_queue: media_queue::MediaJobQueue::new(),
                rotation_secs: AtomicU64::new(restored_playback.rotation_secs),
                display_mode: Mutex::new(restored_playback.display_mode),
                focus_mode: focus::FocusMode::new(focus_enabled),
                shutdown_requested: std::sync::atomic::AtomicBool::new(false),
                background_threads: Mutex::new(Vec::new()),
                rotation_signal: RotationSignal::new(),
                focus_signal: RotationSignal::new(),
            });

            if let Some(action) = initial_cli_action {
                handle_forwarded_cli_action(app.handle(), action);
                app.handle().exit(0);
                return Ok(());
            }

            restore_persisted_folder_watchers(app.handle(), watched_folders);
            show_main_window(app.handle());

            let rotation_thread = start_rotation_timer(app.handle().clone());
            let focus_thread = start_focus_monitor(app.handle().clone());
            let media_threads = start_media_workers(app.handle().clone());
            if let Some(state) = app.handle().try_state::<AppState>() {
                if let Ok(mut handles) = state.background_threads.lock() {
                    handles.push(rotation_thread);
                    handles.push(focus_thread);
                    handles.extend(media_threads);
                }
            }
            tray::create_tray(app)?;

            let app_handle = app.handle().clone();
            if let Some(main_window) = app.get_webview_window("main") {
                main_window.on_window_event(move |event| {
                    if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                        shutdown_background_threads(&app_handle);
                    }
                });
            }

            let app_handle = app.handle().clone();
            if let Some(widget) = app.get_webview_window("widget") {
                widget.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        if let Some(win) = app_handle.get_webview_window("widget") {
                            let _ = win.hide();
                        }
                        let _ = app_handle.emit("widget-visibility-changed", false);
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_updates::fetch_update,
            app_updates::install_update,
            get_wallpapers,
            get_wallpapers_filtered,
            get_wallpapers_page,
            get_wallpaper_by_path,
            get_wallpapers_by_filter,
            commands::imports::set_wallpaper_folder,
            commands::library_sources::list_library_sources,
            commands::backup::export_library_backup,
            commands::backup::preview_backup_import,
            commands::backup::import_library_backup,
            commands::library_sources::rescan_library_source,
            commands::library_sources::retry_library_source,
            commands::library_sources::preview_remove_library_source,
            commands::library_sources::remove_library_source,
            commands::library_sources::relocate_library_source,
            commands::imports::import_wallpaper_folder,
            commands::imports::import_wallpaper_files,
            commands::imports::import_dropped_paths,
            get_image_metadata,
            get_shell_metadata,
            commands::playback::next_wallpaper,
            commands::playback::run_playback_action,
            commands::playback::set_current_wallpaper,
            commands::playback::like_wallpaper,
            commands::playback::dislike_wallpaper,
            commands::playback::reset_rating,
            commands::playback::delete_wallpaper,
            commands::taxonomy::get_tags,
            commands::taxonomy::get_collections,
            commands::taxonomy::create_collection,
            commands::taxonomy::delete_collection,
            commands::taxonomy::assign_collection,
            commands::taxonomy::unassign_collection,
            commands::taxonomy::create_tag,
            commands::taxonomy::delete_tag,
            commands::taxonomy::assign_tag,
            commands::taxonomy::unassign_tag,
            commands::taxonomy::set_blacklisted,
            commands::taxonomy::set_wallpaper_display_title,
            commands::taxonomy::batch_set_rating,
            commands::taxonomy::batch_assign_tag,
            commands::taxonomy::batch_unassign_tag,
            commands::taxonomy::batch_assign_collection,
            commands::taxonomy::batch_unassign_collection,
            commands::taxonomy::batch_blacklist,
            commands::taxonomy::batch_delete_wallpapers,
            commands::playback::toggle_pause,
            commands::playback::is_paused,
            commands::playback::get_stats,
            commands::media::load_thumbnails_batch,
            commands::media::bootstrap_active_wallpaper,
            commands::media::prewarm_preview_images,
            commands::media::load_preview_image,
            commands::playback::set_rotation_interval,
            commands::playback::get_rotation_interval,
            commands::playback::get_displays,
            commands::playback::get_display_mode,
            commands::playback::set_display_mode,
            commands::playback::get_focus_mode_status,
            commands::playback::set_focus_mode_enabled,
            commands::playback::get_yearly_stats,
            register_context_menu,
            unregister_context_menu,
            is_context_menu_registered,
            set_autostart,
            is_autostart_enabled,
            widget::toggle_widget,
            widget::hide_widget,
            widget::widget_next,
            widget::widget_like,
            widget::widget_dislike,
            commands::playback::read_current_wallpaper_path_cmd,
            diagnostics::read_cli_log,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod main_tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_db_path(test_name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "purewall-main-{test_name}-{}-{nanos}.db",
            std::process::id()
        ))
    }

    fn remove_sqlite_files(path: &Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
    }

    #[test]
    fn import_images_into_database_returns_counts_without_loading_wallpapers() {
        let db_path = unique_temp_db_path("import-summary");
        remove_sqlite_files(&db_path);
        let first_path = db_path.with_extension("mounted-a.jpg");
        let second_path = db_path.with_extension("mounted-b.jpg");
        let _ = std::fs::remove_file(&first_path);
        let _ = std::fs::remove_file(&second_path);
        std::fs::write(&first_path, b"first").expect("first source file should exist");
        std::fs::write(&second_path, b"second").expect("second source file should exist");
        let db = db::Database::new(&db_path).expect("database should initialize");
        let images = vec![
            scanner::ImageInfo {
                path: first_path.to_string_lossy().to_string(),
                hash: "hash-a".to_string(),
                width: 1920,
                height: 1080,
                file_size: 1200,
            },
            scanner::ImageInfo {
                path: second_path.to_string_lossy().to_string(),
                hash: "hash-b".to_string(),
                width: 2560,
                height: 1440,
                file_size: 2400,
            },
        ];

        let result = import_images_into_database(&db, &images, "mounted")
            .expect("image import summary should succeed");

        assert_eq!(result.scanned, 2);
        assert_eq!(result.imported, 2);
        let first = db
            .get_wallpaper_by_path(&images[0].path)
            .expect("wallpaper lookup should succeed")
            .expect("first wallpaper should exist");
        assert_eq!(first.source, "mounted");
        assert_eq!(first.width, 1920);
        assert_eq!(first.file_size, 1200);

        drop(db);
        let _ = std::fs::remove_file(first_path);
        let _ = std::fs::remove_file(second_path);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn import_skips_source_removed_after_scan() {
        let db_path = unique_temp_db_path("import-removed-source");
        let removed_path = db_path.with_extension("removed-after-scan.jpg");
        remove_sqlite_files(&db_path);
        let _ = std::fs::remove_file(&removed_path);
        let db = db::Database::new(&db_path).expect("database should initialize");
        let image = scanner::ImageInfo {
            path: removed_path.to_string_lossy().to_string(),
            hash: "stale-hash".to_string(),
            width: 1920,
            height: 1080,
            file_size: 1200,
        };

        let result = import_images_into_database(&db, std::slice::from_ref(&image), "mounted")
            .expect("stale import should be handled");

        assert_eq!(result.scanned, 1);
        assert_eq!(result.imported, 0);
        assert!(db
            .get_wallpaper_by_path(&image.path)
            .expect("stale row lookup should succeed")
            .is_none());

        drop(db);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn watched_path_reconciliation_imports_new_image_before_refresh() {
        let db_path = unique_temp_db_path("watcher-reconcile");
        let watched_dir = db_path.with_extension("watched");
        remove_sqlite_files(&db_path);
        let _ = std::fs::remove_dir_all(&watched_dir);
        std::fs::create_dir_all(&watched_dir).expect("watched directory should be created");
        let image_path = watched_dir.join("new-wallpaper.jpg");
        image::RgbImage::new(12, 8)
            .save(&image_path)
            .expect("test wallpaper should be saved");
        let stored_path = image_path
            .canonicalize()
            .expect("test wallpaper path should canonicalize")
            .to_string_lossy()
            .to_string();
        let db = db::Database::new(&db_path).expect("database should initialize");

        assert!(db
            .get_wallpaper_by_path(&stored_path)
            .expect("initial lookup should succeed")
            .is_none());

        let result = reconcile_watched_paths(std::slice::from_ref(&image_path), |inspection| {
            db.mark_paths_unavailable(&inspection.missing_paths)
                .map_err(CommandError::from_display)?;
            import_images_into_database(&db, &inspection.images, "mounted")
        })
        .expect("watcher reconciliation should succeed");

        assert_eq!(result.scanned, 1);
        assert_eq!(result.imported, 1);
        let wallpaper = db
            .get_wallpaper_by_path(&stored_path)
            .expect("wallpaper lookup should succeed")
            .expect("watcher must persist the new wallpaper before refresh");
        assert_eq!(wallpaper.width, 12);
        assert_eq!(wallpaper.height, 8);

        drop(db);
        let _ = std::fs::remove_dir_all(&watched_dir);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn watched_directory_removal_hides_descendants_without_deleting_metadata() {
        let db_path = unique_temp_db_path("watcher-remove-directory");
        let watched_dir = db_path.with_extension("watched-remove");
        remove_sqlite_files(&db_path);
        let _ = std::fs::remove_dir_all(&watched_dir);
        std::fs::create_dir_all(&watched_dir).expect("watched directory should be created");
        let watched_root = watched_dir
            .canonicalize()
            .expect("watched root should canonicalize");
        let image_path = watched_root.join("removed-wallpaper.jpg");
        image::RgbImage::new(12, 8)
            .save(&image_path)
            .expect("test wallpaper should be saved");
        let stored_path = image_path
            .canonicalize()
            .expect("test wallpaper path should canonicalize")
            .to_string_lossy()
            .to_string();
        let db = db::Database::new(&db_path).expect("database should initialize");

        reconcile_watched_paths(std::slice::from_ref(&image_path), |inspection| {
            import_images_into_database(&db, &inspection.images, "mounted")
        })
        .expect("initial watcher reconciliation should succeed");
        assert_eq!(
            db.get_wallpapers_page("all", "created", "", 0, 10)
                .expect("initial page should load")
                .total,
            1
        );

        std::fs::remove_dir_all(&watched_dir).expect("watched directory should be removed");
        let result = reconcile_watched_paths(std::slice::from_ref(&watched_root), |inspection| {
            db.mark_paths_unavailable(&inspection.missing_paths)
                .map_err(CommandError::from_display)?;
            import_images_into_database(&db, &inspection.images, "mounted")
        })
        .expect("removed directory reconciliation should succeed");

        assert_eq!(result.scanned, 0);
        assert_eq!(result.imported, 0);
        assert_eq!(
            db.get_wallpapers_page("all", "created", "", 0, 10)
                .expect("page after removal should load")
                .total,
            0
        );
        assert!(db
            .get_wallpaper_by_path(&stored_path)
            .expect("removed row lookup should succeed")
            .is_some());

        drop(db);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn watcher_registry_retains_unique_roots_until_shutdown() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        struct FakeWatcher(Arc<AtomicUsize>);

        impl Drop for FakeWatcher {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }

        let drops = Arc::new(AtomicUsize::new(0));
        let mut watchers = std::collections::HashMap::new();
        assert!(
            ensure_watcher_registered(&mut watchers, "root-a", "imported-folder", || Ok::<_, ()>(
                FakeWatcher(drops.clone())
            ))
            .expect("first watcher should register")
        );
        assert!(!ensure_watcher_registered(
            &mut watchers,
            "root-a",
            "imported-folder",
            || -> Result<FakeWatcher, ()> {
                panic!("duplicate root and source must not start a second watcher")
            }
        )
        .expect("duplicate watcher should be reused"));
        assert!(
            ensure_watcher_registered(&mut watchers, "root-a", "mounted", || Ok::<_, ()>(
                FakeWatcher(drops.clone())
            ))
            .expect("source change should replace the watcher")
        );
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert!(
            ensure_watcher_registered(&mut watchers, "root-b", "imported-folder", || Ok::<_, ()>(
                FakeWatcher(drops.clone())
            ))
            .expect("second unique watcher should register")
        );

        assert_eq!(watchers.len(), 2);
        assert_eq!(watchers["root-a"].0, "mounted");
        drop(watchers);
        assert_eq!(drops.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn display_title_file_failure_does_not_update_database() {
        let db_path = unique_temp_db_path("title-file-failure");
        let wallpaper_path = db_path.with_extension("title-file-failure.jpg");
        remove_sqlite_files(&db_path);
        let _ = std::fs::remove_file(&wallpaper_path);
        std::fs::write(&wallpaper_path, b"source").expect("source file should exist");
        let database = db::Database::new(&db_path).expect("database should initialize");
        database
            .upsert_wallpaper(
                &wallpaper_path.to_string_lossy(),
                "hash",
                "test",
                100,
                100,
                6,
            )
            .expect("wallpaper should insert");
        database
            .set_wallpaper_display_title(&wallpaper_path.to_string_lossy(), "Original title")
            .expect("original title should save");
        let database = std::sync::Mutex::new(database);
        let wallpaper_path = wallpaper_path.to_string_lossy().to_string();

        let result = save_display_title_with_writer(
            &database,
            &wallpaper_path,
            "New title",
            |_path, _title| Err("read-only property store".to_string()),
        );

        let error = result.expect_err("file metadata failure should reach the command boundary");
        assert_eq!(error.code, "file_metadata_write_failed");
        let saved = database
            .lock()
            .expect("database lock should succeed")
            .get_wallpaper_by_path(&wallpaper_path)
            .expect("wallpaper lookup should succeed")
            .expect("wallpaper should remain registered");
        assert_eq!(saved.display_title, "Original title");

        drop(database);
        let _ = std::fs::remove_file(wallpaper_path);
        remove_sqlite_files(&db_path);
    }
}
