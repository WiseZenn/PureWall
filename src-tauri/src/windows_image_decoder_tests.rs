use image::GenericImageView;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_image() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("purewall-wic-{}-{nanos}.jpg", std::process::id()))
}

#[test]
fn wic_decodes_directly_to_the_requested_bounds() {
    let path = unique_temp_image();
    image::RgbImage::new(2400, 1200)
        .save(&path)
        .expect("test image should be saved");

    let decoded = crate::windows_image_decoder::decode_target(
        path.to_str().expect("temp path should be utf-8"),
        600,
    )
    .expect("WIC should decode the JPEG");

    assert_eq!(decoded.dimensions(), (600, 300));
    let _ = std::fs::remove_file(path);
}
