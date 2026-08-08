use crate::thumbnails::{ThumbnailCache, ThumbnailCacheFreshness};
use std::collections::hash_map::DefaultHasher;
use std::fmt::Write;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const THUMBNAIL_CACHE_PROFILE: &str = "thumb-512-catmullrom-q88";

fn unique_temp_path(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("purewall-{label}-{nanos}"))
}

fn legacy_profile_thumbnail_path(cache_root: &Path, source_path: &str) -> PathBuf {
    let mut input = format!("{}|{}", THUMBNAIL_CACHE_PROFILE, source_path);
    if let Ok(metadata) = std::fs::metadata(source_path) {
        let _ = write!(input, "|{}", metadata.len());
        if let Ok(modified) = metadata.modified() {
            if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
                let _ = write!(input, "|{}.{}", duration.as_secs(), duration.subsec_nanos());
            }
        }
    }

    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    cache_root
        .join("thumbnails")
        .join(format!("{:016x}.jpg", hasher.finish()))
}

fn original_path_only_thumbnail_path(cache_root: &Path, source_path: &str) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    source_path.hash(&mut hasher);
    cache_root
        .join("thumbnails")
        .join(format!("{:016x}.jpg", hasher.finish()))
}

#[test]
fn thumbnail_lookup_returns_legacy_profile_cache_as_stale_fallback() {
    let cache_root = unique_temp_path("legacy-thumbnail-cache");
    let source_path = unique_temp_path("legacy-thumbnail-source.jpg");
    std::fs::write(&source_path, b"source").expect("source should be writable");
    let cache = ThumbnailCache::new(&cache_root).expect("cache should initialize");
    let source = source_path.to_str().expect("source path should be utf-8");
    let legacy_path = legacy_profile_thumbnail_path(&cache_root, source);
    std::fs::write(&legacy_path, b"legacy-thumbnail").expect("legacy cache should be writable");

    let hit = cache
        .lookup_thumbnail_path(source)
        .expect("legacy thumbnail should be returned immediately");

    assert_eq!(hit.path, legacy_path);
    assert_eq!(hit.freshness, ThumbnailCacheFreshness::Stale);
    let _ = std::fs::remove_file(source_path);
    let _ = std::fs::remove_dir_all(cache_root);
}

#[test]
fn thumbnail_lookup_returns_original_path_only_cache_as_stale_fallback() {
    let cache_root = unique_temp_path("original-thumbnail-cache");
    let source_path = unique_temp_path("original-thumbnail-source.jpg");
    std::fs::write(&source_path, b"source").expect("source should be writable");
    let cache = ThumbnailCache::new(&cache_root).expect("cache should initialize");
    let source = source_path.to_str().expect("source path should be utf-8");
    let legacy_path = original_path_only_thumbnail_path(&cache_root, source);
    std::fs::write(&legacy_path, b"original-thumbnail").expect("original cache should be writable");

    let hit = cache
        .lookup_thumbnail_path(source)
        .expect("original path-only thumbnail should be returned immediately");

    assert_eq!(hit.path, legacy_path);
    assert_eq!(hit.freshness, ThumbnailCacheFreshness::Stale);
    let _ = std::fs::remove_file(source_path);
    let _ = std::fs::remove_dir_all(cache_root);
}

#[test]
fn thumbnail_lookup_prefers_current_cache_over_legacy_fallback() {
    let cache_root = unique_temp_path("current-thumbnail-cache");
    let source_path = unique_temp_path("current-thumbnail-source.jpg");
    std::fs::write(&source_path, b"source").expect("source should be writable");
    let cache = ThumbnailCache::new(&cache_root).expect("cache should initialize");
    let source = source_path.to_str().expect("source path should be utf-8");
    let legacy_path = original_path_only_thumbnail_path(&cache_root, source);
    std::fs::write(&legacy_path, b"legacy-thumbnail").expect("legacy cache should be writable");
    let current_path = cache.thumbnail_path(source);
    std::fs::write(&current_path, b"current-thumbnail").expect("current cache should be writable");

    let hit = cache
        .lookup_thumbnail_path(source)
        .expect("current thumbnail should be returned");

    assert_eq!(hit.path, current_path);
    assert_eq!(hit.freshness, ThumbnailCacheFreshness::Current);
    let _ = std::fs::remove_file(source_path);
    let _ = std::fs::remove_dir_all(cache_root);
}
