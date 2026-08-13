use crate::{
    active_preview, advance_independent_wallpapers, advance_shared_wallpaper, current_display_mode,
    db, deletion, emit_focus_mode_status, execute_playback_action,
    execute_playback_action_for_caller, focus, is_valid_display_mode, legacy_pause_state,
    legacy_wallpaper_path, persist_current_wallpaper, playback_command_error,
    read_current_wallpaper_path, require_registered_wallpaper_file, sync_pause_state_from_settings,
    wallpaper, AppState, CommandError, CommandResult, PlaybackAction, PlaybackActionOutcome,
    MAX_ROTATION_SECS, SETTING_DISPLAY_MODE, SETTING_FOCUS_MODE, SETTING_ROTATION_SECS,
};
use std::sync::atomic::Ordering;
use tauri::Emitter;

#[tauri::command]
pub(crate) fn run_playback_action(
    state: tauri::State<AppState>,
    app: tauri::AppHandle,
    action: PlaybackAction,
    caller: tauri::WebviewWindow,
) -> CommandResult<PlaybackActionOutcome> {
    execute_playback_action_for_caller(state.inner(), &app, action, Some(caller.label()))
        .map_err(playback_command_error)
}

#[tauri::command]
pub(crate) fn next_wallpaper(
    state: tauri::State<AppState>,
    app: tauri::AppHandle,
) -> CommandResult<String> {
    execute_playback_action(state.inner(), &app, PlaybackAction::Next)
        .map_err(playback_command_error)
        .and_then(legacy_wallpaper_path)
}

#[tauri::command]
pub(crate) fn set_current_wallpaper(
    state: tauri::State<AppState>,
    path: String,
) -> CommandResult<String> {
    let path = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        require_registered_wallpaper_file(&db, &path)?
    };

    let placement = if current_display_mode(state.inner()) == "span" {
        wallpaper::WallpaperPlacement::Span
    } else {
        wallpaper::WallpaperPlacement::Fill
    };
    wallpaper::set_wallpaper_all(&path, placement).map_err(|e| e.to_string())?;
    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.record_play(&path).map_err(|e| e.to_string())?;
        persist_current_wallpaper(&db, &path)?;
    }
    active_preview::prewarm_active_preview(&path, &state.thumb_cache, &state.media_queue);
    Ok(path)
}

#[tauri::command]
pub(crate) fn like_wallpaper(state: tauri::State<AppState>, path: String) -> CommandResult<()> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let path = require_registered_wallpaper_file(&db, &path)?;
    db.set_rating(&path, 1).map_err(CommandError::from_display)
}

#[tauri::command]
pub(crate) fn dislike_wallpaper(state: tauri::State<AppState>, path: String) -> CommandResult<()> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let path = require_registered_wallpaper_file(&db, &path)?;
    db.set_rating(&path, -1).map_err(CommandError::from_display)
}

#[tauri::command]
pub(crate) fn read_current_wallpaper_path_cmd(
    state: tauri::State<AppState>,
) -> CommandResult<String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    read_current_wallpaper_path(&db).map_err(Into::into)
}

#[tauri::command]
pub(crate) fn reset_rating(state: tauri::State<AppState>, path: String) -> CommandResult<()> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let path = require_registered_wallpaper_file(&db, &path)?;
    db.set_rating(&path, 0).map_err(CommandError::from_display)
}

#[tauri::command]
pub(crate) fn delete_wallpaper(
    state: tauri::State<AppState>,
    path: String,
) -> CommandResult<crate::DeleteResult> {
    deletion::delete_one(&state.db, path)
}

#[tauri::command]
pub(crate) fn toggle_pause(
    state: tauri::State<AppState>,
    app: tauri::AppHandle,
) -> CommandResult<bool> {
    execute_playback_action(state.inner(), &app, PlaybackAction::TogglePause)
        .map_err(playback_command_error)
        .and_then(legacy_pause_state)
}

#[tauri::command]
pub(crate) fn is_paused(state: tauri::State<AppState>) -> CommandResult<bool> {
    sync_pause_state_from_settings(state.inner(), None).map_err(Into::into)
}

#[tauri::command]
pub(crate) fn get_stats(state: tauri::State<AppState>) -> CommandResult<db::Stats> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_stats().map_err(CommandError::from_display)
}

#[tauri::command]
pub(crate) fn set_rotation_interval(
    state: tauri::State<AppState>,
    seconds: u64,
) -> CommandResult<()> {
    let seconds = seconds.min(MAX_ROTATION_SECS);
    state.rotation_secs.store(seconds, Ordering::Relaxed);
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.set_setting(SETTING_ROTATION_SECS, &seconds.to_string())
        .map_err(|e| e.to_string())?;
    state.rotation_signal.notify();
    Ok(())
}

#[tauri::command]
pub(crate) fn get_rotation_interval(state: tauri::State<AppState>) -> CommandResult<u64> {
    Ok(state.rotation_secs.load(Ordering::Relaxed))
}

#[tauri::command]
pub(crate) fn get_displays() -> CommandResult<Vec<wallpaper::DisplayInfo>> {
    wallpaper::get_displays().map_err(CommandError::from_display)
}

#[tauri::command]
pub(crate) fn get_display_mode(state: tauri::State<AppState>) -> CommandResult<String> {
    Ok(current_display_mode(state.inner()))
}

#[tauri::command]
pub(crate) fn set_display_mode(
    state: tauri::State<AppState>,
    app: tauri::AppHandle,
    mode: String,
) -> CommandResult<String> {
    if !is_valid_display_mode(&mode) {
        return Err(CommandError::new(
            "invalid_display_mode",
            "Invalid display mode",
        ));
    }

    let rotated_path = match mode.as_str() {
        "independent" => Some(advance_independent_wallpapers(state.inner())?),
        "span" | "all" => {
            let placement = if mode == "span" {
                wallpaper::WallpaperPlacement::Span
            } else {
                wallpaper::WallpaperPlacement::Fill
            };
            let maybe_current_path = {
                let db = state.db.lock().map_err(|e| e.to_string())?;
                read_current_wallpaper_path(&db).ok()
            };

            if let Some(path) = maybe_current_path.filter(|path| !path.is_empty()) {
                wallpaper::set_wallpaper_all(&path, placement).map_err(|e| e.to_string())?;
                None
            } else {
                Some(advance_shared_wallpaper(state.inner(), placement)?)
            }
        }
        _ => None,
    };

    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.set_setting(SETTING_DISPLAY_MODE, &mode)
        .map_err(|e| e.to_string())?;
    drop(db);

    {
        let mut display_mode = state.display_mode.lock().map_err(|e| e.to_string())?;
        *display_mode = mode.clone();
    }

    if let Some(path) = rotated_path {
        let _ = app.emit("auto-rotated", path);
    }

    Ok(mode)
}

#[tauri::command]
pub(crate) fn get_focus_mode_status(
    state: tauri::State<AppState>,
) -> CommandResult<focus::FocusModeStatus> {
    Ok(state.focus_mode.snapshot())
}

#[tauri::command]
pub(crate) fn set_focus_mode_enabled(
    state: tauri::State<AppState>,
    app: tauri::AppHandle,
    enabled: bool,
) -> CommandResult<focus::FocusModeStatus> {
    state.focus_mode.set_enabled(enabled);

    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.set_setting(SETTING_FOCUS_MODE, if enabled { "true" } else { "false" })
            .map_err(|e| e.to_string())?;
    }

    if !enabled {
        state.focus_mode.set_fullscreen_detected(false);
    }

    if !enabled && state.focus_mode.auto_paused() {
        state.focus_mode.set_auto_paused(false);
        state.focus_mode.set_auto_pause_suppressed(false);
        let _ = sync_pause_state_from_settings(state.inner(), Some(&app));
    } else if !enabled {
        state.focus_mode.set_auto_pause_suppressed(false);
    }
    state.rotation_signal.notify();
    state.focus_signal.notify();

    emit_focus_mode_status(&app, state.inner());
    Ok(state.focus_mode.snapshot())
}

#[tauri::command]
pub(crate) fn get_yearly_stats(
    state: tauri::State<AppState>,
    year: i32,
) -> CommandResult<db::YearlyStats> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_yearly_stats(year)
        .map_err(CommandError::from_display)
}
