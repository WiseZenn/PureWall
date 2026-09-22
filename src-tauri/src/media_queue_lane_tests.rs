use crate::media_queue::{MediaJobKind, MediaJobQueue};

#[test]
fn preview_consumer_never_claims_thumbnail_only_work() {
    let queue = MediaJobQueue::new();
    assert!(queue.enqueue(MediaJobKind::Thumbnail, "thumb".to_string()));

    assert!(queue.try_pop_preview().is_none());
    assert_eq!(
        queue
            .try_pop()
            .expect("general worker should get thumbnail")
            .kind,
        MediaJobKind::Thumbnail
    );
}

#[test]
fn preview_consumer_claims_preview_before_general_thumbnail_work() {
    let queue = MediaJobQueue::new();
    assert!(queue.enqueue(MediaJobKind::Thumbnail, "thumb".to_string()));
    assert!(queue.enqueue(MediaJobKind::Preview, "preview".to_string()));

    let preview = queue
        .try_pop_preview()
        .expect("reserved worker should get preview");
    assert_eq!(preview.kind, MediaJobKind::Preview);
    assert_eq!(preview.path, "preview");
    assert_eq!(
        queue
            .try_pop()
            .expect("thumbnail should remain queued")
            .kind,
        MediaJobKind::Thumbnail
    );
}
