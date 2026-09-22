use crate::{db::Database, load_restored_playback_state};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) struct TempPlaybackFiles {
    db_path: PathBuf,
    wallpaper_paths: Vec<PathBuf>,
}

impl TempPlaybackFiles {
    pub(super) fn new(db_path: PathBuf, wallpaper_paths: Vec<PathBuf>) -> Self {
        Self {
            db_path,
            wallpaper_paths,
        }
    }
}

impl Drop for TempPlaybackFiles {
    fn drop(&mut self) {
        for path in &self.wallpaper_paths {
            let _ = std::fs::remove_file(path);
        }
        for path in [
            self.db_path.clone(),
            self.db_path.with_extension("db-shm"),
            self.db_path.with_extension("db-wal"),
        ] {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn create_registered_temp_wallpaper(name: &str) -> (TempPlaybackFiles, String) {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after the Unix epoch")
        .as_nanos();
    let prefix = format!("purewall-playback-{name}-{}-{unique}", std::process::id());
    let db_path = std::env::temp_dir().join(format!("{prefix}.db"));
    let wallpaper_path = std::env::temp_dir().join(format!("{prefix}.jpg"));
    std::fs::write(&wallpaper_path, b"test wallpaper")
        .expect("temporary wallpaper should be writable");

    let files = TempPlaybackFiles::new(db_path, vec![wallpaper_path.clone()]);
    (files, wallpaper_path.to_string_lossy().to_string())
}

#[test]
fn restart_restores_current_pause_interval_and_display_mode() {
    let (files, wallpaper_path) = create_registered_temp_wallpaper("restart-state");
    {
        let db = Database::new(&files.db_path).expect("database should initialize");
        db.upsert_wallpaper(&wallpaper_path, "hash", "test", 100, 100, 10)
            .expect("wallpaper should register");
        db.set_setting("current_wallpaper", &wallpaper_path)
            .expect("current wallpaper should save");
        db.set_setting("paused", "true")
            .expect("paused state should save");
        db.set_setting("rotation_secs", "1800")
            .expect("rotation interval should save");
        db.set_setting("display_mode", "independent")
            .expect("display mode should save");
    }

    let reopened = Database::new(&files.db_path).expect("database should reopen");
    let restored =
        load_restored_playback_state(&reopened).expect("settings should restore after reopening");
    assert_eq!(
        restored.current_wallpaper_path.as_deref(),
        Some(wallpaper_path.as_str())
    );
    assert!(restored.manual_paused);
    assert_eq!(restored.rotation_secs, 1800);
    assert_eq!(restored.display_mode, "independent");
}

#[test]
fn restart_defaults_malformed_values_and_clamps_oversized_interval() {
    let (files, wallpaper_path) = create_registered_temp_wallpaper("fallback-state");
    {
        let db = Database::new(&files.db_path).expect("database should initialize");
        db.set_setting("current_wallpaper", "   ")
            .expect("blank current wallpaper should save");
        db.set_setting("paused", "not-a-bool")
            .expect("malformed pause state should save");
        db.set_setting("rotation_secs", "not-a-number")
            .expect("malformed rotation interval should save");
        db.set_setting("display_mode", "not-a-mode")
            .expect("malformed display mode should save");
    }

    let reopened = Database::new(&files.db_path).expect("database should reopen");
    let restored =
        load_restored_playback_state(&reopened).expect("malformed settings should restore safely");
    assert_eq!(restored.current_wallpaper_path, None);
    assert!(!restored.manual_paused);
    assert_eq!(restored.rotation_secs, 600);
    assert_eq!(restored.display_mode, "all");
    drop(reopened);

    {
        let db = Database::new(&files.db_path).expect("database should reopen for interval update");
        db.set_setting("current_wallpaper", &wallpaper_path)
            .expect("current wallpaper should save");
        db.set_setting("rotation_secs", "999999")
            .expect("oversized rotation interval should save");
    }
    let reopened =
        Database::new(&files.db_path).expect("database should reopen after interval update");
    let restored =
        load_restored_playback_state(&reopened).expect("oversized interval should restore safely");
    assert_eq!(restored.rotation_secs, 86_400);
}
