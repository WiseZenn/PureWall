use std::io::{Read, Seek, SeekFrom};

use crate::CommandResult;

const CLI_LOG_TAIL_BYTES: u64 = 32 * 1024;

fn cli_log_file() -> std::path::PathBuf {
    crate::paths::app_data_dir().join("purewall-cli.log")
}

const CLI_LOG_MAX_BYTES: u64 = 512 * 1024;

pub fn append_cli_log(action: &str, message: &str) {
    let log_path = cli_log_file();
    let _ = std::fs::create_dir_all(crate::paths::app_data_dir());

    // Trim the log when it exceeds the size cap to prevent unbounded
    // growth over years of use (MED-08).
    if let Ok(meta) = std::fs::metadata(&log_path) {
        if meta.len() > CLI_LOG_MAX_BYTES {
            if let Ok(bytes) = std::fs::read(&log_path) {
                let start = bytes.len().saturating_sub(CLI_LOG_MAX_BYTES as usize / 2);
                if let Some(cut) = bytes[start..].iter().position(|b| *b == b'\n') {
                    let _ = std::fs::write(&log_path, &bytes[start + cut + 1..]);
                }
            }
        }
    }

    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
    {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs().to_string())
            .unwrap_or_else(|_| "unknown-time".to_string());
        let _ = std::io::Write::write_fmt(
            &mut file,
            format_args!("[{}] --action {}: {}\n", timestamp, action, message),
        );
    }
}

#[tauri::command]
pub fn read_cli_log() -> CommandResult<String> {
    let log_path = cli_log_file();
    if !log_path.exists() {
        return Ok(String::new());
    }

    let mut file =
        std::fs::File::open(&log_path).map_err(|e| format!("Failed to open CLI log: {}", e))?;
    let len = file
        .metadata()
        .map_err(|e| format!("Failed to inspect CLI log: {}", e))?
        .len();
    let start = len.saturating_sub(CLI_LOG_TAIL_BYTES);
    file.seek(SeekFrom::Start(start))
        .map_err(|e| format!("Failed to seek CLI log: {}", e))?;

    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|e| format!("Failed to read CLI log: {}", e))?;

    let mut text = String::from_utf8_lossy(&bytes).to_string();
    if start > 0 {
        if let Some(newline) = text.find('\n') {
            text = text[newline + 1..].to_string();
        }
    }

    Ok(text)
}
