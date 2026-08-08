use crate::image_decoder::target_dimensions;
use crate::media_queue::MediaNeeds;
use crate::thumbnails::ThumbnailCache;
use image::GenericImageView;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

fn unique_temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "purewall-preview-perf-{}-{nanos}",
        std::process::id()
    ))
}

fn generate_with_image_fallback(source_path: &Path, output_path: &Path) -> usize {
    let image = image::open(source_path).expect("fallback source should decode");
    let (target_width, target_height) = target_dimensions(image.width(), image.height(), 1440);
    let resized = if image.dimensions() == (target_width, target_height) {
        image
    } else {
        image.resize_exact(
            target_width,
            target_height,
            image::imageops::FilterType::Lanczos3,
        )
    };
    let mut output = Cursor::new(Vec::new());
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, 92);
    resized
        .write_with_encoder(encoder)
        .expect("fallback preview should encode");
    let bytes = output.into_inner();
    std::fs::write(output_path, &bytes).expect("fallback preview should write");
    bytes.len()
}

#[test]
#[ignore = "manual release-mode benchmark; set PUREWALL_PERF_IMAGES with | separated paths"]
fn report_real_preview_pipeline_timings() {
    let raw_paths = std::env::var("PUREWALL_PERF_IMAGES")
        .expect("PUREWALL_PERF_IMAGES must contain | separated image paths");
    let paths: Vec<PathBuf> = raw_paths
        .split('|')
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
        .collect();
    assert!(
        !paths.is_empty(),
        "at least one performance sample is required"
    );

    for source_path in paths {
        assert!(
            source_path.is_file(),
            "sample must exist: {}",
            source_path.display()
        );
        let (width, height) =
            image::image_dimensions(&source_path).expect("sample dimensions should be readable");
        let source_bytes = std::fs::metadata(&source_path)
            .expect("sample metadata should be readable")
            .len();
        let root = unique_temp_dir();
        let cache = ThumbnailCache::new(&root).expect("benchmark cache should initialize");
        let source = source_path.to_string_lossy().to_string();

        let cold_started = Instant::now();
        let cold = cache.generate_derivatives(
            &source,
            MediaNeeds {
                thumbnail: false,
                preview: true,
            },
        );
        let cold_elapsed = cold_started.elapsed();
        let cold_path = cold.preview.expect("cold WIC preview should be generated");
        let output_bytes = std::fs::metadata(&cold_path)
            .expect("cold preview metadata should exist")
            .len();

        let hit_started = Instant::now();
        let hit = cache.generate_derivatives(
            &source,
            MediaNeeds {
                thumbnail: false,
                preview: true,
            },
        );
        let hit_elapsed = hit_started.elapsed();
        assert!(hit.preview.is_some(), "cache hit should return the preview");

        let fallback_path = root.join("forced-image-fallback.jpg");
        let fallback_started = Instant::now();
        let fallback_bytes = generate_with_image_fallback(&source_path, &fallback_path);
        let fallback_elapsed = fallback_started.elapsed();

        println!(
            "PUREWALL_PERF\t{}\t{}x{}\t{:.1}MP\t{:.1}MiB source\t{:.1}ms cold-wic\t{:.3}ms cache-hit\t{:.1}ms forced-image\t{:.1}KiB preview\t{:.1}KiB fallback",
            source_path.display(),
            width,
            height,
            width as f64 * height as f64 / 1_000_000.0,
            source_bytes as f64 / 1_048_576.0,
            cold_elapsed.as_secs_f64() * 1000.0,
            hit_elapsed.as_secs_f64() * 1000.0,
            fallback_elapsed.as_secs_f64() * 1000.0,
            output_bytes as f64 / 1024.0,
            fallback_bytes as f64 / 1024.0,
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
