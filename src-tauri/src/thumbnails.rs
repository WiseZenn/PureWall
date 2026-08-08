use crate::media_queue::MediaNeeds;
use anyhow::Result;
use std::collections::{hash_map::DefaultHasher, HashSet};
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const THUMBNAIL_MAX_DIMENSION: u32 = 512;
const THUMBNAIL_JPEG_QUALITY: u8 = 88;
const THUMBNAIL_CACHE_PROFILE: &str = "thumb-512-catmullrom-q88";
const PREVIEW_MAX_DIMENSION: u32 = 1440;
const PREVIEW_JPEG_QUALITY: u8 = 92;
const PREVIEW_CACHE_PROFILE: &str = "preview-1440-lanczos-q92";
const LEGACY_PREVIEW_CACHE_PROFILES: &[&str] =
    &["preview-960-lanczos-q82", "preview-960-catmullrom-q82"];

/// Thumbnail disk cache manager.
/// Caches generated derivatives as JPEG files under app_data_dir/thumbnails/.
#[derive(Clone)]
pub struct ThumbnailCache {
    cache_dir: PathBuf,
}

#[derive(Default)]
pub struct DerivativePaths {
    pub thumbnail: Option<PathBuf>,
    pub preview: Option<PathBuf>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThumbnailCacheFreshness {
    Current,
    Stale,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThumbnailCacheHit {
    pub path: PathBuf,
    pub freshness: ThumbnailCacheFreshness,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreviewCacheFreshness {
    Current,
    Stale,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreviewCacheHit {
    pub path: PathBuf,
    pub freshness: PreviewCacheFreshness,
}

impl ThumbnailCache {
    pub fn new(app_data_dir: &Path) -> Result<Self> {
        let cache_dir = app_data_dir.join("thumbnails");
        std::fs::create_dir_all(&cache_dir)?;
        Ok(Self { cache_dir })
    }

    fn cache_identity(path: &str, profile: &str) -> String {
        let mut input = format!("{}|{}", profile, path);
        if let Ok(metadata) = std::fs::metadata(path) {
            use std::fmt::Write;
            let _ = write!(input, "|{}", metadata.len());
            if let Ok(modified) = metadata.modified() {
                if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
                    let _ = write!(input, "|{}.{}", duration.as_secs(), duration.subsec_nanos());
                }
            }
        }
        input
    }

    /// Get disk cache path for a source image.
    /// Uses `stable_fingerprint` (FNV-1a 128-bit) instead of `DefaultHasher`
    /// so cache keys stay consistent across Rust version upgrades (CRT-01).
    fn cache_key(path: &str, profile: &str, extension: &str) -> String {
        let input = Self::cache_identity(path, profile);
        format!(
            "{:016x}{}",
            crate::scanner::stable_fingerprint(&input),
            extension
        )
    }

    fn legacy_cache_key(path: &str, profile: &str, extension: &str) -> String {
        let input = Self::cache_identity(path, profile);
        let mut hasher = DefaultHasher::new();
        input.hash(&mut hasher);
        format!("{:016x}{}", hasher.finish(), extension)
    }
    fn original_legacy_cache_key(path: &str, extension: &str) -> String {
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        format!("{:016x}{}", hasher.finish(), extension)
    }

    pub fn thumbnail_path(&self, source_path: &str) -> PathBuf {
        self.cache_dir.join(Self::cache_key(
            source_path,
            THUMBNAIL_CACHE_PROFILE,
            ".jpg",
        ))
    }

    fn legacy_thumbnail_path(&self, source_path: &str) -> PathBuf {
        self.cache_dir.join(Self::legacy_cache_key(
            source_path,
            THUMBNAIL_CACHE_PROFILE,
            ".jpg",
        ))
    }
    fn original_legacy_thumbnail_path(&self, source_path: &str) -> PathBuf {
        self.cache_dir
            .join(Self::original_legacy_cache_key(source_path, ".jpg"))
    }

    fn preview_path_for_profile(&self, source_path: &str, profile: &str) -> PathBuf {
        self.cache_dir
            .join(Self::cache_key(source_path, profile, ".preview.jpg"))
    }

    pub fn preview_path(&self, source_path: &str) -> PathBuf {
        self.preview_path_for_profile(source_path, PREVIEW_CACHE_PROFILE)
    }

    /// Return an existing thumbnail cache file without generating a missing one.
    pub fn cached_path(&self, source_path: &str) -> Option<PathBuf> {
        let cache_path = self.thumbnail_path(source_path);
        if cache_path.exists() {
            touch_cache_file(&cache_path);
            Some(cache_path)
        } else {
            None
        }
    }

    /// Return the current thumbnail when available, otherwise reuse the last
    /// DefaultHasher-keyed derivative while the stable-key version is rebuilt.
    pub fn lookup_thumbnail_path(&self, source_path: &str) -> Option<ThumbnailCacheHit> {
        let current_path = self.thumbnail_path(source_path);
        if current_path.exists() {
            touch_cache_file(&current_path);
            return Some(ThumbnailCacheHit {
                path: current_path,
                freshness: ThumbnailCacheFreshness::Current,
            });
        }

        let legacy_path = self.legacy_thumbnail_path(source_path);
        if legacy_path.exists() {
            touch_cache_file(&legacy_path);
            return Some(ThumbnailCacheHit {
                path: legacy_path,
                freshness: ThumbnailCacheFreshness::Stale,
            });
        }

        let original_legacy_path = self.original_legacy_thumbnail_path(source_path);
        if original_legacy_path.exists() {
            touch_cache_file(&original_legacy_path);
            return Some(ThumbnailCacheHit {
                path: original_legacy_path,
                freshness: ThumbnailCacheFreshness::Stale,
            });
        }

        None
    }

    /// Get thumbnail cache file path, from cache or freshly generated.
    pub fn get_path(&self, source_path: &str) -> Option<PathBuf> {
        self.cached_path(source_path)
            .or_else(|| self.generate_path(source_path))
    }

    /// Generate a thumbnail cache file and return its path.
    pub fn generate_path(&self, source_path: &str) -> Option<PathBuf> {
        let cache_path = self.thumbnail_path(source_path);
        if cache_path.exists() {
            return Some(cache_path);
        }

        let jpeg_bytes = generate_thumbnail(source_path)?;
        write_derivative_atomic(&cache_path, &jpeg_bytes).ok()?;

        Some(cache_path)
    }

    /// Return a cached preview path without generating a missing one.
    /// Used by the IPC command so the response is always fast.
    pub fn cached_preview_path(&self, source_path: &str) -> Option<PathBuf> {
        let cache_path = self.preview_path(source_path);
        if cache_path.exists() {
            touch_cache_file(&cache_path);
            Some(cache_path)
        } else {
            None
        }
    }

    pub fn lookup_preview_path(&self, source_path: &str) -> Option<PreviewCacheHit> {
        let current_path = self.preview_path(source_path);
        if current_path.exists() {
            touch_cache_file(&current_path);
            return Some(PreviewCacheHit {
                path: current_path,
                freshness: PreviewCacheFreshness::Current,
            });
        }

        for profile in LEGACY_PREVIEW_CACHE_PROFILES {
            let legacy_path = self.preview_path_for_profile(source_path, profile);
            if legacy_path.exists() {
                touch_cache_file(&legacy_path);
                return Some(PreviewCacheHit {
                    path: legacy_path,
                    freshness: PreviewCacheFreshness::Stale,
                });
            }
        }

        None
    }

    /// Get a higher-quality preview cache file path, generating if missing.
    /// Use this on background threads only — it performs synchronous image decode/resize/encode.
    pub fn get_preview_path(&self, source_path: &str) -> Option<PathBuf> {
        self.cached_preview_path(source_path).or_else(|| {
            let jpeg_bytes = generate_preview(source_path)?;
            let cache_path = self.preview_path(source_path);
            write_derivative_atomic(&cache_path, &jpeg_bytes).ok()?;
            Some(cache_path)
        })
    }

    pub fn generate_derivatives(&self, source_path: &str, needs: MediaNeeds) -> DerivativePaths {
        let max_dimension = if needs.preview {
            PREVIEW_MAX_DIMENSION
        } else {
            THUMBNAIL_MAX_DIMENSION
        };
        self.generate_derivatives_with_loader(source_path, needs, |path| {
            crate::image_decoder::decode_target_image(path, max_dimension)
                .map(|(image, _backend)| image)
        })
    }

    pub(crate) fn generate_derivatives_with_loader<F>(
        &self,
        source_path: &str,
        needs: MediaNeeds,
        loader: F,
    ) -> DerivativePaths
    where
        F: FnOnce(&str) -> Option<image::DynamicImage>,
    {
        let mut result = DerivativePaths {
            thumbnail: if needs.thumbnail {
                self.cached_path(source_path)
            } else {
                None
            },
            preview: if needs.preview {
                self.cached_preview_path(source_path)
            } else {
                None
            },
        };
        let thumbnail_missing = needs.thumbnail && result.thumbnail.is_none();
        let preview_missing = needs.preview && result.preview.is_none();
        if !thumbnail_missing && !preview_missing {
            return result;
        }

        let Some(image) = loader(source_path) else {
            return result;
        };

        if preview_missing {
            if let Some(bytes) = encode_resized_jpeg(
                &image,
                PREVIEW_MAX_DIMENSION,
                PREVIEW_JPEG_QUALITY,
                image::imageops::FilterType::Lanczos3,
            ) {
                let path = self.preview_path(source_path);
                if write_derivative_atomic(&path, &bytes).is_ok() {
                    result.preview = Some(path);
                }
            }
        }
        if thumbnail_missing {
            if let Some(bytes) = encode_resized_jpeg(
                &image,
                THUMBNAIL_MAX_DIMENSION,
                THUMBNAIL_JPEG_QUALITY,
                image::imageops::FilterType::CatmullRom,
            ) {
                let path = self.thumbnail_path(source_path);
                if write_derivative_atomic(&path, &bytes).is_ok() {
                    result.thumbnail = Some(path);
                }
            }
        }

        result
    }

    /// Remove the oldest cache files when the total exceeds `max_files`.
    /// Uses file modification time as a proxy for recency — files that haven't
    /// been touched recently (because their source wallpaper was deleted or
    /// changed) are removed first.
    pub fn cleanup_with_protected(
        &self,
        max_files: usize,
        protected_source_paths: &[String],
    ) -> Result<usize> {
        let protected_paths: HashSet<PathBuf> = protected_source_paths
            .iter()
            .flat_map(|source_path| {
                let mut paths = vec![
                    self.thumbnail_path(source_path),
                    self.preview_path(source_path),
                ];
                paths.extend(
                    LEGACY_PREVIEW_CACHE_PROFILES
                        .iter()
                        .map(|profile| self.preview_path_for_profile(source_path, profile)),
                );
                paths
            })
            .collect();
        let mut entries: Vec<(PathBuf, std::time::SystemTime)> =
            std::fs::read_dir(&self.cache_dir)?
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.path().is_file())
                .filter_map(|entry| {
                    let metadata = entry.metadata().ok()?;
                    let modified = metadata.modified().ok()?;
                    Some((entry.path(), modified))
                })
                .collect();

        if entries.len() <= max_files {
            return Ok(0);
        }

        entries.sort_by_key(|(_, mtime)| *mtime);

        let to_remove = entries.len() - max_files;
        let mut removed = 0;
        for (path, _) in entries {
            if removed >= to_remove {
                break;
            }
            if protected_paths.contains(&path) {
                continue;
            }
            if std::fs::remove_file(path).is_ok() {
                removed += 1;
            }
        }

        Ok(removed)
    }
}

/// Generate a 512px JPEG thumbnail from an image file.
fn generate_thumbnail(source_path: &str) -> Option<Vec<u8>> {
    generate_resized_jpeg(
        source_path,
        THUMBNAIL_MAX_DIMENSION,
        THUMBNAIL_JPEG_QUALITY,
        image::imageops::FilterType::CatmullRom,
    )
}

/// Generate a 1440px JPEG preview from an image file.
fn generate_preview(source_path: &str) -> Option<Vec<u8>> {
    generate_resized_jpeg(
        source_path,
        PREVIEW_MAX_DIMENSION,
        PREVIEW_JPEG_QUALITY,
        image::imageops::FilterType::Lanczos3,
    )
}

fn generate_resized_jpeg(
    source_path: &str,
    max_width: u32,
    quality: u8,
    filter: image::imageops::FilterType,
) -> Option<Vec<u8>> {
    let (image, _backend) = crate::image_decoder::decode_target_image(source_path, max_width)?;
    encode_resized_jpeg(&image, max_width, quality, filter)
}

fn encode_resized_jpeg(
    image: &image::DynamicImage,
    max_width: u32,
    quality: u8,
    filter: image::imageops::FilterType,
) -> Option<Vec<u8>> {
    let resized = if image.width() > max_width || image.height() > max_width {
        image.resize(max_width, max_width, filter)
    } else {
        image.clone()
    };

    let mut buf = Cursor::new(Vec::new());
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality);
    resized.write_with_encoder(encoder).ok()?;

    Some(buf.into_inner())
}

fn touch_cache_file(cache_path: &Path) {
    let Ok(file) = std::fs::OpenOptions::new().write(true).open(cache_path) else {
        return;
    };
    let times = std::fs::FileTimes::new().set_modified(SystemTime::now());
    let _ = file.set_times(times);
}

pub(crate) fn write_derivative_atomic(cache_path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let temp_path = cache_path.with_extension("tmp");
    std::fs::write(&temp_path, bytes)?;
    if std::fs::rename(&temp_path, cache_path).is_err() {
        let _ = std::fs::remove_file(cache_path);
        std::fs::rename(&temp_path, cache_path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    fn unique_temp_image_path(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "purewall-{}-{}-{}.jpg",
            name,
            std::process::id(),
            nanos
        ))
    }

    fn write_test_image(path: &Path, width: u32, height: u32) {
        let image = image::RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x % 255) as u8, (y % 255) as u8, ((x + y) % 255) as u8])
        });
        image.save(path).expect("test image should be saved");
    }

    #[test]
    fn thumbnail_generation_caps_both_dimensions() {
        let path = unique_temp_image_path("thumbnail");
        write_test_image(&path, 2400, 1200);

        let bytes = generate_thumbnail(path.to_str().expect("temp path should be utf-8"))
            .expect("thumbnail should be generated");
        let decoded = image::load_from_memory(&bytes).expect("thumbnail should decode");

        assert_eq!(decoded.dimensions(), (THUMBNAIL_MAX_DIMENSION, 256));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn preview_generation_caps_portrait_height() {
        let path = unique_temp_image_path("preview");
        write_test_image(&path, 1080, 1920);

        let bytes = generate_preview(path.to_str().expect("temp path should be utf-8"))
            .expect("preview should be generated");
        let decoded = image::load_from_memory(&bytes).expect("preview should decode");

        assert_eq!(decoded.dimensions(), (810, PREVIEW_MAX_DIMENSION));
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn cached_thumbnail_hit_refreshes_cache_file_mtime() {
        let source_path = unique_temp_image_path("cache-hit-source");
        let nanos = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let cache_root =
            std::env::temp_dir().join(format!("purewall-cache-hit-{}-{nanos}", std::process::id()));
        let cache = ThumbnailCache::new(&cache_root).expect("cache should initialize");
        write_test_image(&source_path, 320, 180);

        let cache_path = cache.thumbnail_path(source_path.to_str().expect("source path utf-8"));
        std::fs::write(&cache_path, b"cached").expect("cache file should be writable");
        let before = std::fs::metadata(&cache_path)
            .expect("cache metadata should exist")
            .modified()
            .expect("cache mtime should exist");
        std::thread::sleep(std::time::Duration::from_millis(1100));

        let hit = cache
            .cached_path(source_path.to_str().expect("source path utf-8"))
            .expect("cache should hit");
        let after = std::fs::metadata(&hit)
            .expect("cache metadata should still exist")
            .modified()
            .expect("cache mtime should still exist");

        assert!(after > before);

        let _ = std::fs::remove_file(source_path);
        let _ = std::fs::remove_dir_all(cache_root);
    }

    #[test]
    fn preview_lookup_prefers_current_format_and_accepts_a_legacy_fallback() {
        let source_path = unique_temp_image_path("legacy-preview-source");
        let cache_root = unique_temp_image_path("legacy-preview-cache");
        let cache = ThumbnailCache::new(&cache_root).expect("cache should initialize");
        write_test_image(&source_path, 640, 360);
        let source = source_path.to_str().expect("source path utf-8");

        let legacy_path = cache.preview_path_for_profile(source, LEGACY_PREVIEW_CACHE_PROFILES[0]);
        std::fs::write(&legacy_path, b"legacy-preview").expect("legacy preview should be writable");

        let stale = cache
            .lookup_preview_path(source)
            .expect("legacy preview should be a compatible fallback");
        assert_eq!(stale.path, legacy_path);
        assert_eq!(stale.freshness, PreviewCacheFreshness::Stale);

        let current_path = cache.preview_path(source);
        std::fs::write(&current_path, b"current-preview")
            .expect("current preview should be writable");
        let current = cache
            .lookup_preview_path(source)
            .expect("current preview should win");
        assert_eq!(current.path, current_path);
        assert_eq!(current.freshness, PreviewCacheFreshness::Current);

        let _ = std::fs::remove_file(source_path);
        let _ = std::fs::remove_dir_all(cache_root);
    }

    #[test]
    fn cleanup_never_removes_protected_current_or_active_derivatives() {
        let source_path = unique_temp_image_path("protected-cache-source");
        let cache_root = unique_temp_image_path("protected-cache-root");
        let cache = ThumbnailCache::new(&cache_root).expect("cache should initialize");
        write_test_image(&source_path, 640, 360);
        let source = source_path.to_str().expect("source path utf-8").to_string();

        let protected_thumbnail = cache.thumbnail_path(&source);
        let protected_preview = cache.preview_path(&source);
        let disposable = cache.cache_dir.join("disposable.jpg");
        std::fs::write(&protected_thumbnail, b"thumbnail")
            .expect("protected thumbnail should be writable");
        std::fs::write(&protected_preview, b"preview")
            .expect("protected preview should be writable");
        std::fs::write(&disposable, b"disposable").expect("disposable cache should be writable");

        let removed = cache
            .cleanup_with_protected(1, std::slice::from_ref(&source))
            .expect("cleanup should succeed");

        assert_eq!(removed, 1);
        assert!(protected_thumbnail.exists());
        assert!(protected_preview.exists());
        assert!(!disposable.exists());

        let _ = std::fs::remove_file(source_path);
        let _ = std::fs::remove_dir_all(cache_root);
    }
}
