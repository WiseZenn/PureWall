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
    lexical_path_identity(candidate)
        .zip(lexical_path_identity(root))
        .is_some_and(|(candidate, root)| {
            candidate == root
                || candidate
                    .strip_prefix(&root)
                    .is_some_and(|suffix| suffix.starts_with('\\'))
        })
}

/// Component-aware, no-IO containment for untrusted filesystem event paths.
/// ParentDir is rejected rather than resolved so lexical escapes fail closed.
#[cfg(windows)]
pub(crate) fn lexical_path_identity(path: &Path) -> Option<String> {
    use std::path::{Component, Prefix};
    let raw = path.to_string_lossy().replace('/', "\\");
    let raw = if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = raw.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        raw
    };
    let mut output = String::new();
    let mut rooted = false;
    for component in Path::new(&raw).components() {
        match component {
            Component::Prefix(prefix) => {
                rooted = true;
                output.push_str(&prefix.as_os_str().to_string_lossy().to_lowercase());
                if matches!(prefix.kind(), Prefix::UNC(_, _) | Prefix::VerbatimUNC(_, _)) {
                    output.push('\\');
                }
            }
            Component::RootDir => {
                rooted = true;
                if !output.ends_with('\\') {
                    output.push('\\');
                }
            }
            Component::CurDir => {}
            Component::ParentDir => return None,
            Component::Normal(value) => {
                if !output.is_empty() && !output.ends_with('\\') {
                    output.push('\\');
                }
                output.push_str(&value.to_string_lossy().to_lowercase());
            }
        }
    }
    if !rooted {
        return None;
    }
    while output.ends_with('\\') && output.len() > 1 {
        output.pop();
    }
    Some(output)
}

#[cfg(not(windows))]
pub(crate) fn lexical_path_identity(path: &Path) -> Option<String> {
    use std::path::Component;
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::CurDir => {}
            Component::ParentDir => return None,
            Component::Normal(value) => parts.push(value.to_string_lossy().to_string()),
            Component::Prefix(_) => return None,
        }
    }
    Some(format!("/{}", parts.join("/")))
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
