use crate::media_queue::{MediaJobKind, MediaJobQueue, MediaNeeds};

#[test]
fn queued_thumbnail_is_upgraded_to_one_preview_job_for_the_same_path() {
    let queue = MediaJobQueue::new();
    assert!(queue.enqueue(MediaJobKind::Thumbnail, "same".to_string()));
    assert!(queue.enqueue(MediaJobKind::Preview, "same".to_string()));

    let job = queue.try_pop().expect("upgraded job should be ready");
    assert_eq!(job.kind, MediaJobKind::Preview);
    assert_eq!(job.path, "same");
    assert_eq!(
        job.needs,
        MediaNeeds {
            thumbnail: true,
            preview: true,
        }
    );
    assert!(queue.try_pop().is_none());
}

#[test]
fn running_path_collects_new_needs_without_concurrent_decode() {
    let queue = MediaJobQueue::new();
    assert!(queue.enqueue(MediaJobKind::Thumbnail, "same".to_string()));
    let thumbnail_job = queue.try_pop().expect("thumbnail should start");

    assert!(queue.enqueue(MediaJobKind::Preview, "same".to_string()));
    assert!(queue.try_pop().is_none());
    assert!(queue.finish(&thumbnail_job));

    let preview_job = queue.try_pop().expect("pending preview should follow");
    assert_eq!(preview_job.kind, MediaJobKind::Preview);
    assert_eq!(
        preview_job.needs,
        MediaNeeds {
            thumbnail: false,
            preview: true,
        }
    );
}

#[test]
fn repeated_need_for_a_queued_or_running_path_is_idempotent() {
    let queue = MediaJobQueue::new();
    assert!(queue.enqueue(MediaJobKind::Preview, "same".to_string()));
    assert!(!queue.enqueue(MediaJobKind::Preview, "same".to_string()));

    let job = queue.try_pop().expect("preview should start");
    assert!(!queue.enqueue(MediaJobKind::Preview, "same".to_string()));
    assert!(!queue.finish(&job));
}
