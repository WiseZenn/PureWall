use std::path::{Path, PathBuf};

/// App data directory shared by CLI actions, diagnostics, and context-menu
/// registration. Must match the Tauri bundle identifier (`com.purewall.app`).
pub(crate) fn app_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_default()
        .join("com.purewall.app")
}

#[cfg(windows)]
pub(crate) fn path_identity_key(path: &Path) -> String {
    let normalized = path
        .to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase();
    if let Some(path) = normalized.strip_prefix(r"\\?\unc\") {
        format!(r"\\{path}")
    } else {
        normalized
            .strip_prefix(r"\\?\")
            .unwrap_or(&normalized)
            .to_string()
    }
}

#[cfg(not(windows))]
pub(crate) fn path_identity_key(path: &Path) -> String {
    path.to_string_lossy().trim_end_matches('/').to_string()
}

#[cfg(windows)]
pub(crate) fn path_is_same_or_descendant(candidate: &Path, root: &Path) -> bool {
    let candidate = path_identity_key(candidate);
    let root = path_identity_key(root);
    candidate == root
        || candidate
            .strip_prefix(&root)
            .is_some_and(|suffix| suffix.starts_with('\\'))
}

#[cfg(not(windows))]
pub(crate) fn path_is_same_or_descendant(candidate: &Path, root: &Path) -> bool {
    candidate == root || candidate.starts_with(root)
}

pub(crate) fn paths_overlap(left: &Path, right: &Path) -> bool {
    path_is_same_or_descendant(left, right) || path_is_same_or_descendant(right, left)
}

pub(crate) fn relative_identity(candidate: &Path, root: &Path) -> Option<String> {
    let candidate = path_identity_key(candidate);
    let root = path_identity_key(root);
    if candidate == root {
        return Some(String::new());
    }
    let separator = if cfg!(windows) { '\\' } else { '/' };
    candidate
        .strip_prefix(&root)
        .and_then(|suffix| suffix.strip_prefix(separator))
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::{path_is_same_or_descendant, paths_overlap, relative_identity};
    use std::path::Path;

    #[test]
    fn source_paths_compare_by_windows_identity() {
        assert!(path_is_same_or_descendant(
            Path::new(r"\\?\D:\Walls\Nature\lake.jpg"),
            Path::new(r"d:\walls"),
        ));
        assert!(paths_overlap(
            Path::new(r"D:\Walls"),
            Path::new(r"d:/walls/Nature"),
        ));
        assert_eq!(
            relative_identity(
                Path::new(r"\\?\D:\Walls\Nature\Lake.JPG"),
                Path::new(r"d:\walls"),
            )
            .as_deref(),
            Some(r"nature\lake.jpg"),
        );
    }

    #[test]
    fn sibling_sources_do_not_overlap() {
        assert!(!paths_overlap(
            Path::new(r"D:\Walls-A"),
            Path::new(r"D:\Walls-B"),
        ));
        assert_eq!(
            relative_identity(Path::new(r"D:\Walls-B\image.jpg"), Path::new(r"D:\Walls-A"),),
            None,
        );
    }
}
