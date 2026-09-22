use crate::library_backup::BackupSettings;
use anyhow::{Context, Result};
use rusqlite::{params, Transaction};
use std::collections::HashMap;

#[derive(Debug)]
pub(crate) struct ExistingBackupWallpaper {
    pub(crate) id: i64,
    pub(crate) path: String,
    pub(crate) display_title: String,
    pub(crate) rating: i32,
    pub(crate) hidden: bool,
}

pub(crate) fn backup_setting_count(settings: &BackupSettings) -> usize {
    [
        settings.rotation_secs.is_some(),
        settings.display_mode.is_some(),
        settings.focus_mode_enabled.is_some(),
        settings.paused.is_some(),
        settings.theme.is_some(),
        settings.workspace_mode.is_some(),
    ]
    .into_iter()
    .filter(|present| *present)
    .count()
}

pub(crate) fn load_named_ids(
    transaction: &Transaction<'_>,
    sql: &str,
) -> Result<HashMap<String, i64>> {
    let mut statement = transaction.prepare(sql)?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut ids = HashMap::new();
    for row in rows {
        let (id, name) = row?;
        ids.insert(name, id);
    }
    Ok(ids)
}

pub(crate) fn merge_backup_setting(
    transaction: &Transaction<'_>,
    key: &str,
    value: &str,
) -> Result<usize> {
    let current =
        match transaction.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
            row.get::<_, String>(0)
        }) {
            Ok(value) => Some(value),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(error) => return Err(error.into()),
        };
    if current.as_deref() == Some(value) {
        return Ok(0);
    }
    transaction.execute(
        "INSERT INTO settings (key, value)
         VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(1)
}

pub(crate) fn resolve_relation_ids(
    names: &[String],
    definitions: &HashMap<String, i64>,
    label: &str,
) -> Result<Vec<i64>> {
    let mut ids = names
        .iter()
        .map(|name| {
            definitions
                .get(name)
                .copied()
                .with_context(|| format!("undefined backup {label}: {name}"))
        })
        .collect::<Result<Vec<_>>>()?;
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

pub(crate) fn current_tag_ids(
    transaction: &Transaction<'_>,
    wallpaper_id: i64,
) -> Result<Vec<i64>> {
    let mut statement = transaction
        .prepare("SELECT tag_id FROM wallpaper_tags WHERE wallpaper_id = ?1 ORDER BY tag_id ASC")?;
    let rows = statement.query_map([wallpaper_id], |row| row.get::<_, i64>(0))?;
    let ids = rows.collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(ids)
}

pub(crate) fn current_collection_ids(
    transaction: &Transaction<'_>,
    wallpaper_id: i64,
) -> Result<Vec<i64>> {
    let mut statement = transaction.prepare(
        "SELECT collection_id
         FROM collection_wallpapers
         WHERE wallpaper_id = ?1
         ORDER BY collection_id ASC",
    )?;
    let rows = statement.query_map([wallpaper_id], |row| row.get::<_, i64>(0))?;
    let ids = rows.collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(ids)
}

pub(crate) fn replace_tag_relations(
    transaction: &Transaction<'_>,
    wallpaper_id: i64,
    tag_ids: &[i64],
) -> Result<()> {
    transaction.execute(
        "DELETE FROM wallpaper_tags WHERE wallpaper_id = ?1",
        [wallpaper_id],
    )?;
    for tag_id in tag_ids {
        transaction.execute(
            "INSERT INTO wallpaper_tags (wallpaper_id, tag_id) VALUES (?1, ?2)",
            params![wallpaper_id, tag_id],
        )?;
    }
    Ok(())
}

pub(crate) fn replace_collection_relations(
    transaction: &Transaction<'_>,
    wallpaper_id: i64,
    collection_ids: &[i64],
) -> Result<()> {
    transaction.execute(
        "DELETE FROM collection_wallpapers WHERE wallpaper_id = ?1",
        [wallpaper_id],
    )?;
    for collection_id in collection_ids {
        transaction.execute(
            "INSERT INTO collection_wallpapers (wallpaper_id, collection_id) VALUES (?1, ?2)",
            params![wallpaper_id, collection_id],
        )?;
    }
    Ok(())
}

pub(crate) fn parse_backup_bool(value: Option<String>) -> Option<bool> {
    match value.as_deref().map(str::trim) {
        Some("true" | "1") => Some(true),
        Some("false" | "0") => Some(false),
        _ => None,
    }
}
