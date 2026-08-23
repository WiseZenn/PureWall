use std::path::Path;

use anyhow::{bail, Context, Result};

const OWNED_ROOT_PREFIX: &str = "purewall-watcher-lifecycle-";
const OWNERSHIP_MARKER: &[u8] = b"purewall-watcher-lifecycle-v1\n";

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
