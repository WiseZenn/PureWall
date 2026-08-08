use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LibrarySourceStatus {
    Online,
    Offline,
    Scanning,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct LibrarySource {
    pub path: String,
    pub source: String,
    pub status: LibrarySourceStatus,
    pub available_count: usize,
    pub unavailable_count: usize,
    pub last_scan_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoveLibrarySourceMode {
    KeepMetadata,
    ClearMetadata,
}

impl From<RemoveLibrarySourceMode> for crate::db::SourceRemovalMode {
    fn from(value: RemoveLibrarySourceMode) -> Self {
        match value {
            RemoveLibrarySourceMode::KeepMetadata => Self::KeepMetadata,
            RemoveLibrarySourceMode::ClearMetadata => Self::ClearMetadata,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RemoveLibrarySourceImpact {
    pub affected_wallpapers: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct LibrarySourceMutation {
    pub affected_wallpapers: usize,
    pub matched_wallpapers: usize,
    pub imported_wallpapers: usize,
    pub unavailable_wallpapers: usize,
    pub watcher_warning: Option<String>,
}

pub fn status_for(
    path: &str,
    scanning_paths: &HashSet<String>,
    last_error: Option<&str>,
) -> LibrarySourceStatus {
    if scanning_paths.contains(path) {
        LibrarySourceStatus::Scanning
    } else if !Path::new(path).is_dir() {
        LibrarySourceStatus::Offline
    } else if last_error.is_some() {
        LibrarySourceStatus::Error
    } else {
        LibrarySourceStatus::Online
    }
}

pub struct SourceOperationGuard<'a> {
    path: String,
    operations: &'a Mutex<HashSet<String>>,
}

impl<'a> SourceOperationGuard<'a> {
    pub fn begin(operations: &'a Mutex<HashSet<String>>, path: &str) -> Result<Self, &'static str> {
        let mut active = operations
            .lock()
            .map_err(|_| "source operations unavailable")?;
        if !active.insert(path.to_string()) {
            return Err("source operation already running");
        }
        Ok(Self {
            path: path.to_string(),
            operations,
        })
    }
}

impl Drop for SourceOperationGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut active) = self.operations.lock() {
            active.remove(&self.path);
        }
    }
}
