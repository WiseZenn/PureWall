use crate::library_backup::{
    BackupClientSettings, BackupDocumentV1, BackupImportPreview, BackupMergeResult,
    BackupNamedEntity, BackupSettings, BackupSource, BackupWallpaper, NormalizedBackup, BACKUP_APP,
    BACKUP_SCHEMA_VERSION, MAX_BACKUP_COLLECTIONS, MAX_BACKUP_SOURCES, MAX_BACKUP_TAGS,
    MAX_BACKUP_WALLPAPERS, MAX_RELATIONS_PER_WALLPAPER,
};
use crate::paths::path_identity_key;
use anyhow::{Context, Result};
use rusqlite::{
    backup::{Backup, StepResult},
    params, params_from_iter,
    types::Value,
    Connection, Row, TransactionBehavior,
};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[path = "db_backup.rs"]
mod db_backup;
#[path = "db_sources.rs"]
mod db_sources;
#[path = "db_taxonomy.rs"]
mod db_taxonomy;

use db_backup::{
    backup_setting_count, current_collection_ids, current_tag_ids, load_named_ids,
    merge_backup_setting, parse_backup_bool, replace_collection_relations, replace_tag_relations,
    resolve_relation_ids, ExistingBackupWallpaper,
};
#[derive(Debug, Clone, Serialize)]
pub struct TagEntry {
    pub id: i64,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CollectionEntry {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub wallpaper_count: i64,
}

#[derive(Debug, Clone)]
pub struct WatchedFolderEntry {
    pub path: String,
    pub source: String,
    pub last_scan_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WatchedFolderSummary {
    pub entry: WatchedFolderEntry,
    pub available_count: usize,
    pub unavailable_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceRemovalMode {
    KeepMetadata,
    ClearMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRemovalSummary {
    pub affected_wallpapers: usize,
}

#[derive(Debug, Clone)]
pub struct ScannedWallpaperRecord {
    pub path: String,
    pub hash: String,
    pub width: u32,
    pub height: u32,
    pub file_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRelocationSummary {
    pub matched: usize,
    pub imported: usize,
    pub unavailable: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct WallpaperEntry {
    pub id: i64,
    pub path: String,
    pub hash: String,
    pub source: String,
    pub display_title: String,
    pub rating: i32,
    pub play_count: i32,
    pub last_played: Option<String>,
    pub created_at: String,
    pub blacklisted: bool,
    pub width: u32,
    pub height: u32,
    pub file_size: u64,
    pub tags: Vec<TagEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WallpaperPage {
    pub items: Vec<WallpaperEntry>,
    pub total: i64,
    pub offset: i64,
    pub limit: i64,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Stats {
    pub total: i64,
    pub liked: i64,
    pub disliked: i64,
    pub blacklisted: i64,
    pub total_plays: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MonthlyPlayStats {
    pub month: u32,
    pub plays: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TopWallpaperStats {
    pub path: String,
    pub plays: i64,
    pub rating: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct YearlyStats {
    pub year: i32,
    pub total_plays: i64,
    pub unique_wallpapers: i64,
    pub liked_plays: i64,
    pub monthly: Vec<MonthlyPlayStats>,
    pub top_wallpapers: Vec<TopWallpaperStats>,
}

pub struct Database {
    conn: Connection,
}

pub const DEFAULT_TAG_COLOR: &str = "#0a84ff";
const SCHEMA_VERSION: i64 = 7;
const WALLPAPER_PAGE_MAX_LIMIT: i64 = 240;
const WALLPAPER_SELECT: &str = "SELECT wallpapers.id, wallpapers.path, wallpapers.hash, wallpapers.source, wallpapers.display_title, wallpapers.rating, wallpapers.play_count, wallpapers.last_played, wallpapers.created_at, wallpapers.blacklisted, wallpapers.width, wallpapers.height, wallpapers.file_size FROM wallpapers";

struct WallpaperFilterQuery {
    join_clause: &'static str,
    where_clause: String,
    params: Vec<Value>,
}

fn playback_ticket_weight(rating: i32) -> Option<usize> {
    if rating < 0 {
        None
    } else if rating == 1 {
        Some(2)
    } else {
        Some(1)
    }
}

fn sample_weighted_without_replacement<F>(
    mut candidates: Vec<(String, i32)>,
    requested: usize,
    mut ticket_for: F,
) -> Vec<String>
where
    F: FnMut(usize) -> usize,
{
    candidates.retain(|(_, rating)| playback_ticket_weight(*rating).is_some());
    let mut selected = Vec::with_capacity(requested.min(candidates.len()));

    while selected.len() < requested && !candidates.is_empty() {
        let total_weight = candidates
            .iter()
            .filter_map(|(_, rating)| playback_ticket_weight(*rating))
            .sum::<usize>();
        let mut ticket = ticket_for(total_weight) % total_weight;
        let selected_index = candidates
            .iter()
            .position(|(_, rating)| {
                let weight = playback_ticket_weight(*rating)
                    .expect("ineligible candidates must be removed before sampling");
                if ticket < weight {
                    true
                } else {
                    ticket -= weight;
                    false
                }
            })
            .expect("weighted ticket must select a candidate");
        let (path, _) = candidates.remove(selected_index);
        selected.push(path);
    }

    selected
}

const BACKUP_STEP_PAGES: i32 = 128;
const BACKUP_BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const BACKUP_TOTAL_TIMEOUT: Duration = Duration::from_secs(30);

struct TemporaryBackup {
    path: PathBuf,
    keep: bool,
}

impl TemporaryBackup {
    fn new(path: PathBuf) -> Self {
        Self { path, keep: false }
    }

    fn publish(mut self, final_path: &Path) -> Result<()> {
        fs::hard_link(&self.path, final_path).with_context(|| {
            format!(
                "Failed to publish SQLite migration backup without overwriting {}; temporary copy was at {}, and cleanup will be attempted",
                final_path.display(),
                self.path.display()
            )
        })?;
        fs::remove_file(&self.path).with_context(|| {
            format!(
                "Published SQLite migration backup at {}, but could not remove temporary copy {}; preserve both paths and resolve manually",
                final_path.display(),
                self.path.display()
            )
        })?;
        self.keep = true;
        Ok(())
    }
}

impl Drop for TemporaryBackup {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn migration_backup_paths(db_path: &Path, from: i64, to: i64) -> (PathBuf, PathBuf) {
    let name = db_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("purewall.db");
    let stem = format!("{name}.pre-migration-v{from}-to-v{to}.bak");
    let final_path = db_path.with_file_name(stem);
    let temp_path = PathBuf::from(format!("{}.tmp", final_path.display()));
    (final_path, temp_path)
}

fn quick_check(conn: &Connection, path: &Path) -> Result<()> {
    let mut statement = conn.prepare("PRAGMA quick_check").with_context(|| {
        format!(
            "Failed to prepare SQLite quick_check for {}",
            path.display()
        )
    })?;
    let mut rows = statement
        .query([])
        .with_context(|| format!("Failed to run SQLite quick_check for {}", path.display()))?;
    let mut diagnostics = Vec::new();
    while let Some(row) = rows
        .next()
        .with_context(|| format!("Failed to read SQLite quick_check for {}", path.display()))?
    {
        diagnostics.push(row.get::<_, String>(0).with_context(|| {
            format!("Failed to decode SQLite quick_check for {}", path.display())
        })?);
    }
    if diagnostics.len() == 1 && diagnostics[0] == "ok" {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "SQLite quick_check failed for {}: {}",
            path.display(),
            diagnostics.join("; ")
        ))
    }
}

fn has_user_objects(conn: &Connection, path: &Path) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM sqlite_master
            WHERE name NOT LIKE 'sqlite_%'
        )",
        [],
        |row| row.get(0),
    )
    .with_context(|| format!("Failed to inspect SQLite objects for {}", path.display()))
}

fn validate_backup(path: &Path, expected_version: i64) -> Result<()> {
    let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| {
            format!(
                "Failed to open existing SQLite backup read-only {}",
                path.display()
            )
        })?;
    conn.busy_timeout(Duration::from_secs(5)).with_context(|| {
        format!(
            "Failed to set busy timeout for SQLite backup {}",
            path.display()
        )
    })?;
    quick_check(&conn, path)?;
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .with_context(|| format!("Failed to read SQLite backup version {}", path.display()))?;
    if version != expected_version {
        return Err(anyhow::anyhow!(
            "SQLite migration backup {} has schema version {version}, expected {expected_version}",
            path.display()
        ));
    }
    Ok(())
}

fn create_migration_backup(
    source: &Connection,
    db_path: &Path,
    from: i64,
    to: i64,
) -> Result<PathBuf> {
    let (final_path, temp_path) = migration_backup_paths(db_path, from, to);
    match fs::remove_file(&temp_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "Failed to remove stale temporary SQLite migration backup {}",
                    temp_path.display()
                )
            });
        }
    }
    if final_path.exists() {
        validate_backup(&final_path, from).with_context(|| {
            format!(
                "Refusing to overwrite invalid or incompatible SQLite migration backup {}",
                final_path.display()
            )
        })?;
        return Ok(final_path);
    }
    let cleanup = TemporaryBackup::new(temp_path.clone());
    let mut destination = Connection::open(&temp_path).with_context(|| {
        format!(
            "Failed to create temporary SQLite migration backup {}",
            temp_path.display()
        )
    })?;
    destination
        .busy_timeout(Duration::from_secs(5))
        .with_context(|| {
            format!(
                "Failed to set temporary backup busy timeout {}",
                temp_path.display()
            )
        })?;
    let backup = Backup::new(source, &mut destination).with_context(|| {
        format!(
            "Failed to initialize SQLite online backup to {}",
            temp_path.display()
        )
    })?;
    let started_at = Instant::now();
    let mut blocked_since = None;
    loop {
        if started_at.elapsed() >= BACKUP_TOTAL_TIMEOUT {
            return Err(anyhow::anyhow!(
                "Timed out after {} seconds while copying SQLite online backup; temporary copy was at {}, and cleanup will be attempted; final path is {}",
                BACKUP_TOTAL_TIMEOUT.as_secs(),
                temp_path.display(),
                final_path.display()
            ));
        }
        match backup.step(BACKUP_STEP_PAGES).with_context(|| {
            format!(
                "Failed while copying SQLite migration backup to {}; final path is {}",
                temp_path.display(),
                final_path.display()
            )
        })? {
            StepResult::Done => break,
            StepResult::More => {
                blocked_since = None;
                std::thread::sleep(Duration::from_millis(5));
            }
            StepResult::Busy | StepResult::Locked => {
                let blocked_at = blocked_since.get_or_insert_with(Instant::now);
                if blocked_at.elapsed() >= BACKUP_BUSY_TIMEOUT {
                    return Err(anyhow::anyhow!(
                        "Timed out after {} seconds waiting for SQLite online backup; temporary copy was at {}, and cleanup will be attempted; final path is {}",
                        BACKUP_BUSY_TIMEOUT.as_secs(),
                        temp_path.display(),
                        final_path.display()
                    ));
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "SQLite online backup returned an unknown state; temporary copy was at {}, and cleanup will be attempted; final path is {}",
                    temp_path.display(),
                    final_path.display()
                ));
            }
        }
    }
    drop(backup);
    drop(destination);
    validate_backup(&temp_path, from)?;
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&temp_path)
        .with_context(|| {
            format!(
                "Failed to open SQLite backup for sync {}",
                temp_path.display()
            )
        })?
        .sync_all()
        .with_context(|| format!("Failed to sync SQLite backup {}", temp_path.display()))?;
    cleanup.publish(&final_path)?;
    Ok(final_path)
}

impl Database {
    pub fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        let db_path = db_path.as_ref();
        let existed_before_open = db_path.exists();
        let mut conn = Connection::open(db_path).context("Failed to open database")?;
        let found_schema_version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .context("Failed to read SQLite schema version")?;
        if found_schema_version > SCHEMA_VERSION {
            return Err(anyhow::anyhow!(
                "Unsupported SQLite schema version {found_schema_version}; this PureWall build supports up to {SCHEMA_VERSION}"
            ));
        }
        conn.busy_timeout(Duration::from_secs(5))
            .context("Failed to set SQLite busy timeout")?;

        let needs_migration = found_schema_version < SCHEMA_VERSION;
        let migration_safety_backup =
            if needs_migration && existed_before_open && has_user_objects(&conn, db_path)? {
                quick_check(&conn, db_path)?;
                Some(create_migration_backup(
                    &conn,
                    db_path,
                    found_schema_version,
                    SCHEMA_VERSION,
                )?)
            } else {
                None
            };

        if needs_migration {
            let migration_result = (|| -> Result<()> {
                let transaction = conn
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .context("Failed to begin transactional SQLite migration")?;
                transaction
                    .execute_batch(
                        "CREATE TABLE IF NOT EXISTS wallpapers (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        path TEXT UNIQUE NOT NULL,
                        hash TEXT NOT NULL DEFAULT '',
                        source TEXT NOT NULL DEFAULT 'mounted',
                        display_title TEXT NOT NULL DEFAULT '',
                        rating INTEGER NOT NULL DEFAULT 0,
                        play_count INTEGER NOT NULL DEFAULT 0,
                        last_played TEXT,
                        created_at TEXT NOT NULL DEFAULT (datetime('now')),
                        blacklisted INTEGER NOT NULL DEFAULT 0,
                        width INTEGER NOT NULL DEFAULT 0,
                        height INTEGER NOT NULL DEFAULT 0,
                        file_size INTEGER NOT NULL DEFAULT 0,
                        file_available INTEGER NOT NULL DEFAULT 1
                    );
                    CREATE TABLE IF NOT EXISTS tags (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        name TEXT UNIQUE NOT NULL,
                        color TEXT NOT NULL DEFAULT '#0a84ff',
                        created_at TEXT NOT NULL DEFAULT (datetime('now'))
                    );
                    CREATE TABLE IF NOT EXISTS collections (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        name TEXT UNIQUE NOT NULL,
                        color TEXT NOT NULL DEFAULT '#0a84ff',
                        created_at TEXT NOT NULL DEFAULT (datetime('now'))
                    );
                    CREATE TABLE IF NOT EXISTS wallpaper_tags (
                        wallpaper_id INTEGER NOT NULL,
                        tag_id INTEGER NOT NULL,
                        PRIMARY KEY (wallpaper_id, tag_id),
                        FOREIGN KEY (wallpaper_id) REFERENCES wallpapers(id) ON DELETE CASCADE,
                        FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
                    );
                    CREATE TABLE IF NOT EXISTS collection_wallpapers (
                        collection_id INTEGER NOT NULL,
                        wallpaper_id INTEGER NOT NULL,
                        PRIMARY KEY (collection_id, wallpaper_id),
                        FOREIGN KEY (collection_id) REFERENCES collections(id) ON DELETE CASCADE,
                        FOREIGN KEY (wallpaper_id) REFERENCES wallpapers(id) ON DELETE CASCADE
                    );
                    CREATE TABLE IF NOT EXISTS play_events (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        wallpaper_id INTEGER,
                        display_id TEXT,
                        played_at TEXT NOT NULL DEFAULT (datetime('now')),
                        FOREIGN KEY (wallpaper_id) REFERENCES wallpapers(id) ON DELETE SET NULL
                    );
                    CREATE TABLE IF NOT EXISTS watched_folders (
                        path TEXT PRIMARY KEY,
                        source TEXT NOT NULL CHECK(source IN ('mounted', 'imported-folder')),
                        created_at TEXT NOT NULL DEFAULT (datetime('now')),
                        last_scan_at TEXT,
                        last_error TEXT
                    );
                    CREATE TABLE IF NOT EXISTS settings (
                        key TEXT PRIMARY KEY,
                        value TEXT NOT NULL
                    );",
                    )
                    .context("Failed to create base SQLite schema inside migration transaction")?;
                Self::migrate(&transaction)
                    .context("Failed to migrate SQLite schema transactionally")?;
                Self::create_indexes(&transaction)
                    .context("Failed to create SQLite indexes transactionally")?;
                Self::foreign_key_check(&transaction)
                    .context("SQLite foreign_key_check failed during migration")?;
                transaction
                    .pragma_update(None, "user_version", SCHEMA_VERSION)
                    .context("Failed to set SQLite schema version transactionally")?;
                transaction
                    .commit()
                    .context("Failed to commit transactional SQLite migration")?;
                Ok(())
            })();
            if let Err(error) = migration_result {
                if let Some(backup_path) = migration_safety_backup.as_ref() {
                    return Err(error.context(format!(
                        "SQLite migration failed; safety backup retained at {}",
                        backup_path.display()
                    )));
                }
                return Err(error);
            }
        }

        conn.pragma_update(None, "journal_mode", "WAL")
            .context("Failed to enable SQLite WAL mode")?;
        conn.pragma_update(None, "wal_autocheckpoint", "1000")
            .context("Failed to set SQLite WAL autocheckpoint")?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .context("Failed to enable SQLite foreign keys")?;

        let db = Self { conn };
        db.maybe_vacuum()?;
        Ok(db)
    }

    fn migrate(conn: &Connection) -> Result<()> {
        if !Self::column_exists(conn, "wallpapers", "blacklisted")? {
            conn.execute(
                "ALTER TABLE wallpapers ADD COLUMN blacklisted INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }
        for (column, sql_type) in [
            ("display_title", "TEXT NOT NULL DEFAULT ''"),
            ("width", "INTEGER NOT NULL DEFAULT 0"),
            ("height", "INTEGER NOT NULL DEFAULT 0"),
            ("file_size", "INTEGER NOT NULL DEFAULT 0"),
        ] {
            if !Self::column_exists(conn, "wallpapers", column)? {
                conn.execute(
                    &format!("ALTER TABLE wallpapers ADD COLUMN {column} {sql_type}"),
                    [],
                )?;
            }
        }
        if !Self::column_exists(conn, "wallpapers", "file_available")? {
            conn.execute(
                "ALTER TABLE wallpapers ADD COLUMN file_available INTEGER NOT NULL DEFAULT 1",
                [],
            )?;
            Self::backfill_file_availability(conn)?;
        }
        for (column, sql_type) in [("last_scan_at", "TEXT"), ("last_error", "TEXT")] {
            if !Self::column_exists(conn, "watched_folders", column)? {
                conn.execute(
                    &format!("ALTER TABLE watched_folders ADD COLUMN {column} {sql_type}"),
                    [],
                )?;
            }
        }
        if !Self::table_has_foreign_keys(conn, "wallpaper_tags")?
            || !Self::table_has_foreign_keys(conn, "play_events")?
        {
            Self::rebuild_child_tables_with_foreign_keys(conn)?;
        }
        Ok(())
    }

    fn backfill_file_availability(conn: &Connection) -> Result<()> {
        let wallpapers = {
            let mut stmt = conn.prepare("SELECT id, path FROM wallpapers")?;
            let wallpapers = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            wallpapers
        };

        for (id, path) in wallpapers {
            conn.execute(
                "UPDATE wallpapers SET file_available = ?1 WHERE id = ?2",
                params![if Path::new(&path).is_file() { 1 } else { 0 }, id],
            )?;
        }
        Ok(())
    }

    fn create_indexes(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_wallpapers_rating ON wallpapers(rating);
            CREATE INDEX IF NOT EXISTS idx_wallpapers_play_count ON wallpapers(play_count);
            CREATE INDEX IF NOT EXISTS idx_wallpapers_last_played ON wallpapers(last_played);
            CREATE INDEX IF NOT EXISTS idx_wallpapers_created ON wallpapers(created_at);
            CREATE INDEX IF NOT EXISTS idx_wallpapers_blacklisted ON wallpapers(blacklisted);
            CREATE INDEX IF NOT EXISTS idx_wallpapers_availability ON wallpapers(file_available, blacklisted);
            CREATE INDEX IF NOT EXISTS idx_wallpaper_tags_tag ON wallpaper_tags(tag_id);
            CREATE INDEX IF NOT EXISTS idx_collection_wallpapers_collection ON collection_wallpapers(collection_id);
            CREATE INDEX IF NOT EXISTS idx_collection_wallpapers_wallpaper ON collection_wallpapers(wallpaper_id);
            CREATE INDEX IF NOT EXISTS idx_play_events_played_at ON play_events(played_at);
            CREATE INDEX IF NOT EXISTS idx_play_events_wallpaper ON play_events(wallpaper_id);",
        )?;
        Ok(())
    }

    fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
        let cols = stmt.query_map([], |row| row.get::<_, String>(1))?;
        for col in cols {
            if col? == column {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn table_has_foreign_keys(conn: &Connection, table: &str) -> Result<bool> {
        let mut stmt = conn.prepare(&format!("PRAGMA foreign_key_list({})", table))?;
        let mut rows = stmt.query([])?;
        Ok(rows.next()?.is_some())
    }

    fn rebuild_child_tables_with_foreign_keys(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS wallpaper_tags_new (
                wallpaper_id INTEGER NOT NULL,
                tag_id INTEGER NOT NULL,
                PRIMARY KEY (wallpaper_id, tag_id),
                FOREIGN KEY (wallpaper_id) REFERENCES wallpapers(id) ON DELETE CASCADE,
                FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
            );

            INSERT OR IGNORE INTO wallpaper_tags_new (wallpaper_id, tag_id)
            SELECT wt.wallpaper_id, wt.tag_id
            FROM wallpaper_tags wt
            JOIN wallpapers w ON w.id = wt.wallpaper_id
            JOIN tags t ON t.id = wt.tag_id;

            DROP TABLE wallpaper_tags;
            ALTER TABLE wallpaper_tags_new RENAME TO wallpaper_tags;

            CREATE TABLE IF NOT EXISTS play_events_new (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                wallpaper_id INTEGER,
                display_id TEXT,
                played_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (wallpaper_id) REFERENCES wallpapers(id) ON DELETE SET NULL
            );

            INSERT OR IGNORE INTO play_events_new (id, wallpaper_id, display_id, played_at)
            SELECT pe.id,
                   CASE WHEN w.id IS NULL THEN NULL ELSE pe.wallpaper_id END,
                   pe.display_id,
                   pe.played_at
            FROM play_events pe
            LEFT JOIN wallpapers w ON w.id = pe.wallpaper_id;

            DROP TABLE play_events;
            ALTER TABLE play_events_new RENAME TO play_events;",
        )?;
        Ok(())
    }

    fn foreign_key_check(conn: &Connection) -> Result<()> {
        let mut statement = conn.prepare("PRAGMA foreign_key_check")?;
        let mut rows = statement.query([])?;
        if let Some(row) = rows.next()? {
            let table: String = row.get(0)?;
            let row_id: Option<i64> = row.get(1)?;
            let parent: String = row.get(2)?;
            let foreign_key_id: i64 = row.get(3)?;
            return Err(anyhow::anyhow!(
                "foreign key violation in table {table}, row {row_id:?}, parent {parent}, constraint {foreign_key_id}"
            ));
        }
        Ok(())
    }

    pub fn upsert_wallpaper(
        &self,
        path: &str,
        hash: &str,
        source: &str,
        width: u32,
        height: u32,
        file_size: u64,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO wallpapers (path, hash, source, width, height, file_size)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(path) DO UPDATE SET
                hash = ?2,
                source = ?3,
                width = ?4,
                height = ?5,
                file_size = ?6,
                file_available = 1",
            params![path, hash, source, width, height, file_size],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_wallpaper_metadata(
        &self,
        path: &str,
        width: u32,
        height: u32,
        file_size: u64,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE wallpapers SET width = ?2, height = ?3, file_size = ?4 WHERE path = ?1",
            params![path, width, height, file_size],
        )?;
        Ok(())
    }

    pub fn get_wallpaper_metadata(&self, path: &str) -> Result<Option<(u32, u32, u64)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT width, height, file_size FROM wallpapers WHERE path = ?1")?;
        let mut rows = stmt.query([path])?;
        Ok(rows.next()?.map(|row| {
            (
                row.get::<_, u32>(0).unwrap_or(0),
                row.get::<_, u32>(1).unwrap_or(0),
                row.get::<_, u64>(2).unwrap_or(0),
            )
        }))
    }

    pub fn get_all_wallpapers(&self) -> Result<Vec<WallpaperEntry>> {
        self.get_wallpapers_filtered("all", "created")
    }

    pub fn get_liked_wallpapers(&self) -> Result<Vec<WallpaperEntry>> {
        self.get_wallpapers_filtered("liked", "created")
    }

    pub fn get_disliked_wallpapers(&self) -> Result<Vec<WallpaperEntry>> {
        self.get_wallpapers_filtered("disliked", "created")
    }

    pub fn get_wallpapers_filtered(&self, filter: &str, sort: &str) -> Result<Vec<WallpaperEntry>> {
        let order_by = Self::order_by(sort);

        if let Some(tag_id) = filter
            .strip_prefix("tag:")
            .and_then(|s| s.parse::<i64>().ok())
        {
            let sql = format!(
                "{WALLPAPER_SELECT} JOIN wallpaper_tags wt ON wt.wallpaper_id = wallpapers.id WHERE wt.tag_id = ?1 AND wallpapers.file_available = 1 AND wallpapers.blacklisted = 0 {order_by}"
            );
            return Ok(Self::existing_wallpapers(
                self.query_wallpapers_with_i64(&sql, tag_id)?,
            ));
        }

        if let Some(collection_id) = filter
            .strip_prefix("collection:")
            .and_then(|s| s.parse::<i64>().ok())
        {
            let sql = format!(
                "{WALLPAPER_SELECT} JOIN collection_wallpapers cw ON cw.wallpaper_id = wallpapers.id WHERE cw.collection_id = ?1 AND wallpapers.file_available = 1 AND wallpapers.blacklisted = 0 {order_by}"
            );
            return Ok(Self::existing_wallpapers(
                self.query_wallpapers_with_i64(&sql, collection_id)?,
            ));
        }

        let where_clause = match filter {
            "liked" => "WHERE wallpapers.file_available = 1 AND wallpapers.rating = 1 AND wallpapers.blacklisted = 0",
            "disliked" => "WHERE wallpapers.file_available = 1 AND wallpapers.rating = -1 AND wallpapers.blacklisted = 0",
            "blacklisted" => "WHERE wallpapers.file_available = 1 AND wallpapers.blacklisted = 1",
            _ => "WHERE wallpapers.file_available = 1 AND wallpapers.blacklisted = 0",
        };
        let sql = format!("{WALLPAPER_SELECT} {where_clause} {order_by}");
        Ok(Self::existing_wallpapers(self.query_wallpapers(&sql)?))
    }

    pub fn get_wallpapers_page(
        &self,
        filter: &str,
        sort: &str,
        search: &str,
        offset: i64,
        limit: i64,
    ) -> Result<WallpaperPage> {
        let offset = offset.max(0);
        let limit = limit.clamp(1, WALLPAPER_PAGE_MAX_LIMIT);
        let order_by = Self::order_by(sort);
        let query = Self::wallpaper_filter_query(filter, search);

        let count_sql = format!(
            "SELECT COUNT(*) FROM wallpapers {} {}",
            query.join_clause, query.where_clause
        );
        let total = self.count_wallpapers_with_values(&count_sql, &query.params)?;

        let mut page_params = query.params.clone();
        page_params.push(Value::Integer(limit));
        page_params.push(Value::Integer(offset));
        let page_sql = format!(
            "{WALLPAPER_SELECT} {} {} {order_by} LIMIT ? OFFSET ?",
            query.join_clause, query.where_clause
        );
        let items =
            Self::existing_wallpapers(self.query_wallpapers_with_values(&page_sql, &page_params)?);

        Ok(WallpaperPage {
            items,
            total,
            offset,
            limit,
            has_more: offset.saturating_add(limit) < total,
        })
    }
    pub fn has_registered_wallpaper(&self, path: &str) -> Result<bool> {
        self.conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM wallpapers WHERE path = ?1)",
                [path],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn get_wallpaper_by_path(&self, path: &str) -> Result<Option<WallpaperEntry>> {
        let sql = format!("{WALLPAPER_SELECT} WHERE wallpapers.path = ?1");
        let mut entries = self.query_wallpapers_with_str(&sql, path)?;
        Ok(entries.pop())
    }

    fn wallpaper_filter_query(filter: &str, search: &str) -> WallpaperFilterQuery {
        let mut join_clause = "";
        let mut clauses = vec!["wallpapers.file_available = 1".to_string()];
        let mut params = Vec::new();

        if let Some(tag_id) = filter
            .strip_prefix("tag:")
            .and_then(|s| s.parse::<i64>().ok())
        {
            join_clause = "JOIN wallpaper_tags wt ON wt.wallpaper_id = wallpapers.id";
            clauses.push("wt.tag_id = ?".to_string());
            clauses.push("wallpapers.blacklisted = 0".to_string());
            params.push(Value::Integer(tag_id));
        } else if let Some(collection_id) = filter
            .strip_prefix("collection:")
            .and_then(|s| s.parse::<i64>().ok())
        {
            join_clause = "JOIN collection_wallpapers cw ON cw.wallpaper_id = wallpapers.id";
            clauses.push("cw.collection_id = ?".to_string());
            clauses.push("wallpapers.blacklisted = 0".to_string());
            params.push(Value::Integer(collection_id));
        } else {
            clauses.push(match filter {
                "liked" => "wallpapers.rating = 1 AND wallpapers.blacklisted = 0".to_string(),
                "disliked" => "wallpapers.rating = -1 AND wallpapers.blacklisted = 0".to_string(),
                "blacklisted" => "wallpapers.blacklisted = 1".to_string(),
                _ => "wallpapers.blacklisted = 0".to_string(),
            });
        }

        let search = search.trim();
        if !search.is_empty() {
            let pattern = Self::like_pattern(search);
            clauses.push(
                "(wallpapers.path LIKE ? ESCAPE '\\' OR wallpapers.display_title LIKE ? ESCAPE '\\' OR wallpapers.source LIKE ? ESCAPE '\\' OR EXISTS (SELECT 1 FROM wallpaper_tags search_wt JOIN tags search_tags ON search_tags.id = search_wt.tag_id WHERE search_wt.wallpaper_id = wallpapers.id AND search_tags.name LIKE ? ESCAPE '\\'))"
                    .to_string(),
            );
            for _ in 0..4 {
                params.push(Value::Text(pattern.clone()));
            }
        }

        WallpaperFilterQuery {
            join_clause,
            where_clause: format!("WHERE {}", clauses.join(" AND ")),
            params,
        }
    }

    fn like_pattern(search: &str) -> String {
        let mut pattern = String::with_capacity(search.len() + 2);
        pattern.push('%');
        for ch in search.chars() {
            if matches!(ch, '%' | '_' | '\\') {
                pattern.push('\\');
            }
            pattern.push(ch);
        }
        pattern.push('%');
        pattern
    }
    fn existing_wallpapers(entries: Vec<WallpaperEntry>) -> Vec<WallpaperEntry> {
        entries
            .into_iter()
            .filter(|entry| Path::new(&entry.path).exists())
            .collect()
    }

    fn order_by(sort: &str) -> &'static str {
        match sort {
            "liked" => "ORDER BY wallpapers.rating DESC, wallpapers.created_at DESC",
            "plays" => "ORDER BY wallpapers.play_count DESC, wallpapers.created_at DESC",
            "recent" => "ORDER BY wallpapers.last_played IS NULL, wallpapers.last_played DESC, wallpapers.created_at DESC",
            _ => "ORDER BY wallpapers.created_at DESC",
        }
    }

    fn query_wallpapers(&self, sql: &str) -> Result<Vec<WallpaperEntry>> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map([], Self::wallpaper_from_row)?;
        let mut entries = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);
        self.attach_tags(&mut entries)?;
        Ok(entries)
    }

    fn query_wallpapers_with_i64(&self, sql: &str, value: i64) -> Result<Vec<WallpaperEntry>> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map([value], Self::wallpaper_from_row)?;
        let mut entries = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);
        self.attach_tags(&mut entries)?;
        Ok(entries)
    }

    fn query_wallpapers_with_str(&self, sql: &str, value: &str) -> Result<Vec<WallpaperEntry>> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map([value], Self::wallpaper_from_row)?;
        let mut entries = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);
        self.attach_tags(&mut entries)?;
        Ok(entries)
    }
    fn query_wallpapers_with_values(
        &self,
        sql: &str,
        values: &[Value],
    ) -> Result<Vec<WallpaperEntry>> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params_from_iter(values.iter()), Self::wallpaper_from_row)?;
        let mut entries = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);
        self.attach_tags(&mut entries)?;
        Ok(entries)
    }

    fn count_wallpapers_with_values(&self, sql: &str, values: &[Value]) -> Result<i64> {
        self.conn
            .query_row(sql, params_from_iter(values.iter()), |row| row.get(0))
            .map_err(Into::into)
    }

    fn wallpaper_from_row(row: &Row<'_>) -> rusqlite::Result<WallpaperEntry> {
        Ok(WallpaperEntry {
            id: row.get(0)?,
            path: row.get(1)?,
            hash: row.get(2)?,
            source: row.get(3)?,
            display_title: row.get(4)?,
            rating: row.get(5)?,
            play_count: row.get(6)?,
            last_played: row.get(7)?,
            created_at: row.get(8)?,
            blacklisted: row.get::<_, i64>(9)? != 0,
            width: row.get(10)?,
            height: row.get(11)?,
            file_size: row.get(12)?,
            tags: Vec::new(),
        })
    }

    fn attach_tags(&self, entries: &mut [WallpaperEntry]) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }

        let ids = entries.iter().map(|entry| entry.id).collect::<Vec<_>>();
        let placeholders = std::iter::repeat_n("?", ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT wt.wallpaper_id, tags.id, tags.name, tags.color
             FROM wallpaper_tags wt
             JOIN tags ON tags.id = wt.tag_id
             WHERE wt.wallpaper_id IN ({placeholders})
             ORDER BY wt.wallpaper_id ASC, tags.name ASC"
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(ids.iter()), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                TagEntry {
                    id: row.get(1)?,
                    name: row.get(2)?,
                    color: row.get(3)?,
                },
            ))
        })?;

        let mut tags_by_wallpaper: HashMap<i64, Vec<TagEntry>> = HashMap::new();
        for row in rows {
            let (wallpaper_id, tag) = row?;
            tags_by_wallpaper.entry(wallpaper_id).or_default().push(tag);
        }

        for entry in entries {
            entry.tags = tags_by_wallpaper.remove(&entry.id).unwrap_or_default();
        }

        Ok(())
    }

    pub fn remove_wallpaper(&self, path: &str) -> Result<()> {
        self.remove_wallpapers(&[path.to_string()])
    }

    pub fn remove_wallpapers(&self, paths: &[String]) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }

        self.conn.execute_batch("BEGIN")?;
        let result = (|| {
            for path in paths {
                if let Ok(wallpaper_id) = self.wallpaper_id(path) {
                    self.conn.execute(
                        "DELETE FROM wallpaper_tags WHERE wallpaper_id = ?1",
                        [wallpaper_id],
                    )?;
                    self.conn.execute(
                        "DELETE FROM collection_wallpapers WHERE wallpaper_id = ?1",
                        [wallpaper_id],
                    )?;
                }
                self.conn
                    .execute("DELETE FROM wallpapers WHERE path = ?1", params![path])?;
            }
            Ok(())
        })();

        match result {
            Ok(()) => match self.conn.execute_batch("COMMIT") {
                Ok(()) => Ok(()),
                Err(error) => {
                    let _ = self.conn.execute_batch("ROLLBACK");
                    Err(error.into())
                }
            },
            Err(error) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }

    pub fn record_play(&self, path: &str) -> Result<()> {
        self.record_play_for_display(path, None)
    }

    pub fn get_next_wallpaper(&self) -> Result<String> {
        self.get_next_wallpapers(1)?
            .into_iter()
            .next()
            .context("No wallpapers available")
    }

    pub fn get_next_wallpapers(&self, count: usize) -> Result<Vec<String>> {
        if count == 0 {
            return Ok(Vec::new());
        }
        let requested = count.max(1);
        let unique_target = requested.max(8);
        let base_limit = (requested * 12).clamp(64, 500);
        let mut candidates = Vec::with_capacity(unique_target);
        let mut seen_paths = HashSet::with_capacity(unique_target);

        for attempt in 1..=4 {
            let candidate_limit = (base_limit * attempt).min(2_000) as i64;
            let mut stmt = self.conn.prepare(
                "SELECT path, rating FROM wallpapers
                 WHERE file_available = 1 AND rating >= 0 AND blacklisted = 0
                 ORDER BY RANDOM()
                 LIMIT ?1",
            )?;

            let random_candidates: Vec<(String, i32)> = stmt
                .query_map([candidate_limit], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<std::result::Result<Vec<_>, _>>()?;

            for (path, rating) in random_candidates {
                if candidates.len() >= unique_target {
                    break;
                }
                if !seen_paths.insert(path.clone()) || !Path::new(&path).exists() {
                    continue;
                }
                candidates.push((path, rating));
            }

            if candidates.len() >= unique_target {
                break;
            }
        }

        if candidates.is_empty() {
            anyhow::bail!("No existing wallpaper files found");
        }

        // ADR-003: liked wallpapers own two tickets while normal wallpapers own one.
        // Sampling without replacement keeps independent displays unique whenever
        // the available candidate pool is large enough.
        let mut paths =
            sample_weighted_without_replacement(candidates, requested, |total_weight| {
                fastrand::usize(..total_weight)
            });
        let unique_paths = paths.clone();
        let mut repeat_index = 0;
        while paths.len() < requested {
            paths.push(unique_paths[repeat_index % unique_paths.len()].clone());
            repeat_index += 1;
        }

        Ok(paths)
    }

    pub fn get_stats(&self) -> Result<Stats> {
        self.conn.query_row(
            "SELECT
                COALESCE(SUM(CASE WHEN file_available = 1 AND blacklisted = 0 THEN 1 ELSE 0 END), 0) AS total,
                COALESCE(SUM(CASE WHEN file_available = 1 AND rating = 1 AND blacklisted = 0 THEN 1 ELSE 0 END), 0) AS liked,
                COALESCE(SUM(CASE WHEN file_available = 1 AND rating = -1 AND blacklisted = 0 THEN 1 ELSE 0 END), 0) AS disliked,
                COALESCE(SUM(CASE WHEN file_available = 1 AND blacklisted = 1 THEN 1 ELSE 0 END), 0) AS blacklisted,
                COALESCE(SUM(CASE WHEN file_available = 1 AND blacklisted = 0 THEN play_count ELSE 0 END), 0) AS total_plays
             FROM wallpapers",
            [],
            |row| {
                Ok(Stats {
                    total: row.get(0)?,
                    liked: row.get(1)?,
                    disliked: row.get(2)?,
                    blacklisted: row.get(3)?,
                    total_plays: row.get(4)?,
                })
            },
        ).map_err(Into::into)
    }

    pub fn record_play_for_display(&self, path: &str, display_id: Option<&str>) -> Result<()> {
        let wallpaper_id = self.wallpaper_id(path)?;
        // Keep the aggregate counter and the per-display event history atomic so
        // a failure between the two statements cannot diverge yearly stats.
        let tx = self
            .conn
            .unchecked_transaction()
            .context("Failed to begin play-record transaction")?;
        tx.execute(
            "UPDATE wallpapers SET play_count = play_count + 1, last_played = datetime('now') WHERE id = ?1",
            [wallpaper_id],
        )?;
        tx.execute(
            "INSERT INTO play_events (wallpaper_id, display_id) VALUES (?1, ?2)",
            params![wallpaper_id, display_id],
        )?;
        tx.commit().context("Failed to commit play record")
    }

    pub fn get_yearly_stats(&self, year: i32) -> Result<YearlyStats> {
        let year_text = year.to_string();
        let mut monthly = (1..=12)
            .map(|month| MonthlyPlayStats { month, plays: 0 })
            .collect::<Vec<_>>();

        let mut stmt = self.conn.prepare(
            "SELECT CAST(strftime('%m', played_at) AS INTEGER) AS month, COUNT(*)
             FROM play_events
             WHERE strftime('%Y', played_at) = ?1
             GROUP BY month
             ORDER BY month ASC",
        )?;
        let rows = stmt.query_map([year_text.as_str()], |row| {
            Ok((row.get::<_, u32>(0)?, row.get::<_, i64>(1)?))
        })?;
        for row in rows {
            let (month, plays) = row?;
            if (1..=12).contains(&month) {
                monthly[(month - 1) as usize].plays = plays;
            }
        }
        drop(stmt);

        let (total_plays, unique_wallpapers, liked_plays): (i64, i64, i64) = self.conn.query_row(
            "SELECT
                COUNT(*) AS total_plays,
                COUNT(DISTINCT pe.wallpaper_id) AS unique_wallpapers,
                COALESCE(SUM(CASE WHEN w.rating = 1 THEN 1 ELSE 0 END), 0) AS liked_plays
             FROM play_events pe
             LEFT JOIN wallpapers w ON w.id = pe.wallpaper_id
             WHERE strftime('%Y', pe.played_at) = ?1",
            [year_text.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;

        let mut top_stmt = self.conn.prepare(
            "SELECT w.path, COUNT(*) AS plays, w.rating
             FROM play_events pe
             JOIN wallpapers w ON w.id = pe.wallpaper_id
             WHERE strftime('%Y', pe.played_at) = ?1
             GROUP BY w.id, w.path, w.rating
             ORDER BY plays DESC, MAX(pe.played_at) DESC
             LIMIT 5",
        )?;
        let top_wallpapers = top_stmt
            .query_map([year_text.as_str()], |row| {
                Ok(TopWallpaperStats {
                    path: row.get(0)?,
                    plays: row.get(1)?,
                    rating: row.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(YearlyStats {
            year,
            total_plays,
            unique_wallpapers,
            liked_plays,
            monthly,
            top_wallpapers,
        })
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        match self
            .conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                row.get(0)
            }) {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value)
             VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub(crate) fn export_backup_snapshot(
        &self,
        client: BackupClientSettings,
        exported_at: String,
    ) -> Result<BackupDocumentV1> {
        let transaction = self
            .conn
            .unchecked_transaction()
            .context("Failed to begin backup snapshot transaction")?;

        let mut sources = {
            let mut statement = transaction
                .prepare("SELECT path, source FROM watched_folders")
                .context("Failed to prepare backup source snapshot")?;
            let rows = statement
                .query_map([], |row| {
                    Ok(BackupSource {
                        path: row.get(0)?,
                        source: row.get(1)?,
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };
        if sources.len() > MAX_BACKUP_SOURCES {
            anyhow::bail!("backup source count exceeds {MAX_BACKUP_SOURCES}");
        }
        sources.sort_by(|left, right| {
            path_identity_key(Path::new(&left.path))
                .cmp(&path_identity_key(Path::new(&right.path)))
                .then_with(|| left.path.cmp(&right.path))
        });

        let mut tags = {
            let mut statement = transaction
                .prepare("SELECT name, color FROM tags")
                .context("Failed to prepare backup tag snapshot")?;
            let rows = statement
                .query_map([], |row| {
                    Ok(BackupNamedEntity {
                        name: row.get(0)?,
                        color: row.get(1)?,
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };
        if tags.len() > MAX_BACKUP_TAGS {
            anyhow::bail!("backup tag count exceeds {MAX_BACKUP_TAGS}");
        }
        tags.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.color.cmp(&right.color))
        });

        let mut collections = {
            let mut statement = transaction
                .prepare("SELECT name, color FROM collections")
                .context("Failed to prepare backup collection snapshot")?;
            let rows = statement
                .query_map([], |row| {
                    Ok(BackupNamedEntity {
                        name: row.get(0)?,
                        color: row.get(1)?,
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };
        if collections.len() > MAX_BACKUP_COLLECTIONS {
            anyhow::bail!("backup collection count exceeds {MAX_BACKUP_COLLECTIONS}");
        }
        collections.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.color.cmp(&right.color))
        });

        let wallpaper_rows = {
            let mut statement = transaction
                .prepare(
                    "SELECT id, path, hash, source, display_title, rating, blacklisted,
                            width, height, file_size
                     FROM wallpapers",
                )
                .context("Failed to prepare backup wallpaper snapshot")?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i32>(5)?,
                        row.get::<_, bool>(6)?,
                        row.get::<_, u32>(7)?,
                        row.get::<_, u32>(8)?,
                        row.get::<_, u64>(9)?,
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };
        if wallpaper_rows.len() > MAX_BACKUP_WALLPAPERS {
            anyhow::bail!("backup wallpaper count exceeds {MAX_BACKUP_WALLPAPERS}");
        }

        let mut tags_by_wallpaper = HashMap::<i64, Vec<String>>::new();
        {
            let mut statement = transaction
                .prepare(
                    "SELECT wallpaper_tags.wallpaper_id, tags.name
                     FROM wallpaper_tags
                     JOIN tags ON tags.id = wallpaper_tags.tag_id",
                )
                .context("Failed to prepare backup wallpaper-tag snapshot")?;
            let rows = statement.query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?;
            for row in rows {
                let (wallpaper_id, name) = row?;
                tags_by_wallpaper
                    .entry(wallpaper_id)
                    .or_default()
                    .push(name);
            }
        }

        let mut collections_by_wallpaper = HashMap::<i64, Vec<String>>::new();
        {
            let mut statement = transaction
                .prepare(
                    "SELECT collection_wallpapers.wallpaper_id, collections.name
                     FROM collection_wallpapers
                     JOIN collections ON collections.id = collection_wallpapers.collection_id",
                )
                .context("Failed to prepare backup wallpaper-collection snapshot")?;
            let rows = statement.query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?;
            for row in rows {
                let (wallpaper_id, name) = row?;
                collections_by_wallpaper
                    .entry(wallpaper_id)
                    .or_default()
                    .push(name);
            }
        }

        let mut wallpapers = Vec::with_capacity(wallpaper_rows.len());
        for (id, path, hash, source, display_title, rating, hidden, width, height, file_size) in
            wallpaper_rows
        {
            let mut wallpaper_tags = tags_by_wallpaper.remove(&id).unwrap_or_default();
            let mut wallpaper_collections =
                collections_by_wallpaper.remove(&id).unwrap_or_default();
            wallpaper_tags.sort();
            wallpaper_collections.sort();
            if wallpaper_tags.len() > MAX_RELATIONS_PER_WALLPAPER
                || wallpaper_collections.len() > MAX_RELATIONS_PER_WALLPAPER
            {
                anyhow::bail!(
                    "backup wallpaper relation count exceeds {MAX_RELATIONS_PER_WALLPAPER}"
                );
            }
            let display_title = display_title.trim();
            wallpapers.push(BackupWallpaper {
                path,
                source,
                hash,
                display_title: (!display_title.is_empty()).then(|| display_title.to_string()),
                rating,
                hidden,
                width,
                height,
                file_size,
                tags: wallpaper_tags,
                collections: wallpaper_collections,
            });
        }
        wallpapers.sort_by(|left, right| {
            path_identity_key(Path::new(&left.path))
                .cmp(&path_identity_key(Path::new(&right.path)))
                .then_with(|| left.path.cmp(&right.path))
        });

        let get_setting = |key: &str| -> Result<Option<String>> {
            match transaction.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                row.get(0)
            }) {
                Ok(value) => Ok(Some(value)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(error) => Err(error.into()),
            }
        };
        let settings = BackupSettings {
            rotation_secs: get_setting("rotation_secs")?
                .and_then(|value| value.parse::<u64>().ok())
                .filter(|seconds| *seconds <= 86_400),
            display_mode: get_setting("display_mode")?
                .filter(|value| matches!(value.as_str(), "all" | "span" | "independent")),
            focus_mode_enabled: parse_backup_bool(get_setting("focus_mode_enabled")?),
            paused: parse_backup_bool(get_setting("paused")?),
            theme: client.theme,
            workspace_mode: client.workspace_mode,
        };

        let document = BackupDocumentV1 {
            app: BACKUP_APP.to_string(),
            schema_version: BACKUP_SCHEMA_VERSION,
            exported_at,
            sources,
            wallpapers,
            tags,
            collections,
            settings,
        };
        transaction
            .commit()
            .context("Failed to complete backup snapshot transaction")?;
        Ok(document)
    }

    pub(crate) fn preview_backup_import(
        &self,
        backup: &NormalizedBackup,
        content_digest: String,
    ) -> Result<BackupImportPreview> {
        let mut current_identities = HashSet::new();
        {
            let mut statement = self
                .conn
                .prepare("SELECT path FROM wallpapers")
                .context("Failed to prepare backup preview")?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            for row in rows {
                let path = row?;
                if !current_identities.insert(path_identity_key(Path::new(&path))) {
                    anyhow::bail!("ambiguous local wallpaper path identity: {path}");
                }
            }
        }

        let mut new_wallpapers = 0;
        let mut overwritten_wallpapers = 0;
        let mut missing_paths = 0;
        for wallpaper in &backup.document.wallpapers {
            if current_identities.contains(&path_identity_key(Path::new(&wallpaper.path))) {
                overwritten_wallpapers += 1;
            } else {
                new_wallpapers += 1;
            }
            if !Path::new(&wallpaper.path).is_file() {
                missing_paths += 1;
            }
        }

        let mut warnings = Vec::new();
        if missing_paths > 0 {
            warnings.push(format!(
                "{missing_paths} wallpaper path(s) are currently unavailable and will be restored as metadata only"
            ));
        }

        Ok(BackupImportPreview {
            content_digest,
            source_count: backup.document.sources.len(),
            wallpaper_count: backup.document.wallpapers.len(),
            tag_count: backup.document.tags.len(),
            collection_count: backup.document.collections.len(),
            setting_count: backup_setting_count(&backup.document.settings),
            new_wallpapers,
            overwritten_wallpapers,
            missing_paths,
            warnings,
        })
    }

    pub(crate) fn merge_backup(&mut self, backup: &NormalizedBackup) -> Result<BackupMergeResult> {
        let transaction = self
            .conn
            .transaction()
            .context("Failed to begin backup merge transaction")?;

        let mut tag_ids = load_named_ids(&transaction, "SELECT id, name FROM tags")?;
        let mut created_tags = 0;
        for tag in &backup.document.tags {
            if !tag_ids.contains_key(&tag.name) {
                transaction.execute(
                    "INSERT INTO tags (name, color) VALUES (?1, ?2)",
                    params![tag.name, tag.color],
                )?;
                tag_ids.insert(tag.name.clone(), transaction.last_insert_rowid());
                created_tags += 1;
            }
        }

        let mut collection_ids = load_named_ids(&transaction, "SELECT id, name FROM collections")?;
        let mut created_collections = 0;
        for collection in &backup.document.collections {
            if !collection_ids.contains_key(&collection.name) {
                transaction.execute(
                    "INSERT INTO collections (name, color) VALUES (?1, ?2)",
                    params![collection.name, collection.color],
                )?;
                collection_ids.insert(collection.name.clone(), transaction.last_insert_rowid());
                created_collections += 1;
            }
        }

        let mut source_identities = HashSet::new();
        {
            let mut statement = transaction
                .prepare("SELECT path FROM watched_folders")
                .context("Failed to prepare local source identities")?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            for row in rows {
                let path = row?;
                if !source_identities.insert(path_identity_key(Path::new(&path))) {
                    anyhow::bail!("ambiguous local source path identity: {path}");
                }
            }
        }
        let mut added_sources = 0;
        for source in &backup.document.sources {
            let identity = path_identity_key(Path::new(&source.path));
            if source_identities.insert(identity) {
                transaction.execute(
                    "INSERT INTO watched_folders (path, source) VALUES (?1, ?2)",
                    params![source.path, source.source],
                )?;
                added_sources += 1;
            }
        }

        let mut updated_settings = 0;
        if let Some(seconds) = backup.document.settings.rotation_secs {
            updated_settings +=
                merge_backup_setting(&transaction, "rotation_secs", &seconds.to_string())?;
        }
        if let Some(mode) = &backup.document.settings.display_mode {
            updated_settings += merge_backup_setting(&transaction, "display_mode", mode)?;
        }
        if let Some(enabled) = backup.document.settings.focus_mode_enabled {
            updated_settings += merge_backup_setting(
                &transaction,
                "focus_mode_enabled",
                if enabled { "true" } else { "false" },
            )?;
        }
        if let Some(paused) = backup.document.settings.paused {
            updated_settings += merge_backup_setting(
                &transaction,
                "paused",
                if paused { "true" } else { "false" },
            )?;
        }

        let mut existing_wallpapers = HashMap::new();
        {
            let mut statement = transaction
                .prepare(
                    "SELECT id, path, display_title, rating, blacklisted
                     FROM wallpapers",
                )
                .context("Failed to prepare local wallpaper identities")?;
            let rows = statement.query_map([], |row| {
                Ok(ExistingBackupWallpaper {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    display_title: row.get(2)?,
                    rating: row.get(3)?,
                    hidden: row.get(4)?,
                })
            })?;
            for row in rows {
                let wallpaper = row?;
                let identity = path_identity_key(Path::new(&wallpaper.path));
                if existing_wallpapers.insert(identity, wallpaper).is_some() {
                    anyhow::bail!("ambiguous local wallpaper path identity");
                }
            }
        }

        let mut added_wallpapers = 0;
        let mut updated_wallpapers = 0;
        for wallpaper in &backup.document.wallpapers {
            let identity = path_identity_key(Path::new(&wallpaper.path));
            let target_title = wallpaper.display_title.clone().unwrap_or_default();

            if let Some(existing) = existing_wallpapers.get(&identity) {
                let metadata_changed = existing.display_title != target_title
                    || existing.rating != wallpaper.rating
                    || existing.hidden != wallpaper.hidden;
                if metadata_changed {
                    transaction.execute(
                        "UPDATE wallpapers
                         SET display_title = ?1, rating = ?2, blacklisted = ?3
                         WHERE id = ?4",
                        params![
                            target_title,
                            wallpaper.rating,
                            wallpaper.hidden,
                            existing.id
                        ],
                    )?;
                }

                let target_tag_ids = resolve_relation_ids(&wallpaper.tags, &tag_ids, "tag")?;
                let target_collection_ids =
                    resolve_relation_ids(&wallpaper.collections, &collection_ids, "collection")?;
                let current_tag_ids = current_tag_ids(&transaction, existing.id)?;
                let current_collection_ids = current_collection_ids(&transaction, existing.id)?;
                let tags_changed = current_tag_ids != target_tag_ids;
                let collections_changed = current_collection_ids != target_collection_ids;
                if tags_changed {
                    replace_tag_relations(&transaction, existing.id, &target_tag_ids)?;
                }
                if collections_changed {
                    replace_collection_relations(
                        &transaction,
                        existing.id,
                        &target_collection_ids,
                    )?;
                }
                if metadata_changed || tags_changed || collections_changed {
                    updated_wallpapers += 1;
                }
            } else {
                transaction.execute(
                    "INSERT INTO wallpapers (
                        path, hash, source, display_title, rating, blacklisted,
                        width, height, file_size, file_available
                     )
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![
                        wallpaper.path,
                        wallpaper.hash,
                        wallpaper.source,
                        target_title,
                        wallpaper.rating,
                        wallpaper.hidden,
                        wallpaper.width,
                        wallpaper.height,
                        wallpaper.file_size,
                        Path::new(&wallpaper.path).is_file(),
                    ],
                )?;
                let wallpaper_id = transaction.last_insert_rowid();
                let target_tag_ids = resolve_relation_ids(&wallpaper.tags, &tag_ids, "tag")?;
                let target_collection_ids =
                    resolve_relation_ids(&wallpaper.collections, &collection_ids, "collection")?;
                replace_tag_relations(&transaction, wallpaper_id, &target_tag_ids)?;
                replace_collection_relations(&transaction, wallpaper_id, &target_collection_ids)?;
                added_wallpapers += 1;
            }
        }

        let result = BackupMergeResult {
            added_wallpapers,
            updated_wallpapers,
            added_sources,
            created_tags,
            created_collections,
            updated_settings,
            client_settings: BackupClientSettings {
                theme: backup.document.settings.theme.clone(),
                workspace_mode: backup.document.settings.workspace_mode.clone(),
            },
        };
        transaction
            .commit()
            .context("Failed to commit backup merge transaction")?;
        Ok(result)
    }

    /// Run VACUUM only when enough free pages have accumulated AND the last
    /// vacuum was at least 7 days ago. VACUUM rewrites the entire database
    /// file so running it on every startup is wasteful (MED-05).
    fn maybe_vacuum(&self) -> Result<()> {
        let freelist_pct = match (
            self.conn
                .pragma_query_value(None, "freelist_count", |row| row.get::<_, i64>(0)),
            self.conn
                .pragma_query_value(None, "page_count", |row| row.get::<_, i64>(0)),
        ) {
            (Ok(free), Ok(total)) if total > 0 => free * 100 / total,
            _ => return Ok(()),
        };

        if freelist_pct < 10 {
            return Ok(());
        }

        let last_vacuum = self
            .get_setting("last_vacuum_date")?
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0);
        let today = chrono::Utc::now().date_naive();
        let last_date = chrono::DateTime::from_timestamp(last_vacuum, 0)
            .map(|dt| dt.date_naive())
            .unwrap_or(today - chrono::Duration::days(30));

        if (today - last_date).num_days() < 7 {
            return Ok(());
        }

        self.conn.execute_batch("VACUUM")?;
        self.set_setting(
            "last_vacuum_date",
            &today
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc()
                .timestamp()
                .to_string(),
        )?;
        eprintln!("[PureWall] Database VACUUM completed (freelist {freelist_pct}%)");
        Ok(())
    }

    /// Truncate the WAL file on clean shutdown. Safe to call anytime;
    /// SQLite will simply skip the checkpoint if no WAL pages need flushing.
    pub fn checkpoint_wal(&self) -> Result<()> {
        self.conn
            .pragma_update(None, "wal_checkpoint", "TRUNCATE")
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_db_path(test_name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "purewall-{test_name}-{}-{nanos}.db",
            std::process::id()
        ))
    }

    fn remove_sqlite_files(path: &Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
        for from in 0..=SCHEMA_VERSION {
            let (backup, temp) = migration_backup_paths(path, from, SCHEMA_VERSION);
            let _ = std::fs::remove_file(backup);
            let _ = std::fs::remove_file(temp);
        }
    }

    fn migration_backup_path(path: &Path, from: i64) -> PathBuf {
        migration_backup_paths(path, from, SCHEMA_VERSION).0
    }

    fn migration_temp_path(path: &Path, from: i64) -> PathBuf {
        migration_backup_paths(path, from, SCHEMA_VERSION).1
    }

    fn assert_no_migration_artifacts(path: &Path) {
        for from in 0..=SCHEMA_VERSION {
            assert!(!migration_backup_path(path, from).exists());
            assert!(!migration_temp_path(path, from).exists());
        }
    }

    struct SqliteFixtureCleanup(PathBuf);

    impl Drop for SqliteFixtureCleanup {
        fn drop(&mut self) {
            remove_sqlite_files(&self.0);
        }
    }

    fn relocation_roots(
        db_path: &Path,
        suffix: &str,
    ) -> (PathBuf, PathBuf, PathBuf, String, String) {
        let base = db_path.with_extension(suffix);
        let old_dir = base.join("Old");
        let new_dir = base.join("New");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&old_dir).expect("old source should exist");
        std::fs::create_dir_all(&new_dir).expect("new source should exist");
        let old_root = old_dir
            .canonicalize()
            .expect("old source should canonicalize")
            .to_string_lossy()
            .to_string();
        let new_root = new_dir
            .canonicalize()
            .expect("new source should canonicalize")
            .to_string_lossy()
            .to_string();
        (base, old_dir, new_dir, old_root, new_root)
    }

    #[test]
    fn future_schema_is_rejected_without_mutating_database() {
        let db_path = unique_temp_db_path("future-schema");
        let _cleanup = SqliteFixtureCleanup(db_path.clone());
        remove_sqlite_files(&db_path);
        let future_version = SCHEMA_VERSION + 1;
        let sentinel_schema;
        let sentinel_data;
        {
            let conn = Connection::open(&db_path).expect("future database should open");
            conn.execute_batch(
                "CREATE TABLE future_schema_sentinel (
                    id INTEGER PRIMARY KEY,
                    value TEXT NOT NULL
                );
                INSERT INTO future_schema_sentinel (id, value)
                VALUES (1, 'do-not-mutate');",
            )
            .expect("future sentinel should be created");
            conn.pragma_update(None, "user_version", future_version)
                .expect("future schema version should be written");
            sentinel_schema = conn
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE name = 'future_schema_sentinel'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .expect("sentinel schema should be readable");
            sentinel_data = conn
                .query_row(
                    "SELECT value FROM future_schema_sentinel WHERE id = 1",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .expect("sentinel data should be readable");
            assert_eq!(
                conn.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                    .expect("future schema version should be readable"),
                future_version
            );
        }

        let before_bytes =
            std::fs::read(&db_path).expect("future database bytes should be readable");
        let error = match Database::new(&db_path) {
            Ok(_) => panic!("future schema should be rejected"),
            Err(error) => error,
        };
        assert_eq!(
            error.to_string(),
            format!(
                "Unsupported SQLite schema version {future_version}; this PureWall build supports up to {SCHEMA_VERSION}"
            )
        );
        assert_eq!(
            std::fs::read(&db_path).expect("database bytes should remain readable"),
            before_bytes,
            "rejecting a future schema must not mutate database bytes"
        );

        let read_only =
            Connection::open_with_flags(&db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .expect("future database should reopen read-only");
        assert_eq!(
            read_only
                .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                .expect("schema version should remain readable"),
            future_version
        );
        assert_eq!(
            read_only
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE name = 'future_schema_sentinel'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .expect("sentinel schema should remain readable"),
            sentinel_schema
        );
        assert_eq!(
            read_only
                .query_row(
                    "SELECT value FROM future_schema_sentinel WHERE id = 1",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .expect("sentinel data should remain readable"),
            sentinel_data
        );
        drop(read_only);
        assert_no_migration_artifacts(&db_path);
    }

    #[test]
    fn fresh_and_current_schema_do_not_create_migration_backups() {
        let db_path = unique_temp_db_path("fresh-no-backup");
        let _cleanup = SqliteFixtureCleanup(db_path.clone());
        remove_sqlite_files(&db_path);
        let db = Database::new(&db_path).expect("fresh database should initialize");
        drop(db);
        assert_no_migration_artifacts(&db_path);

        let db = Database::new(&db_path).expect("current database should reopen");
        drop(db);
        assert_no_migration_artifacts(&db_path);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn v6_wal_database_migrates_with_valid_adjacent_backup() {
        let db_path = unique_temp_db_path("wal-migration-backup");
        let _cleanup = SqliteFixtureCleanup(db_path.clone());
        remove_sqlite_files(&db_path);
        let wal_source = Connection::open(&db_path).expect("legacy database should open");
        wal_source
            .pragma_update(None, "journal_mode", "WAL")
            .expect("legacy database should use WAL");
        wal_source
            .pragma_update(None, "wal_autocheckpoint", 0)
            .expect("legacy database should disable autocheckpoint");
        wal_source
            .execute_batch(
                "CREATE TABLE migration_sentinel (value TEXT NOT NULL);
                 INSERT INTO migration_sentinel(value) VALUES ('committed-in-wal');
                 PRAGMA user_version = 6;",
            )
            .expect("legacy sentinel should be committed");
        assert!(
            db_path.with_extension("db-wal").exists(),
            "committed legacy data should remain in the WAL sidecar"
        );
        let db = Database::new(&db_path).expect("legacy database should migrate");
        assert_eq!(
            db.conn
                .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                .expect("source schema version should be readable"),
            SCHEMA_VERSION
        );
        drop(db);

        let backup_path = migration_backup_path(&db_path, 6);
        assert!(backup_path.exists(), "migration backup should be published");
        assert!(!migration_temp_path(&db_path, 6).exists());
        let backup = Connection::open(&backup_path).expect("backup should open");
        quick_check(&backup, &backup_path).expect("backup should pass quick_check");
        assert_eq!(
            backup
                .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                .expect("backup version should be readable"),
            6
        );
        assert_eq!(
            backup
                .query_row("SELECT value FROM migration_sentinel", [], |row| row
                    .get::<_, String>(0),)
                .expect("WAL sentinel should be in backup"),
            "committed-in-wal"
        );
        drop(backup);
        drop(wal_source);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn valid_existing_migration_backup_is_reused_without_temp() {
        let db_path = unique_temp_db_path("reuse-migration-backup");
        let _cleanup = SqliteFixtureCleanup(db_path.clone());
        remove_sqlite_files(&db_path);
        let source = Connection::open(&db_path).expect("source should open");
        source
            .execute_batch(
                "CREATE TABLE sentinel(value TEXT); INSERT INTO sentinel VALUES ('x'); PRAGMA user_version = 6;",
            )
            .expect("source should seed");
        let backup_path = migration_backup_path(&db_path, 6);
        create_migration_backup(&source, &db_path, 6, SCHEMA_VERSION)
            .expect("initial backup should publish");
        let before = std::fs::read(&backup_path).expect("backup bytes should be readable");
        std::fs::write(migration_temp_path(&db_path, 6), b"stale temporary copy")
            .expect("stale temporary backup should be seedable");
        create_migration_backup(&source, &db_path, 6, SCHEMA_VERSION)
            .expect("valid backup should be reusable");
        assert_eq!(
            std::fs::read(&backup_path).expect("reused backup bytes should be readable"),
            before
        );
        assert!(!migration_temp_path(&db_path, 6).exists());
        drop(source);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn invalid_migration_backup_collision_fails_without_clobbering() {
        let db_path = unique_temp_db_path("collision-migration-backup");
        let _cleanup = SqliteFixtureCleanup(db_path.clone());
        remove_sqlite_files(&db_path);
        let source = Connection::open(&db_path).expect("source should open");
        source
            .execute_batch(
                "CREATE TABLE sentinel(value TEXT); INSERT INTO sentinel VALUES ('source');",
            )
            .expect("source should seed");
        let source_before = std::fs::read(&db_path).expect("source bytes should be readable");
        let backup_path = migration_backup_path(&db_path, 6);
        let collision = Connection::open(&backup_path).expect("collision should open");
        collision
            .execute_batch("CREATE TABLE collision(value TEXT); PRAGMA user_version = 99;")
            .expect("collision should seed");
        drop(collision);
        let backup_before =
            std::fs::read(&backup_path).expect("collision bytes should be readable");
        let error = create_migration_backup(&source, &db_path, 6, SCHEMA_VERSION)
            .expect_err("wrong-version collision must fail closed");
        assert!(error
            .to_string()
            .contains(&backup_path.display().to_string()));
        assert_eq!(
            std::fs::read(&db_path).expect("source should remain"),
            source_before
        );
        assert_eq!(
            std::fs::read(&backup_path).expect("collision should remain"),
            backup_before
        );
        assert!(!migration_temp_path(&db_path, 6).exists());
        drop(source);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn migration_backup_publish_collision_preserves_final_and_cleans_temp() {
        let db_path = unique_temp_db_path("publish-collision");
        let _cleanup = SqliteFixtureCleanup(db_path.clone());
        remove_sqlite_files(&db_path);
        let (final_path, temp_path) = migration_backup_paths(&db_path, 6, SCHEMA_VERSION);
        let final_before = b"existing-final-backup";
        std::fs::write(&final_path, final_before).expect("final collision should be seeded");
        std::fs::write(&temp_path, b"temporary-candidate")
            .expect("temporary candidate should be seeded");

        let error = TemporaryBackup::new(temp_path.clone())
            .publish(&final_path)
            .expect_err("publish must fail without overwriting an existing final backup");

        assert_eq!(
            std::fs::read(&final_path).expect("final collision should remain readable"),
            final_before,
            "publish collision must preserve the existing final backup"
        );
        assert!(
            !temp_path.exists(),
            "consumed TemporaryBackup guard should clean the temporary candidate"
        );
        let message = error.to_string();
        assert!(message.contains(&final_path.display().to_string()));
        assert!(message.contains(&temp_path.display().to_string()));
    }

    #[test]
    fn quick_check_failure_precedes_backup_and_migration() {
        let db_path = unique_temp_db_path("quick-check-failure");
        let _cleanup = SqliteFixtureCleanup(db_path.clone());
        remove_sqlite_files(&db_path);
        let conn = Connection::open(&db_path).expect("corrupt fixture should open");
        conn.execute_batch(
            "CREATE TABLE checked(value INTEGER);
             INSERT INTO checked VALUES (-1);
             PRAGMA writable_schema = ON;
             UPDATE sqlite_master SET sql = 'CREATE TABLE checked(value INTEGER CHECK(value > 0))' WHERE name = 'checked';
             PRAGMA schema_version = 2;
             PRAGMA writable_schema = OFF;
             PRAGMA user_version = 6;",
        )
        .expect("corrupt fixture should seed");
        drop(conn);
        let before = std::fs::read(&db_path).expect("fixture bytes should be readable");
        let error = match Database::new(&db_path) {
            Ok(_) => panic!("quick_check should reject fixture"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("quick_check"));
        assert_eq!(
            std::fs::read(&db_path).expect("fixture should remain"),
            before
        );
        assert_no_migration_artifacts(&db_path);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn late_migration_failure_rolls_back_earlier_schema_changes() {
        let db_path = unique_temp_db_path("late-migration-failure");
        let _cleanup = SqliteFixtureCleanup(db_path.clone());
        remove_sqlite_files(&db_path);
        let conn = Connection::open(&db_path).expect("legacy fixture should open");
        conn.execute_batch(
            "CREATE TABLE wallpapers (id INTEGER PRIMARY KEY, path TEXT UNIQUE NOT NULL, hash TEXT NOT NULL, source TEXT NOT NULL);
             CREATE TABLE tags (name TEXT);
             CREATE TABLE wallpaper_tags (wallpaper_id INTEGER, tag_id INTEGER);
             PRAGMA user_version = 4;
             INSERT INTO wallpapers(path, hash, source) VALUES ('missing.jpg', 'h', 'mounted');",
        )
        .expect("late-failure fixture should seed");
        drop(conn);
        let backup_path = migration_backup_path(&db_path, 4);
        let error = match Database::new(&db_path) {
            Ok(_) => panic!("malformed child migration should fail"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("SQLite migration failed"));
        assert!(error.to_string().contains(&format!(
            "safety backup retained at {}",
            backup_path.display()
        )));
        let source = Connection::open(&db_path).expect("rolled-back source should open");
        assert_eq!(
            source
                .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                .expect("source version should remain old"),
            4
        );
        let columns: Vec<String> = source
            .prepare("PRAGMA table_info(wallpapers)")
            .expect("columns should prepare")
            .query_map([], |row| row.get(1))
            .expect("columns should query")
            .collect::<std::result::Result<_, _>>()
            .expect("columns should collect");
        assert!(!columns.contains(&"blacklisted".to_string()));
        assert!(!source
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name = 'idx_wallpapers_rating')",
                [],
                |row| row.get::<_, bool>(0),
            )
            .expect("index state should be readable"));
        assert_eq!(
            source
                .query_row("SELECT path FROM wallpapers", [], |row| row
                    .get::<_, String>(0))
                .expect("rolled-back source row should remain"),
            "missing.jpg"
        );
        drop(source);
        validate_backup(&backup_path, 4).expect("published backup should remain valid");
        let backup =
            Connection::open_with_flags(&backup_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .expect("published backup should reopen read-only");
        assert_eq!(
            backup
                .query_row("SELECT path FROM wallpapers", [], |row| row
                    .get::<_, String>(0))
                .expect("backup source row should remain"),
            "missing.jpg"
        );
        drop(backup);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn database_smoke_preserves_wallpaper_tag_and_play_stats() {
        let db_path = unique_temp_db_path("smoke");
        remove_sqlite_files(&db_path);

        let db = Database::new(&db_path).expect("database should initialize");
        let wallpaper_path = db_path
            .with_file_name("registered-wallpaper.jpg")
            .to_string_lossy()
            .to_string();

        let id = db
            .upsert_wallpaper(&wallpaper_path, "hash", "test", 1920, 1080, 12345)
            .expect("wallpaper insert should succeed");
        assert!(id > 0);

        let tag = db
            .create_tag("Landscape", DEFAULT_TAG_COLOR)
            .expect("tag insert should succeed");
        db.assign_tag(&wallpaper_path, tag.id)
            .expect("tag assignment should succeed");
        db.record_play(&wallpaper_path)
            .expect("recording play should succeed");

        let entry = db
            .get_wallpaper_by_path(&wallpaper_path)
            .expect("wallpaper lookup should succeed")
            .expect("wallpaper should exist");
        assert_eq!(entry.width, 1920);
        assert_eq!(entry.height, 1080);
        assert_eq!(entry.file_size, 12345);
        assert_eq!(entry.tags.len(), 1);
        assert_eq!(entry.tags[0].name, "Landscape");

        let stats = db.get_stats().expect("stats query should succeed");
        assert_eq!(stats.total, 1);
        assert_eq!(stats.total_plays, 1);

        drop(db);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn registered_available_paths_filters_batch_without_loading_wallpaper_rows() {
        let db_path = unique_temp_db_path("registered-path-batch");
        remove_sqlite_files(&db_path);
        let db = Database::new(&db_path).expect("database should initialize");
        let available = db_path
            .with_file_name("registered-available.jpg")
            .to_string_lossy()
            .to_string();
        let unavailable = db_path
            .with_file_name("registered-unavailable.jpg")
            .to_string_lossy()
            .to_string();
        let unregistered = db_path
            .with_file_name("unregistered.jpg")
            .to_string_lossy()
            .to_string();

        db.upsert_wallpaper(&available, "hash-a", "test", 100, 100, 10)
            .expect("available wallpaper should insert");
        db.upsert_wallpaper(&unavailable, "hash-u", "test", 100, 100, 10)
            .expect("unavailable wallpaper should insert");
        db.mark_paths_unavailable(std::slice::from_ref(&unavailable))
            .expect("wallpaper should become unavailable");

        let registered = db
            .registered_available_paths(&[
                available.clone(),
                unavailable,
                unregistered,
                available.clone(),
            ])
            .expect("registered path batch should load");

        assert_eq!(registered, HashSet::from([available]));

        drop(db);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn batch_rating_and_blacklist_return_affected_counts() {
        let db_path = unique_temp_db_path("batch-counts");
        remove_sqlite_files(&db_path);
        let db = Database::new(&db_path).expect("database should initialize");
        let first = r"C:\purewall-test\batch-count-first.jpg".to_string();
        let second = r"C:\purewall-test\batch-count-second.jpg".to_string();
        let paths = vec![first.clone(), second.clone()];

        db.upsert_wallpaper(&first, "hash-1", "test", 100, 100, 10)
            .expect("first wallpaper should insert");
        db.upsert_wallpaper(&second, "hash-2", "test", 100, 100, 10)
            .expect("second wallpaper should insert");

        assert_eq!(
            db.batch_set_rating(&paths, 1)
                .expect("ratings should update"),
            2
        );
        assert_eq!(
            db.batch_blacklist(&paths, true)
                .expect("hidden state should update"),
            2
        );
        assert_eq!(
            db.get_wallpaper_by_path(&first)
                .expect("first lookup should succeed")
                .expect("first wallpaper should exist")
                .rating,
            1
        );
        assert!(
            db.get_wallpaper_by_path(&second)
                .expect("second lookup should succeed")
                .expect("second wallpaper should exist")
                .blacklisted
        );

        drop(db);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn batch_tag_assignment_rolls_back_when_a_later_insert_fails() {
        let db_path = unique_temp_db_path("batch-tag-rollback");
        remove_sqlite_files(&db_path);
        let db = Database::new(&db_path).expect("database should initialize");
        let first = r"C:\purewall-test\batch-tag-first.jpg".to_string();
        let second = r"C:\purewall-test\batch-tag-second.jpg".to_string();

        db.upsert_wallpaper(&first, "hash-1", "test", 100, 100, 10)
            .expect("first wallpaper should insert");
        db.upsert_wallpaper(&second, "hash-2", "test", 100, 100, 10)
            .expect("second wallpaper should insert");
        let tag = db
            .create_tag("Rollback", DEFAULT_TAG_COLOR)
            .expect("tag should insert");
        let second_id = db
            .wallpaper_id(&second)
            .expect("second wallpaper ID should load");
        db.conn
            .execute_batch(&format!(
                "CREATE TRIGGER fail_second_batch_tag
                 BEFORE INSERT ON wallpaper_tags
                 WHEN NEW.wallpaper_id = {second_id}
                 BEGIN
                   SELECT RAISE(ABORT, 'forced second insert failure');
                 END;"
            ))
            .expect("failure trigger should install");

        let result = db.batch_assign_tag(&[first.clone(), second], tag.id);

        assert!(result.is_err());
        let remaining_links: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM wallpaper_tags", [], |row| row.get(0))
            .expect("tag link count should load");
        assert_eq!(remaining_links, 0);

        drop(db);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn batch_tag_removal_is_counted_and_idempotent() {
        let db_path = unique_temp_db_path("batch-tag-remove");
        remove_sqlite_files(&db_path);
        let db = Database::new(&db_path).expect("database should initialize");
        let first = r"C:\purewall-test\batch-tag-remove-first.jpg".to_string();
        let second = r"C:\purewall-test\batch-tag-remove-second.jpg".to_string();
        let paths = vec![first.clone(), second.clone()];

        db.upsert_wallpaper(&first, "hash-1", "test", 100, 100, 10)
            .expect("first wallpaper should insert");
        db.upsert_wallpaper(&second, "hash-2", "test", 100, 100, 10)
            .expect("second wallpaper should insert");
        let tag = db
            .create_tag("Remove", DEFAULT_TAG_COLOR)
            .expect("tag should insert");

        assert!(db.tag_exists(tag.id).expect("tag existence should load"));
        assert!(!db
            .tag_exists(tag.id + 100)
            .expect("missing tag should load"));
        assert_eq!(
            db.batch_assign_tag(&paths, tag.id)
                .expect("tags should assign"),
            2
        );
        assert_eq!(
            db.batch_unassign_tag(&paths, tag.id)
                .expect("tags should unassign"),
            2
        );
        assert_eq!(
            db.batch_unassign_tag(&paths, tag.id)
                .expect("repeated unassign should succeed"),
            0
        );

        drop(db);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn batch_collection_removal_is_counted_and_idempotent() {
        let db_path = unique_temp_db_path("batch-collection-remove");
        remove_sqlite_files(&db_path);
        let db = Database::new(&db_path).expect("database should initialize");
        let first = r"C:\purewall-test\batch-collection-remove-first.jpg".to_string();
        let second = r"C:\purewall-test\batch-collection-remove-second.jpg".to_string();
        let paths = vec![first.clone(), second.clone()];

        db.upsert_wallpaper(&first, "hash-1", "test", 100, 100, 10)
            .expect("first wallpaper should insert");
        db.upsert_wallpaper(&second, "hash-2", "test", 100, 100, 10)
            .expect("second wallpaper should insert");
        let collection = db
            .create_collection("Remove", "#4cc9f0")
            .expect("collection should insert");

        assert!(db
            .collection_exists(collection.id)
            .expect("collection existence should load"));
        assert!(!db
            .collection_exists(collection.id + 100)
            .expect("missing collection should load"));
        assert_eq!(
            db.batch_assign_collection(&paths, collection.id)
                .expect("collections should assign"),
            2
        );
        assert_eq!(
            db.batch_unassign_collection(&paths, collection.id)
                .expect("collections should unassign"),
            2
        );
        assert_eq!(
            db.batch_unassign_collection(&paths, collection.id)
                .expect("repeated unassign should succeed"),
            0
        );

        drop(db);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn playback_sampling_excludes_disliked_and_keeps_liked_two_tickets() {
        let candidates = vec![
            ("disliked".to_string(), -1),
            ("liked".to_string(), 1),
            ("normal-a".to_string(), 0),
            ("normal-b".to_string(), 0),
        ];

        for ticket in [0, 1] {
            let selected = sample_weighted_without_replacement(candidates.clone(), 1, |_| ticket);
            assert_eq!(selected, vec!["liked"]);
        }
        let normal_ticket = sample_weighted_without_replacement(candidates.clone(), 1, |_| 2);
        assert_eq!(normal_ticket, vec!["normal-a"]);

        let selected = sample_weighted_without_replacement(candidates, 3, |_| 0);
        assert_eq!(selected, vec!["liked", "normal-a", "normal-b"]);
        assert_eq!(
            selected.iter().collect::<HashSet<_>>().len(),
            selected.len()
        );
    }

    #[test]
    fn next_wallpapers_exclude_negative_hidden_and_unavailable_rows() {
        let db_path = unique_temp_db_path("playback-eligibility");
        remove_sqlite_files(&db_path);
        let db = Database::new(&db_path).expect("database should initialize");
        let file_prefix = db_path
            .file_stem()
            .expect("temporary database should have a file name")
            .to_string_lossy();
        let files = ["normal", "liked", "disliked", "hidden", "unavailable"]
            .map(|name| db_path.with_file_name(format!("{file_prefix}-{name}.jpg")));

        for (index, path) in files.iter().enumerate() {
            std::fs::write(path, b"image").expect("wallpaper file should exist");
            db.upsert_wallpaper(
                &path.to_string_lossy(),
                &format!("hash-{index}"),
                "test",
                100,
                100,
                10,
            )
            .expect("wallpaper should insert");
        }

        let normal = files[0].to_string_lossy().to_string();
        let liked = files[1].to_string_lossy().to_string();
        let disliked = files[2].to_string_lossy().to_string();
        let hidden = files[3].to_string_lossy().to_string();
        let unavailable = files[4].to_string_lossy().to_string();
        db.set_rating(&liked, 1).expect("liked rating should save");
        db.set_rating(&disliked, -1)
            .expect("disliked rating should save");
        db.set_blacklisted(&hidden, true)
            .expect("hidden state should save");
        db.mark_paths_unavailable(std::slice::from_ref(&unavailable))
            .expect("unavailable state should save");

        let selected = db
            .get_next_wallpapers(8)
            .expect("eligible wallpapers should be selected");
        let eligible = HashSet::from([normal, liked]);
        assert_eq!(selected.len(), 8);
        assert!(selected.iter().all(|path| eligible.contains(path)));
        assert_eq!(selected.into_iter().collect::<HashSet<_>>(), eligible);

        drop(db);
        for path in files {
            let _ = std::fs::remove_file(path);
        }
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn multi_display_selection_is_unique_until_candidates_are_exhausted() {
        let db_path = unique_temp_db_path("multi-display-selection");
        remove_sqlite_files(&db_path);
        let db = Database::new(&db_path).expect("database should initialize");
        let prefix = db_path
            .file_stem()
            .expect("temporary database should have a file name")
            .to_string_lossy();
        let files = ["first", "second", "third"]
            .map(|name| db_path.with_file_name(format!("{prefix}-{name}.jpg")));
        for (index, path) in files.iter().enumerate() {
            std::fs::write(path, b"image").expect("wallpaper file should exist");
            db.upsert_wallpaper(
                &path.to_string_lossy(),
                &format!("multi-display-hash-{index}"),
                "test",
                100,
                100,
                10,
            )
            .expect("wallpaper should insert");
        }

        let selected = db
            .get_next_wallpapers(3)
            .expect("three eligible wallpapers should be selected");
        assert_eq!(selected.len(), 3);
        assert_eq!(selected.iter().collect::<HashSet<_>>().len(), 3);

        db.mark_paths_unavailable(std::slice::from_ref(
            &files[2].to_string_lossy().to_string(),
        ))
        .expect("third wallpaper should become unavailable");
        let selected = db
            .get_next_wallpapers(3)
            .expect("two eligible wallpapers should fill displays");
        assert_eq!(selected.len(), 3);
        assert_eq!(selected.iter().collect::<HashSet<_>>().len(), 2);

        drop(db);
        for path in files {
            std::fs::remove_file(path).expect("temporary wallpaper file should be removed");
        }
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn multi_display_play_history_counts_each_successful_display_application() {
        let db_path = unique_temp_db_path("multi-display-history");
        remove_sqlite_files(&db_path);
        let db = Database::new(&db_path).expect("database should initialize");
        let prefix = db_path
            .file_stem()
            .expect("temporary database should have a file name")
            .to_string_lossy();
        let files =
            ["first", "second"].map(|name| db_path.with_file_name(format!("{prefix}-{name}.jpg")));
        for (index, path) in files.iter().enumerate() {
            std::fs::write(path, b"image").expect("wallpaper file should exist");
            db.upsert_wallpaper(
                &path.to_string_lossy(),
                &format!("multi-display-history-hash-{index}"),
                "test",
                100,
                100,
                10,
            )
            .expect("wallpaper should insert");
        }

        db.record_play_for_display(&files[0].to_string_lossy(), Some("DISPLAY-A"))
            .expect("first display application should record");
        db.record_play_for_display(&files[1].to_string_lossy(), Some("DISPLAY-B"))
            .expect("second display application should record");

        let stats = db
            .get_yearly_stats(chrono::Utc::now().year())
            .expect("yearly stats should load");
        assert_eq!(stats.total_plays, 2);
        assert_eq!(stats.unique_wallpapers, 2);
        assert_eq!(stats.top_wallpapers.len(), 2);
        assert!(stats.top_wallpapers.iter().all(|row| row.plays == 1));
        let distinct_display_ids: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(DISTINCT display_id) FROM play_events WHERE display_id IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .expect("display history should be queryable");
        assert_eq!(distinct_display_ids, 2);

        drop(db);
        for path in files {
            std::fs::remove_file(path).expect("temporary wallpaper file should be removed");
        }
        remove_sqlite_files(&db_path);
    }
    #[test]
    fn remove_wallpapers_removes_rows_and_tag_links_in_one_call() {
        let db_path = unique_temp_db_path("batch-remove");
        remove_sqlite_files(&db_path);
        let db = Database::new(&db_path).expect("database should initialize");

        let first = r"C:\purewall-test\first.jpg";
        let second = r"C:\purewall-test\second.jpg";
        db.upsert_wallpaper(first, "hash-1", "test", 100, 100, 10)
            .expect("first wallpaper insert should succeed");
        db.upsert_wallpaper(second, "hash-2", "test", 100, 100, 10)
            .expect("second wallpaper insert should succeed");
        let tag = db
            .create_tag("Batch", DEFAULT_TAG_COLOR)
            .expect("tag insert should succeed");
        db.assign_tag(first, tag.id)
            .expect("first tag assignment should succeed");
        db.assign_tag(second, tag.id)
            .expect("second tag assignment should succeed");

        db.remove_wallpapers(&[first.to_string(), second.to_string()])
            .expect("batch remove should succeed");

        assert!(db
            .get_wallpaper_by_path(first)
            .expect("first lookup should succeed")
            .is_none());
        assert!(db
            .get_wallpaper_by_path(second)
            .expect("second lookup should succeed")
            .is_none());
        let remaining_links: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM wallpaper_tags", [], |row| row.get(0))
            .expect("tag link count should succeed");
        assert_eq!(remaining_links, 0);

        drop(db);
        remove_sqlite_files(&db_path);
    }
    #[test]
    fn collections_are_separate_from_tags_and_filter_wallpapers() {
        let db_path = unique_temp_db_path("collections");
        remove_sqlite_files(&db_path);
        let db = Database::new(&db_path).expect("database should initialize");
        let wallpaper_path = db_path.with_file_name("collection-wallpaper.jpg");
        std::fs::write(&wallpaper_path, b"image").expect("wallpaper file should exist");
        let wallpaper = wallpaper_path.to_string_lossy().to_string();

        db.upsert_wallpaper(&wallpaper, "hash", "test", 100, 100, 10)
            .expect("wallpaper insert should succeed");
        let tag = db
            .create_tag("Blue", DEFAULT_TAG_COLOR)
            .expect("tag insert should succeed");
        let collection = db
            .create_collection("Weekend", "#4cc9f0")
            .expect("collection insert should succeed");
        db.assign_tag(&wallpaper, tag.id)
            .expect("tag assignment should succeed");
        db.assign_collection(&wallpaper, collection.id)
            .expect("collection assignment should succeed");

        let tags = db.get_tags().expect("tags should load");
        let collections = db.get_collections().expect("collections should load");
        assert_eq!(tags.len(), 1);
        assert_eq!(collections.len(), 1);
        assert_eq!(collections[0].wallpaper_count, 1);

        let by_tag = db
            .get_wallpapers_filtered(&format!("tag:{}", tag.id), "created")
            .expect("tag filter should load");
        let by_collection = db
            .get_wallpapers_filtered(&format!("collection:{}", collection.id), "created")
            .expect("collection filter should load");
        assert_eq!(by_tag.len(), 1);
        assert_eq!(by_collection.len(), 1);
        assert_eq!(by_collection[0].tags[0].name, "Blue");

        drop(db);
        let _ = std::fs::remove_file(wallpaper_path);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn wallpaper_page_returns_bounded_slice_and_total_count() {
        let db_path = unique_temp_db_path("wallpaper-page");
        remove_sqlite_files(&db_path);
        let db = Database::new(&db_path).expect("database should initialize");
        let mut paths = Vec::new();

        for index in 0..5 {
            let wallpaper_path = db_path.with_file_name(format!("page-wallpaper-{index}.jpg"));
            std::fs::write(&wallpaper_path, b"image").expect("wallpaper file should exist");
            let wallpaper = wallpaper_path.to_string_lossy().to_string();
            db.upsert_wallpaper(&wallpaper, "hash", "test", 100, 100, 10)
                .expect("wallpaper insert should succeed");
            db.conn
                .execute(
                    "UPDATE wallpapers SET created_at = ?2 WHERE path = ?1",
                    params![wallpaper, format!("2026-01-0{} 00:00:00", index + 1)],
                )
                .expect("created_at update should succeed");
            paths.push(wallpaper_path);
        }

        let page = db
            .get_wallpapers_page("all", "created", "", 1, 2)
            .expect("wallpaper page should load");
        assert_eq!(page.total, 5);
        assert_eq!(page.offset, 1);
        assert_eq!(page.limit, 2);
        assert!(page.has_more);
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.items[0].path, paths[3].to_string_lossy());
        assert_eq!(page.items[1].path, paths[2].to_string_lossy());

        let tail = db
            .get_wallpapers_page("all", "created", "", 4, 2)
            .expect("tail page should load");
        assert_eq!(tail.items.len(), 1);
        assert_eq!(tail.items[0].path, paths[0].to_string_lossy());
        assert!(!tail.has_more);

        drop(db);
        for path in paths {
            let _ = std::fs::remove_file(path);
        }
        remove_sqlite_files(&db_path);
    }
    #[test]
    fn wallpaper_page_searches_metadata_and_escapes_like_wildcards() {
        let db_path = unique_temp_db_path("wallpaper-page-search");
        remove_sqlite_files(&db_path);
        let db = Database::new(&db_path).expect("database should initialize");
        let lake_path = db_path.with_file_name("soft-lake.jpg");
        let tagged_path = db_path.with_file_name("tagged-wallpaper.jpg");
        let percent_path = db_path.with_file_name("percent-wallpaper.jpg");
        for path in [&lake_path, &tagged_path, &percent_path] {
            std::fs::write(path, b"image").expect("wallpaper file should exist");
        }

        let lake = lake_path.to_string_lossy().to_string();
        let tagged = tagged_path.to_string_lossy().to_string();
        let percent = percent_path.to_string_lossy().to_string();
        db.upsert_wallpaper(&lake, "hash-1", "camera", 100, 100, 10)
            .expect("lake wallpaper insert should succeed");
        db.upsert_wallpaper(&tagged, "hash-2", "archive", 100, 100, 10)
            .expect("tagged wallpaper insert should succeed");
        db.upsert_wallpaper(&percent, "hash-3", "archive", 100, 100, 10)
            .expect("percent wallpaper insert should succeed");
        db.set_wallpaper_display_title(&lake, "Soft Lake")
            .expect("lake title update should succeed");
        db.set_wallpaper_display_title(&percent, "100% Pure")
            .expect("percent title update should succeed");
        let tag = db
            .create_tag("Blue Hour", DEFAULT_TAG_COLOR)
            .expect("tag insert should succeed");
        db.assign_tag(&tagged, tag.id)
            .expect("tag assignment should succeed");

        let by_title = db
            .get_wallpapers_page("all", "created", "lake", 0, 10)
            .expect("title search should load");
        assert_eq!(by_title.total, 1);
        assert_eq!(by_title.items[0].path, lake);

        let by_tag = db
            .get_wallpapers_page("all", "created", "blue", 0, 10)
            .expect("tag search should load");
        assert_eq!(by_tag.total, 1);
        assert_eq!(by_tag.items[0].path, tagged);

        let by_literal_percent = db
            .get_wallpapers_page("all", "created", "100%", 0, 10)
            .expect("literal percent search should load");
        assert_eq!(by_literal_percent.total, 1);
        assert_eq!(by_literal_percent.items[0].path, percent);

        drop(db);
        for path in [lake_path, tagged_path, percent_path] {
            let _ = std::fs::remove_file(path);
        }
        remove_sqlite_files(&db_path);
    }
    #[test]
    fn custom_display_title_round_trips_and_clears_to_empty() {
        let db_path = unique_temp_db_path("display-title");
        remove_sqlite_files(&db_path);
        let db = Database::new(&db_path).expect("database should initialize");
        let wallpaper_path = r"C:\purewall-test\titled.jpg";

        db.upsert_wallpaper(wallpaper_path, "hash", "test", 1200, 800, 2048)
            .expect("wallpaper insert should succeed");
        db.set_wallpaper_display_title(wallpaper_path, "  Lake Light  ")
            .expect("title update should succeed");

        let entry = db
            .get_wallpaper_by_path(wallpaper_path)
            .expect("wallpaper lookup should succeed")
            .expect("wallpaper should exist");
        assert_eq!(entry.display_title, "Lake Light");

        db.set_wallpaper_display_title(wallpaper_path, "  ")
            .expect("title clear should succeed");
        let entry = db
            .get_wallpaper_by_path(wallpaper_path)
            .expect("wallpaper lookup should succeed")
            .expect("wallpaper should exist");
        assert_eq!(entry.display_title, "");

        drop(db);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn unavailable_wallpapers_do_not_distort_page_stats_or_collection_counts() {
        let db_path = unique_temp_db_path("file-availability");
        let available_path = db_path.with_extension("available.jpg");
        let missing_path = db_path.with_extension("missing.jpg");
        remove_sqlite_files(&db_path);
        let _ = std::fs::remove_file(&available_path);
        let _ = std::fs::remove_file(&missing_path);
        std::fs::write(&available_path, b"available")
            .expect("available wallpaper file should exist");
        let available = available_path.to_string_lossy().to_string();
        let missing = missing_path.to_string_lossy().to_string();
        let db = Database::new(&db_path).expect("database should initialize");

        db.upsert_wallpaper(&available, "hash-a", "test", 100, 100, 10)
            .expect("available wallpaper insert should succeed");
        db.upsert_wallpaper(&missing, "hash-m", "test", 100, 100, 10)
            .expect("missing wallpaper insert should succeed");
        let collection = db
            .create_collection("Availability", DEFAULT_TAG_COLOR)
            .expect("collection should be created");
        db.assign_collection(&available, collection.id)
            .expect("available wallpaper should join collection");
        db.assign_collection(&missing, collection.id)
            .expect("missing wallpaper should join collection");

        db.mark_paths_unavailable(std::slice::from_ref(&missing))
            .expect("missing wallpaper should become unavailable");

        let page = db
            .get_wallpapers_page("all", "created", "", 0, 1)
            .expect("available wallpaper page should load");
        assert_eq!(page.total, 1);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].path, available);
        assert!(!page.has_more);
        assert_eq!(db.get_stats().expect("stats should load").total, 1);
        assert_eq!(
            db.get_collections().expect("collections should load")[0].wallpaper_count,
            1
        );
        assert!(db
            .get_wallpaper_by_path(&missing)
            .expect("missing row lookup should succeed")
            .is_some());

        std::fs::write(&missing_path, b"restored").expect("missing wallpaper should be restored");
        db.upsert_wallpaper(&missing, "hash-restored", "test", 100, 100, 20)
            .expect("upsert should restore availability");
        assert_eq!(db.get_stats().expect("restored stats should load").total, 2);

        drop(db);
        let _ = std::fs::remove_file(available_path);
        let _ = std::fs::remove_file(missing_path);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn migration_backfills_file_availability_from_disk() {
        let db_path = unique_temp_db_path("file-availability-migration");
        let available_path = db_path.with_extension("legacy-available.jpg");
        let missing_path = db_path.with_extension("legacy-missing.jpg");
        remove_sqlite_files(&db_path);
        let _ = std::fs::remove_file(&available_path);
        let _ = std::fs::remove_file(&missing_path);
        std::fs::write(&available_path, b"available")
            .expect("legacy available wallpaper should exist");
        let available = available_path.to_string_lossy().to_string();
        let missing = missing_path.to_string_lossy().to_string();

        let legacy = Connection::open(&db_path).expect("legacy database should open");
        legacy
            .execute_batch(
                "CREATE TABLE wallpapers (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    path TEXT UNIQUE NOT NULL,
                    hash TEXT NOT NULL DEFAULT '',
                    source TEXT NOT NULL DEFAULT 'mounted',
                    display_title TEXT NOT NULL DEFAULT '',
                    rating INTEGER NOT NULL DEFAULT 0,
                    play_count INTEGER NOT NULL DEFAULT 0,
                    last_played TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    blacklisted INTEGER NOT NULL DEFAULT 0,
                    width INTEGER NOT NULL DEFAULT 0,
                    height INTEGER NOT NULL DEFAULT 0,
                    file_size INTEGER NOT NULL DEFAULT 0
                );
                PRAGMA user_version = 4;",
            )
            .expect("legacy schema should initialize");
        legacy
            .execute(
                "INSERT INTO wallpapers (path, hash) VALUES (?1, 'available')",
                [&available],
            )
            .expect("legacy available row should insert");
        legacy
            .execute(
                "INSERT INTO wallpapers (path, hash) VALUES (?1, 'missing')",
                [&missing],
            )
            .expect("legacy missing row should insert");
        drop(legacy);

        let db = Database::new(&db_path).expect("legacy database should migrate");
        let page = db
            .get_wallpapers_page("all", "created", "", 0, 10)
            .expect("migrated page should load");
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].path, available);
        let missing_available: i64 = db
            .conn
            .query_row(
                "SELECT file_available FROM wallpapers WHERE path = ?1",
                [&missing],
                |row| row.get(0),
            )
            .expect("legacy missing row should remain");
        assert_eq!(missing_available, 0);

        drop(db);
        let _ = std::fs::remove_file(available_path);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn watched_folders_persist_and_snapshot_reconciliation_marks_missing_descendants() {
        let db_path = unique_temp_db_path("watched-folder-registry");
        let watched_dir = db_path.with_extension("watched-root");
        remove_sqlite_files(&db_path);
        let _ = std::fs::remove_dir_all(&watched_dir);
        std::fs::create_dir_all(&watched_dir).expect("watched directory should exist");
        let available_path = watched_dir.join("available.jpg");
        std::fs::write(&available_path, b"available").expect("available wallpaper should exist");
        let root = watched_dir
            .canonicalize()
            .expect("watched root should canonicalize")
            .to_string_lossy()
            .to_string();
        let available = available_path
            .canonicalize()
            .expect("available wallpaper should canonicalize")
            .to_string_lossy()
            .to_string();
        let missing = Path::new(&root)
            .join("missing.jpg")
            .to_string_lossy()
            .to_string();
        let db = Database::new(&db_path).expect("database should initialize");

        db.upsert_watched_folder(&root, "imported-folder")
            .expect("watched folder should persist");
        db.upsert_watched_folder(&root, "mounted")
            .expect("duplicate watched folder should update in place");
        let watched_folders = db
            .get_watched_folders()
            .expect("watched folders should load");
        assert_eq!(watched_folders.len(), 1);
        assert_eq!(watched_folders[0].path, root);
        assert_eq!(watched_folders[0].source, "mounted");

        db.upsert_wallpaper(&available, "available", "mounted", 100, 100, 10)
            .expect("available wallpaper should insert");
        db.upsert_wallpaper(&missing, "missing", "mounted", 100, 100, 10)
            .expect("missing wallpaper should insert");
        let changed = db
            .reconcile_watched_folder_availability(
                &watched_folders[0].path,
                std::slice::from_ref(&available),
            )
            .expect("folder snapshot should reconcile");
        assert_eq!(changed, 1);
        assert_eq!(db.get_stats().expect("stats should load").total, 1);
        assert!(db
            .get_wallpaper_by_path(&missing)
            .expect("missing metadata lookup should succeed")
            .is_some());

        drop(db);
        let _ = std::fs::remove_dir_all(watched_dir);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn schema_seven_migrates_existing_watched_folder_scan_columns() {
        let db_path = unique_temp_db_path("source-schema-seven");
        remove_sqlite_files(&db_path);
        {
            let conn = Connection::open(&db_path).expect("legacy database should open");
            conn.execute_batch(
                "CREATE TABLE watched_folders (
                    path TEXT PRIMARY KEY,
                    source TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
                INSERT INTO watched_folders(path, source)
                VALUES ('D:\\PureWall-Test\\Walls', 'mounted');
                PRAGMA user_version = 6;",
            )
            .expect("legacy source schema should seed");
        }

        let db = Database::new(&db_path).expect("schema seven should migrate");
        let columns = {
            let mut stmt = db
                .conn
                .prepare("PRAGMA table_info(watched_folders)")
                .expect("source columns should prepare");
            stmt.query_map([], |row| row.get::<_, String>(1))
                .expect("source columns should query")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("source columns should collect")
        };
        assert!(columns.contains(&"last_scan_at".to_string()));
        assert!(columns.contains(&"last_error".to_string()));
        assert_eq!(db.get_watched_folders().unwrap().len(), 1);

        drop(db);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn watched_folder_scan_state_and_counts_round_trip() {
        let db_path = unique_temp_db_path("source-summaries");
        remove_sqlite_files(&db_path);
        let db = Database::new(&db_path).expect("database should initialize");
        let root = r"D:\PureWall-Test\Walls";

        db.upsert_watched_folder(root, "mounted")
            .expect("source should persist");
        db.record_watched_folder_error(root, "permission denied")
            .expect("source error should persist");
        db.upsert_wallpaper(
            r"D:\PureWall-Test\Walls\available.jpg",
            "available",
            "mounted",
            10,
            10,
            1,
        )
        .expect("available wallpaper should insert");
        db.upsert_wallpaper(
            r"D:\PureWall-Test\Walls\missing.jpg",
            "missing",
            "mounted",
            10,
            10,
            1,
        )
        .expect("missing wallpaper should insert");
        db.mark_paths_unavailable(&[r"D:\PureWall-Test\Walls\missing.jpg".to_string()])
            .expect("missing wallpaper should become unavailable");

        let summaries = db
            .get_watched_folder_summaries()
            .expect("source summaries should load");
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].available_count, 1);
        assert_eq!(summaries[0].unavailable_count, 1);
        assert_eq!(
            summaries[0].entry.last_error.as_deref(),
            Some("permission denied"),
        );

        db.record_watched_folder_scan_success(root)
            .expect("scan success should clear the error");
        let entry = db
            .get_watched_folders()
            .expect("sources should load")
            .remove(0);
        assert!(entry.last_scan_at.is_some());
        assert_eq!(entry.last_error, None);

        drop(db);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn removing_parent_source_protects_wallpapers_covered_by_child_source() {
        let db_path = unique_temp_db_path("source-overlap-remove");
        remove_sqlite_files(&db_path);
        let parent_dir = db_path.with_extension("source-overlap-root");
        let child_dir = parent_dir.join("Keep");
        let _ = std::fs::remove_dir_all(&parent_dir);
        std::fs::create_dir_all(&child_dir).expect("source roots should exist");
        let exclusive_path = parent_dir.join("exclusive.jpg");
        let covered_path = child_dir.join("covered.jpg");
        std::fs::write(&exclusive_path, b"exclusive").expect("exclusive file should exist");
        std::fs::write(&covered_path, b"covered").expect("covered file should exist");
        let parent = parent_dir
            .canonicalize()
            .expect("parent source should canonicalize")
            .to_string_lossy()
            .to_string();
        let child = child_dir
            .canonicalize()
            .expect("child source should canonicalize")
            .to_string_lossy()
            .to_string();
        let exclusive = exclusive_path
            .canonicalize()
            .expect("exclusive file should canonicalize")
            .to_string_lossy()
            .to_string();
        let covered = covered_path
            .canonicalize()
            .expect("covered file should canonicalize")
            .to_string_lossy()
            .to_string();
        let db = Database::new(&db_path).expect("database should initialize");

        db.upsert_watched_folder(&parent, "mounted").unwrap();
        db.upsert_watched_folder(&child, "imported-folder").unwrap();
        db.upsert_wallpaper(&exclusive, "exclusive", "mounted", 10, 10, 1)
            .unwrap();
        db.upsert_wallpaper(&covered, "covered", "mounted", 10, 10, 1)
            .unwrap();

        assert_eq!(db.source_removal_impact(&parent).unwrap(), 1);
        let summary = db
            .remove_watched_folder(&parent, SourceRemovalMode::KeepMetadata)
            .unwrap();
        assert_eq!(summary.affected_wallpapers, 1);
        assert!(db.get_wallpaper_by_path(&exclusive).unwrap().is_some());
        assert_eq!(db.get_stats().unwrap().total, 1);
        assert_eq!(
            db.get_wallpapers_page("all", "created", "", 0, 10)
                .unwrap()
                .items[0]
                .path,
            covered,
        );
        assert!(exclusive_path.is_file());
        assert!(covered_path.is_file());

        drop(db);
        let _ = std::fs::remove_dir_all(parent_dir);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn clear_source_deletes_only_exclusive_metadata() {
        let db_path = unique_temp_db_path("source-clear-remove");
        remove_sqlite_files(&db_path);
        let parent_dir = db_path.with_extension("source-clear-root");
        let child_dir = parent_dir.join("Keep");
        let _ = std::fs::remove_dir_all(&parent_dir);
        std::fs::create_dir_all(&child_dir).expect("source roots should exist");
        let exclusive_path = parent_dir.join("exclusive.jpg");
        let covered_path = child_dir.join("covered.jpg");
        std::fs::write(&exclusive_path, b"exclusive").expect("exclusive file should exist");
        std::fs::write(&covered_path, b"covered").expect("covered file should exist");
        let parent = parent_dir
            .canonicalize()
            .expect("parent source should canonicalize")
            .to_string_lossy()
            .to_string();
        let child = child_dir
            .canonicalize()
            .expect("child source should canonicalize")
            .to_string_lossy()
            .to_string();
        let exclusive = exclusive_path
            .canonicalize()
            .expect("exclusive file should canonicalize")
            .to_string_lossy()
            .to_string();
        let covered = covered_path
            .canonicalize()
            .expect("covered file should canonicalize")
            .to_string_lossy()
            .to_string();
        let db = Database::new(&db_path).expect("database should initialize");

        db.upsert_watched_folder(&parent, "mounted").unwrap();
        db.upsert_watched_folder(&child, "imported-folder").unwrap();
        db.upsert_wallpaper(&exclusive, "exclusive", "mounted", 10, 10, 1)
            .unwrap();
        db.upsert_wallpaper(&covered, "covered", "mounted", 10, 10, 1)
            .unwrap();

        db.remove_watched_folder(&parent, SourceRemovalMode::ClearMetadata)
            .unwrap();
        assert!(db.get_wallpaper_by_path(&exclusive).unwrap().is_none());
        assert!(db.get_wallpaper_by_path(&covered).unwrap().is_some());
        assert!(exclusive_path.is_file());
        assert!(covered_path.is_file());

        drop(db);
        let _ = std::fs::remove_dir_all(parent_dir);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn relocate_source_preserves_ids_and_metadata_by_relative_path() {
        let db_path = unique_temp_db_path("source-relocate");
        remove_sqlite_files(&db_path);
        let (base, old_dir, new_dir, old_root, new_root) =
            relocation_roots(&db_path, "source-relocate-root");
        let old_nature = old_dir.join("Nature");
        let new_nature = new_dir.join("Nature");
        std::fs::create_dir_all(&old_nature).unwrap();
        std::fs::create_dir_all(&new_nature).unwrap();
        let old_file = old_nature.join("lake.jpg");
        let new_file = new_nature.join("lake.jpg");
        std::fs::write(&old_file, b"old").unwrap();
        std::fs::write(&new_file, b"new").unwrap();
        let old_path = old_file
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let new_path = new_file
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let db = Database::new(&db_path).unwrap();

        db.upsert_watched_folder(&old_root, "mounted").unwrap();
        db.upsert_wallpaper(&old_path, "old", "mounted", 10, 10, 1)
            .unwrap();
        db.set_rating(&old_path, 1).unwrap();
        db.set_wallpaper_display_title(&old_path, "Quiet Lake")
            .unwrap();
        let tag = db.create_tag("Nature", DEFAULT_TAG_COLOR).unwrap();
        db.assign_tag(&old_path, tag.id).unwrap();
        let collection = db
            .create_collection("Relocation", DEFAULT_TAG_COLOR)
            .unwrap();
        db.assign_collection(&old_path, collection.id).unwrap();
        db.record_play(&old_path).unwrap();
        let before = db.get_wallpaper_by_path(&old_path).unwrap().unwrap();

        let result = db
            .relocate_watched_folder(
                &old_root,
                &new_root,
                "mounted",
                &[ScannedWallpaperRecord {
                    path: new_path.clone(),
                    hash: "new".to_string(),
                    width: 1920,
                    height: 1080,
                    file_size: 42,
                }],
            )
            .unwrap();

        assert_eq!(
            result,
            SourceRelocationSummary {
                matched: 1,
                imported: 0,
                unavailable: 0,
            },
        );
        let after = db.get_wallpaper_by_path(&new_path).unwrap().unwrap();
        assert_eq!(after.id, before.id);
        assert_eq!(after.rating, 1);
        assert_eq!(after.display_title, "Quiet Lake");
        assert_eq!(after.play_count, 1);
        assert_eq!(after.tags.len(), 1);
        assert_eq!(after.tags[0].name, "Nature");
        assert_eq!(after.width, 1920);
        assert!(db.get_wallpaper_by_path(&old_path).unwrap().is_none());
        let source = db.get_watched_folders().unwrap().remove(0);
        assert_eq!(source.path, new_root);
        assert!(source.last_scan_at.is_some());
        assert_eq!(source.last_error, None);
        assert_eq!(db.get_collections().unwrap()[0].wallpaper_count, 1);

        drop(db);
        let _ = std::fs::remove_dir_all(base);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn relocate_source_keeps_unmatched_metadata_unavailable_and_imports_new_files() {
        let db_path = unique_temp_db_path("source-relocate-partial");
        remove_sqlite_files(&db_path);
        let (base, old_dir, new_dir, old_root, new_root) =
            relocation_roots(&db_path, "source-relocate-partial-root");
        let missing_file = old_dir.join("missing.jpg");
        let added_file = new_dir.join("added.jpg");
        std::fs::write(&missing_file, b"missing").unwrap();
        std::fs::write(&added_file, b"added").unwrap();
        let missing_old = missing_file
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let added_new = added_file
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let db = Database::new(&db_path).unwrap();

        db.upsert_watched_folder(&old_root, "mounted").unwrap();
        db.upsert_wallpaper(&missing_old, "missing", "mounted", 10, 10, 1)
            .unwrap();

        let result = db
            .relocate_watched_folder(
                &old_root,
                &new_root,
                "mounted",
                &[ScannedWallpaperRecord {
                    path: added_new.clone(),
                    hash: "added".to_string(),
                    width: 20,
                    height: 10,
                    file_size: 2,
                }],
            )
            .unwrap();

        assert_eq!(result.matched, 0);
        assert_eq!(result.imported, 1);
        assert_eq!(result.unavailable, 1);
        assert!(db.get_wallpaper_by_path(&missing_old).unwrap().is_some());
        assert_eq!(db.get_stats().unwrap().total, 1);
        assert_eq!(
            db.get_wallpapers_page("all", "created", "", 0, 10)
                .unwrap()
                .items[0]
                .path,
            added_new,
        );

        drop(db);
        let _ = std::fs::remove_dir_all(base);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn relocate_collision_rolls_back_source_and_wallpaper_paths() {
        let db_path = unique_temp_db_path("source-relocate-collision");
        remove_sqlite_files(&db_path);
        let (base, old_dir, new_dir, old_root, new_root) =
            relocation_roots(&db_path, "source-relocate-collision-root");
        let old_file = old_dir.join("same.jpg");
        let occupied_file = new_dir.join("same.jpg");
        std::fs::write(&old_file, b"old").unwrap();
        std::fs::write(&occupied_file, b"occupied").unwrap();
        let old_path = old_file
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let occupied = occupied_file
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let db = Database::new(&db_path).unwrap();

        db.upsert_watched_folder(&old_root, "mounted").unwrap();
        db.upsert_wallpaper(&old_path, "old", "mounted", 10, 10, 1)
            .unwrap();
        db.upsert_wallpaper(&occupied, "occupied", "mounted", 10, 10, 1)
            .unwrap();

        let error = db
            .relocate_watched_folder(
                &old_root,
                &new_root,
                "mounted",
                &[ScannedWallpaperRecord {
                    path: occupied.clone(),
                    hash: "new".to_string(),
                    width: 20,
                    height: 10,
                    file_size: 2,
                }],
            )
            .expect_err("collision must abort");
        assert!(error.to_string().contains("relocation collision"));
        assert_eq!(db.get_watched_folders().unwrap()[0].path, old_root);
        assert!(db.get_wallpaper_by_path(&old_path).unwrap().is_some());
        assert!(db.get_wallpaper_by_path(&occupied).unwrap().is_some());

        drop(db);
        let _ = std::fs::remove_dir_all(base);
        remove_sqlite_files(&db_path);
    }

    #[test]
    fn relocate_source_rejects_overlap_with_remaining_root() {
        let db_path = unique_temp_db_path("source-relocate-overlap");
        remove_sqlite_files(&db_path);
        let (base, _old_dir, _new_dir, old_root, new_root) =
            relocation_roots(&db_path, "source-relocate-overlap-root");
        let remaining_root = base.canonicalize().unwrap().to_string_lossy().to_string();
        let db = Database::new(&db_path).unwrap();
        db.upsert_watched_folder(&old_root, "mounted").unwrap();
        db.upsert_watched_folder(&remaining_root, "imported-folder")
            .unwrap();

        let error = db
            .relocate_watched_folder(&old_root, &new_root, "mounted", &[])
            .expect_err("overlapping target must abort");
        assert!(error.to_string().contains("source path conflict"));
        assert!(db
            .get_watched_folders()
            .unwrap()
            .iter()
            .any(|entry| entry.path == old_root));

        drop(db);
        let _ = std::fs::remove_dir_all(base);
        remove_sqlite_files(&db_path);
    }
}
