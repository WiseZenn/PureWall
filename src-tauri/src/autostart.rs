use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

const REG_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const APP_NAME: &str = "PureWall";

fn autostart_value_name() -> &'static str {
    APP_NAME
}
fn get_exe_path() -> Result<PathBuf> {
    std::env::current_exe().map_err(|e| anyhow::anyhow!("Failed to get exe path: {}", e))
}

fn quoted_exe_value(exe: &Path) -> Result<String> {
    let exe_str = exe.to_string_lossy();
    if exe_str.contains('"') {
        anyhow::bail!("Executable path contains an unsupported quote character");
    }
    Ok(format!("\"{}\"", exe_str))
}

fn command_stderr(command_name: &str, output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        format!("{command_name} exited with status {}", output.status)
    } else {
        stderr
    }
}

/// Enable autostart by adding a registry Run entry
pub fn enable() -> Result<()> {
    let exe = get_exe_path()?;
    let value = quoted_exe_value(&exe)?;

    let output = Command::new("reg.exe")
        .args([
            "add",
            REG_KEY,
            "/v",
            autostart_value_name(),
            "/t",
            "REG_SZ",
            "/d",
            &value,
            "/f",
        ])
        .output()
        .context("Failed to run reg.exe")?;

    if !output.status.success() {
        anyhow::bail!(
            "Failed to enable autostart: {}",
            command_stderr("reg add", &output)
        );
    }

    Ok(())
}

/// Disable autostart by removing the registry Run entry
pub fn disable() -> Result<()> {
    if !is_enabled() {
        return Ok(());
    }

    let output = Command::new("reg.exe")
        .args(["delete", REG_KEY, "/v", autostart_value_name(), "/f"])
        .output()
        .context("Failed to run reg.exe")?;

    if !output.status.success() {
        anyhow::bail!(
            "Failed to disable autostart: {}",
            command_stderr("reg delete", &output)
        );
    }

    Ok(())
}

/// Check if autostart is enabled
pub fn is_enabled() -> bool {
    if let Ok(output) = Command::new("reg.exe")
        .args(["query", REG_KEY, "/v", autostart_value_name()])
        .output()
    {
        output.status.success()
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn autostart_targets_only_purewall_run_value() {
        assert_eq!(
            REG_KEY,
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run"
        );
        assert_eq!(autostart_value_name(), "PureWall");
    }

    #[test]
    fn quoted_exe_value_wraps_path_and_rejects_quotes() {
        let value = quoted_exe_value(Path::new(r"C:\Program Files\PureWall\purewall.exe"))
            .expect("normal executable paths should quote successfully");
        assert_eq!(value, r#""C:\Program Files\PureWall\purewall.exe""#);

        let err = quoted_exe_value(Path::new("C:\\bad\"path\\purewall.exe"))
            .expect_err("embedded quotes must be rejected");
        assert!(
            err.to_string().contains("unsupported quote"),
            "unexpected error: {err}"
        );
    }
}
