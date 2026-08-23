use crate::watcher_queue::{PathSignal, Signal};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, Prefix};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    pub path: String,
    pub hash: String,
    pub width: u32,
    pub height: u32,
    pub file_size: u64,
}

const SUPPORTED_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "bmp", "webp"];
pub const MAX_SCAN_DEPTH: usize = 24;
pub const MAX_SCAN_IMAGES: usize = 20_000;
pub const MAX_SCAN_ENTRIES: usize = 100_000;

pub struct FolderWatcher {
    _watcher: notify::RecommendedWatcher,
}

impl Drop for FolderWatcher {
    fn drop(&mut self) {}
}

/// Recursively scan a folder for image files
pub fn scan_folder(folder_path: &str) -> Result<Vec<ImageInfo>> {
    scan_folder_with_root_inspection(folder_path, inspect_scan_root)
}

struct ScanRootMetadata {
    is_directory: bool,
    is_symlink: bool,
    is_reparse_point: bool,
}

fn inspect_scan_root(path: &Path) -> Result<ScanRootMetadata> {
    let metadata = std::fs::symlink_metadata(path).context("Failed to inspect folder")?;
    #[cfg(windows)]
    let is_reparse_point = {
        use std::os::windows::fs::MetadataExt;
        windows_attributes_are_reparse_point(metadata.file_attributes())
    };
    #[cfg(not(windows))]
    let is_reparse_point = false;

    Ok(ScanRootMetadata {
        is_directory: metadata.is_dir(),
        is_symlink: metadata.file_type().is_symlink(),
        is_reparse_point,
    })
}

pub(crate) fn ensure_scan_root(path: &Path) -> Result<()> {
    ensure_local_path(path)?;
    validate_scan_root_metadata(path, inspect_scan_root(path)?)
}

fn validate_scan_root_metadata(path: &Path, metadata: ScanRootMetadata) -> Result<()> {
    if metadata.is_symlink || metadata.is_reparse_point {
        anyhow::bail!(
            "Folder root cannot be a symbolic link or reparse point: {}",
            path.display()
        );
    }
    if !metadata.is_directory {
        anyhow::bail!(
            "Folder does not exist or is not a directory: {}",
            path.display()
        );
    }
    Ok(())
}

fn scan_folder_with_root_inspection<F>(folder_path: &str, inspect_root: F) -> Result<Vec<ImageInfo>>
where
    F: FnMut(&Path) -> Result<ScanRootMetadata>,
{
    scan_folder_with_root_inspection_and_canonicalization(folder_path, inspect_root, |path| {
        std::fs::canonicalize(path)
    })
}

fn scan_folder_with_root_inspection_and_canonicalization<F, C>(
    folder_path: &str,
    mut inspect_root: F,
    canonicalize_root: C,
) -> Result<Vec<ImageInfo>>
where
    F: FnMut(&Path) -> Result<ScanRootMetadata>,
    C: FnOnce(&Path) -> std::io::Result<std::path::PathBuf>,
{
    let root = Path::new(folder_path);
    ensure_local_path(root)?;
    validate_scan_root_metadata(root, inspect_root(root)?)?;
    let canonical_root = canonicalize_root(root).context("Failed to resolve folder path")?;
    ensure_local_path(&canonical_root)?;
    validate_scan_root_metadata(&canonical_root, inspect_root(&canonical_root)?)?;

    let mut images = Vec::new();
    let mut budget = ScanBudget::default();
    collect_images(&canonical_root, &mut images, 0, &mut budget)?;

    // Sort by file name for consistent ordering
    images.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(images)
}

/// Scan an explicit list of files for supported image entries
pub fn scan_files(paths: &[String]) -> Result<Vec<ImageInfo>> {
    if paths.len() > MAX_SCAN_IMAGES {
        anyhow::bail!(
            "Too many files selected. PureWall can import up to {} images at once.",
            MAX_SCAN_IMAGES
        );
    }

    for path in paths {
        ensure_local_path(Path::new(path))?;
    }

    let mut images = paths
        .iter()
        .map(Path::new)
        .filter(|path| path.is_file() && is_supported_image(path))
        .filter_map(get_image_info)
        .collect::<Vec<_>>();

    images.sort_by(|a, b| a.path.cmp(&b.path));
    images.dedup_by(|a, b| a.path == b.path);
    Ok(images)
}

pub fn image_info(path: &str) -> Option<ImageInfo> {
    let path = Path::new(path);
    if ensure_local_path(path).is_ok() && path.is_file() && is_supported_image(path) {
        get_image_info(path)
    } else {
        None
    }
}

#[derive(Default)]
struct ScanBudget {
    entries_seen: usize,
}

struct DirectoryEntryInspection {
    file_type: std::fs::FileType,
    metadata: Option<std::fs::Metadata>,
    excluded: bool,
}

fn collect_images(
    dir: &Path,
    images: &mut Vec<ImageInfo>,
    depth: usize,
    budget: &mut ScanBudget,
) -> Result<()> {
    if depth > MAX_SCAN_DEPTH {
        anyhow::bail!(
            "Folder is too deeply nested. PureWall scans up to {} levels deep.",
            MAX_SCAN_DEPTH
        );
    }

    if dir.is_dir() {
        for entry in std::fs::read_dir(dir).context("Failed to read directory")? {
            let entry = entry.context("Failed to read directory entry")?;
            let inspection = inspect_directory_entry(&entry)?;
            if inspection.excluded {
                continue;
            }

            budget.entries_seen += 1;
            if budget.entries_seen > MAX_SCAN_ENTRIES {
                anyhow::bail!(
                    "Folder is too large to scan safely. PureWall scans up to {} directory entries at once.",
                    MAX_SCAN_ENTRIES
                );
            }

            let path = entry.path();

            if inspection.file_type.is_dir() {
                collect_images(&path, images, depth + 1, budget)?;
            } else if inspection.file_type.is_file() && is_supported_image(&path) {
                if images.len() >= MAX_SCAN_IMAGES {
                    anyhow::bail!(
                        "Too many images found. PureWall scans up to {} images at once.",
                        MAX_SCAN_IMAGES
                    );
                }
                if let Some(info) = get_scanned_image_info(&path, inspection.metadata.as_ref()) {
                    images.push(info);
                }
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
fn inspect_directory_entry(entry: &std::fs::DirEntry) -> Result<DirectoryEntryInspection> {
    use std::os::windows::fs::MetadataExt;

    let metadata = std::fs::symlink_metadata(entry.path())
        .context("Failed to inspect directory entry attributes")?;
    let file_type = metadata.file_type();
    let excluded =
        file_type.is_symlink() || windows_attributes_are_reparse_point(metadata.file_attributes());
    Ok(DirectoryEntryInspection {
        file_type,
        metadata: Some(metadata),
        excluded,
    })
}

#[cfg(not(windows))]
fn inspect_directory_entry(entry: &std::fs::DirEntry) -> Result<DirectoryEntryInspection> {
    let file_type = entry
        .file_type()
        .context("Failed to inspect directory entry type")?;
    let excluded = file_type.is_symlink();
    Ok(DirectoryEntryInspection {
        file_type,
        metadata: None,
        excluded,
    })
}

#[cfg(windows)]
fn windows_attributes_are_reparse_point(attributes: u32) -> bool {
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

pub(crate) fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| SUPPORTED_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

pub(crate) fn ensure_local_path(path: &Path) -> Result<()> {
    if is_unc_path(path) {
        anyhow::bail!(
            "Network paths are not supported for wallpaper libraries: {}",
            path.display()
        );
    }

    #[cfg(windows)]
    reject_remote_drive(path)?;

    Ok(())
}

fn is_unc_path(path: &Path) -> bool {
    matches!(
        path.components().next(),
        Some(Component::Prefix(prefix))
            if matches!(prefix.kind(), Prefix::UNC(_, _) | Prefix::VerbatimUNC(_, _))
    )
}

#[cfg(windows)]
fn reject_remote_drive(path: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Prefix;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetDriveTypeW;

    const DRIVE_REMOTE: u32 = 4;

    let Some(Component::Prefix(prefix)) = path.components().next() else {
        return Ok(());
    };

    let drive_letter = match prefix.kind() {
        Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => letter as char,
        _ => return Ok(()),
    };

    let root = format!("{}:\\", drive_letter);
    let wide = std::ffi::OsStr::new(&root)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();

    if unsafe { GetDriveTypeW(PCWSTR(wide.as_ptr())) } == DRIVE_REMOTE {
        anyhow::bail!(
            "Network drives are not supported for wallpaper libraries: {}",
            path.display()
        );
    }

    Ok(())
}

fn get_image_info(path: &Path) -> Option<ImageInfo> {
    // Canonicalize the path before storing so validation later matches (HIG-06).
    let stored_path = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string();
    get_image_info_with_stored_path(path, stored_path)
}

fn get_scanned_image_info(
    path: &Path,
    verified_metadata: Option<&std::fs::Metadata>,
) -> Option<ImageInfo> {
    let stored_path = path.to_string_lossy().into_owned();
    match verified_metadata {
        Some(metadata) => build_image_info(path, stored_path, metadata),
        None => get_image_info_with_stored_path(path, stored_path),
    }
}

fn get_image_info_with_stored_path(path: &Path, stored_path: String) -> Option<ImageInfo> {
    let metadata = std::fs::metadata(path).ok()?;
    build_image_info(path, stored_path, &metadata)
}

fn build_image_info(
    path: &Path,
    stored_path: String,
    metadata: &std::fs::Metadata,
) -> Option<ImageInfo> {
    let file_size = metadata.len();

    // Calculate hash based on canonical path + size + modified time
    let modified = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let hash = format!(
        "{:x}",
        stable_fingerprint(&format!("{}:{}:{}", stored_path, file_size, modified))
    );

    // Try to get image dimensions
    let (width, height) = get_dimensions(path).unwrap_or((0, 0));

    Some(ImageInfo {
        path: stored_path,
        hash,
        width,
        height,
        file_size,
    })
}

fn get_dimensions(path: &Path) -> Option<(u32, u32)> {
    // Use image crate to get dimensions without fully decoding
    image::image_dimensions(path).ok()
}

/// Stable FNV-1a 128-bit fingerprint. Deterministic, fast, and produces the
/// same output across all Rust versions — unlike `DefaultHasher` whose output
/// is not guaranteed stable. Used for wallpaper identity and derivative cache
/// keys where cross-upgrade consistency matters.
pub(crate) fn stable_fingerprint(input: &str) -> u128 {
    const FNV_PRIME: u128 = 0x0000000001000000000000000000013B;
    const FNV_OFFSET: u128 = 0x6C62272E07BB014262B821756295C58D;

    let mut hash = FNV_OFFSET;
    for byte in input.bytes() {
        hash ^= byte as u128;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn classify_watcher_event(event: notify::Event) -> Vec<Signal> {
    use notify::event::ModifyKind;
    use notify::EventKind;

    if event.need_rescan() {
        return vec![Signal::NeedRescan];
    }

    let (scan_existing_directory, include_missing) = match event.kind {
        EventKind::Access(_) => return Vec::new(),
        EventKind::Create(_) => (true, false),
        EventKind::Remove(_) => (false, true),
        EventKind::Modify(ModifyKind::Name(_)) => (true, true),
        EventKind::Modify(_) | EventKind::Any | EventKind::Other => (false, false),
    };
    if event.paths.is_empty() {
        return Vec::new();
    }
    vec![Signal::Paths(
        event
            .paths
            .into_iter()
            .map(|path| PathSignal::new(path, scan_existing_directory, include_missing))
            .collect(),
    )]
}

/// Start watching a folder and deliver de-duplicated change batches after a short quiet period.
pub fn start_watcher<F>(folder_path: String, on_signal: F) -> Result<FolderWatcher>
where
    F: Fn(Signal) + Send + Sync + 'static,
{
    use notify::{recommended_watcher, RecursiveMode, Watcher};
    let folder = Path::new(&folder_path);
    ensure_local_path(folder)?;
    if !folder.is_dir() {
        anyhow::bail!(
            "Folder does not exist or is not a directory: {}",
            folder_path
        );
    }
    let mut watcher = recommended_watcher(move |res: notify::Result<notify::Event>| match res {
        Ok(event) => {
            for signal in classify_watcher_event(event) {
                on_signal(signal);
            }
        }
        Err(error) => on_signal(Signal::NotifyError(error.to_string())),
    })
    .context("Failed to create file watcher")?;
    watcher
        .watch(folder, RecursiveMode::Recursive)
        .context("Failed to start watching folder")?;
    Ok(FolderWatcher { _watcher: watcher })
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, DataChange, MetadataKind, ModifyKind, RemoveKind, RenameMode};
    use notify::{Event, EventKind};
    use std::cell::Cell;
    use std::path::{Path, PathBuf};

    struct TestDirectory(PathBuf);

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn unique_test_directory(name: &str) -> TestDirectory {
        let root = std::env::temp_dir().join(format!(
            "purewall-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("test root should be created");
        TestDirectory(root)
    }

    #[test]
    fn folder_scan_canonicalizes_the_root_once_and_preserves_canonical_image_paths() {
        let root = unique_test_directory("scan-root-canonicalization");
        let nested = root.0.join("nested");
        std::fs::create_dir_all(&nested).expect("nested test directory should be created");
        let first = root.0.join("first.png");
        let second = nested.join("second.png");
        image::RgbImage::new(2, 3)
            .save(&first)
            .expect("first test image should be written");
        image::RgbImage::new(4, 5)
            .save(&second)
            .expect("second test image should be written");

        let canonicalize_calls = Cell::new(0);
        let images = scan_folder_with_root_inspection_and_canonicalization(
            &root.0.to_string_lossy(),
            inspect_scan_root,
            |path| {
                canonicalize_calls.set(canonicalize_calls.get() + 1);
                std::fs::canonicalize(path)
            },
        )
        .expect("folder scan should succeed");

        assert_eq!(canonicalize_calls.get(), 1);
        let mut expected_paths = vec![
            std::fs::canonicalize(first)
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            std::fs::canonicalize(second)
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        ];
        expected_paths.sort();
        assert_eq!(
            images
                .iter()
                .map(|image| image.path.clone())
                .collect::<Vec<_>>(),
            expected_paths
        );
        assert_eq!(
            images
                .iter()
                .map(|image| (image.width, image.height))
                .collect::<Vec<_>>(),
            vec![(2, 3), (4, 5)]
        );
    }

    #[cfg(windows)]
    #[test]
    fn scanned_image_info_reuses_verified_entry_metadata() {
        let root = unique_test_directory("scan-entry-metadata");
        let image_path = root.0.join("metadata.png");
        image::RgbImage::new(3, 7)
            .save(&image_path)
            .expect("test image should be written");
        let canonical_path = std::fs::canonicalize(&image_path).unwrap();
        let stored_path = canonical_path.to_string_lossy().into_owned();
        let metadata = std::fs::symlink_metadata(&image_path).unwrap();
        let expected_size = metadata.len();
        std::fs::remove_file(&image_path).unwrap();

        let info = get_scanned_image_info(&canonical_path, Some(&metadata))
            .expect("verified entry metadata should avoid a second metadata lookup");

        assert_eq!(info.path, stored_path);
        assert_eq!(info.file_size, expected_size);
        assert_eq!((info.width, info.height), (0, 0));
    }

    #[test]
    fn scan_files_rejects_too_many_paths_before_touching_disk() {
        let paths = (0..=MAX_SCAN_IMAGES)
            .map(|i| format!(r"C:\purewall-test\{i}.jpg"))
            .collect::<Vec<_>>();

        let err = scan_files(&paths).expect_err("oversized imports must fail");
        assert!(
            err.to_string().contains("Too many files selected"),
            "unexpected error: {err}"
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_reparse_point_attribute_is_always_excluded_from_traversal() {
        const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

        assert!(!windows_attributes_are_reparse_point(0));
        assert!(!windows_attributes_are_reparse_point(
            FILE_ATTRIBUTE_DIRECTORY
        ));
        assert!(windows_attributes_are_reparse_point(
            FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT
        ));
    }

    #[cfg(windows)]
    #[test]
    fn scan_folder_entry_rejects_a_root_with_reparse_attributes() {
        let root = std::env::temp_dir().join(format!(
            "purewall-root-reparse-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("ordinary test root should be created");

        let error = scan_folder_with_root_inspection(&root.to_string_lossy(), |_| {
            Ok(ScanRootMetadata {
                is_directory: true,
                is_symlink: false,
                is_reparse_point: true,
            })
        })
        .expect_err("a reparse-backed root must be rejected before traversal");

        assert!(
            error.to_string().contains("reparse point"),
            "unexpected error: {error}"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn ensure_local_path_rejects_unc_paths() {
        let err = ensure_local_path(Path::new(r"\\server\share\wallpaper.jpg"))
            .expect_err("UNC paths must be rejected");
        assert!(
            err.to_string().contains("Network paths are not supported"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn modify_data_and_metadata_events_never_request_directory_scans() {
        for kind in [
            EventKind::Modify(ModifyKind::Data(DataChange::Any)),
            EventKind::Modify(ModifyKind::Metadata(MetadataKind::Any)),
        ] {
            let path = PathBuf::from("ordinary-directory");
            let signals = classify_watcher_event(Event::new(kind).add_path(path.clone()));
            assert_eq!(signals.len(), 1);
            let Signal::Paths(paths) = &signals[0] else {
                panic!("ordinary modify should stay incremental");
            };
            assert_eq!(paths.len(), 1);
            assert_eq!(paths[0].path, path);
            assert!(!paths[0].scan_existing_directory);
            assert!(!paths[0].include_missing);
        }
    }

    #[test]
    fn create_remove_and_rename_paths_keep_independent_final_state_semantics() {
        let created = PathBuf::from("created");
        let created_signals = classify_watcher_event(
            Event::new(EventKind::Create(CreateKind::Any)).add_path(created.clone()),
        );
        let Signal::Paths(created_paths) = &created_signals[0] else {
            panic!("create should be incremental");
        };
        assert_eq!(created_paths[0].path, created);
        assert!(created_paths[0].scan_existing_directory);
        assert!(!created_paths[0].include_missing);

        let removed = PathBuf::from("removed");
        let removed_signals = classify_watcher_event(
            Event::new(EventKind::Remove(RemoveKind::Any)).add_path(removed.clone()),
        );
        let Signal::Paths(removed_paths) = &removed_signals[0] else {
            panic!("remove should be incremental");
        };
        assert_eq!(removed_paths[0].path, removed);
        assert!(!removed_paths[0].scan_existing_directory);
        assert!(removed_paths[0].include_missing);

        let from = PathBuf::from("rename-from");
        let to = PathBuf::from("rename-to");
        let rename_signals = classify_watcher_event(
            Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
                .add_path(from.clone())
                .add_path(to.clone()),
        );
        let Signal::Paths(rename_paths) = &rename_signals[0] else {
            panic!("rename should be incremental");
        };
        assert_eq!(rename_paths.len(), 2);
        assert_eq!(rename_paths[0].path, from);
        assert_eq!(rename_paths[1].path, to);
        assert!(rename_paths
            .iter()
            .all(|path| path.scan_existing_directory && path.include_missing));
    }
}
