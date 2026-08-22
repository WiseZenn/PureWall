use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};

const MARKER_SCHEMA_VERSION: u32 = 1;
const MARKER_FILE_NAME: &str = ".purewall-performance-run.json";
const TEMP_PARENT_DIRECTORY: &str = "purewall-performance";
const RUN_SUBDIRECTORIES: [&str; 4] = ["sources", "database", "cache", "reports"];

#[cfg(test)]
static TEST_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct RunMarker {
    schema_version: u32,
    run_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RootInspection {
    is_directory: bool,
    is_symlink: bool,
    is_reparse_point: bool,
}

#[derive(Debug)]
pub(crate) struct OwnedRunRoot {
    temp_root: PathBuf,
    root: PathBuf,
    run_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CleanupFailureState {
    RemovalNotAttempted,
    RemovalMayBePartial,
}

#[derive(Debug)]
pub(crate) struct CleanupFailure {
    owned: OwnedRunRoot,
    error: anyhow::Error,
    state: CleanupFailureState,
}

impl CleanupFailure {
    pub(crate) fn into_parts(self) -> (OwnedRunRoot, anyhow::Error, CleanupFailureState) {
        (self.owned, self.error, self.state)
    }
}

impl OwnedRunRoot {
    pub(crate) fn create(run_id: &str) -> Result<Self> {
        Self::create_under(&std::env::temp_dir(), run_id, run_id)
    }

    fn create_under(base: &Path, directory_name: &str, marker_run_id: &str) -> Result<Self> {
        validate_run_id(marker_run_id)?;
        if !directory_name_matches_run_id(directory_name, marker_run_id) {
            bail!("performance run directory name does not match the run id");
        }
        validate_creation_temp_root(base)?;

        let harness_parent = base.join(TEMP_PARENT_DIRECTORY);
        match fs::create_dir(&harness_parent) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "create performance temporary parent {}",
                        harness_parent.display()
                    )
                });
            }
        }
        ensure_safe_directory(&harness_parent, "performance temporary parent")?;

        let root = harness_parent.join(directory_name);
        match fs::create_dir(&root) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                bail!("performance run root already exists");
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("create performance run root {}", root.display()));
            }
        }

        let setup_result = (|| -> Result<()> {
            ensure_safe_directory(&root, "performance run root")?;
            write_marker_no_clobber(&root, marker_run_id)?;
            for directory in RUN_SUBDIRECTORIES {
                let path = root.join(directory);
                fs::create_dir(&path).with_context(|| {
                    format!("create performance run directory {}", path.display())
                })?;
                ensure_safe_directory(&path, "performance run directory")?;
            }
            Ok(())
        })();

        if let Err(error) = setup_result {
            rollback_incomplete_creation(&root);
            return Err(error);
        }

        let validated_root =
            match validate_cleanup_target(base, &root, marker_run_id, inspect_root_no_follow) {
                Ok(validated_root) => validated_root,
                Err(error) => {
                    rollback_incomplete_creation(&root);
                    return Err(error);
                }
            };

        Ok(Self {
            temp_root: base.to_path_buf(),
            root: validated_root,
            run_id: marker_run_id.to_owned(),
        })
    }

    #[cfg(test)]
    pub(crate) fn create_for_test(run_id: &str) -> Result<Self> {
        validate_run_id(run_id)?;
        let directory_name = unique_test_directory_name(run_id);
        Self::create_under(&std::env::temp_dir(), &directory_name, run_id)
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn sources_dir(&self) -> PathBuf {
        self.root.join("sources")
    }

    pub(crate) fn database_dir(&self) -> PathBuf {
        self.root.join("database")
    }

    pub(crate) fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }

    pub(crate) fn reports_dir(&self) -> PathBuf {
        self.root.join("reports")
    }

    pub(crate) fn cleanup(self) -> std::result::Result<(), CleanupFailure> {
        self.cleanup_with(
            |owned| {
                validate_cleanup_target(
                    &owned.temp_root,
                    &owned.root,
                    &owned.run_id,
                    inspect_root_no_follow,
                )
            },
            |validated| {
                fs::remove_dir_all(validated).with_context(|| {
                    format!(
                        "remove validated performance run root {}",
                        validated.display()
                    )
                })
            },
        )
    }

    fn cleanup_with<Validate, Remove>(
        self,
        validate: Validate,
        remove: Remove,
    ) -> std::result::Result<(), CleanupFailure>
    where
        Validate: FnOnce(&OwnedRunRoot) -> Result<PathBuf>,
        Remove: FnOnce(&Path) -> Result<()>,
    {
        let validated = match validate(&self) {
            Ok(validated) => validated,
            Err(error) => {
                return Err(CleanupFailure {
                    owned: self,
                    error,
                    state: CleanupFailureState::RemovalNotAttempted,
                });
            }
        };
        match remove(&validated) {
            Ok(()) => Ok(()),
            Err(error) => Err(CleanupFailure {
                owned: self,
                error,
                state: CleanupFailureState::RemovalMayBePartial,
            }),
        }
    }

    #[cfg(test)]
    pub(crate) fn cleanup_with_test_operations<Validate, Remove>(
        self,
        validate: Validate,
        remove: Remove,
    ) -> std::result::Result<(), CleanupFailure>
    where
        Validate: FnOnce(&OwnedRunRoot) -> Result<PathBuf>,
        Remove: FnOnce(&Path) -> Result<()>,
    {
        self.cleanup_with(validate, remove)
    }

    pub(crate) fn retain(self) -> PathBuf {
        self.root
    }
}

fn validate_run_id(run_id: &str) -> Result<()> {
    if run_id.is_empty()
        || !run_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        bail!("performance run id must use only non-empty ASCII letters, digits, '_' or '-'");
    }
    Ok(())
}

fn directory_name_matches_run_id(directory_name: &str, run_id: &str) -> bool {
    if directory_name == run_id {
        return true;
    }

    let Some(suffix) = directory_name.strip_prefix(run_id) else {
        return false;
    };
    let Some(suffix) = suffix.strip_prefix("-test-") else {
        return false;
    };
    let mut parts = suffix.split('-');
    let Some(process_id) = parts.next() else {
        return false;
    };
    let Some(sequence) = parts.next() else {
        return false;
    };
    parts.next().is_none()
        && !process_id.is_empty()
        && !sequence.is_empty()
        && process_id.bytes().all(|byte| byte.is_ascii_digit())
        && sequence.bytes().all(|byte| byte.is_ascii_digit())
}

#[cfg(test)]
fn unique_test_directory_name(run_id: &str) -> String {
    let sequence = TEST_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{run_id}-test-{}-{sequence}", std::process::id())
}

#[cfg(test)]
fn create_unmarked_test_child(run_id: &str) -> PathBuf {
    let owned = OwnedRunRoot::create_for_test(run_id).expect("create marked test root");
    let root = owned.retain();
    fs::remove_file(root.join(MARKER_FILE_NAME)).expect("remove marker from owned test root");
    root
}

fn marker_bytes(run_id: &str) -> Result<Vec<u8>> {
    serde_json::to_vec(&RunMarker {
        schema_version: MARKER_SCHEMA_VERSION,
        run_id: run_id.to_owned(),
    })
    .context("serialize performance run marker")
}

fn write_marker_no_clobber(root: &Path, run_id: &str) -> Result<()> {
    validate_run_id(run_id)?;
    let marker_path = root.join(MARKER_FILE_NAME);
    let mut marker = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&marker_path)
    {
        Ok(marker) => marker,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            bail!("performance run marker already exists");
        }
        Err(error) => {
            return Err(error).with_context(|| {
                format!("create performance run marker {}", marker_path.display())
            });
        }
    };

    marker
        .write_all(&marker_bytes(run_id)?)
        .context("write performance run marker")?;
    marker.flush().context("flush performance run marker")?;
    marker.sync_all().context("sync performance run marker")?;
    Ok(())
}

fn rollback_incomplete_creation(root: &Path) {
    for directory in RUN_SUBDIRECTORIES.iter().rev() {
        let _ = fs::remove_dir(root.join(directory));
    }

    let marker_path = root.join(MARKER_FILE_NAME);
    if let Ok(metadata) = fs::symlink_metadata(&marker_path) {
        if metadata.file_type().is_file()
            && !metadata.file_type().is_symlink()
            && !metadata_is_reparse_point(&metadata)
        {
            let _ = fs::remove_file(marker_path);
        }
    }
    let _ = fs::remove_dir(root);
}

fn validate_creation_temp_root(temp_root: &Path) -> Result<()> {
    validate_absolute_path(temp_root, "performance temporary root")?;
    inspect_directory_and_ancestors(temp_root, inspect_root_no_follow)?;

    let canonical = fs::canonicalize(temp_root).with_context(|| {
        format!(
            "canonicalize performance temporary root {}",
            temp_root.display()
        )
    })?;
    let system_temp =
        fs::canonicalize(std::env::temp_dir()).context("canonicalize the system temporary root")?;
    if !same_path(&canonical, &system_temp) {
        bail!("performance temporary root is not the system temporary root");
    }
    Ok(())
}

fn validate_absolute_path(path: &Path, label: &str) -> Result<()> {
    if !path.is_absolute() {
        bail!("{label} must be absolute");
    }
    let raw_has_alias = path
        .to_string_lossy()
        .split(['/', '\\'])
        .any(|component| matches!(component, "." | ".."));
    if raw_has_alias
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        bail!("performance cleanup path aliases are not allowed");
    }
    Ok(())
}

fn ensure_safe_directory(path: &Path, label: &str) -> Result<()> {
    let inspection = inspect_root_no_follow(path)?;
    if inspection.is_symlink {
        bail!("{label} is a symbolic link");
    }
    if inspection.is_reparse_point {
        bail!("{label} is a reparse point");
    }
    if !inspection.is_directory {
        bail!("{label} is not a directory");
    }
    Ok(())
}

fn inspect_directory_and_ancestors<F>(target: &Path, mut inspect: F) -> Result<()>
where
    F: FnMut(&Path) -> Result<RootInspection>,
{
    let target_inspection = inspect(target)?;
    if target_inspection.is_symlink {
        bail!("performance cleanup target is a symbolic link");
    }
    if target_inspection.is_reparse_point {
        bail!("performance cleanup target is a reparse point");
    }
    if !target_inspection.is_directory {
        bail!("performance cleanup target is not a directory");
    }

    for ancestor in target.ancestors().skip(1) {
        let inspection = inspect(ancestor)?;
        if inspection.is_symlink {
            bail!("performance cleanup ancestor is a symbolic link");
        }
        if inspection.is_reparse_point {
            bail!("performance cleanup ancestor is a reparse point");
        }
        if !inspection.is_directory {
            bail!("performance cleanup ancestor is not a directory");
        }
    }
    Ok(())
}

fn inspect_root_no_follow(path: &Path) -> Result<RootInspection> {
    let metadata = fs::symlink_metadata(path).with_context(|| {
        format!(
            "inspect performance path without following {}",
            path.display()
        )
    })?;
    Ok(RootInspection {
        is_directory: metadata.file_type().is_dir(),
        is_symlink: metadata.file_type().is_symlink(),
        is_reparse_point: metadata_is_reparse_point(&metadata),
    })
}

#[cfg(windows)]
fn metadata_is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn metadata_is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

fn path_identity(path: &Path) -> String {
    let mut identity = path.to_string_lossy().replace('\\', "/");

    #[cfg(windows)]
    {
        if identity
            .as_bytes()
            .get(..8)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"//?/UNC/"))
        {
            identity = format!("//{}", &identity[8..]);
        } else if identity
            .as_bytes()
            .get(..4)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"//?/"))
        {
            identity = identity[4..].to_owned();
        }
        identity.make_ascii_lowercase();
    }

    while identity.len() > 1
        && identity.ends_with('/')
        && !(identity.len() == 3 && identity.as_bytes().get(1) == Some(&b':'))
    {
        identity.pop();
    }
    identity
}

fn same_path(left: &Path, right: &Path) -> bool {
    path_identity(left) == path_identity(right)
}

fn is_strict_descendant(path: &Path, parent: &Path) -> bool {
    let path = path_identity(path);
    let parent = path_identity(parent);
    if path == parent {
        return false;
    }
    let prefix = if parent.ends_with('/') {
        parent
    } else {
        format!("{parent}/")
    };
    path.starts_with(&prefix)
}

fn paths_overlap(left: &Path, right: &Path) -> bool {
    same_path(left, right) || is_strict_descendant(left, right) || is_strict_descendant(right, left)
}

fn protected_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn reject_protected_targets_with(target: &Path, repository: &Path, app_data: &Path) -> Result<()> {
    let drive_root = target
        .ancestors()
        .last()
        .ok_or_else(|| anyhow!("performance cleanup target has no filesystem root"))?;
    if same_path(target, drive_root) {
        bail!("performance cleanup target is a drive root");
    }

    if paths_overlap(target, repository) {
        bail!("performance cleanup target overlaps the repository");
    }
    if paths_overlap(target, app_data) {
        bail!("performance cleanup target overlaps PureWall AppData");
    }
    Ok(())
}

fn reject_protected_targets(target: &Path) -> Result<()> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| anyhow!("performance repository root is unavailable"))?;
    let repository = protected_path(repository);
    let app_data = protected_path(&crate::paths::app_data_dir());
    reject_protected_targets_with(target, &repository, &app_data)
}

fn validate_marker(root: &Path, run_id: &str) -> Result<()> {
    let marker_path = root.join(MARKER_FILE_NAME);
    let metadata = match fs::symlink_metadata(&marker_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            bail!("performance cleanup marker is missing");
        }
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "inspect performance cleanup marker {}",
                    marker_path.display()
                )
            });
        }
    };
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata_is_reparse_point(&metadata)
    {
        bail!("performance cleanup marker is not a regular file");
    }

    let expected = marker_bytes(run_id)?;
    if metadata.len() != expected.len() as u64 {
        bail!("performance cleanup marker does not match this run");
    }
    let actual = fs::read(&marker_path)
        .with_context(|| format!("read performance cleanup marker {}", marker_path.display()))?;
    if actual != expected {
        bail!("performance cleanup marker does not match this run");
    }

    let marker: RunMarker =
        serde_json::from_slice(&actual).context("parse the exact performance cleanup marker")?;
    if marker.schema_version != MARKER_SCHEMA_VERSION || marker.run_id != run_id {
        bail!("performance cleanup marker does not match this run");
    }
    Ok(())
}

fn validate_cleanup_target<F>(
    temp_root: &Path,
    target: &Path,
    run_id: &str,
    inspect: F,
) -> Result<PathBuf>
where
    F: FnMut(&Path) -> Result<RootInspection>,
{
    validate_run_id(run_id)?;
    validate_absolute_path(temp_root, "performance temporary root")?;
    validate_absolute_path(target, "performance cleanup target")?;
    inspect_directory_and_ancestors(target, inspect)?;

    let canonical_system_temp =
        fs::canonicalize(std::env::temp_dir()).context("canonicalize the system temporary root")?;
    let canonical_temp = fs::canonicalize(temp_root).with_context(|| {
        format!(
            "canonicalize performance temporary root {}",
            temp_root.display()
        )
    })?;
    if !same_path(&canonical_temp, &canonical_system_temp) {
        bail!("performance temporary root is not the system temporary root");
    }

    let fixed_parent = temp_root.join(TEMP_PARENT_DIRECTORY);
    let canonical_parent = fs::canonicalize(&fixed_parent).with_context(|| {
        format!(
            "canonicalize performance temporary parent {}",
            fixed_parent.display()
        )
    })?;
    let canonical_target = fs::canonicalize(target).with_context(|| {
        format!(
            "canonicalize performance cleanup target {}",
            target.display()
        )
    })?;

    if !is_strict_descendant(&canonical_parent, &canonical_temp)
        || !is_strict_descendant(&canonical_target, &canonical_parent)
        || canonical_target
            .parent()
            .is_none_or(|parent| !same_path(parent, &canonical_parent))
        || target
            .parent()
            .is_none_or(|parent| !same_path(parent, &fixed_parent))
    {
        bail!("performance cleanup target is outside the owned temporary parent");
    }
    if !same_path(target, &canonical_target) {
        bail!("performance cleanup target uses an unresolved path alias");
    }

    let directory_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("performance cleanup target name is not valid UTF-8"))?;
    if !directory_name_matches_run_id(directory_name, run_id) {
        bail!("performance cleanup target name does not match the run id");
    }

    reject_protected_targets(&canonical_target)?;
    validate_marker(&canonical_target, run_id)?;

    inspect_directory_and_ancestors(&canonical_target, inspect_root_no_follow)?;
    validate_marker(&canonical_target, run_id)?;
    let final_target = fs::canonicalize(&canonical_target).with_context(|| {
        format!(
            "revalidate performance cleanup target {}",
            canonical_target.display()
        )
    })?;
    if !same_path(&final_target, &canonical_target) {
        bail!("performance cleanup target changed during validation");
    }
    Ok(canonical_target)
}

fn validate_real_cleanup_target(target: &Path, run_id: &str) -> Result<PathBuf> {
    validate_cleanup_target(
        &std::env::temp_dir(),
        target,
        run_id,
        inspect_root_no_follow,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn restore_marker(root: &Path, run_id: &str) {
        write_marker_no_clobber(root, run_id).expect("restore exact test marker");
    }

    fn remove_validated_test_root(root: &Path, run_id: &str) {
        let validated =
            validate_real_cleanup_target(root, run_id).expect("test root must still be owned");
        std::fs::remove_dir_all(validated).expect("remove validated test root");
    }

    #[test]
    fn marked_owned_child_cleans_and_unmarked_child_is_refused() {
        let owned = OwnedRunRoot::create_for_test("owned-cleanup").unwrap();
        let root = owned.root().to_path_buf();
        std::fs::write(root.join("payload.bin"), b"owned").unwrap();
        owned.cleanup().unwrap();
        assert!(!root.exists());

        let unmarked = create_unmarked_test_child("unmarked-cleanup");
        assert_eq!(
            validate_real_cleanup_target(&unmarked, "unmarked-cleanup")
                .unwrap_err()
                .to_string(),
            "performance cleanup marker is missing"
        );
        assert!(unmarked.exists());
        restore_marker(&unmarked, "unmarked-cleanup");
        remove_validated_test_root(&unmarked, "unmarked-cleanup");
    }

    #[test]
    fn cleanup_validation_failure_returns_ownership_without_attempting_removal() {
        let owned = OwnedRunRoot::create_for_test("cleanup-validation-failure").unwrap();
        let root = owned.root().to_path_buf();

        let failure = owned
            .cleanup_with(
                |_| bail!("injected cleanup validation failure"),
                |_| panic!("remove must not run after validation failure"),
            )
            .expect_err("validation failure must return ownership");
        let (owned, error, state) = failure.into_parts();

        assert_eq!(state, CleanupFailureState::RemovalNotAttempted);
        assert!(format!("{error:#}").contains("injected cleanup validation failure"));
        assert_eq!(owned.root(), root);
        assert!(root.exists());
        owned.cleanup().unwrap();
    }

    #[test]
    fn cleanup_removal_failure_returns_the_owned_root_as_possibly_partial() {
        let owned = OwnedRunRoot::create_for_test("cleanup-removal-failure").unwrap();
        let root = owned.root().to_path_buf();

        let failure = owned
            .cleanup_with(
                |owned| Ok(owned.root().to_path_buf()),
                |_| bail!("injected remove_dir_all failure"),
            )
            .expect_err("removal failure must return ownership");
        let (owned, error, state) = failure.into_parts();

        assert_eq!(state, CleanupFailureState::RemovalMayBePartial);
        assert!(format!("{error:#}").contains("injected remove_dir_all failure"));
        assert_eq!(owned.root(), root);
        assert!(root.exists());
        owned.cleanup().unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn cleanup_returns_ownership_after_remove_dir_all_actually_fails() {
        use std::os::windows::fs::OpenOptionsExt;

        let owned = OwnedRunRoot::create_for_test("cleanup-real-removal-failure").unwrap();
        let root = owned.root().to_path_buf();
        let marker_lock = OpenOptions::new()
            .read(true)
            .share_mode(0x1 | 0x2)
            .open(root.join(MARKER_FILE_NAME))
            .unwrap();

        let failure = owned
            .cleanup()
            .expect_err("the marker's no-delete share mode must make remove_dir_all fail");
        let (owned, error, state) = failure.into_parts();

        assert_eq!(state, CleanupFailureState::RemovalMayBePartial);
        assert!(format!("{error:#}").contains("remove validated performance run root"));
        assert_eq!(owned.root(), root);
        assert!(root.exists());
        drop(marker_lock);
        owned.cleanup().unwrap();
    }

    #[test]
    fn cleanup_rejects_mismatched_run_and_injected_reparse() {
        let owned = OwnedRunRoot::create_for_test("run-a").unwrap();
        assert!(validate_real_cleanup_target(owned.root(), "run-b").is_err());
        let error = validate_cleanup_target(&std::env::temp_dir(), owned.root(), "run-a", |_| {
            Ok(RootInspection {
                is_directory: true,
                is_symlink: false,
                is_reparse_point: true,
            })
        })
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "performance cleanup target is a reparse point"
        );
        let symlink_error =
            validate_cleanup_target(&std::env::temp_dir(), owned.root(), "run-a", |_| {
                Ok(RootInspection {
                    is_directory: true,
                    is_symlink: true,
                    is_reparse_point: false,
                })
            })
            .unwrap_err();
        assert_eq!(
            symlink_error.to_string(),
            "performance cleanup target is a symbolic link"
        );
        owned.cleanup().unwrap();
    }

    #[test]
    fn retain_leaves_and_returns_the_marked_root() {
        let owned = OwnedRunRoot::create_for_test("retained").unwrap();
        let expected = owned.root().to_path_buf();
        let retained = owned.retain();

        assert_eq!(retained, expected);
        assert!(retained.is_dir());
        assert!(retained.join(MARKER_FILE_NAME).is_file());

        remove_validated_test_root(&retained, "retained");
    }

    #[test]
    fn drop_and_panic_deliberately_retain_the_root() {
        let owned = OwnedRunRoot::create_for_test("panic-retained").unwrap();
        let root = owned.root().to_path_buf();
        std::fs::write(root.join("payload.bin"), b"retained").unwrap();

        let panic = std::panic::catch_unwind(move || {
            let _owned = owned;
            panic!("owned-temp-sentinel");
        })
        .expect_err("sentinel panic must be captured");
        let panic_message = panic
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| panic.downcast_ref::<String>().map(String::as_str));
        assert_eq!(panic_message, Some("owned-temp-sentinel"));
        assert!(root.join("payload.bin").is_file());

        remove_validated_test_root(&root, "panic-retained");
    }

    #[test]
    fn marker_must_have_exact_schema_run_id_and_bytes() {
        let owned = OwnedRunRoot::create_for_test("marker-exact").unwrap();
        let root = owned.root().to_path_buf();
        let marker = root.join(MARKER_FILE_NAME);

        std::fs::write(&marker, br#"{"schema_version":2,"run_id":"marker-exact"}"#).unwrap();
        assert_eq!(
            validate_real_cleanup_target(&root, "marker-exact")
                .unwrap_err()
                .to_string(),
            "performance cleanup marker does not match this run"
        );

        std::fs::write(&marker, br#"{"schema_version":1,"run_id":"different"}"#).unwrap();
        assert!(validate_real_cleanup_target(&root, "marker-exact").is_err());

        std::fs::write(
            &marker,
            br#"{ "schema_version": 1, "run_id": "marker-exact" }"#,
        )
        .unwrap();
        assert_eq!(
            validate_real_cleanup_target(&root, "marker-exact")
                .unwrap_err()
                .to_string(),
            "performance cleanup marker does not match this run"
        );

        std::fs::remove_file(&marker).unwrap();
        restore_marker(&root, "marker-exact");
        owned.cleanup().unwrap();
    }

    #[test]
    fn marker_must_be_a_regular_file_and_marker_creation_is_no_clobber() {
        let owned = OwnedRunRoot::create_for_test("marker-regular").unwrap();
        let root = owned.root().to_path_buf();
        let marker = root.join(MARKER_FILE_NAME);

        assert_eq!(
            write_marker_no_clobber(&root, "marker-regular")
                .unwrap_err()
                .to_string(),
            "performance run marker already exists"
        );

        std::fs::remove_file(&marker).unwrap();
        std::fs::create_dir(&marker).unwrap();
        assert_eq!(
            validate_real_cleanup_target(&root, "marker-regular")
                .unwrap_err()
                .to_string(),
            "performance cleanup marker is not a regular file"
        );
        std::fs::remove_dir(&marker).unwrap();
        restore_marker(&root, "marker-regular");
        owned.cleanup().unwrap();
    }

    #[test]
    fn cleanup_rejects_root_parent_outside_and_alias_targets() {
        let owned = OwnedRunRoot::create_for_test("containment").unwrap();
        let root = owned.root().to_path_buf();
        let temp = std::env::temp_dir();
        let fixed_parent = root.parent().unwrap().to_path_buf();

        assert!(validate_real_cleanup_target(&temp, "containment").is_err());
        assert!(validate_real_cleanup_target(&fixed_parent, "containment").is_err());
        assert!(validate_cleanup_target(
            Path::new(env!("CARGO_MANIFEST_DIR")),
            &root,
            "containment",
            inspect_root_no_follow,
        )
        .is_err());

        let alias = temp
            .join(TEMP_PARENT_DIRECTORY)
            .join("..")
            .join(TEMP_PARENT_DIRECTORY)
            .join(root.file_name().unwrap());
        assert_eq!(
            validate_real_cleanup_target(&alias, "containment")
                .unwrap_err()
                .to_string(),
            "performance cleanup path aliases are not allowed"
        );
        assert!(root.exists());
        owned.cleanup().unwrap();
    }

    #[test]
    fn cleanup_rejects_reparse_ancestor_and_wrong_physical_name() {
        let owned = OwnedRunRoot::create_for_test("naming").unwrap();
        let root = owned.root().to_path_buf();
        let parent = root.parent().unwrap().to_path_buf();
        let error = validate_cleanup_target(&std::env::temp_dir(), &root, "naming", |path| {
            Ok(RootInspection {
                is_directory: true,
                is_symlink: false,
                is_reparse_point: path == parent,
            })
        })
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "performance cleanup ancestor is a reparse point"
        );

        let wrong_name = parent.join(unique_test_directory_name("wrong-name"));
        std::fs::rename(&root, &wrong_name).unwrap();
        let naming_error = validate_real_cleanup_target(&wrong_name, "naming")
            .unwrap_err()
            .to_string();
        std::fs::rename(&wrong_name, &root).unwrap();
        assert_eq!(
            naming_error,
            "performance cleanup target name does not match the run id"
        );
        owned.cleanup().unwrap();
    }

    #[test]
    fn repository_overlap_is_rejected_in_both_directions() {
        let repository = Path::new(r"C:\synthetic-repository-parent\repository");
        let app_data = Path::new(r"C:\synthetic-app-data-parent\app-data");

        for target in [
            repository.to_path_buf(),
            repository.join("nested-run"),
            repository.parent().unwrap().to_path_buf(),
        ] {
            assert_eq!(
                reject_protected_targets_with(&target, repository, app_data)
                    .unwrap_err()
                    .to_string(),
                "performance cleanup target overlaps the repository"
            );
        }
        assert!(reject_protected_targets_with(
            Path::new(r"C:\synthetic-repository-parent\repository-backup\run"),
            repository,
            app_data
        )
        .is_ok());
    }

    #[test]
    fn app_data_overlap_is_rejected_in_both_directions() {
        let repository = Path::new(r"C:\synthetic-repository-parent\repository");
        let app_data = Path::new(r"C:\synthetic-app-data-parent\app-data");

        for target in [
            app_data.to_path_buf(),
            app_data.join("nested-run"),
            app_data.parent().unwrap().to_path_buf(),
        ] {
            assert_eq!(
                reject_protected_targets_with(&target, repository, app_data)
                    .unwrap_err()
                    .to_string(),
                "performance cleanup target overlaps PureWall AppData"
            );
        }
        assert!(reject_protected_targets_with(
            Path::new(r"C:\synthetic-app-data-parent\app-data-backup\run"),
            repository,
            app_data
        )
        .is_ok());
    }

    #[test]
    fn create_rejects_invalid_run_ids_and_existing_roots() {
        assert!(OwnedRunRoot::create("").is_err());
        for invalid in ["", "space here", "slash/name", ".", "风景"] {
            assert!(
                OwnedRunRoot::create_for_test(invalid).is_err(),
                "{invalid:?}"
            );
        }

        assert!(directory_name_matches_run_id(
            "production-run",
            "production-run"
        ));
        assert!(!directory_name_matches_run_id(
            "production-run-other",
            "production-run"
        ));

        let first = OwnedRunRoot::create_for_test("no-clobber").unwrap();
        let first_root = first.retain();
        let directory_name = first_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap()
            .to_owned();
        let second =
            OwnedRunRoot::create_under(&std::env::temp_dir(), &directory_name, "no-clobber")
                .unwrap_err();
        assert_eq!(second.to_string(), "performance run root already exists");
        remove_validated_test_root(&first_root, "no-clobber");
    }

    #[test]
    fn creation_provides_only_the_explicit_harness_subdirectories() {
        let owned = OwnedRunRoot::create_for_test("paths").unwrap();

        assert_eq!(owned.sources_dir(), owned.root().join("sources"));
        assert_eq!(owned.database_dir(), owned.root().join("database"));
        assert_eq!(owned.cache_dir(), owned.root().join("cache"));
        assert_eq!(owned.reports_dir(), owned.root().join("reports"));
        assert!(owned.sources_dir().is_dir());
        assert!(owned.database_dir().is_dir());
        assert!(owned.cache_dir().is_dir());
        assert!(owned.reports_dir().is_dir());

        owned.cleanup().unwrap();
    }
}
