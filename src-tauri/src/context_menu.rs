use anyhow::Result;
use std::path::PathBuf;

const CONTEXT_MENU_ICON: &[u8] = include_bytes!("../icons/icon.ico");
const CONTEXT_MENU_BASE: &str = r"HKCU\Software\Classes\Directory\Background\shell";
const LEGACY_COMMANDS_ROOT: &str = r"HKCU\Software\Classes\PureWall_Commands";

fn get_exe_path() -> Result<PathBuf> {
    std::env::current_exe().map_err(|e| anyhow::anyhow!("Failed to get exe path: {}", e))
}

fn app_data_dir() -> PathBuf {
    crate::paths::app_data_dir()
}

fn context_menu_key(name: &str) -> String {
    format!(r"{}\{}", CONTEXT_MENU_BASE, name)
}

fn purewall_context_menu_root() -> String {
    context_menu_key("PureWall")
}

fn unregister_targets() -> Vec<String> {
    ["PWNext", "PWLike", "PWDislike", "PWPause", "PureWall"]
        .into_iter()
        .map(context_menu_key)
        .chain(std::iter::once(LEGACY_COMMANDS_ROOT.to_string()))
        .collect()
}
fn ensure_context_menu_icon() -> Result<PathBuf> {
    let icon_path = app_data_dir().join("purewall-menu.ico");
    if let Some(parent) = icon_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&icon_path, CONTEXT_MENU_ICON)?;
    Ok(icon_path)
}

/// Register cascading context menu using direct reg.exe calls from Rust
/// std::process::Command passes args directly — no shell escaping issues
pub fn register() -> Result<()> {
    let _ = unregister();

    let exe = get_exe_path()?;
    let icon = ensure_context_menu_icon()?;
    let exe_str = exe.to_string_lossy();
    let icon_str = icon.to_string_lossy();
    let base = CONTEXT_MENU_BASE;

    // Parent
    reg_add(&format!(r"{}\PureWall", base), "MUIVerb", "PureWall")?;
    reg_add(&format!(r"{}\PureWall", base), "Icon", &icon_str)?;
    reg_add(&format!(r"{}\PureWall", base), "SubCommands", "")?;

    // Sub-items: reg.exe add <path>\command /ve /t REG_SZ /d "<exe>" --action <x> /f
    let items: &[(&str, &str, &str)] = &[
        ("01_Next", "Next", "next"),
        ("02_Like", "Like", "like"),
        ("03_Dislike", "Dislike", "dislike"),
        ("04_Pause", "Pause", "pause"),
    ];

    for (id, label, action) in items {
        let sub = format!(r"{}\PureWall\shell\{}", base, id);
        reg_add(&sub, "MUIVerb", label)?;
        reg_add(&sub, "Icon", &icon_str)?;
        let cmd_val = format!("\"{}\" --action {}", exe_str, action);
        reg_add_ve(&format!(r"{}\command", sub), &cmd_val)?;
    }

    Ok(())
}

/// Remove all PureWall entries (current + all historical)
pub fn unregister() -> Result<()> {
    for key in unregister_targets() {
        let _ = reg_delete(&key);
    }

    Ok(())
}

pub fn is_registered() -> bool {
    std::process::Command::new("reg")
        .args(["query", &purewall_context_menu_root()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// === reg.exe helpers (direct process calls, no shell) ===

/// reg.exe add <key> /v <name> /t REG_SZ /d <value> /f
fn reg_add(key: &str, name: &str, value: &str) -> Result<()> {
    let status = std::process::Command::new("reg")
        .args(["add", key, "/v", name, "/t", "REG_SZ", "/d", value, "/f"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| anyhow::anyhow!("reg add failed: {}", e))?;

    if !status.success() {
        anyhow::bail!("reg add {} /v {} failed", key, name);
    }
    Ok(())
}

/// reg.exe add <key> /ve /t REG_SZ /d <value> /f  (default/unnamed value)
fn reg_add_ve(key: &str, value: &str) -> Result<()> {
    let status = std::process::Command::new("reg")
        .args(["add", key, "/ve", "/t", "REG_SZ", "/d", value, "/f"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| anyhow::anyhow!("reg add /ve failed: {}", e))?;

    if !status.success() {
        anyhow::bail!("reg add {} /ve failed", key);
    }
    Ok(())
}

/// reg.exe delete <key> /f
fn reg_delete(key: &str) -> Result<()> {
    let status = std::process::Command::new("reg")
        .args(["delete", key, "/f"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| anyhow::anyhow!("reg delete failed: {}", e))?;

    if !status.success() {
        anyhow::bail!("reg delete {} failed", key);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unregister_targets_are_limited_to_purewall_owned_keys() {
        let targets = unregister_targets();
        assert!(targets.contains(&purewall_context_menu_root()));
        assert!(targets.contains(&LEGACY_COMMANDS_ROOT.to_string()));
        assert_eq!(targets.len(), 6);

        for target in targets {
            let is_shell_entry = target.starts_with(CONTEXT_MENU_BASE)
                && ["PWNext", "PWLike", "PWDislike", "PWPause", "PureWall"]
                    .iter()
                    .any(|name| target.ends_with(name));
            let is_legacy_commands = target == LEGACY_COMMANDS_ROOT;
            assert!(
                is_shell_entry || is_legacy_commands,
                "unexpected unregister target: {target}"
            );
        }
    }
}
