#[cfg(windows)]
use crate::scanner;
use crate::{batch_operations, db, CommandError, CommandResult};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const MAX_DELETE_PATHS: usize = 500;
const FILE_ATTRIBUTE_DIRECTORY_BITS: u32 = 0x10;
const FILE_ATTRIBUTE_REPARSE_POINT_BITS: u32 = 0x400;
const DRIVE_FIXED: u32 = 3;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DeleteStatus {
    Rejected,
    NotRecycled,
    RecycleOutcomeUnknown,
    Recycled,
    RecycledMetadataCleanupFailed,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct DeleteResult {
    pub(crate) path: String,
    pub(crate) status: DeleteStatus,
    pub(crate) code: Option<String>,
    pub(crate) message: Option<String>,
}

impl DeleteResult {
    fn rejected(path: String, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            path,
            status: DeleteStatus::Rejected,
            code: Some(code.to_string()),
            message: Some(message.into()),
        }
    }

    fn recycled(path: String) -> Self {
        Self {
            path,
            status: DeleteStatus::Recycled,
            code: None,
            message: None,
        }
    }

    fn failed(
        path: String,
        status: DeleteStatus,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            path,
            status,
            code: Some(code.to_string()),
            message: Some(message.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileIdentity {
    volume_serial: u64,
    file_id: [u8; 16],
    attributes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IdentitySnapshot {
    resolved_path: String,
    components: Vec<FileIdentity>,
}

#[derive(Debug, Clone)]
struct PreparedDelete {
    path: String,
    identity: IdentitySnapshot,
}

#[derive(Debug, Clone)]
struct Rejection {
    code: &'static str,
    message: String,
}

fn normalize_delete_paths(paths: &[String]) -> CommandResult<Vec<String>> {
    let normalized = batch_operations::normalize_batch_paths(paths)?;
    if normalized.len() > MAX_DELETE_PATHS {
        return Err(CommandError::new(
            "delete_limit_exceeded",
            format!("PureWall can delete at most {MAX_DELETE_PATHS} wallpaper files at once."),
        ));
    }
    Ok(normalized)
}

fn rejection_result(path: String, rejection: Rejection) -> DeleteResult {
    DeleteResult::rejected(path, rejection.code, rejection.message)
}

fn preflight_registered(db: &db::Database, paths: &[String]) -> Result<Vec<bool>, String> {
    paths
        .iter()
        .map(|path| {
            db.has_registered_wallpaper(path)
                .map_err(|error| error.to_string())
        })
        .collect()
}

pub(crate) fn delete_one(db: &Mutex<db::Database>, path: String) -> CommandResult<DeleteResult> {
    let results = delete_many(db, vec![path])?;
    results.into_iter().next().ok_or_else(|| {
        CommandError::new(
            "delete_invalid_request",
            "Wallpaper deletion requires one non-empty path.",
        )
    })
}

pub(crate) fn delete_many(
    db: &Mutex<db::Database>,
    raw_paths: Vec<String>,
) -> CommandResult<Vec<DeleteResult>> {
    let paths = normalize_delete_paths(&raw_paths)?;
    if paths.is_empty() {
        return Ok(Vec::new());
    }

    // Registration is the only database work in preflight. The mutex is released before
    // filesystem inspection or any Shell operation so slow/hostile paths cannot block SQLite.
    let registered = {
        let database = db.lock().map_err(|error| error.to_string())?;
        preflight_registered(&database, &paths).map_err(CommandError::from)?
    };
    let prepared = paths
        .iter()
        .zip(registered)
        .map(|(path, is_registered)| {
            if !is_registered {
                Err(Rejection {
                    code: "wallpaper_not_registered",
                    message: format!("Wallpaper is not registered: {path}"),
                })
            } else {
                // Filesystem inspection deliberately happens after the DB mutex is released.
                inspect_and_prepare(path)
            }
        })
        .collect::<Vec<_>>();

    let (mut results, successful) = process_candidates(paths, prepared, recycle_candidate);
    reconcile_metadata(db, &successful, &mut results);
    Ok(results)
}

fn process_candidates<Recycle>(
    paths: Vec<String>,
    prepared: Vec<Result<PreparedDelete, Rejection>>,
    recycle: Recycle,
) -> (Vec<DeleteResult>, Vec<String>)
where
    Recycle: Fn(&PreparedDelete) -> RecycleAttempt,
{
    let mut results = Vec::with_capacity(paths.len());
    let mut successful = Vec::new();
    for (path, candidate) in paths.into_iter().zip(prepared) {
        match candidate {
            Err(rejection) => results.push(rejection_result(path, rejection)),
            Ok(candidate) => match recycle(&candidate) {
                RecycleAttempt::Recycled => {
                    successful.push(candidate.path.clone());
                    results.push(DeleteResult::recycled(candidate.path));
                }
                RecycleAttempt::Failed {
                    status,
                    code,
                    message,
                } => results.push(DeleteResult::failed(candidate.path, status, code, message)),
            },
        }
    }
    (results, successful)
}

fn reconcile_metadata(
    db: &Mutex<db::Database>,
    successful: &[String],
    results: &mut [DeleteResult],
) {
    if successful.is_empty() {
        return;
    }

    let database = match db.lock() {
        Ok(database) => database,
        Err(error) => {
            mark_cleanup_failed(
                results,
                successful,
                format!("Database lock failed after Recycle Bin move: {error}"),
            );
            return;
        }
    };

    reconcile_metadata_with(
        successful,
        results,
        |paths| {
            database
                .remove_wallpapers(paths)
                .map_err(|error| error.to_string())
        },
        |path| {
            database
                .remove_wallpaper(path)
                .map_err(|error| error.to_string())
        },
    );
}

fn reconcile_metadata_with<Bulk, Single>(
    successful: &[String],
    results: &mut [DeleteResult],
    bulk_cleanup: Bulk,
    mut single_cleanup: Single,
) where
    Bulk: FnOnce(&[String]) -> Result<(), String>,
    Single: FnMut(&str) -> Result<(), String>,
{
    if bulk_cleanup(successful).is_ok() {
        return;
    }

    // The bulk operation rolls back on failure. Retry each path independently so one
    // malformed row cannot hide successful cleanup for the remaining recycled files.
    for path in successful {
        if single_cleanup(path).is_err() {
            mark_cleanup_failed(
                results,
                std::slice::from_ref(path),
                "Database cleanup failed after Recycle Bin move".to_string(),
            );
        }
    }
}

fn mark_cleanup_failed(results: &mut [DeleteResult], paths: &[String], message: String) {
    for result in results.iter_mut() {
        if paths.iter().any(|path| path == &result.path) && result.status == DeleteStatus::Recycled
        {
            result.status = DeleteStatus::RecycledMetadataCleanupFailed;
            result.code = Some("metadata_cleanup_failed".to_string());
            result.message = Some(message.clone());
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecycleEvidence {
    ConfirmedRecycled,
    OutcomeUnknown,
}

fn is_fixed_drive_type(drive_type: u32) -> bool {
    drive_type == DRIVE_FIXED
}

fn shell_parsing_path(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("\\\\?\\UNC\\") {
        format!("\\\\{rest}")
    } else if let Some(rest) = path.strip_prefix("\\\\?\\") {
        rest.to_string()
    } else {
        path.to_string()
    }
}

fn map_recycle_worker_join(
    result: std::thread::Result<Result<RecycleEvidence, String>>,
) -> Result<RecycleEvidence, String> {
    match result {
        Ok(result) => result,
        Err(_) => Ok(RecycleEvidence::OutcomeUnknown),
    }
}

enum RecycleAttempt {
    Recycled,
    Failed {
        status: DeleteStatus,
        code: &'static str,
        message: String,
    },
}

fn recycle_candidate(candidate: &PreparedDelete) -> RecycleAttempt {
    recycle_candidate_with(candidate, inspect_path, recycle_path)
}

fn recycle_candidate_with<Inspect, Recycle>(
    candidate: &PreparedDelete,
    inspect: Inspect,
    recycle: Recycle,
) -> RecycleAttempt
where
    Inspect: Fn(&str) -> Result<IdentitySnapshot, Rejection>,
    Recycle: Fn(&str) -> Result<RecycleEvidence, String>,
{
    let current = match inspect(&candidate.path) {
        Ok(identity) if identity == candidate.identity => identity,
        Ok(_) => {
            return RecycleAttempt::Failed {
                status: DeleteStatus::Rejected,
                code: "wallpaper_identity_changed",
                message: format!(
                    "Wallpaper identity changed before deletion: {}",
                    candidate.path
                ),
            }
        }
        Err(rejection) => {
            return RecycleAttempt::Failed {
                status: DeleteStatus::Rejected,
                code: rejection.code,
                message: rejection.message,
            }
        }
    };

    match recycle(&candidate.path) {
        Ok(RecycleEvidence::ConfirmedRecycled) => RecycleAttempt::Recycled,
        Ok(RecycleEvidence::OutcomeUnknown) => RecycleAttempt::Failed {
            status: DeleteStatus::RecycleOutcomeUnknown,
            code: "recycle_outcome_unknown",
            message: format!(
                "Recycle Bin outcome could not be confirmed for {}",
                candidate.path
            ),
        },
        Err(error) => match inspect(&candidate.path) {
            Ok(after) if after == current => RecycleAttempt::Failed {
                status: DeleteStatus::NotRecycled,
                code: "recycle_failed",
                message: error,
            },
            _ => RecycleAttempt::Failed {
                status: DeleteStatus::RecycleOutcomeUnknown,
                code: "recycle_outcome_unknown",
                message: format!(
                    "Recycle Bin outcome is unknown for {}: {error}",
                    candidate.path
                ),
            },
        },
    }
}

#[cfg(not(windows))]
fn inspect_and_prepare(_path: &str) -> Result<PreparedDelete, Rejection> {
    Err(Rejection {
        code: "unsupported_platform",
        message:
            "Recycle Bin deletion is supported only on Windows; use Explorer on this platform."
                .to_string(),
    })
}

#[cfg(not(windows))]
fn inspect_path(_path: &str) -> Result<IdentitySnapshot, Rejection> {
    Err(Rejection {
        code: "unsupported_platform",
        message:
            "Recycle Bin deletion is supported only on Windows; use Explorer on this platform."
                .to_string(),
    })
}

#[cfg(not(windows))]
fn recycle_path(_path: &str) -> Result<RecycleEvidence, String> {
    Err("Recycle Bin deletion is supported only on Windows".to_string())
}

#[cfg(windows)]
fn inspect_and_prepare(path: &str) -> Result<PreparedDelete, Rejection> {
    let identity = inspect_path(path)?;
    Ok(PreparedDelete {
        path: path.to_string(),
        identity,
    })
}

#[cfg(windows)]
fn inspect_path(path: &str) -> Result<IdentitySnapshot, Rejection> {
    use crate::paths::path_identity_key;
    use std::path::{Component, Prefix};

    let input = Path::new(path);
    if path.trim().is_empty() || !input.is_absolute() {
        return Err(rejection(
            "invalid_path",
            "Wallpaper deletion requires an absolute local path.",
        ));
    }
    if matches!(input.components().next(), Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::UNC(_, _) | Prefix::VerbatimUNC(_, _)))
    {
        return Err(rejection(
            "remote_path",
            "UNC and remote wallpaper paths cannot be deleted by PureWall.",
        ));
    }
    if !is_fixed_local_volume(input) {
        return Err(rejection(
            "unsupported_volume",
            "Only local fixed-volume wallpaper paths can be deleted by PureWall.",
        ));
    }
    if !scanner::is_supported_image(input) {
        return Err(rejection(
            "unsupported_file",
            format!("Unsupported wallpaper file type: {path}"),
        ));
    }

    // Capture the no-follow component chain before any call that follows path links.
    let components = capture_component_chain(input)?;
    validate_component_identities(&components, path)?;

    let resolved_path = handle_resolved_path(input)?;
    if path_identity_key(Path::new(&resolved_path)) != path_identity_key(input) {
        return Err(rejection(
            "canonical_path_mismatch",
            format!("Wallpaper handle resolved to a different path: {path}"),
        ));
    }

    Ok(IdentitySnapshot {
        resolved_path: path_identity_key(Path::new(&resolved_path)),
        components,
    })
}

#[cfg(windows)]
fn is_fixed_local_volume(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Component, Prefix};
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetDriveTypeW;

    let Some(Component::Prefix(prefix)) = path.components().next() else {
        return false;
    };
    let drive_letter = match prefix.kind() {
        Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => letter as char,
        _ => return false,
    };
    let root = format!("{drive_letter}:\\");
    let wide = std::ffi::OsStr::new(&root)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    // Require DRIVE_FIXED positively: removable, remote, CD, RAM, unknown,
    // and no-root volumes all fail closed.
    unsafe { is_fixed_drive_type(GetDriveTypeW(PCWSTR(wide.as_ptr()))) }
}

fn validate_component_identities(components: &[FileIdentity], path: &str) -> Result<(), Rejection> {
    let target = components.last().ok_or_else(|| Rejection {
        code: "path_inspection_failed",
        message: "Wallpaper path has no file component.".to_string(),
    })?;
    if components
        .iter()
        .any(|component| component.attributes & FILE_ATTRIBUTE_REPARSE_POINT_BITS != 0)
    {
        return Err(Rejection {
            code: "reparse_path",
            message: format!(
                "Reparse-backed wallpaper paths are rejected; delete this file through Explorer: {path}"
            ),
        });
    }
    if target.attributes & FILE_ATTRIBUTE_DIRECTORY_BITS != 0 {
        return Err(Rejection {
            code: "directory_not_allowed",
            message: format!("Folders cannot be deleted as wallpapers: {path}"),
        });
    }
    Ok(())
}

#[cfg(windows)]
fn rejection(code: &'static str, message: impl Into<String>) -> Rejection {
    Rejection {
        code,
        message: message.into(),
    }
}

#[cfg(windows)]
fn capture_component_chain(path: &Path) -> Result<Vec<FileIdentity>, Rejection> {
    let mut ancestors = path
        .ancestors()
        .filter(|ancestor| !ancestor.as_os_str().is_empty())
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    ancestors.reverse();
    ancestors.dedup();
    ancestors
        .iter()
        .map(|ancestor| capture_component(ancestor))
        .collect()
}

#[cfg(windows)]
struct WindowsHandle(windows::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Drop for WindowsHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(windows)]
fn capture_component(path: &Path) -> Result<FileIdentity, Rejection> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, GetFileInformationByHandle, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_EXISTING,
    };

    let wide = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            FILE_READ_ATTRIBUTES.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            None,
        )
    }
    .map_err(|error| {
        rejection(
            "path_inspection_failed",
            format!(
                "Failed to inspect wallpaper path component {}: {error}",
                path.display()
            ),
        )
    })?;
    let handle = WindowsHandle(handle);
    let mut info = windows::Win32::Storage::FileSystem::BY_HANDLE_FILE_INFORMATION::default();
    unsafe { GetFileInformationByHandle(handle.0, &mut info) }.map_err(|error| {
        rejection(
            "path_inspection_failed",
            format!(
                "Failed to read wallpaper path identity {}: {error}",
                path.display()
            ),
        )
    })?;
    if info.dwFileAttributes & windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT.0
        != 0
    {
        return Err(rejection(
            "reparse_path",
            format!(
                "Reparse-backed wallpaper path component is rejected: {}",
                path.display()
            ),
        ));
    }
    let mut file_id = windows::Win32::Storage::FileSystem::FILE_ID_INFO::default();
    unsafe {
        windows::Win32::Storage::FileSystem::GetFileInformationByHandleEx(
            handle.0,
            windows::Win32::Storage::FileSystem::FileIdInfo,
            &mut file_id as *mut _ as *mut std::ffi::c_void,
            std::mem::size_of::<windows::Win32::Storage::FileSystem::FILE_ID_INFO>() as u32,
        )
    }
    .map_err(|error| {
        rejection(
            "file_identity_unsupported",
            format!(
                "Windows could not provide a stable file identity for {}: {error}",
                path.display()
            ),
        )
    })?;
    Ok(FileIdentity {
        volume_serial: file_id.VolumeSerialNumber,
        file_id: file_id.FileId.Identifier,
        attributes: info.dwFileAttributes,
    })
}

#[cfg(windows)]
fn handle_resolved_path(path: &Path) -> Result<String, Rejection> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, GetFinalPathNameByHandleW, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_NAME_NORMALIZED, FILE_READ_ATTRIBUTES,
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    let wide = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            FILE_READ_ATTRIBUTES.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            None,
        )
    }
    .map_err(|error| {
        rejection(
            "path_inspection_failed",
            format!(
                "Failed to resolve wallpaper handle {}: {error}",
                path.display()
            ),
        )
    })?;
    let handle = WindowsHandle(handle);
    let mut buffer = vec![0u16; 512];
    loop {
        let length =
            unsafe { GetFinalPathNameByHandleW(handle.0, &mut buffer, FILE_NAME_NORMALIZED) };
        if length == 0 {
            return Err(rejection(
                "path_inspection_failed",
                format!("Failed to resolve wallpaper handle {}", path.display()),
            ));
        }
        if (length as usize) < buffer.len() {
            return Ok(String::from_utf16_lossy(&buffer[..length as usize]));
        }
        buffer.resize(buffer.len() * 2, 0);
        if buffer.len() > 32_768 {
            return Err(rejection(
                "path_inspection_failed",
                format!("Wallpaper path is too long to inspect: {}", path.display()),
            ));
        }
    }
}

#[cfg(windows)]
fn recycle_path(path: &str) -> Result<RecycleEvidence, String> {
    let path = path.to_string();
    let join = std::thread::Builder::new()
        .name("purewall-recycle-bin".to_string())
        .spawn(move || recycle_path_on_sta(&path))
        .map_err(|error| format!("Failed to start Recycle Bin STA worker: {error}"))?;

    map_recycle_worker_join(join.join())
}

#[cfg(windows)]
struct ComApartment;

#[cfg(windows)]
impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe { windows::Win32::System::Com::CoUninitialize() };
    }
}

#[cfg(windows)]
fn recycle_path_on_sta(path: &str) -> Result<RecycleEvidence, String> {
    use windows::Win32::System::Com::{
        CoInitializeEx, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
    };

    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) }
        .ok()
        .map_err(|error| format!("Failed to initialize Recycle Bin STA: {error}"))?;
    let _apartment = ComApartment;
    recycle_path_on_sta_inner(path)
}

#[cfg(windows)]
fn recycle_path_on_sta_inner(path: &str) -> Result<RecycleEvidence, String> {
    use crate::paths::path_identity_key;
    use std::collections::HashSet;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
    use windows::Win32::UI::Shell::{
        FileOperation, IFileOperation, IShellItem, SHCreateItemFromParsingName, FOFX_ADDUNDORECORD,
        FOFX_EARLYFAILURE, FOFX_RECYCLEONDELETE, FOF_WANTNUKEWARNING,
    };

    fn recycle_item_ids(path: &str) -> Result<HashSet<String>, String> {
        let target = path_identity_key(Path::new(path));
        trash::os_limited::list()
            .map_err(|error| format!("Failed to enumerate Recycle Bin evidence: {error}"))
            .map(|items| {
                items
                    .into_iter()
                    .filter(|item| path_identity_key(&item.original_path()) == target)
                    .map(|item| item.id.to_string_lossy().into_owned())
                    .collect()
            })
    }

    let ids_before = recycle_item_ids(path)?;
    // Shell parsing does not accept the verbatim prefix returned by Windows
    // canonicalization. Keep identity/evidence matching on the registered path;
    // normalize only this path-bound Shell-item creation call.
    let shell_path = shell_parsing_path(path);
    let wide = std::ffi::OsStr::new(&shell_path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let item: IShellItem = unsafe { SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None) }
        .map_err(|error| format!("Failed to create Recycle Bin shell item for {path}: {error}"))?;
    let operation: IFileOperation =
        unsafe { CoCreateInstance(&FileOperation, None, CLSCTX_ALL) }
            .map_err(|error| format!("Failed to create Recycle Bin operation: {error}"))?;

    // Deliberately omit FOF_NO_UI, FOF_NOCONFIRMATION, and FOF_ALLOWUNDO.
    // Windows may show an explicit prompt when recycling is impossible; PureWall
    // never auto-confirms a permanent-delete choice.
    unsafe {
        operation
            .SetOperationFlags(
                FOFX_RECYCLEONDELETE | FOFX_ADDUNDORECORD | FOFX_EARLYFAILURE | FOF_WANTNUKEWARNING,
            )
            .map_err(|error| format!("Failed to configure Recycle Bin operation: {error}"))?;
        operation
            .DeleteItem(
                &item,
                None::<&windows::Win32::UI::Shell::IFileOperationProgressSink>,
            )
            .map_err(|error| {
                format!("Failed to queue Recycle Bin operation for {path}: {error}")
            })?;
        operation
            .PerformOperations()
            .map_err(|error| format!("Recycle Bin operation failed for {path}: {error}"))?;
        let aborted = operation.GetAnyOperationsAborted().map_err(|error| {
            format!("Could not confirm Recycle Bin operation for {path}: {error}")
        })?;
        if aborted.as_bool() {
            return Err(format!("Recycle Bin operation was aborted for {path}"));
        }
    }

    let ids_after = recycle_item_ids(path)?;
    if ids_after.difference(&ids_before).next().is_some() {
        return Ok(RecycleEvidence::ConfirmedRecycled);
    }
    Ok(RecycleEvidence::OutcomeUnknown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_reparse_component_is_rejected_without_filesystem_access() {
        let error = validate_component_identities(
            &[FileIdentity {
                volume_serial: 1,
                file_id: [1; 16],
                attributes: FILE_ATTRIBUTE_REPARSE_POINT_BITS,
            }],
            "C:\\Walls\\link.jpg",
        )
        .expect_err("reparse components must fail closed");
        assert_eq!(error.code, "reparse_path");
    }

    #[test]
    fn synthetic_directory_target_is_rejected() {
        let error = validate_component_identities(
            &[FileIdentity {
                volume_serial: 1,
                file_id: [1; 16],
                attributes: FILE_ATTRIBUTE_DIRECTORY_BITS,
            }],
            "C:\\Walls\\folder",
        )
        .expect_err("directory targets must fail closed");
        assert_eq!(error.code, "directory_not_allowed");
    }

    #[test]
    fn injected_recycler_success_returns_recycled_without_real_shell_call() {
        let identity = IdentitySnapshot {
            resolved_path: "C:\\Walls\\ok.jpg".into(),
            components: Vec::new(),
        };
        let candidate = PreparedDelete {
            path: "C:\\Walls\\ok.jpg".into(),
            identity: identity.clone(),
        };
        let result = recycle_candidate_with(
            &candidate,
            |path| {
                assert_eq!(path, "C:\\Walls\\ok.jpg");
                Ok(identity.clone())
            },
            |_path| Ok(RecycleEvidence::ConfirmedRecycled),
        );
        assert!(matches!(result, RecycleAttempt::Recycled));
    }

    #[test]
    fn injected_recycler_error_with_unchanged_identity_is_not_recycled() {
        let identity = IdentitySnapshot {
            resolved_path: "C:\\Walls\\blocked.jpg".into(),
            components: Vec::new(),
        };
        let candidate = PreparedDelete {
            path: "C:\\Walls\\blocked.jpg".into(),
            identity: identity.clone(),
        };
        let result = recycle_candidate_with(
            &candidate,
            |_path| Ok(identity.clone()),
            |_path| Err("sharing violation".to_string()),
        );
        assert!(matches!(
            result,
            RecycleAttempt::Failed {
                status: DeleteStatus::NotRecycled,
                ..
            }
        ));
    }

    #[test]
    fn recycle_error_with_changed_identity_is_unknown() {
        let identity = IdentitySnapshot {
            resolved_path: "x".into(),
            components: Vec::new(),
        };
        let changed = IdentitySnapshot {
            resolved_path: "y".into(),
            components: Vec::new(),
        };
        let calls = std::cell::Cell::new(0);
        let candidate = PreparedDelete {
            path: "x".into(),
            identity: identity.clone(),
        };
        let result = recycle_candidate_with(
            &candidate,
            |_path| {
                let call = calls.get();
                calls.set(call + 1);
                if call == 0 {
                    Ok(identity.clone())
                } else {
                    Ok(changed.clone())
                }
            },
            |_path| Err("shell failed".to_string()),
        );
        assert!(matches!(
            result,
            RecycleAttempt::Failed {
                status: DeleteStatus::RecycleOutcomeUnknown,
                ..
            }
        ));
    }

    #[test]
    fn recycle_error_with_missing_identity_is_unknown() {
        let identity = IdentitySnapshot {
            resolved_path: "x".into(),
            components: Vec::new(),
        };
        let calls = std::cell::Cell::new(0);
        let candidate = PreparedDelete {
            path: "x".into(),
            identity: identity.clone(),
        };
        let result = recycle_candidate_with(
            &candidate,
            |_path| {
                let call = calls.get();
                calls.set(call + 1);
                if call == 0 {
                    Ok(identity.clone())
                } else {
                    Err(Rejection {
                        code: "missing",
                        message: "missing".to_string(),
                    })
                }
            },
            |_path| Err("shell failed".to_string()),
        );
        assert!(matches!(
            result,
            RecycleAttempt::Failed {
                status: DeleteStatus::RecycleOutcomeUnknown,
                ..
            }
        ));
    }

    #[test]
    fn shell_success_without_recycle_evidence_is_unknown() {
        let identity = IdentitySnapshot {
            resolved_path: "x".into(),
            components: Vec::new(),
        };
        let candidate = PreparedDelete {
            path: "x".into(),
            identity: identity.clone(),
        };
        let result = recycle_candidate_with(
            &candidate,
            |_path| Ok(identity.clone()),
            |_path| Ok(RecycleEvidence::OutcomeUnknown),
        );
        assert!(matches!(
            result,
            RecycleAttempt::Failed {
                status: DeleteStatus::RecycleOutcomeUnknown,
                ..
            }
        ));
    }

    #[test]
    fn mixed_candidates_preserve_order_and_truthful_statuses() {
        let paths = vec![
            "confirmed".to_string(),
            "rejected".to_string(),
            "unknown".to_string(),
        ];
        let prepared = vec![
            Ok(PreparedDelete {
                path: "confirmed".to_string(),
                identity: IdentitySnapshot {
                    resolved_path: "confirmed".to_string(),
                    components: Vec::new(),
                },
            }),
            Err(Rejection {
                code: "reparse_path",
                message: "reparse".to_string(),
            }),
            Ok(PreparedDelete {
                path: "unknown".to_string(),
                identity: IdentitySnapshot {
                    resolved_path: "unknown".to_string(),
                    components: Vec::new(),
                },
            }),
        ];
        let (results, successful) = process_candidates(paths, prepared, |candidate| {
            if candidate.path == "confirmed" {
                RecycleAttempt::Recycled
            } else {
                RecycleAttempt::Failed {
                    status: DeleteStatus::RecycleOutcomeUnknown,
                    code: "recycle_outcome_unknown",
                    message: "unknown".to_string(),
                }
            }
        });
        assert_eq!(
            results
                .iter()
                .map(|result| result.path.as_str())
                .collect::<Vec<_>>(),
            vec!["confirmed", "rejected", "unknown"]
        );
        assert_eq!(results[0].status, DeleteStatus::Recycled);
        assert_eq!(results[1].status, DeleteStatus::Rejected);
        assert_eq!(results[2].status, DeleteStatus::RecycleOutcomeUnknown);
        assert_eq!(successful, vec!["confirmed"]);
    }

    #[test]
    fn fixed_drive_predicate_accepts_only_fixed_volumes() {
        assert!(is_fixed_drive_type(DRIVE_FIXED));
        for drive_type in [0, 1, 2, 4, 5, 6] {
            assert!(!is_fixed_drive_type(drive_type), "drive type {drive_type}");
        }
    }

    #[test]
    fn shell_parsing_path_removes_only_verbatim_prefixes() {
        assert_eq!(
            shell_parsing_path(r"\\?\C:\Walls\wallpaper.jpg"),
            r"C:\Walls\wallpaper.jpg"
        );
        assert_eq!(
            shell_parsing_path(r"\\?\UNC\server\share\wallpaper.jpg"),
            r"\\server\share\wallpaper.jpg"
        );
        assert_eq!(
            shell_parsing_path(r"C:\Walls\wallpaper.jpg"),
            r"C:\Walls\wallpaper.jpg"
        );
        assert_eq!(
            shell_parsing_path(r"c:\Walls\wallpaper.jpg"),
            r"c:\Walls\wallpaper.jpg"
        );
    }

    #[test]
    fn recycle_worker_panic_maps_to_unknown_without_unwinding() {
        let joined = std::thread::spawn(|| -> Result<RecycleEvidence, String> {
            panic!("injected STA panic");
        })
        .join();
        assert_eq!(
            map_recycle_worker_join(joined).expect("panic is represented as unknown"),
            RecycleEvidence::OutcomeUnknown
        );
    }

    #[test]
    fn recycle_worker_join_preserves_result_and_error() {
        let success = std::thread::spawn(|| Ok(RecycleEvidence::ConfirmedRecycled))
            .join()
            .expect("worker should join");
        assert_eq!(
            map_recycle_worker_join(Ok(success)).expect("success should pass through"),
            RecycleEvidence::ConfirmedRecycled
        );
        let error = std::thread::spawn(|| Err::<RecycleEvidence, _>("shell".to_string()))
            .join()
            .expect("worker should join");
        assert_eq!(map_recycle_worker_join(Ok(error)), Err("shell".to_string()));
    }

    #[test]
    fn bulk_cleanup_failure_retries_each_recycled_path() {
        let paths = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let mut results = paths
            .iter()
            .cloned()
            .map(DeleteResult::recycled)
            .collect::<Vec<_>>();
        let mut retried = Vec::new();
        reconcile_metadata_with(
            &paths,
            &mut results,
            |_paths| Err("bulk failed".to_string()),
            |path| {
                retried.push(path.to_string());
                if path == "b" {
                    Err("single failed".to_string())
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(retried, paths);
        assert_eq!(results[0].status, DeleteStatus::Recycled);
        assert_eq!(
            results[1].status,
            DeleteStatus::RecycledMetadataCleanupFailed
        );
        assert_eq!(results[2].status, DeleteStatus::Recycled);
    }

    #[cfg(windows)]
    mod windows_inspection_tests {
        use super::*;
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        fn fixture_root(label: &str) -> PathBuf {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos();
            let root = std::env::temp_dir().join(format!("purewall-delete-{label}-{nonce}"));
            fs::create_dir_all(&root).expect("fixture root should be created");
            root
        }

        fn cleanup(root: &Path) {
            let _ = fs::remove_dir_all(root);
        }

        #[test]
        fn regular_fixed_volume_file_is_accepted_without_shell_effect() {
            let root = fixture_root("regular");
            let path = root.join("wallpaper.jpg");
            fs::write(&path, b"fixture").expect("fixture file should be written");

            let result = inspect_path(&path.to_string_lossy());

            assert!(
                result.is_ok(),
                "regular file should be inspectable: {result:?}"
            );
            cleanup(&root);
        }

        #[test]
        fn same_path_replacement_is_rejected_by_production_candidate_check() {
            let root = fixture_root("same-path");
            let path = root.join("wallpaper.jpg");
            fs::write(&path, b"first").expect("first fixture should be written");
            let identity =
                inspect_path(&path.to_string_lossy()).expect("first identity should capture");
            let candidate = PreparedDelete {
                path: path.to_string_lossy().into_owned(),
                identity,
            };
            fs::remove_file(&path).expect("first fixture should be removed");
            fs::write(&path, b"second").expect("replacement fixture should be written");

            let result = recycle_candidate_with(&candidate, inspect_path, |_path| {
                Err("injected shell must not run after identity mismatch".to_string())
            });

            assert!(matches!(
                result,
                RecycleAttempt::Failed {
                    status: DeleteStatus::Rejected,
                    code: "wallpaper_identity_changed",
                    ..
                }
            ));
            cleanup(&root);
        }

        #[test]
        fn parent_directory_replacement_is_rejected_by_chain_check() {
            let root = fixture_root("parent-replacement");
            let parent = root.join("parent");
            let old_parent = root.join("old-parent");
            fs::create_dir_all(&parent).expect("parent should be created");
            let path = parent.join("wallpaper.jpg");
            fs::write(&path, b"first").expect("first fixture should be written");
            let identity =
                inspect_path(&path.to_string_lossy()).expect("first identity should capture");
            let candidate = PreparedDelete {
                path: path.to_string_lossy().into_owned(),
                identity,
            };
            fs::rename(&parent, &old_parent).expect("parent should be replaced");
            fs::create_dir_all(&parent).expect("replacement parent should be created");
            fs::write(parent.join("wallpaper.jpg"), b"second")
                .expect("replacement file should be written");

            let result = recycle_candidate_with(&candidate, inspect_path, |_path| {
                Err("injected shell must not run after chain mismatch".to_string())
            });

            assert!(matches!(
                result,
                RecycleAttempt::Failed {
                    status: DeleteStatus::Rejected,
                    code: "wallpaper_identity_changed",
                    ..
                }
            ));
            cleanup(&root);
        }

        #[test]
        fn noncanonical_spelling_is_rejected_without_shell_effect() {
            let root = fixture_root("canonical");
            let path = root.join("wallpaper.jpg");
            fs::write(&path, b"fixture").expect("fixture file should be written");
            let noncanonical = root.join(".").join("wallpaper.jpg");

            let result = inspect_path(&noncanonical.to_string_lossy());

            assert!(matches!(
                result,
                Err(Rejection {
                    code: "canonical_path_mismatch",
                    ..
                })
            ));
            cleanup(&root);
        }
    }
}
