use tauri::{Emitter, Manager};

use crate::{
    execute_playback_action, legacy_wallpaper_path, playback_action::PlaybackAction,
    playback_command_error, AppState, CommandResult,
};

#[tauri::command]
pub fn toggle_widget(app: tauri::AppHandle) -> CommandResult<bool> {
    let w = app
        .get_webview_window("widget")
        .ok_or_else(|| "Widget window not found".to_string())?;

    let visible = w.is_visible().map_err(|e| e.to_string())?;
    let next_visible = !visible;
    if next_visible {
        w.show().map_err(|e| e.to_string())?;
        let _ = w.unminimize();
        let _ = w.set_focus();
    } else {
        w.hide().map_err(|e| e.to_string())?;
    }

    let _ = app.emit("widget-visibility-changed", next_visible);
    Ok(next_visible)
}

#[tauri::command]
pub fn hide_widget(app: tauri::AppHandle) -> CommandResult<bool> {
    if let Some(w) = app.get_webview_window("widget") {
        w.hide().map_err(|e| e.to_string())?;
    }
    let _ = app.emit("widget-visibility-changed", false);
    Ok(false)
}

#[tauri::command]
pub fn widget_next(state: tauri::State<AppState>, app: tauri::AppHandle) -> CommandResult<String> {
    execute_playback_action(state.inner(), &app, PlaybackAction::Next)
        .map_err(playback_command_error)
        .and_then(legacy_wallpaper_path)
}

#[tauri::command]
pub fn widget_like(state: tauri::State<AppState>, app: tauri::AppHandle) -> CommandResult<String> {
    execute_playback_action(state.inner(), &app, PlaybackAction::Like)
        .map_err(playback_command_error)
        .and_then(legacy_wallpaper_path)
}

#[tauri::command]
pub fn widget_dislike(
    state: tauri::State<AppState>,
    app: tauri::AppHandle,
) -> CommandResult<String> {
    execute_playback_action(state.inner(), &app, PlaybackAction::Dislike)
        .map_err(playback_command_error)
        .and_then(legacy_wallpaper_path)
}
