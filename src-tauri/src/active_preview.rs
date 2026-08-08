use crate::db::WallpaperEntry;
use crate::media_queue::MediaJobQueue;
use crate::thumbnails::{PreviewCacheFreshness, PreviewCacheHit, ThumbnailCache};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewState {
    Ready,
    Queued,
    Unavailable,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ActiveWallpaperBootstrap {
    pub path: String,
    pub wallpaper: Option<WallpaperEntry>,
    pub thumbnail_path: Option<String>,
    pub preview_path: Option<String>,
    pub preview_state: PreviewState,
}

fn normalize_cache_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub fn build_bootstrap(
    wallpaper: Option<WallpaperEntry>,
    thumbnail_path: Option<PathBuf>,
    preview_path: Option<PathBuf>,
) -> (ActiveWallpaperBootstrap, bool) {
    let Some(wallpaper) = wallpaper else {
        return (
            ActiveWallpaperBootstrap {
                path: String::new(),
                wallpaper: None,
                thumbnail_path: None,
                preview_path: None,
                preview_state: PreviewState::Unavailable,
            },
            false,
        );
    };

    let path = wallpaper.path.clone();
    let thumbnail_path = thumbnail_path.as_deref().map(normalize_cache_path);
    let preview_path = preview_path.as_deref().map(normalize_cache_path);
    let should_queue_preview = preview_path.is_none();
    let preview_state = if should_queue_preview {
        PreviewState::Queued
    } else {
        PreviewState::Ready
    };

    (
        ActiveWallpaperBootstrap {
            path,
            wallpaper: Some(wallpaper),
            thumbnail_path,
            preview_path,
            preview_state,
        },
        should_queue_preview,
    )
}

pub fn build_bootstrap_with_preview_hit(
    wallpaper: Option<WallpaperEntry>,
    thumbnail_path: Option<PathBuf>,
    preview_hit: Option<PreviewCacheHit>,
) -> (ActiveWallpaperBootstrap, bool) {
    let stale_preview = matches!(
        preview_hit.as_ref().map(|hit| hit.freshness),
        Some(PreviewCacheFreshness::Stale)
    );
    let preview_path = preview_hit.map(|hit| hit.path);
    let (mut bootstrap, should_queue) = build_bootstrap(wallpaper, thumbnail_path, preview_path);

    if stale_preview && bootstrap.wallpaper.is_some() {
        bootstrap.preview_state = PreviewState::Queued;
        return (bootstrap, true);
    }

    (bootstrap, should_queue)
}

pub fn prewarm_active_preview(path: &str, cache: &ThumbnailCache, queue: &MediaJobQueue) -> bool {
    if path.is_empty() || cache.cached_preview_path(path).is_some() {
        return false;
    }

    queue.enqueue_active_preview(path.to_string())
}

pub fn prewarm_speculative_previews(
    paths: &[String],
    cache: &ThumbnailCache,
    queue: &MediaJobQueue,
) -> usize {
    const MAX_SPECULATIVE_PREWARM: usize = 2;
    let mut queued = 0;
    for path in paths {
        if queued >= MAX_SPECULATIVE_PREWARM {
            break;
        }
        if path.is_empty() || cache.cached_preview_path(path).is_some() {
            continue;
        }
        if queue.enqueue_speculative_preview(path.clone()) {
            queued += 1;
        }
    }
    queued
}
