use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use anyhow::{bail, ensure, Context, Result};
use std::os::windows::fs::MetadataExt;

const OWNED_ROOT_PREFIX: &str = "purewall-watcher-lifecycle-";
const OWNERSHIP_MARKER: &[u8] = b"purewall-watcher-lifecycle-v1\n";
const OWNERSHIP_MARKER_NAME: &str = ".purewall-watcher-lifecycle-owner";
const EVENT_TIMEOUT: Duration = Duration::from_secs(15);
const POST_DROP_QUIET_WINDOW: Duration = Duration::from_secs(2);

fn validate_disposable_runner_contract(
    github_actions: Option<&str>,
    runner_environment: Option<&str>,
    runner_temp: Option<&Path>,
    owned_root: Option<&Path>,
) -> Result<()> {
    if github_actions != Some("true") || runner_environment != Some("github-hosted") {
        bail!("native watcher lifecycle tests require a GitHub-hosted disposable runner");
    }
    let runner_temp = runner_temp.context("RUNNER_TEMP is required")?;
    let owned_root = owned_root.context("PUREWALL_WATCHER_LIFECYCLE_ROOT is required")?;
    if !runner_temp.is_absolute() || !owned_root.is_absolute() {
        bail!("runner temp and lifecycle root must be absolute paths");
    }
    let parent = owned_root
        .parent()
        .context("lifecycle root must have a parent directory")?;
    if crate::paths::path_identity_key(parent) != crate::paths::path_identity_key(runner_temp) {
        bail!("lifecycle root must be a direct child of RUNNER_TEMP");
    }
    let name = owned_root
        .file_name()
        .and_then(|name| name.to_str())
        .context("lifecycle root must have a Unicode directory name")?;
    if !name.starts_with(OWNED_ROOT_PREFIX) || name.len() == OWNED_ROOT_PREFIX.len() {
        bail!("lifecycle root must use the owned PureWall prefix and a unique suffix");
    }
    Ok(())
}

fn validate_ownership_marker(bytes: &[u8]) -> Result<()> {
    if bytes != OWNERSHIP_MARKER {
        bail!("lifecycle root ownership marker is missing or invalid");
    }
    Ok(())
}

fn open_disposable_runner_root() -> Result<PathBuf> {
    let github_actions = std::env::var("GITHUB_ACTIONS").ok();
    let runner_environment = std::env::var("RUNNER_ENVIRONMENT").ok();
    let runner_temp = std::env::var_os("RUNNER_TEMP").map(PathBuf::from);
    let owned_root = std::env::var_os("PUREWALL_WATCHER_LIFECYCLE_ROOT").map(PathBuf::from);
    validate_disposable_runner_contract(
        github_actions.as_deref(),
        runner_environment.as_deref(),
        runner_temp.as_deref(),
        owned_root.as_deref(),
    )?;

    let runner_temp = runner_temp.context("RUNNER_TEMP is required")?;
    let owned_root = owned_root.context("PUREWALL_WATCHER_LIFECYCLE_ROOT is required")?;
    validate_plain_directory(&runner_temp, "RUNNER_TEMP")?;
    validate_plain_directory(&owned_root, "watcher lifecycle root")?;
    let canonical_temp = std::fs::canonicalize(&runner_temp)
        .context("failed to canonicalize disposable runner temp")?;
    let canonical_root = std::fs::canonicalize(&owned_root)
        .context("failed to canonicalize watcher lifecycle root")?;
    validate_disposable_runner_contract(
        github_actions.as_deref(),
        runner_environment.as_deref(),
        Some(&canonical_temp),
        Some(&canonical_root),
    )?;

    let marker_path = canonical_root.join(OWNERSHIP_MARKER_NAME);
    let marker_metadata = std::fs::symlink_metadata(&marker_path)
        .context("watcher lifecycle ownership marker is missing")?;
    ensure!(
        marker_metadata.is_file()
            && !marker_metadata.file_type().is_symlink()
            && marker_metadata.file_attributes() & 0x400 == 0,
        "watcher lifecycle ownership marker must be a plain file"
    );
    ensure!(
        marker_metadata.len() == OWNERSHIP_MARKER.len() as u64,
        "watcher lifecycle ownership marker has an unexpected size"
    );
    validate_ownership_marker(
        &std::fs::read(&marker_path).context("failed to read watcher lifecycle marker")?,
    )?;
    Ok(canonical_root)
}

fn validate_plain_directory(path: &Path, label: &str) -> Result<()> {
    let metadata =
        std::fs::symlink_metadata(path).with_context(|| format!("failed to inspect {label}"))?;
    ensure!(
        metadata.is_dir()
            && !metadata.file_type().is_symlink()
            && metadata.file_attributes() & 0x400 == 0,
        "{label} must be a plain directory, not a symlink or reparse point"
    );
    Ok(())
}

fn unique_case_root(owned_root: &Path, label: &str) -> Result<PathBuf> {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("system time is before the Unix epoch")?
        .as_nanos();
    let root = owned_root.join(format!("{label}-{}-{suffix}", std::process::id()));
    ensure!(
        root.parent().is_some_and(|parent| {
            crate::paths::path_identity_key(parent) == crate::paths::path_identity_key(owned_root)
        }),
        "lifecycle case root escaped the owned runner root"
    );
    std::fs::create_dir(&root).context("failed to create unique lifecycle case root")?;
    Ok(root)
}

fn run_native_watcher_lifecycle(owned_root: &Path) -> Result<()> {
    let case_root = unique_case_root(owned_root, "notify")?;
    let source_root = case_root.join("source");
    std::fs::create_dir(&source_root).context("failed to create watcher source root")?;
    let source_root =
        std::fs::canonicalize(source_root).context("failed to canonicalize watcher source root")?;
    let source = "phase-7b-native".to_string();
    let root_text = source_root.to_string_lossy().into_owned();

    let (work_tx, work_rx) = mpsc::channel::<crate::watcher_queue::WorkItem>();
    let queue = crate::watcher_queue::WatcherQueue::new(move |item| {
        let _ = work_tx.send(item);
    });
    queue
        .admit_root(&root_text, &source)
        .map_err(anyhow::Error::msg)?;
    let callback_queue = queue.clone();
    let callback_root = root_text.clone();
    let callback_source = source.clone();
    let watcher = crate::scanner::start_watcher(root_text.clone(), move |signal| {
        let _ = callback_queue.submit(&callback_root, &callback_source, signal);
    })?;

    let nested = source_root.join("nested");
    std::fs::create_dir(&nested).context("failed to create nested watched directory")?;
    await_path_semantics(&work_rx, &root_text, &source, &[(&nested, true, false)])?;

    let created = nested.join("created.png");
    std::fs::write(&created, b"native watcher fixture")
        .context("failed to write watched fixture")?;
    await_path_semantics(&work_rx, &root_text, &source, &[(&created, false, false)])?;

    let renamed = nested.join("renamed.png");
    std::fs::rename(&created, &renamed).context("failed to rename watched fixture")?;
    await_path_semantics(
        &work_rx,
        &root_text,
        &source,
        &[(&created, false, true), (&renamed, true, false)],
    )?;

    drain_work_items(&work_rx);
    std::fs::remove_file(&renamed).context("failed to remove watched fixture")?;
    await_path_semantics(&work_rx, &root_text, &source, &[(&renamed, false, true)])?;

    drop(watcher);
    std::thread::sleep(crate::watcher_queue::QUIET_WINDOW + Duration::from_millis(250));
    drain_work_items(&work_rx);
    let post_drop = nested.join("post-drop.png");
    std::fs::write(&post_drop, b"must not be observed after watcher drop")
        .context("failed to write post-drop fixture")?;
    assert_path_stays_quiet(&work_rx, &post_drop, POST_DROP_QUIET_WINDOW)?;
    queue.close_and_join();
    Ok(())
}

fn await_path_semantics(
    receiver: &Receiver<crate::watcher_queue::WorkItem>,
    expected_root: &str,
    expected_source: &str,
    expected: &[(&Path, bool, bool)],
) -> Result<()> {
    let deadline = Instant::now() + EVENT_TIMEOUT;
    let mut observed = BTreeMap::<String, crate::watcher_queue::PathSignal>::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        ensure!(
            !remaining.is_zero(),
            "timed out waiting for native watcher path semantics"
        );
        let item = receiver
            .recv_timeout(remaining)
            .context("timed out waiting for native watcher work item")?;
        ensure!(item.root == expected_root, "watcher work item root changed");
        ensure!(
            item.source == expected_source,
            "watcher work item source changed"
        );
        ensure!(
            !item.full_snapshot && item.reason.is_none(),
            "ordinary lifecycle unexpectedly required a full snapshot: {:?}",
            item.reason
        );
        for signal in item.paths {
            let key = crate::paths::path_identity_key(&signal.path);
            observed
                .entry(key)
                .and_modify(|current| {
                    current.scan_existing_directory |= signal.scan_existing_directory;
                    current.include_missing |= signal.include_missing;
                })
                .or_insert(signal);
        }
        if expected
            .iter()
            .all(|(path, scan_directory, include_missing)| {
                observed
                    .get(&crate::paths::path_identity_key(path))
                    .is_some_and(|signal| {
                        (!scan_directory || signal.scan_existing_directory)
                            && (!include_missing || signal.include_missing)
                    })
            })
        {
            return Ok(());
        }
    }
}

fn drain_work_items(receiver: &Receiver<crate::watcher_queue::WorkItem>) {
    while receiver.try_recv().is_ok() {}
}

fn assert_path_stays_quiet(
    receiver: &Receiver<crate::watcher_queue::WorkItem>,
    path: &Path,
    duration: Duration,
) -> Result<()> {
    let expected = crate::paths::path_identity_key(path);
    let deadline = Instant::now() + duration;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(());
        }
        match receiver.recv_timeout(remaining) {
            Ok(item) => ensure!(
                item.paths
                    .iter()
                    .all(|signal| crate::paths::path_identity_key(&signal.path) != expected),
                "watcher delivered a path created after its handle was dropped"
            ),
            Err(mpsc::RecvTimeoutError::Timeout) => return Ok(()),
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
        }
    }
}

fn run_native_junction_scan_boundary(owned_root: &Path) -> Result<()> {
    let case_root = unique_case_root(owned_root, "junction")?;
    let source_root = case_root.join("source");
    let outside_target = case_root.join("outside-target");
    std::fs::create_dir(&source_root).context("failed to create junction source root")?;
    std::fs::create_dir(&outside_target).context("failed to create junction target root")?;
    let direct = source_root.join("direct.png");
    let outside = outside_target.join("outside.png");
    image::RgbImage::new(3, 5)
        .save(&direct)
        .context("failed to write direct image")?;
    image::RgbImage::new(7, 11)
        .save(&outside)
        .context("failed to write outside image")?;

    let junction = source_root.join("junction");
    let output = Command::new("cmd.exe")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(&junction)
        .arg(&outside_target)
        .output()
        .context("failed to invoke mklink for owned junction fixture")?;
    ensure!(
        output.status.success(),
        "mklink /J failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata =
        std::fs::symlink_metadata(&junction).context("failed to inspect owned junction fixture")?;
    ensure!(
        metadata.file_attributes() & 0x400 != 0,
        "owned junction fixture is not a reparse point"
    );

    let images = crate::scanner::scan_folder(&source_root.to_string_lossy())?;
    let direct = std::fs::canonicalize(direct)
        .context("failed to canonicalize direct image")?
        .to_string_lossy()
        .into_owned();
    ensure!(
        images.len() == 1 && images[0].path == direct,
        "folder scan traversed the junction boundary: {:?}",
        images.iter().map(|image| &image.path).collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn disposable_runner_contract_requires_exact_host_and_direct_owned_child() {
    let runner_temp = Path::new(r"D:\a\_temp");
    let owned_root = runner_temp.join("purewall-watcher-lifecycle-1234");

    assert!(validate_disposable_runner_contract(
        Some("true"),
        Some("github-hosted"),
        Some(runner_temp),
        Some(&owned_root),
    )
    .is_ok());

    for (github_actions, runner_environment, temp, root) in [
        (
            Some("false"),
            Some("github-hosted"),
            Some(runner_temp),
            Some(owned_root.as_path()),
        ),
        (
            Some("true"),
            Some("self-hosted"),
            Some(runner_temp),
            Some(owned_root.as_path()),
        ),
        (
            Some("true"),
            Some("github-hosted"),
            None,
            Some(owned_root.as_path()),
        ),
        (Some("true"), Some("github-hosted"), Some(runner_temp), None),
        (
            Some("true"),
            Some("github-hosted"),
            Some(runner_temp),
            Some(runner_temp),
        ),
        (
            Some("true"),
            Some("github-hosted"),
            Some(runner_temp),
            Some(
                &runner_temp
                    .join("nested")
                    .join("purewall-watcher-lifecycle-1234"),
            ),
        ),
        (
            Some("true"),
            Some("github-hosted"),
            Some(runner_temp),
            Some(&runner_temp.join("unowned")),
        ),
    ] {
        assert!(validate_disposable_runner_contract(
            github_actions,
            runner_environment,
            temp,
            root,
        )
        .is_err());
    }
}

#[test]
fn ownership_marker_contract_is_exact() {
    assert!(validate_ownership_marker(OWNERSHIP_MARKER).is_ok());
    assert!(validate_ownership_marker(b"purewall-watcher-lifecycle-v1").is_err());
    assert!(validate_ownership_marker(b"purewall-watcher-lifecycle-v2\n").is_err());
}

#[test]
#[ignore = "requires the explicit disposable Windows watcher lifecycle workflow"]
fn native_watcher_delivers_nested_lifecycle_through_queue_and_stops_after_drop() {
    let owned_root = open_disposable_runner_root().expect("disposable runner root must be valid");
    run_native_watcher_lifecycle(&owned_root).expect("native watcher lifecycle must pass");
}

#[test]
#[ignore = "requires the explicit disposable Windows watcher lifecycle workflow"]
fn native_junction_is_not_traversed_by_folder_scan() {
    let owned_root = open_disposable_runner_root().expect("disposable runner root must be valid");
    run_native_junction_scan_boundary(&owned_root)
        .expect("native junction scan boundary must pass");
}
