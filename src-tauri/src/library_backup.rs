use crate::paths::path_identity_key;
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) const MAX_BACKUP_BYTES: u64 = 32 * 1024 * 1024;
pub(crate) const MAX_RELATIONS_PER_WALLPAPER: usize = 256;
pub(crate) const MAX_TITLE_CHARS: usize = 512;

pub(crate) const BACKUP_APP: &str = "PureWall";
pub(crate) const BACKUP_SCHEMA_VERSION: u32 = 1;
pub(crate) const MAX_BACKUP_SOURCES: usize = 256;
pub(crate) const MAX_BACKUP_WALLPAPERS: usize = 100_000;
pub(crate) const MAX_BACKUP_TAGS: usize = 10_000;
pub(crate) const MAX_BACKUP_COLLECTIONS: usize = 10_000;
const MAX_PATH_CHARS: usize = 32_767;
const MAX_NAME_CHARS: usize = 128;
const MAX_COLOR_CHARS: usize = 32;
const MAX_SOURCE_CHARS: usize = 32;
const MAX_HASH_CHARS: usize = 256;
const MAX_ROTATION_SECS: u64 = 86_400;
static BACKUP_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupDocumentV1 {
    pub(crate) app: String,
    pub(crate) schema_version: u32,
    pub(crate) exported_at: String,
    pub(crate) sources: Vec<BackupSource>,
    pub(crate) wallpapers: Vec<BackupWallpaper>,
    pub(crate) tags: Vec<BackupNamedEntity>,
    pub(crate) collections: Vec<BackupNamedEntity>,
    #[serde(default)]
    pub(crate) settings: BackupSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct BackupSource {
    pub(crate) path: String,
    pub(crate) source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct BackupNamedEntity {
    pub(crate) name: String,
    pub(crate) color: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupWallpaper {
    pub(crate) path: String,
    pub(crate) source: String,
    pub(crate) hash: String,
    pub(crate) display_title: Option<String>,
    pub(crate) rating: i32,
    pub(crate) hidden: bool,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) file_size: u64,
    pub(crate) tags: Vec<String>,
    pub(crate) collections: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupSettings {
    pub(crate) rotation_secs: Option<u64>,
    pub(crate) display_mode: Option<String>,
    pub(crate) focus_mode_enabled: Option<bool>,
    pub(crate) paused: Option<bool>,
    pub(crate) theme: Option<String>,
    pub(crate) workspace_mode: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupClientSettings {
    pub(crate) theme: Option<String>,
    pub(crate) workspace_mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupImportPreview {
    pub(crate) content_digest: String,
    pub(crate) source_count: usize,
    pub(crate) wallpaper_count: usize,
    pub(crate) tag_count: usize,
    pub(crate) collection_count: usize,
    pub(crate) setting_count: usize,
    pub(crate) new_wallpapers: usize,
    pub(crate) overwritten_wallpapers: usize,
    pub(crate) missing_paths: usize,
    pub(crate) warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupMergeResult {
    pub(crate) added_wallpapers: usize,
    pub(crate) updated_wallpapers: usize,
    pub(crate) added_sources: usize,
    pub(crate) created_tags: usize,
    pub(crate) created_collections: usize,
    pub(crate) updated_settings: usize,
    pub(crate) client_settings: BackupClientSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupExportResult {
    pub(crate) path: String,
    pub(crate) bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupImportResult {
    pub(crate) committed: bool,
    pub(crate) merge: BackupMergeResult,
    pub(crate) warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BackupSourceFailure {
    pub(crate) path: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BackupSourceReconciliation {
    pub(crate) warnings: Vec<String>,
    pub(crate) failures: Vec<BackupSourceFailure>,
}

#[derive(Debug, Clone)]
pub(crate) struct NormalizedBackup {
    pub(crate) document: BackupDocumentV1,
}

#[derive(Debug, Clone, Copy)]
enum BackupErrorKind {
    TooLarge,
    InvalidApp,
    UnsupportedSchema,
    InvalidData,
    WriteFailed,
    UnsafeDestination,
    PreviewChanged,
}

#[derive(Debug, Clone)]
pub(crate) struct BackupError {
    kind: BackupErrorKind,
    message: String,
}

impl BackupError {
    fn new(kind: BackupErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub(crate) fn code(&self) -> &'static str {
        match self.kind {
            BackupErrorKind::TooLarge => "BACKUP_TOO_LARGE",
            BackupErrorKind::InvalidApp => "BACKUP_INVALID_APP",
            BackupErrorKind::UnsupportedSchema => "BACKUP_UNSUPPORTED_SCHEMA",
            BackupErrorKind::InvalidData => "BACKUP_INVALID_DATA",
            BackupErrorKind::WriteFailed => "BACKUP_WRITE_FAILED",
            BackupErrorKind::UnsafeDestination => "BACKUP_UNSAFE_DESTINATION",
            BackupErrorKind::PreviewChanged => "BACKUP_PREVIEW_CHANGED",
        }
    }

    fn invalid_data(message: impl Into<String>) -> Self {
        Self::new(BackupErrorKind::InvalidData, message)
    }

    fn write_failed(message: impl Into<String>) -> Self {
        Self::new(BackupErrorKind::WriteFailed, message)
    }

    fn unsafe_destination(message: impl Into<String>) -> Self {
        Self::new(BackupErrorKind::UnsafeDestination, message)
    }

    fn preview_changed(message: impl Into<String>) -> Self {
        Self::new(BackupErrorKind::PreviewChanged, message)
    }
}

impl fmt::Display for BackupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code(), self.message)
    }
}

impl std::error::Error for BackupError {}

pub(crate) fn parse_and_normalize(bytes: &[u8]) -> Result<NormalizedBackup, BackupError> {
    if bytes.len() as u64 > MAX_BACKUP_BYTES {
        return Err(too_large_error());
    }

    let mut document: BackupDocumentV1 = serde_json::from_slice(bytes)
        .map_err(|error| BackupError::invalid_data(format!("invalid backup JSON: {error}")))?;

    if document.app != BACKUP_APP {
        return Err(BackupError::new(
            BackupErrorKind::InvalidApp,
            format!("expected app {BACKUP_APP}"),
        ));
    }
    if document.schema_version != BACKUP_SCHEMA_VERSION {
        return Err(BackupError::new(
            BackupErrorKind::UnsupportedSchema,
            format!(
                "unsupported schema version {}; expected {BACKUP_SCHEMA_VERSION}",
                document.schema_version
            ),
        ));
    }
    DateTime::parse_from_rfc3339(&document.exported_at)
        .map_err(|_| BackupError::invalid_data("exportedAt must be RFC 3339"))?;

    check_count("sources", document.sources.len(), MAX_BACKUP_SOURCES)?;
    check_count(
        "wallpapers",
        document.wallpapers.len(),
        MAX_BACKUP_WALLPAPERS,
    )?;
    check_count("tags", document.tags.len(), MAX_BACKUP_TAGS)?;
    check_count(
        "collections",
        document.collections.len(),
        MAX_BACKUP_COLLECTIONS,
    )?;

    normalize_sources(&mut document.sources)?;
    let tag_names = normalize_named_entities(&mut document.tags, "tag")?;
    let collection_names = normalize_named_entities(&mut document.collections, "collection")?;
    normalize_wallpapers(&mut document.wallpapers, &tag_names, &collection_names)?;
    normalize_settings(&mut document.settings)?;

    Ok(NormalizedBackup { document })
}

pub(crate) fn read_bounded(path: &Path) -> Result<Vec<u8>, BackupError> {
    if !path.is_absolute() {
        return Err(BackupError::write_failed("backup path must be absolute"));
    }
    crate::scanner::ensure_local_path(path)
        .map_err(|error| BackupError::write_failed(error.to_string()))?;
    let metadata = fs::metadata(path)
        .map_err(|error| BackupError::write_failed(format!("cannot read backup: {error}")))?;
    if metadata.len() > MAX_BACKUP_BYTES {
        return Err(too_large_error());
    }

    let file = File::open(path)
        .map_err(|error| BackupError::write_failed(format!("cannot open backup: {error}")))?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_BACKUP_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| BackupError::write_failed(format!("cannot read backup: {error}")))?;
    if bytes.len() as u64 > MAX_BACKUP_BYTES {
        return Err(too_large_error());
    }
    Ok(bytes)
}

pub(crate) fn backup_content_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256:{digest:x}")
}

pub(crate) fn verify_preview_digest(
    bytes: &[u8],
    expected_digest: &str,
) -> Result<(), BackupError> {
    let actual_digest = backup_content_digest(bytes);
    if expected_digest.is_empty() || actual_digest != expected_digest {
        return Err(BackupError::preview_changed(
            "The backup file changed after preview. Preview it again before importing.",
        ));
    }
    Ok(())
}

fn guarded_path(path: &Path) -> PathBuf {
    if path.exists() {
        return path
            .canonicalize()
            .unwrap_or_else(|_| lexical_normalize(path));
    }

    match (path.parent(), path.file_name()) {
        (Some(parent), Some(file_name)) => {
            let parent = parent
                .canonicalize()
                .unwrap_or_else(|_| lexical_normalize(parent));
            parent.join(file_name)
        }
        _ => lexical_normalize(path),
    }
}

pub(crate) fn validate_export_destination(
    destination: &Path,
    protected_wallpapers: &[String],
    app_data_root: Option<&Path>,
) -> Result<(), BackupError> {
    if !destination.is_absolute() {
        return Err(BackupError::unsafe_destination(
            "Backup destination must be absolute.",
        ));
    }
    crate::scanner::ensure_local_path(destination)
        .map_err(|error| BackupError::unsafe_destination(error.to_string()))?;
    if !destination
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    {
        return Err(BackupError::unsafe_destination(
            "Backup destination must use the .json extension.",
        ));
    }

    let guarded_destination = guarded_path(destination);
    let destination_identity = path_identity_key(&guarded_destination);
    if protected_wallpapers.iter().any(|wallpaper| {
        path_identity_key(&guarded_path(Path::new(wallpaper))) == destination_identity
    }) {
        return Err(BackupError::unsafe_destination(
            "Backup destination cannot replace a registered wallpaper.",
        ));
    }

    if app_data_root.is_some_and(|root| {
        crate::paths::path_is_same_or_descendant(&guarded_destination, &guarded_path(root))
    }) {
        return Err(BackupError::unsafe_destination(
            "Backup destination cannot be inside PureWall application data.",
        ));
    }

    Ok(())
}

pub(crate) fn serialize_bounded(document: &BackupDocumentV1) -> Result<Vec<u8>, BackupError> {
    let compact = serde_json::to_vec(document)
        .map_err(|error| BackupError::invalid_data(format!("cannot serialize backup: {error}")))?;
    if compact.len() as u64 > MAX_BACKUP_BYTES {
        return Err(too_large_error());
    }
    parse_and_normalize(&compact)?;

    let mut bytes = serde_json::to_vec_pretty(document)
        .map_err(|error| BackupError::invalid_data(format!("cannot serialize backup: {error}")))?;
    bytes.push(b'\n');
    if bytes.len() as u64 > MAX_BACKUP_BYTES {
        return Err(too_large_error());
    }
    Ok(bytes)
}

pub(crate) fn write_atomically(destination: &Path, bytes: &[u8]) -> Result<(), BackupError> {
    if bytes.len() as u64 > MAX_BACKUP_BYTES {
        return Err(too_large_error());
    }
    if !destination.is_absolute() {
        return Err(BackupError::write_failed(
            "backup destination must be absolute",
        ));
    }
    crate::scanner::ensure_local_path(destination)
        .map_err(|error| BackupError::write_failed(error.to_string()))?;

    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| BackupError::write_failed("backup destination has no parent"))?;
    let file_name = destination
        .file_name()
        .ok_or_else(|| BackupError::write_failed("backup destination has no file name"))?
        .to_string_lossy();

    for _ in 0..64 {
        let sequence = BACKUP_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temp_path = parent.join(format!(
            ".{file_name}.purewall-{}-{sequence}.tmp",
            std::process::id()
        ));
        let mut temp_file = match OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(BackupError::write_failed(format!(
                    "cannot create backup temp file: {error}"
                )));
            }
        };

        let write_result = temp_file
            .write_all(bytes)
            .and_then(|_| temp_file.flush())
            .and_then(|_| temp_file.sync_all());
        drop(temp_file);
        if let Err(error) = write_result {
            let _ = fs::remove_file(&temp_path);
            return Err(BackupError::write_failed(format!(
                "cannot write backup temp file: {error}"
            )));
        }

        if let Err(error) = install_temp_file(&temp_path, destination) {
            let _ = fs::remove_file(&temp_path);
            return Err(error);
        }
        return Ok(());
    }

    Err(BackupError::write_failed(
        "cannot reserve a unique backup temp file",
    ))
}

fn too_large_error() -> BackupError {
    BackupError::new(
        BackupErrorKind::TooLarge,
        format!("backup exceeds the {MAX_BACKUP_BYTES}-byte limit"),
    )
}

#[cfg(windows)]
fn install_temp_file(temp_path: &Path, destination: &Path) -> Result<(), BackupError> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let temp_wide = temp_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let destination_wide = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    unsafe {
        MoveFileExW(
            PCWSTR(temp_wide.as_ptr()),
            PCWSTR(destination_wide.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(|error| BackupError::write_failed(format!("cannot install backup: {error}")))
}

#[cfg(not(windows))]
fn install_temp_file(temp_path: &Path, destination: &Path) -> Result<(), BackupError> {
    fs::rename(temp_path, destination)
        .map_err(|error| BackupError::write_failed(format!("cannot install backup: {error}")))
}

pub(crate) fn reconcile_persisted_sources<F>(
    sources: &[BackupSource],
    mut start_watcher: F,
) -> BackupSourceReconciliation
where
    F: FnMut(&str, &str) -> Result<(), String>,
{
    let mut warnings = Vec::new();
    let mut failures = Vec::new();
    for source in sources {
        if !Path::new(&source.path).is_dir() {
            let message = format!("Source is offline: {}", source.path);
            warnings.push(message.clone());
            failures.push(BackupSourceFailure {
                path: source.path.clone(),
                message,
            });
            continue;
        }

        if let Err(error) = start_watcher(&source.path, &source.source) {
            let message = format!("Watcher could not start for {}: {error}", source.path);
            warnings.push(message.clone());
            failures.push(BackupSourceFailure {
                path: source.path.clone(),
                message,
            });
        }
    }

    BackupSourceReconciliation { warnings, failures }
}

fn check_count(label: &str, actual: usize, maximum: usize) -> Result<(), BackupError> {
    if actual > maximum {
        Err(BackupError::invalid_data(format!(
            "too many {label}: {actual} exceeds {maximum}"
        )))
    } else {
        Ok(())
    }
}

fn normalize_sources(sources: &mut [BackupSource]) -> Result<(), BackupError> {
    let mut identities = HashSet::with_capacity(sources.len());
    for source in sources {
        source.source =
            normalize_source_kind(&source.source, &["mounted", "imported-folder"], "source")?;
        source.path = normalize_path(&source.path, ExistingPathKind::Directory)?;
        if !identities.insert(path_identity_key(Path::new(&source.path))) {
            return Err(BackupError::invalid_data("duplicate source path"));
        }
    }
    Ok(())
}

fn normalize_named_entities(
    values: &mut [BackupNamedEntity],
    label: &str,
) -> Result<HashSet<String>, BackupError> {
    let mut names = HashSet::with_capacity(values.len());
    for value in values {
        value.name = normalize_required_text(&value.name, MAX_NAME_CHARS, label)?;
        value.color =
            normalize_optional_text(&value.color, MAX_COLOR_CHARS, "color")?.unwrap_or_default();
        if !names.insert(value.name.clone()) {
            return Err(BackupError::invalid_data(format!("duplicate {label} name")));
        }
    }
    Ok(names)
}

fn normalize_wallpapers(
    wallpapers: &mut [BackupWallpaper],
    tag_names: &HashSet<String>,
    collection_names: &HashSet<String>,
) -> Result<(), BackupError> {
    let mut identities = HashSet::with_capacity(wallpapers.len());
    for wallpaper in wallpapers {
        wallpaper.source = normalize_source_kind(
            &wallpaper.source,
            &["mounted", "imported-folder", "imported-file", "dropped"],
            "wallpaper source",
        )?;
        wallpaper.path = normalize_path(&wallpaper.path, ExistingPathKind::File)?;
        if !identities.insert(path_identity_key(Path::new(&wallpaper.path))) {
            return Err(BackupError::invalid_data("duplicate wallpaper path"));
        }
        wallpaper.hash =
            normalize_optional_text(&wallpaper.hash, MAX_HASH_CHARS, "hash")?.unwrap_or_default();
        wallpaper.display_title = match wallpaper.display_title.take() {
            Some(value) => normalize_optional_text(&value, MAX_TITLE_CHARS, "display title")?,
            None => None,
        };
        if !matches!(wallpaper.rating, -1..=1) {
            return Err(BackupError::invalid_data("rating must be between -1 and 1"));
        }
        normalize_relation_names(&mut wallpaper.tags, tag_names, "tag")?;
        normalize_relation_names(&mut wallpaper.collections, collection_names, "collection")?;
    }
    Ok(())
}

fn normalize_relation_names(
    relations: &mut Vec<String>,
    definitions: &HashSet<String>,
    label: &str,
) -> Result<(), BackupError> {
    check_count(label, relations.len(), MAX_RELATIONS_PER_WALLPAPER)?;
    let mut seen = HashSet::with_capacity(relations.len());
    let mut normalized = Vec::with_capacity(relations.len());
    for relation in relations.drain(..) {
        let relation = normalize_required_text(&relation, MAX_NAME_CHARS, label)?;
        if !definitions.contains(&relation) {
            return Err(BackupError::invalid_data(format!(
                "undefined {label}: {relation}"
            )));
        }
        if seen.insert(relation.clone()) {
            normalized.push(relation);
        }
    }
    *relations = normalized;
    Ok(())
}

fn normalize_settings(settings: &mut BackupSettings) -> Result<(), BackupError> {
    if settings
        .rotation_secs
        .is_some_and(|seconds| seconds > MAX_ROTATION_SECS)
    {
        return Err(BackupError::invalid_data(
            "rotationSecs exceeds the supported limit",
        ));
    }
    normalize_setting_choice(
        &mut settings.display_mode,
        &["all", "span", "independent"],
        "displayMode",
    )?;
    normalize_setting_choice(&mut settings.theme, &["system", "light", "dark"], "theme")?;
    normalize_setting_choice(
        &mut settings.workspace_mode,
        &["workbench", "quiet"],
        "workspaceMode",
    )?;
    Ok(())
}

fn normalize_setting_choice(
    value: &mut Option<String>,
    allowed: &[&str],
    label: &str,
) -> Result<(), BackupError> {
    let Some(current) = value.take() else {
        return Ok(());
    };
    let normalized = normalize_required_text(&current, MAX_NAME_CHARS, label)?;
    if !allowed.contains(&normalized.as_str()) {
        return Err(BackupError::invalid_data(format!(
            "unsupported {label}: {normalized}"
        )));
    }
    *value = Some(normalized);
    Ok(())
}

fn normalize_source_kind(
    value: &str,
    allowed: &[&str],
    label: &str,
) -> Result<String, BackupError> {
    let normalized = normalize_required_text(value, MAX_SOURCE_CHARS, label)?;
    if allowed.contains(&normalized.as_str()) {
        Ok(normalized)
    } else {
        Err(BackupError::invalid_data(format!(
            "unsupported {label}: {normalized}"
        )))
    }
}

fn normalize_required_text(
    value: &str,
    maximum: usize,
    label: &str,
) -> Result<String, BackupError> {
    normalize_optional_text(value, maximum, label)?
        .ok_or_else(|| BackupError::invalid_data(format!("{label} cannot be empty")))
}

fn normalize_optional_text(
    value: &str,
    maximum: usize,
    label: &str,
) -> Result<Option<String>, BackupError> {
    let normalized = value.trim();
    if normalized.chars().count() > maximum {
        return Err(BackupError::invalid_data(format!(
            "{label} exceeds {maximum} characters"
        )));
    }
    if normalized.is_empty() {
        Ok(None)
    } else {
        Ok(Some(normalized.to_string()))
    }
}

#[derive(Clone, Copy)]
enum ExistingPathKind {
    Directory,
    File,
}

fn normalize_path(value: &str, existing_kind: ExistingPathKind) -> Result<String, BackupError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(BackupError::invalid_data("path cannot be empty"));
    }
    if value.chars().count() > MAX_PATH_CHARS {
        return Err(BackupError::invalid_data(
            "path exceeds the supported limit",
        ));
    }

    let path = Path::new(value);
    if !path.is_absolute() {
        return Err(BackupError::invalid_data("path must be absolute"));
    }
    crate::scanner::ensure_local_path(path)
        .map_err(|error| BackupError::invalid_data(error.to_string()))?;

    let normalized = if path.exists() {
        match existing_kind {
            ExistingPathKind::Directory if !path.is_dir() => {
                return Err(BackupError::invalid_data(
                    "source path must refer to a directory",
                ));
            }
            ExistingPathKind::File if !path.is_file() => {
                return Err(BackupError::invalid_data(
                    "wallpaper path must refer to a file",
                ));
            }
            _ => {}
        }
        path.canonicalize()
            .map_err(|error| BackupError::invalid_data(error.to_string()))?
    } else {
        lexical_normalize(path)
    };

    let normalized = normalized.to_string_lossy().to_string();
    if normalized.chars().count() > MAX_PATH_CHARS {
        return Err(BackupError::invalid_data(
            "path exceeds the supported limit",
        ));
    }
    Ok(normalized)
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}
