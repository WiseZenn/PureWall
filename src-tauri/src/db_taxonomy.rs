use anyhow::Result;
use rusqlite::params;

use super::{CollectionEntry, Database, TagEntry};

impl Database {
    pub fn get_tags(&self) -> Result<Vec<TagEntry>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, color FROM tags ORDER BY name ASC")?;
        let tags = stmt
            .query_map([], |row| {
                Ok(TagEntry {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(tags)
    }

    pub fn tag_exists(&self, tag_id: i64) -> Result<bool> {
        self.conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM tags WHERE id = ?1)",
                [tag_id],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn create_tag(&self, name: &str, color: &str) -> Result<TagEntry> {
        self.conn.execute(
            "INSERT OR IGNORE INTO tags (name, color) VALUES (?1, ?2)",
            params![name, color],
        )?;

        self.conn
            .query_row(
                "SELECT id, name, color FROM tags WHERE name = ?1",
                [name],
                |row| {
                    Ok(TagEntry {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        color: row.get(2)?,
                    })
                },
            )
            .map_err(Into::into)
    }

    pub fn delete_tag(&self, tag_id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM wallpaper_tags WHERE tag_id = ?1", [tag_id])?;
        self.conn
            .execute("DELETE FROM tags WHERE id = ?1", [tag_id])?;
        Ok(())
    }

    pub fn get_collections(&self) -> Result<Vec<CollectionEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT collections.id, collections.name, collections.color, COUNT(wallpapers.id)
             FROM collections
             LEFT JOIN collection_wallpapers ON collection_wallpapers.collection_id = collections.id
             LEFT JOIN wallpapers ON wallpapers.id = collection_wallpapers.wallpaper_id
                 AND wallpapers.file_available = 1
             GROUP BY collections.id, collections.name, collections.color
             ORDER BY collections.name ASC",
        )?;
        let collections = stmt
            .query_map([], |row| {
                Ok(CollectionEntry {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                    wallpaper_count: row.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(collections)
    }

    pub fn collection_exists(&self, collection_id: i64) -> Result<bool> {
        self.conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM collections WHERE id = ?1)",
                [collection_id],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn create_collection(&self, name: &str, color: &str) -> Result<CollectionEntry> {
        self.conn.execute(
            "INSERT OR IGNORE INTO collections (name, color) VALUES (?1, ?2)",
            params![name, color],
        )?;

        self.conn
            .query_row(
                "SELECT collections.id, collections.name, collections.color, COUNT(wallpapers.id)
                 FROM collections
                 LEFT JOIN collection_wallpapers ON collection_wallpapers.collection_id = collections.id
                 LEFT JOIN wallpapers ON wallpapers.id = collection_wallpapers.wallpaper_id
                     AND wallpapers.file_available = 1
                 WHERE collections.name = ?1
                 GROUP BY collections.id, collections.name, collections.color",
                [name],
                |row| {
                    Ok(CollectionEntry {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        color: row.get(2)?,
                        wallpaper_count: row.get(3)?,
                    })
                },
            )
            .map_err(Into::into)
    }

    pub fn delete_collection(&self, collection_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM collection_wallpapers WHERE collection_id = ?1",
            [collection_id],
        )?;
        self.conn
            .execute("DELETE FROM collections WHERE id = ?1", [collection_id])?;
        Ok(())
    }

    pub fn assign_collection(&self, path: &str, collection_id: i64) -> Result<()> {
        let wallpaper_id = self.wallpaper_id(path)?;
        self.conn.execute(
            "INSERT OR IGNORE INTO collection_wallpapers (collection_id, wallpaper_id) VALUES (?1, ?2)",
            params![collection_id, wallpaper_id],
        )?;
        Ok(())
    }

    pub fn unassign_collection(&self, path: &str, collection_id: i64) -> Result<()> {
        let wallpaper_id = self.wallpaper_id(path)?;
        self.conn.execute(
            "DELETE FROM collection_wallpapers WHERE collection_id = ?1 AND wallpaper_id = ?2",
            params![collection_id, wallpaper_id],
        )?;
        Ok(())
    }

    pub fn assign_tag(&self, path: &str, tag_id: i64) -> Result<()> {
        let wallpaper_id = self.wallpaper_id(path)?;
        self.conn.execute(
            "INSERT OR IGNORE INTO wallpaper_tags (wallpaper_id, tag_id) VALUES (?1, ?2)",
            params![wallpaper_id, tag_id],
        )?;
        Ok(())
    }

    pub fn unassign_tag(&self, path: &str, tag_id: i64) -> Result<()> {
        let wallpaper_id = self.wallpaper_id(path)?;
        self.conn.execute(
            "DELETE FROM wallpaper_tags WHERE wallpaper_id = ?1 AND tag_id = ?2",
            params![wallpaper_id, tag_id],
        )?;
        Ok(())
    }

    pub(crate) fn wallpaper_id(&self, path: &str) -> Result<i64> {
        self.conn
            .query_row("SELECT id FROM wallpapers WHERE path = ?1", [path], |row| {
                row.get(0)
            })
            .map_err(Into::into)
    }

    pub fn set_rating(&self, path: &str, rating: i32) -> Result<()> {
        self.conn.execute(
            "UPDATE wallpapers SET rating = ?1 WHERE path = ?2",
            params![rating, path],
        )?;
        Ok(())
    }

    pub fn set_blacklisted(&self, path: &str, blacklisted: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE wallpapers SET blacklisted = ?1 WHERE path = ?2",
            params![if blacklisted { 1 } else { 0 }, path],
        )?;
        Ok(())
    }

    pub fn set_wallpaper_display_title(&self, path: &str, display_title: &str) -> Result<()> {
        let display_title = display_title.trim();
        self.conn.execute(
            "UPDATE wallpapers SET display_title = ?1 WHERE path = ?2",
            params![display_title, path],
        )?;
        Ok(())
    }

    pub fn batch_set_rating(&self, paths: &[String], rating: i32) -> Result<usize> {
        self.conn.execute_batch("BEGIN")?;
        let result: Result<usize> = (|| {
            let mut affected = 0;
            for path in paths {
                affected += self.conn.execute(
                    "UPDATE wallpapers SET rating = ?1 WHERE path = ?2",
                    params![rating, path],
                )?;
            }
            Ok(affected)
        })();
        match result {
            Ok(affected) => {
                self.conn.execute_batch("COMMIT")?;
                Ok(affected)
            }
            Err(e) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }

    pub fn batch_assign_tag(&self, paths: &[String], tag_id: i64) -> Result<usize> {
        self.conn.execute_batch("BEGIN")?;
        let result: Result<usize> = (|| {
            let mut affected = 0;
            for path in paths {
                let wallpaper_id: i64 = match self.conn.query_row(
                    "SELECT id FROM wallpapers WHERE path = ?1",
                    [path],
                    |row| row.get(0),
                ) {
                    Ok(id) => id,
                    Err(_) => continue,
                };
                affected += self.conn.execute(
                    "INSERT OR IGNORE INTO wallpaper_tags (wallpaper_id, tag_id) VALUES (?1, ?2)",
                    params![wallpaper_id, tag_id],
                )?;
            }
            Ok(affected)
        })();
        match result {
            Ok(affected) => {
                self.conn.execute_batch("COMMIT")?;
                Ok(affected)
            }
            Err(e) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }

    pub fn batch_unassign_tag(&self, paths: &[String], tag_id: i64) -> Result<usize> {
        self.conn.execute_batch("BEGIN")?;
        let result: Result<usize> = (|| {
            let mut affected = 0;
            for path in paths {
                let wallpaper_id = self.wallpaper_id(path)?;
                affected += self.conn.execute(
                    "DELETE FROM wallpaper_tags WHERE wallpaper_id = ?1 AND tag_id = ?2",
                    params![wallpaper_id, tag_id],
                )?;
            }
            Ok(affected)
        })();
        match result {
            Ok(affected) => {
                self.conn.execute_batch("COMMIT")?;
                Ok(affected)
            }
            Err(error) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }

    pub fn batch_assign_collection(&self, paths: &[String], collection_id: i64) -> Result<usize> {
        self.conn.execute_batch("BEGIN")?;
        let result: Result<usize> = (|| {
            let mut affected = 0;
            for path in paths {
                let wallpaper_id: i64 = match self.conn.query_row(
                    "SELECT id FROM wallpapers WHERE path = ?1",
                    [path],
                    |row| row.get(0),
                ) {
                    Ok(id) => id,
                    Err(_) => continue,
                };
                affected += self.conn.execute(
                    "INSERT OR IGNORE INTO collection_wallpapers (collection_id, wallpaper_id) VALUES (?1, ?2)",
                    params![collection_id, wallpaper_id],
                )?;
            }
            Ok(affected)
        })();
        match result {
            Ok(affected) => {
                self.conn.execute_batch("COMMIT")?;
                Ok(affected)
            }
            Err(e) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }

    pub fn batch_unassign_collection(&self, paths: &[String], collection_id: i64) -> Result<usize> {
        self.conn.execute_batch("BEGIN")?;
        let result: Result<usize> = (|| {
            let mut affected = 0;
            for path in paths {
                let wallpaper_id = self.wallpaper_id(path)?;
                affected += self.conn.execute(
                    "DELETE FROM collection_wallpapers WHERE collection_id = ?1 AND wallpaper_id = ?2",
                    params![collection_id, wallpaper_id],
                )?;
            }
            Ok(affected)
        })();
        match result {
            Ok(affected) => {
                self.conn.execute_batch("COMMIT")?;
                Ok(affected)
            }
            Err(error) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }

    pub fn batch_blacklist(&self, paths: &[String], blacklisted: bool) -> Result<usize> {
        self.conn.execute_batch("BEGIN")?;
        let value = if blacklisted { 1 } else { 0 };
        let result: Result<usize> = (|| {
            let mut affected = 0;
            for path in paths {
                affected += self.conn.execute(
                    "UPDATE wallpapers SET blacklisted = ?1 WHERE path = ?2",
                    params![value, path],
                )?;
            }
            Ok(affected)
        })();
        match result {
            Ok(affected) => {
                self.conn.execute_batch("COMMIT")?;
                Ok(affected)
            }
            Err(e) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }
}
