use crate::{
    batch_operations, db, deletion, require_batch_wallpaper_files, require_existing_collection,
    require_existing_tag, require_registered_wallpaper_file, save_display_title_with_writer,
    shell_metadata, AppState, CommandError, CommandResult, DeleteResult,
};

#[tauri::command]
pub(crate) fn get_tags(state: tauri::State<AppState>) -> CommandResult<Vec<db::TagEntry>> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_tags().map_err(CommandError::from_display)
}

#[tauri::command]
pub(crate) fn get_collections(
    state: tauri::State<AppState>,
) -> CommandResult<Vec<db::CollectionEntry>> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_collections().map_err(CommandError::from_display)
}

#[tauri::command]
pub(crate) fn create_collection(
    state: tauri::State<AppState>,
    name: String,
    color: String,
) -> CommandResult<db::CollectionEntry> {
    let name = name.trim();
    if name.is_empty() {
        return Err(CommandError::new(
            "invalid_collection",
            "Collection name cannot be empty",
        ));
    }

    let color = if color.trim().is_empty() {
        db::DEFAULT_TAG_COLOR
    } else {
        color.trim()
    };

    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.create_collection(name, color)
        .map_err(CommandError::from_display)
}

#[tauri::command]
pub(crate) fn delete_collection(
    state: tauri::State<AppState>,
    collection_id: i64,
) -> CommandResult<()> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.delete_collection(collection_id)
        .map_err(CommandError::from_display)
}

#[tauri::command]
pub(crate) fn assign_collection(
    state: tauri::State<AppState>,
    path: String,
    collection_id: i64,
) -> CommandResult<()> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let path = require_registered_wallpaper_file(&db, &path)?;
    db.assign_collection(&path, collection_id)
        .map_err(CommandError::from_display)
}

#[tauri::command]
pub(crate) fn unassign_collection(
    state: tauri::State<AppState>,
    path: String,
    collection_id: i64,
) -> CommandResult<()> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let path = require_registered_wallpaper_file(&db, &path)?;
    db.unassign_collection(&path, collection_id)
        .map_err(CommandError::from_display)
}

#[tauri::command]
pub(crate) fn create_tag(
    state: tauri::State<AppState>,
    name: String,
    color: String,
) -> CommandResult<db::TagEntry> {
    let name = name.trim();
    if name.is_empty() {
        return Err(CommandError::new("invalid_tag", "Tag name cannot be empty"));
    }

    let color = if color.trim().is_empty() {
        db::DEFAULT_TAG_COLOR
    } else {
        color.trim()
    };

    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.create_tag(name, color)
        .map_err(CommandError::from_display)
}

#[tauri::command]
pub(crate) fn delete_tag(state: tauri::State<AppState>, tag_id: i64) -> CommandResult<()> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.delete_tag(tag_id).map_err(CommandError::from_display)
}

#[tauri::command]
pub(crate) fn assign_tag(
    state: tauri::State<AppState>,
    path: String,
    tag_id: i64,
) -> CommandResult<()> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let path = require_registered_wallpaper_file(&db, &path)?;
    db.assign_tag(&path, tag_id)
        .map_err(CommandError::from_display)
}

#[tauri::command]
pub(crate) fn unassign_tag(
    state: tauri::State<AppState>,
    path: String,
    tag_id: i64,
) -> CommandResult<()> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let path = require_registered_wallpaper_file(&db, &path)?;
    db.unassign_tag(&path, tag_id)
        .map_err(CommandError::from_display)
}

#[tauri::command]
pub(crate) fn set_blacklisted(
    state: tauri::State<AppState>,
    path: String,
    blacklisted: bool,
) -> CommandResult<()> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let path = require_registered_wallpaper_file(&db, &path)?;
    db.set_blacklisted(&path, blacklisted)
        .map_err(CommandError::from_display)
}

#[tauri::command]
pub(crate) fn set_wallpaper_display_title(
    state: tauri::State<AppState>,
    path: String,
    display_title: String,
) -> CommandResult<db::WallpaperEntry> {
    let title = display_title.trim();
    if title.chars().count() > 120 {
        return Err(CommandError::new(
            "invalid_title",
            "Title must be 120 characters or fewer",
        ));
    }

    let path = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        require_registered_wallpaper_file(&db, &path)?
    };

    save_display_title_with_writer(&state.db, &path, title, shell_metadata::write_title)
}

#[tauri::command]
pub(crate) fn batch_set_rating(
    state: tauri::State<AppState>,
    paths: Vec<String>,
    rating: i32,
) -> CommandResult<batch_operations::BatchMutationResult> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let rating = batch_operations::validate_batch_rating(rating)?;
    let paths = require_batch_wallpaper_files(&db, &paths)?;
    let affected = db
        .batch_set_rating(&paths, rating)
        .map_err(CommandError::from_display)?;
    Ok(batch_operations::BatchMutationResult {
        affected,
        refresh: vec![batch_operations::BatchRefreshTarget::Stats],
    })
}

#[tauri::command]
pub(crate) fn batch_assign_tag(
    state: tauri::State<AppState>,
    paths: Vec<String>,
    tag_id: i64,
) -> CommandResult<batch_operations::BatchMutationResult> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let tag_id = require_existing_tag(&db, tag_id)?;
    let paths = require_batch_wallpaper_files(&db, &paths)?;
    let affected = db
        .batch_assign_tag(&paths, tag_id)
        .map_err(CommandError::from_display)?;
    Ok(batch_operations::BatchMutationResult {
        affected,
        refresh: vec![batch_operations::BatchRefreshTarget::Wallpapers],
    })
}

#[tauri::command]
pub(crate) fn batch_unassign_tag(
    state: tauri::State<AppState>,
    paths: Vec<String>,
    tag_id: i64,
) -> CommandResult<batch_operations::BatchMutationResult> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let tag_id = require_existing_tag(&db, tag_id)?;
    let paths = require_batch_wallpaper_files(&db, &paths)?;
    let affected = db
        .batch_unassign_tag(&paths, tag_id)
        .map_err(CommandError::from_display)?;
    Ok(batch_operations::BatchMutationResult {
        affected,
        refresh: vec![batch_operations::BatchRefreshTarget::Wallpapers],
    })
}

#[tauri::command]
pub(crate) fn batch_assign_collection(
    state: tauri::State<AppState>,
    paths: Vec<String>,
    collection_id: i64,
) -> CommandResult<batch_operations::BatchMutationResult> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let collection_id = require_existing_collection(&db, collection_id)?;
    let paths = require_batch_wallpaper_files(&db, &paths)?;
    let affected = db
        .batch_assign_collection(&paths, collection_id)
        .map_err(CommandError::from_display)?;
    Ok(batch_operations::BatchMutationResult {
        affected,
        refresh: vec![
            batch_operations::BatchRefreshTarget::Wallpapers,
            batch_operations::BatchRefreshTarget::Collections,
        ],
    })
}

#[tauri::command]
pub(crate) fn batch_unassign_collection(
    state: tauri::State<AppState>,
    paths: Vec<String>,
    collection_id: i64,
) -> CommandResult<batch_operations::BatchMutationResult> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let collection_id = require_existing_collection(&db, collection_id)?;
    let paths = require_batch_wallpaper_files(&db, &paths)?;
    let affected = db
        .batch_unassign_collection(&paths, collection_id)
        .map_err(CommandError::from_display)?;
    Ok(batch_operations::BatchMutationResult {
        affected,
        refresh: vec![
            batch_operations::BatchRefreshTarget::Wallpapers,
            batch_operations::BatchRefreshTarget::Collections,
        ],
    })
}

#[tauri::command]
pub(crate) fn batch_blacklist(
    state: tauri::State<AppState>,
    paths: Vec<String>,
    blacklisted: bool,
) -> CommandResult<batch_operations::BatchMutationResult> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let paths = require_batch_wallpaper_files(&db, &paths)?;
    let affected = db
        .batch_blacklist(&paths, blacklisted)
        .map_err(CommandError::from_display)?;
    Ok(batch_operations::BatchMutationResult {
        affected,
        refresh: vec![batch_operations::BatchRefreshTarget::Stats],
    })
}

#[tauri::command]
pub(crate) fn batch_delete_wallpapers(
    state: tauri::State<AppState>,
    paths: Vec<String>,
) -> CommandResult<Vec<DeleteResult>> {
    deletion::delete_many(&state.db, paths)
}
