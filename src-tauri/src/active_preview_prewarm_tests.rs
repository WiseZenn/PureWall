use crate::active_preview::{prewarm_active_preview, prewarm_speculative_previews};
use crate::media_queue::{MediaJobKind, MediaJobQueue, MediaNeeds, MediaPriority};
use crate::thumbnails::ThumbnailCache;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("purewall-{name}-{}-{nanos}", std::process::id()))
}

fn write_source(path: &Path) {
    std::fs::create_dir_all(path.parent().expect("source should have a parent"))
        .expect("source directory should be created");
    std::fs::write(path, b"source").expect("source should be written");
}

#[test]
fn cached_preview_does_not_enqueue_prewarm_work() {
    let root = unique_temp_dir("prewarm-hit");
    let source = root.join("current.jpg");
    write_source(&source);
    let cache = ThumbnailCache::new(&root).expect("cache should initialize");
    let source_text = source.to_string_lossy().to_string();
    std::fs::write(cache.preview_path(&source_text), b"cached")
        .expect("preview cache should be written");
    let queue = MediaJobQueue::new();

    assert!(!prewarm_active_preview(&source_text, &cache, &queue));
    assert!(queue.try_pop().is_none());

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn missing_preview_is_enqueued_exactly_once() {
    let root = unique_temp_dir("prewarm-miss");
    let source = root.join("current.jpg");
    write_source(&source);
    let cache = ThumbnailCache::new(&root).expect("cache should initialize");
    let source_text = source.to_string_lossy().to_string();
    let queue = MediaJobQueue::new();

    assert!(prewarm_active_preview(&source_text, &cache, &queue));
    assert!(!prewarm_active_preview(&source_text, &cache, &queue));

    let job = queue.try_pop().expect("preview job should be queued");
    assert_eq!(job.kind, MediaJobKind::Preview);
    assert_eq!(job.priority, MediaPriority::ActivePreview);
    assert_eq!(
        job.needs,
        MediaNeeds {
            thumbnail: true,
            preview: true,
        }
    );
    assert_eq!(job.path, source_text);
    queue.finish(&job);
    assert!(queue.try_pop().is_none());

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn speculative_prewarm_skips_hits_and_queues_only_two_cold_candidates() {
    let root = unique_temp_dir("speculative-prewarm");
    let first = root.join("first.jpg");
    let cached = root.join("cached.jpg");
    let second = root.join("second.jpg");
    let third = root.join("third.jpg");
    for source in [&first, &cached, &second, &third] {
        write_source(source);
    }
    let cache = ThumbnailCache::new(&root).expect("cache should initialize");
    let cached_text = cached.to_string_lossy().to_string();
    std::fs::write(cache.preview_path(&cached_text), b"cached")
        .expect("preview cache should be written");
    let candidates = vec![
        first.to_string_lossy().to_string(),
        cached_text,
        second.to_string_lossy().to_string(),
        third.to_string_lossy().to_string(),
    ];
    let queue = MediaJobQueue::new();

    assert_eq!(prewarm_speculative_previews(&candidates, &cache, &queue), 2);

    let first_job = queue.try_pop().expect("first candidate should be queued");
    let second_job = queue.try_pop().expect("second candidate should be queued");
    assert_eq!(first_job.path, candidates[0]);
    assert_eq!(first_job.priority, MediaPriority::SpeculativePreview);
    assert_eq!(second_job.path, candidates[2]);
    assert_eq!(second_job.priority, MediaPriority::SpeculativePreview);
    assert!(queue.try_pop().is_none());

    let _ = std::fs::remove_dir_all(root);
}
