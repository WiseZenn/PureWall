use anyhow::{Context, Result};
use rusqlite::{params, params_from_iter};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::{
    Database, ScannedWallpaperRecord, SourceRelocationSummary, SourceRemovalMode,
    SourceRemovalSummary, WatchedFolderEntry, WatchedFolderSummary,
};
use crate::paths::{
    path_identity_key, path_is_same_or_descendant, paths_overlap, relative_identity,
};

impl Database {
    pub fn upsert_watched_folder(&self, path: &str, source: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO watched_folders (path, source) VALUES (?1, ?2)
             ON CONFLICT(path) DO UPDATE SET source = excluded.source",
            params![path, source],
        )?;
        Ok(())
    }

    pub fn get_watched_folders(&self) -> Result<Vec<WatchedFolderEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT path, source, last_scan_at, last_error
             FROM watched_folders
             ORDER BY created_at ASC, path ASC",
        )?;
        let watched_folders = stmt
            .query_map([], |row| {
                Ok(WatchedFolderEntry {
                    path: row.get(0)?,
                    source: row.get(1)?,
                    last_scan_at: row.get(2)?,
                    last_error: row.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(watched_folders)
    }

    pub fn get_watched_folder_summaries(&self) -> Result<Vec<WatchedFolderSummary>> {
        let entries = self.get_watched_folders()?;
        let wallpapers = {
            let mut stmt = self
                .conn
                .prepare("SELECT path, file_available FROM wallpapers")?;
            let wallpapers = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            wallpapers
        };

        Ok(entries
            .into_iter()
            .map(|entry| {
                let (available_count, unavailable_count) = wallpapers
                    .iter()
                    .filter(|(path, _)| {
                        relative_identity(Path::new(path), Path::new(&entry.path)).is_some()
                    })
                    .fold((0, 0), |(available, unavailable), (_, is_available)| {
                        if *is_available {
                            (available + 1, unavailable)
                        } else {
                            (available, unavailable + 1)
                        }
                    });
                WatchedFolderSummary {
                    entry,
                    available_count,
                    unavailable_count,
                }
            })
            .collect())
    }

    pub fn record_watched_folder_scan_success(&self, path: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE watched_folders
             SET last_scan_at = datetime('now'), last_error = NULL
             WHERE path = ?1",
            [path],
        )?;
        Ok(())
    }

    pub fn record_watched_folder_error(&self, path: &str, message: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE watched_folders SET last_error = ?2 WHERE path = ?1",
            params![path, message],
        )?;
        Ok(())
    }

    pub fn source_removal_impact(&self, path: &str) -> Result<usize> {
        let (_, wallpaper_ids) = self.source_removal_selection(path)?;
        Ok(wallpaper_ids.len())
    }

    pub fn remove_watched_folder(
        &self,
        path: &str,
        mode: SourceRemovalMode,
    ) -> Result<SourceRemovalSummary> {
        let (stored_root, wallpaper_ids) = self.source_removal_selection(path)?;
        let affected_wallpapers = wallpaper_ids.len();

        self.conn.execute_batch("BEGIN")?;
        let result: Result<SourceRemovalSummary> = (|| {
            self.conn.execute(
                "DELETE FROM watched_folders WHERE path = ?1",
                [&stored_root],
            )?;
            for id in wallpaper_ids {
                match mode {
                    SourceRemovalMode::KeepMetadata => {
                        self.conn.execute(
                            "UPDATE wallpapers SET file_available = 0 WHERE id = ?1",
                            [id],
                        )?;
                    }
                    SourceRemovalMode::ClearMetadata => {
                        self.conn
                            .execute("DELETE FROM wallpapers WHERE id = ?1", [id])?;
                    }
                }
            }
            Ok(SourceRemovalSummary {
                affected_wallpapers,
            })
        })();

        match result {
            Ok(summary) => {
                self.conn.execute_batch("COMMIT")?;
                Ok(summary)
            }
            Err(error) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }

    fn source_removal_selection(&self, path: &str) -> Result<(String, Vec<i64>)> {
        let source_roots = self.get_watched_folders()?;
        let requested_key = path_identity_key(Path::new(path));
        let stored_root = source_roots
            .iter()
            .find(|source| path_identity_key(Path::new(&source.path)) == requested_key)
            .map(|source| source.path.clone())
            .with_context(|| format!("Unknown watched folder: {path}"))?;
        let remaining_roots = source_roots
            .iter()
            .filter(|source| {
                path_identity_key(Path::new(&source.path)) != requested_key
                    && paths_overlap(Path::new(&stored_root), Path::new(&source.path))
            })
            .map(|source| source.path.as_str())
            .collect::<Vec<_>>();
        let wallpapers = {
            let mut stmt = self
                .conn
                .prepare("SELECT id, path, file_available FROM wallpapers")?;
            let wallpapers = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, bool>(2)?,
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            wallpapers
        };
        let selected_ids = wallpapers
            .into_iter()
            .filter_map(|(id, wallpaper_path, _file_available)| {
                let wallpaper_path = Path::new(&wallpaper_path);
                (relative_identity(wallpaper_path, Path::new(&stored_root)).is_some()
                    && !remaining_roots
                        .iter()
                        .any(|root| relative_identity(wallpaper_path, Path::new(root)).is_some()))
                .then_some(id)
            })
            .collect();

        Ok((stored_root, selected_ids))
    }

    pub fn relocate_watched_folder(
        &self,
        old_root: &str,
        new_root: &str,
        source: &str,
        scanned: &[ScannedWallpaperRecord],
    ) -> Result<SourceRelocationSummary> {
        let source_roots = self.get_watched_folders()?;
        let old_key = path_identity_key(Path::new(old_root));
        let stored_old_root = source_roots
            .iter()
            .find(|entry| path_identity_key(Path::new(&entry.path)) == old_key)
            .map(|entry| entry.path.clone())
            .with_context(|| format!("Unknown watched folder: {old_root}"))?;

        for remaining in source_roots
            .iter()
            .filter(|entry| path_identity_key(Path::new(&entry.path)) != old_key)
        {
            if paths_overlap(Path::new(new_root), Path::new(&remaining.path)) {
                anyhow::bail!("source path conflict: target overlaps {}", remaining.path);
            }
        }

        let wallpapers = {
            let mut stmt = self.conn.prepare("SELECT id, path FROM wallpapers")?;
            let wallpapers = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            wallpapers
        };

        let mut old_by_relative = HashMap::new();
        for (id, path) in &wallpapers {
            let Some(relative) = relative_identity(Path::new(path), Path::new(&stored_old_root))
            else {
                continue;
            };
            if relative.is_empty()
                || old_by_relative
                    .insert(relative, (*id, path.clone()))
                    .is_some()
            {
                anyhow::bail!("relocation collision: duplicate old-source identity");
            }
        }

        let mut scanned_by_relative = HashMap::new();
        for record in scanned {
            let relative = relative_identity(Path::new(&record.path), Path::new(new_root))
                .with_context(|| {
                    format!(
                        "Scanned wallpaper is outside relocation target: {}",
                        record.path
                    )
                })?;
            if relative.is_empty() || scanned_by_relative.insert(relative, record).is_some() {
                anyhow::bail!("relocation collision: duplicate scanned identity");
            }
        }

        let mut existing_by_identity: HashMap<String, Vec<(i64, Option<String>)>> = HashMap::new();
        for (id, path) in &wallpapers {
            existing_by_identity
                .entry(path_identity_key(Path::new(path)))
                .or_default()
                .push((
                    *id,
                    relative_identity(Path::new(path), Path::new(&stored_old_root)),
                ));
        }
        for (relative, record) in &scanned_by_relative {
            let Some(owners) =
                existing_by_identity.get(&path_identity_key(Path::new(&record.path)))
            else {
                continue;
            };
            let matched_id = old_by_relative.get(relative).map(|(id, _)| *id);
            if owners.iter().any(|(id, _)| Some(*id) != matched_id) {
                anyhow::bail!("relocation collision: target path is already owned");
            }
        }

        let matched = old_by_relative
            .keys()
            .filter(|relative| scanned_by_relative.contains_key(*relative))
            .count();
        let unavailable = old_by_relative.len() - matched;
        let imported = scanned_by_relative
            .keys()
            .filter(|relative| !old_by_relative.contains_key(*relative))
            .count();

        self.conn.execute_batch("BEGIN")?;
        let result: Result<SourceRelocationSummary> = (|| {
            self.conn.execute(
                "UPDATE watched_folders
                 SET path = ?2,
                     source = ?3,
                     last_scan_at = datetime('now'),
                     last_error = NULL
                 WHERE path = ?1",
                params![stored_old_root, new_root, source],
            )?;

            for (relative, (id, _old_path)) in &old_by_relative {
                if let Some(record) = scanned_by_relative.get(relative) {
                    self.conn.execute(
                        "UPDATE wallpapers
                         SET path = ?2,
                             hash = ?3,
                             source = ?4,
                             width = ?5,
                             height = ?6,
                             file_size = ?7,
                             file_available = 1
                         WHERE id = ?1",
                        params![
                            id,
                            record.path,
                            record.hash,
                            source,
                            record.width,
                            record.height,
                            record.file_size,
                        ],
                    )?;
                } else {
                    self.conn.execute(
                        "UPDATE wallpapers SET file_available = 0 WHERE id = ?1",
                        [id],
                    )?;
                }
            }

            for (relative, record) in &scanned_by_relative {
                if old_by_relative.contains_key(relative) {
                    continue;
                }
                self.conn.execute(
                    "INSERT INTO wallpapers (
                        path, hash, source, width, height, file_size, file_available
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)",
                    params![
                        record.path,
                        record.hash,
                        source,
                        record.width,
                        record.height,
                        record.file_size,
                    ],
                )?;
            }

            Ok(SourceRelocationSummary {
                matched,
                imported,
                unavailable,
            })
        })();

        match result {
            Ok(summary) => match self.conn.execute_batch("COMMIT") {
                Ok(()) => Ok(summary),
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

    pub fn mark_paths_unavailable(&self, missing_paths: &[String]) -> Result<usize> {
        if missing_paths.is_empty() {
            return Ok(0);
        }

        let unavailable_roots = missing_paths
            .iter()
            .map(|path| Path::new(path.as_str()))
            .collect::<Vec<_>>();
        let wallpapers = {
            let mut stmt = self
                .conn
                .prepare("SELECT id, path FROM wallpapers WHERE file_available = 1")?;
            let wallpapers = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            wallpapers
        };
        let unavailable_ids = wallpapers
            .into_iter()
            .filter_map(|(id, path)| {
                unavailable_roots
                    .iter()
                    .any(|root| path_is_same_or_descendant(Path::new(&path), root))
                    .then_some(id)
            })
            .collect::<Vec<_>>();

        self.mark_wallpaper_ids_unavailable(unavailable_ids)
    }

    pub fn reconcile_watched_folder_availability(
        &self,
        folder_path: &str,
        existing_paths: &[String],
    ) -> Result<usize> {
        let existing_keys = existing_paths
            .iter()
            .map(|path| path_identity_key(Path::new(path)))
            .collect::<HashSet<_>>();
        let wallpapers = {
            let mut stmt = self
                .conn
                .prepare("SELECT id, path FROM wallpapers WHERE file_available = 1")?;
            let wallpapers = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            wallpapers
        };
        let root = Path::new(folder_path);
        let unavailable_ids = wallpapers
            .into_iter()
            .filter_map(|(id, path)| {
                (path_is_same_or_descendant(Path::new(&path), root)
                    && !existing_keys.contains(&path_identity_key(Path::new(&path))))
                .then_some(id)
            })
            .collect::<Vec<_>>();

        self.mark_wallpaper_ids_unavailable(unavailable_ids)
    }

    fn mark_wallpaper_ids_unavailable(&self, unavailable_ids: Vec<i64>) -> Result<usize> {
        if unavailable_ids.is_empty() {
            return Ok(0);
        }

        self.conn.execute_batch("BEGIN")?;
        let result: Result<usize> = (|| {
            let mut changed = 0;
            for id in unavailable_ids {
                changed += self.conn.execute(
                    "UPDATE wallpapers SET file_available = 0 WHERE id = ?1 AND file_available = 1",
                    [id],
                )?;
            }
            Ok(changed)
        })();

        match result {
            Ok(changed) => {
                self.conn.execute_batch("COMMIT")?;
                Ok(changed)
            }
            Err(error) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }

    pub fn registered_available_paths(&self, paths: &[String]) -> Result<HashSet<String>> {
        const SQLITE_QUERY_CHUNK_SIZE: usize = 500;
        let mut registered = HashSet::with_capacity(paths.len());

        for chunk in paths.chunks(SQLITE_QUERY_CHUNK_SIZE) {
            let placeholders = std::iter::repeat_n("?", chunk.len())
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "SELECT path FROM wallpapers WHERE file_available = 1 AND path IN ({placeholders})"
            );
            let mut statement = self.conn.prepare(&sql)?;
            let matches = statement.query_map(params_from_iter(chunk.iter()), |row| row.get(0))?;
            for path in matches {
                registered.insert(path?);
            }
        }

        Ok(registered)
    }
}
