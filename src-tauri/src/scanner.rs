use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf, Prefix};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

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
const WATCH_DEBOUNCE: Duration = Duration::from_millis(250);

enum WatcherMessage {
    Changed(Vec<PathBuf>),
    Stop,
}

pub struct FolderWatcher {
    _watcher: notify::RecommendedWatcher,
    tx: Sender<WatcherMessage>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for FolderWatcher {
    fn drop(&mut self) {
        let _ = self.tx.send(WatcherMessage::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Recursively scan a folder for image files
pub fn scan_folder(folder_path: &str) -> Result<Vec<ImageInfo>> {
    let root = Path::new(folder_path);
    ensure_local_path(root)?;
    if !root.is_dir() {
        anyhow::bail!(
            "Folder does not exist or is not a directory: {}",
            folder_path
        );
    }

    let mut images = Vec::new();
    let mut budget = ScanBudget::default();
    collect_images(root, &mut images, 0, &mut budget)?;

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
            let file_type = entry
                .file_type()
                .context("Failed to inspect directory entry type")?;
            if file_type.is_symlink() {
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

            if file_type.is_dir() {
                collect_images(&path, images, depth + 1, budget)?;
            } else if file_type.is_file() && is_supported_image(&path) {
                if images.len() >= MAX_SCAN_IMAGES {
                    anyhow::bail!(
                        "Too many images found. PureWall scans up to {} images at once.",
                        MAX_SCAN_IMAGES
                    );
                }
                if let Some(info) = get_image_info(&path) {
                    images.push(info);
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| SUPPORTED_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn watcher_path_is_relevant(
    path: &Path,
    include_existing_directories: bool,
    include_missing_paths: bool,
) -> bool {
    is_supported_image(path)
        || (include_existing_directories && path.is_dir())
        || (include_missing_paths && !path.exists())
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
    let metadata = std::fs::metadata(path).ok()?;
    let file_size = metadata.len();

    // Canonicalize the path before storing so validation later matches (HIG-06).
    let stored_path = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string();

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

fn run_watcher_events<F>(rx: Receiver<WatcherMessage>, debounce: Duration, on_change: F)
where
    F: Fn(Vec<PathBuf>),
{
    let mut pending = BTreeSet::new();

    while let Ok(WatcherMessage::Changed(paths)) = rx.recv() {
        pending.extend(paths);

        loop {
            match rx.recv_timeout(debounce) {
                Ok(WatcherMessage::Changed(paths)) => pending.extend(paths),
                Ok(WatcherMessage::Stop) => return,
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }

        if !pending.is_empty() {
            on_change(std::mem::take(&mut pending).into_iter().collect());
        }
    }
}

/// Start watching a folder and deliver de-duplicated change batches after a short quiet period.
pub fn start_watcher<F>(folder_path: String, on_change: F) -> Result<FolderWatcher>
where
    F: Fn(Vec<PathBuf>) + Send + 'static,
{
    use notify::{event::ModifyKind, recommended_watcher, EventKind, RecursiveMode, Watcher};

    let folder = Path::new(&folder_path);
    ensure_local_path(folder)?;
    if !folder.is_dir() {
        anyhow::bail!(
            "Folder does not exist or is not a directory: {}",
            folder_path
        );
    }

    let (tx, rx) = channel();

    let event_tx = tx.clone();
    let mut watcher = recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            let include_existing_directories = matches!(
                &event.kind,
                EventKind::Create(_) | EventKind::Modify(ModifyKind::Name(_))
            );
            let include_missing_paths = matches!(
                &event.kind,
                EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(_))
            );
            let paths = event
                .paths
                .into_iter()
                .filter(|path| {
                    watcher_path_is_relevant(
                        path,
                        include_existing_directories,
                        include_missing_paths,
                    )
                })
                .collect::<Vec<_>>();
            if !paths.is_empty() {
                let _ = event_tx.send(WatcherMessage::Changed(paths));
            }
        }
    })
    .context("Failed to create file watcher")?;

    watcher
        .watch(folder, RecursiveMode::Recursive)
        .context("Failed to start watching folder")?;

    let thread = std::thread::spawn(move || {
        run_watcher_events(rx, WATCH_DEBOUNCE, on_change);
    });

    Ok(FolderWatcher {
        _watcher: watcher,
        tx,
        thread: Some(thread),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

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
    fn ensure_local_path_rejects_unc_paths() {
        let err = ensure_local_path(Path::new(r"\\server\share\wallpaper.jpg"))
            .expect_err("UNC paths must be rejected");
        assert!(
            err.to_string().contains("Network paths are not supported"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn watcher_events_are_debounced_and_deduplicated() {
        let (tx, rx) = channel();
        let (batch_tx, batch_rx) = channel();
        let thread = std::thread::spawn(move || {
            run_watcher_events(rx, Duration::from_millis(10), move |paths| {
                batch_tx
                    .send(paths)
                    .expect("test batch receiver should stay connected");
            });
        });
        let first = PathBuf::from(r"C:\purewall-test\a.jpg");
        let second = PathBuf::from(r"C:\purewall-test\b.jpg");

        tx.send(WatcherMessage::Changed(vec![second.clone(), first.clone()]))
            .expect("first watcher event should send");
        tx.send(WatcherMessage::Changed(vec![first.clone()]))
            .expect("duplicate watcher event should send");

        let batch = batch_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("one debounced batch should arrive");
        assert_eq!(batch, vec![first, second]);

        tx.send(WatcherMessage::Stop)
            .expect("watcher stop should send");
        thread.join().expect("watcher event thread should stop");
    }

    #[test]
    fn ordinary_directory_changes_do_not_trigger_subtree_scans() {
        let directory = Path::new(".");
        assert!(directory.is_dir());
        assert!(!watcher_path_is_relevant(directory, false, false));
        assert!(watcher_path_is_relevant(directory, true, false));

        let missing_path = Path::new("purewall-watcher-missing-directory-entry");
        assert!(!missing_path.exists());
        assert!(watcher_path_is_relevant(missing_path, false, true));
        assert!(watcher_path_is_relevant(
            Path::new("changed-wallpaper.jpg"),
            false,
            false
        ));
    }
}
