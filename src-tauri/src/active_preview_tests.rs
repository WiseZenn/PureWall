use crate::active_preview::{build_bootstrap, build_bootstrap_with_preview_hit, PreviewState};
use crate::db::WallpaperEntry;
use crate::thumbnails::{PreviewCacheFreshness, PreviewCacheHit};
use std::path::PathBuf;

fn wallpaper(path: &str) -> WallpaperEntry {
    WallpaperEntry {
        id: 1,
        path: path.to_string(),
        hash: "hash".to_string(),
        source: "test".to_string(),
        display_title: "Current".to_string(),
        rating: 0,
        play_count: 0,
        last_played: None,
        created_at: "2026-07-11T00:00:00Z".to_string(),
        blacklisted: false,
        width: 3840,
        height: 2160,
        file_size: 1,
        tags: Vec::new(),
    }
}

#[test]
fn bootstrap_returns_ready_preview_without_queueing() {
    let (bootstrap, should_queue) = build_bootstrap(
        Some(wallpaper("D:/walls/current.jpg")),
        None,
        Some(PathBuf::from("D:/cache/current.preview.jpg")),
    );

    assert_eq!(bootstrap.preview_state, PreviewState::Ready);
    assert_eq!(
        bootstrap.preview_path.as_deref(),
        Some("D:/cache/current.preview.jpg")
    );
    assert!(!should_queue);
}

#[test]
fn bootstrap_returns_thumbnail_and_queues_missing_preview() {
    let (bootstrap, should_queue) = build_bootstrap(
        Some(wallpaper("D:/walls/current.jpg")),
        Some(PathBuf::from("D:/cache/current.jpg")),
        None,
    );

    assert_eq!(bootstrap.preview_state, PreviewState::Queued);
    assert_eq!(
        bootstrap.thumbnail_path.as_deref(),
        Some("D:/cache/current.jpg")
    );
    assert!(should_queue);
}

#[test]
fn bootstrap_marks_missing_or_stale_current_wallpaper_unavailable() {
    let (bootstrap, should_queue) = build_bootstrap(None, None, None);

    assert_eq!(bootstrap.preview_state, PreviewState::Unavailable);
    assert!(bootstrap.wallpaper.is_none());
    assert!(!should_queue);
}

#[test]
fn bootstrap_returns_stale_preview_while_queueing_current_format() {
    let stale_path = PathBuf::from("D:/cache/legacy.preview.jpg");
    let (bootstrap, should_queue) = build_bootstrap_with_preview_hit(
        Some(wallpaper("D:/walls/current.jpg")),
        None,
        Some(PreviewCacheHit {
            path: stale_path,
            freshness: PreviewCacheFreshness::Stale,
        }),
    );

    assert_eq!(bootstrap.preview_state, PreviewState::Queued);
    assert_eq!(
        bootstrap.preview_path.as_deref(),
        Some("D:/cache/legacy.preview.jpg")
    );
    assert!(should_queue);
}
