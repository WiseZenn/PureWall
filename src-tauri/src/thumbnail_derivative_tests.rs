use crate::media_queue::MediaNeeds;
use crate::thumbnails::ThumbnailCache;
use image::GenericImageView;
use std::cell::Cell;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "purewall-derivatives-{}-{nanos}",
        std::process::id()
    ))
}

#[test]
fn combined_derivatives_decode_the_source_once() {
    let root = unique_temp_dir();
    let cache = ThumbnailCache::new(&root).expect("cache should initialize");
    let decode_count = Cell::new(0);
    let result = cache.generate_derivatives_with_loader(
        "D:/walls/combined.jpg",
        MediaNeeds {
            thumbnail: true,
            preview: true,
        },
        |_| {
            decode_count.set(decode_count.get() + 1);
            Some(image::DynamicImage::ImageRgb8(image::RgbImage::new(
                2400, 1200,
            )))
        },
    );

    assert_eq!(decode_count.get(), 1);
    let thumbnail = result.thumbnail.expect("thumbnail should be generated");
    let preview = result.preview.expect("preview should be generated");
    assert_eq!(
        image::open(thumbnail)
            .expect("thumbnail should decode")
            .dimensions(),
        (512, 256)
    );
    assert_eq!(
        image::open(preview)
            .expect("preview should decode")
            .dimensions(),
        (1440, 720)
    );

    let _ = std::fs::remove_dir_all(root);
}
