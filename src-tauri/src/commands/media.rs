use crate::{
    active_preview, emit_thumbnail_generation_failed, media_queue, read_current_wallpaper_path,
    require_registered_wallpaper_file, require_registered_wallpaper_files, thumbnails,
    validate_existing_image_file, AppState, CommandError, CommandResult,
};
use std::collections::HashSet;
use std::path::Path;

pub(crate) fn normalize_cache_path(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[tauri::command]
pub(crate) fn load_thumbnails_batch(
    paths: Vec<String>,
    state: tauri::State<AppState>,
    app: tauri::AppHandle,
) -> CommandResult<std::collections::HashMap<String, String>> {
    let mut result = std::collections::HashMap::new();
    let mut seen_paths = HashSet::with_capacity(paths.len());
    let validated_paths = paths
        .into_iter()
        .filter_map(|path| match validate_existing_image_file(&path) {
            Ok(validated) if seen_paths.insert(validated.clone()) => Some(validated),
            Ok(_) => None,
            Err(error) => {
                emit_thumbnail_generation_failed(&app, &path, error);
                None
            }
        })
        .collect::<Vec<_>>();

    let registered_paths = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.registered_available_paths(&validated_paths)
            .map_err(CommandError::from_display)?
    };

    for validated in validated_paths {
        if !registered_paths.contains(&validated) {
            emit_thumbnail_generation_failed(&app, &validated, "Wallpaper is no longer registered");
            continue;
        }

        if let Some(hit) = state.thumb_cache.lookup_thumbnail_path(&validated) {
            result.insert(validated.clone(), normalize_cache_path(&hit.path));
            if hit.freshness == thumbnails::ThumbnailCacheFreshness::Stale {
                state
                    .media_queue
                    .enqueue(media_queue::MediaJobKind::Thumbnail, validated);
            }
            continue;
        }

        state
            .media_queue
            .enqueue(media_queue::MediaJobKind::Thumbnail, validated);
    }

    Ok(result)
}

#[tauri::command]
pub(crate) fn bootstrap_active_wallpaper(
    state: tauri::State<AppState>,
) -> CommandResult<active_preview::ActiveWallpaperBootstrap> {
    let wallpaper = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        match read_current_wallpaper_path(&db) {
            Ok(path) if Path::new(&path).is_file() => db
                .get_wallpaper_by_path(&path)
                .map_err(CommandError::from_display)?,
            Ok(_) | Err(_) => None,
        }
    };

    let Some(wallpaper) = wallpaper else {
        return Ok(active_preview::build_bootstrap(None, None, None).0);
    };

    let thumbnail_path = state
        .thumb_cache
        .lookup_thumbnail_path(&wallpaper.path)
        .map(|hit| hit.path);
    let preview_hit = state.thumb_cache.lookup_preview_path(&wallpaper.path);
    let (bootstrap, should_queue_preview) = active_preview::build_bootstrap_with_preview_hit(
        Some(wallpaper),
        thumbnail_path,
        preview_hit,
    );

    if should_queue_preview {
        state
            .media_queue
            .enqueue_active_preview(bootstrap.path.clone());
    }

    Ok(bootstrap)
}

#[tauri::command]
pub(crate) fn load_preview_image(
    path: String,
    state: tauri::State<AppState>,
) -> CommandResult<Option<String>> {
    let path = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        require_registered_wallpaper_file(&db, &path)?
    };

    let preview_hit = state.thumb_cache.lookup_preview_path(&path);
    if let Some(hit) = &preview_hit {
        if hit.freshness == thumbnails::PreviewCacheFreshness::Current {
            return Ok(Some(normalize_cache_path(&hit.path)));
        }
    }

    let fallback_path = preview_hit.map(|hit| normalize_cache_path(&hit.path));
    state.media_queue.enqueue_active_preview(path);

    Ok(fallback_path)
}

#[tauri::command]
pub(crate) fn prewarm_preview_images(
    paths: Vec<String>,
    state: tauri::State<AppState>,
) -> CommandResult<usize> {
    if paths.len() > 2 {
        return Err(CommandError::new(
            "invalid_input",
            "At most two speculative previews may be requested",
        ));
    }
    let paths = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        require_registered_wallpaper_files(&db, &paths)?
    };
    Ok(active_preview::prewarm_speculative_previews(
        &paths,
        &state.thumb_cache,
        &state.media_queue,
    ))
}
