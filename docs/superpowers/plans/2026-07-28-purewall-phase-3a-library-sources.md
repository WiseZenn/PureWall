# PureWall Phase 3A Library Sources Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a reliable `Settings → Library Sources` lifecycle for listing, rescanning, retrying, relocating, and removing local wallpaper sources without deleting original files or losing recoverable metadata.

**Architecture:** Keep Tauri commands in `main.rs`, move shared Windows path identity into `paths.rs`, keep transactional source/wallpaper mutations in `db.rs`, and add a focused `library_sources.rs` boundary for DTOs, status derivation, and source-operation helpers. Directory scanning remains outside SQLite locks; database commits are atomic; runtime watcher removal/replacement happens only after the SQLite guard has been released and reports post-commit failures as retryable source state.

**Tech Stack:** Tauri 2, Rust, serde, rusqlite, notify, Vue 3, Pinia, TypeScript, Vitest

## Global Constraints

- PureWall only; PureWall-X is explicitly deferred.
- Windows local folders only; UNC, extended UNC, and mapped network drives remain rejected.
- Removing, rescanning, retrying, or relocating a source must never delete, move, copy, or recycle original wallpaper files.
- Remove offers exactly `keep_metadata` and `clear_metadata`; clear deletes only metadata exclusive to the removed source.
- A wallpaper covered by any remaining source is protected from removal and remains available.
- Relocate matches metadata by relative path, preserves wallpaper IDs, marks unmatched old rows unavailable, imports new target files, and aborts on source overlap or wallpaper-path collision.
- Existing paths are canonicalized; Windows identity comparison normalizes separators, case, trailing separators, `\\?\`, and `\\?\UNC\`.
- Directory scans and filesystem checks occur outside the SQLite mutex.
- A watcher handle whose `Drop` joins its callback thread is never removed or replaced while holding the SQLite mutex.
- SQLite mutation success is not rolled back when post-commit watcher restoration fails; the source records a retryable error.
- No new dependency, registry command, PureWall registry mutation, HKLM/policy/system-setting change, context-menu change, or autostart change.
- Every production behavior follows RED → GREEN → REFACTOR and every completed code task updates `docs/project-docs/CHANGELOG_AI.md`.
- `docs/project-docs/AI_DIARY.md` remains append-only and changes only when execution discovers a genuinely new pitfall.
- Stage only the exact task delta from the isolated `codex/purewall-phase-3` worktree.

---

## File Map

- Modify `src-tauri/src/paths.rs`: shared canonical Windows identity, descendant, overlap, and relative-identity helpers.
- Modify `src-tauri/src/db.rs`: schema version 7, source scan metadata, source summaries, overlap-safe removal, and atomic relocation.
- Create `src-tauri/src/library_sources.rs`: serializable source DTOs, stable command vocabulary, status derivation, and operation guard.
- Create `src-tauri/src/library_source_tests.rs`: application-boundary and watcher-registry regressions without real desktop mutation.
- Modify `src-tauri/src/main.rs`: source commands, scan/watcher coordination, registry updates, command registration, and the GitHub Windows fixture correction.
- Modify `src/stores/wallpapers.ts`: source state, per-row operation state, command methods, event refresh, and typed public interfaces.
- Create `src/stores/librarySources.test.ts`: store command-routing, success refresh, and failure-retention tests.
- Create `src/components/librarySourceModel.ts`: pure UI labels and safe remove-dialog copy.
- Create `src/components/librarySourceModel.test.ts`: status/action/copy contract tests.
- Create `src/components/LibrarySourcesSettings.vue`: Settings source rows, directory chooser, remove confirmation, retry, rescan, and relocate controls.
- Modify `src/components/InspectorPanel.vue`: mount the source settings card under Settings.
- Modify `docs/project-docs/DECISIONS.md`: accepted ADR-029 before production implementation.
- Modify `docs/project-docs/ARCHITECTURE.md`: source lifecycle and watcher/database commit boundary.
- Modify `docs/project-docs/CHANGELOG_AI.md`: plan and per-task implementation evidence.
- Modify `docs/project-docs/AI_DIARY.md`: append only if a new execution pitfall is discovered.

---

### Task 1: Correct the GitHub Windows canonical-root fixture

**Files:**
- Modify: `src-tauri/src/main.rs:2777-2825`
- Modify: `docs/project-docs/CHANGELOG_AI.md`

**Interfaces:**
- Preserves: production `path_is_same_or_descendant` behavior.
- Produces: a runner-independent regression whose removed root and stored child originate from the same canonical identity.
- Consumes: existing `reconcile_watched_paths` and `db::mark_paths_unavailable`.

- [ ] **Step 1: Change the regression so the removed root is canonical before deletion**

In `watched_directory_removal_hides_descendants_without_deleting_metadata`, canonicalize the root while it exists and use that canonical `PathBuf` for the removal event:

```rust
let watched_root = watched_dir
    .canonicalize()
    .expect("watched root should canonicalize");
let image_path = watched_root.join("removed-wallpaper.jpg");
image::RgbImage::new(12, 8)
    .save(&image_path)
    .expect("test wallpaper should be saved");
let stored_path = image_path
    .canonicalize()
    .expect("test wallpaper path should canonicalize")
    .to_string_lossy()
    .to_string();
```

After removing `watched_dir`, pass the saved canonical root rather than the raw temp alias:

```rust
std::fs::remove_dir_all(&watched_dir).expect("watched directory should be removed");
let result = reconcile_watched_paths(std::slice::from_ref(&watched_root), |inspection| {
    db.mark_paths_unavailable(&inspection.missing_paths)
        .map_err(CommandError::from_display)?;
    import_images_into_database(&db, &inspection.images, "mounted")
})
.expect("removed directory reconciliation should succeed");
```

- [ ] **Step 2: Run the focused regression**

Run:

```text
cargo test --manifest-path src-tauri/Cargo.toml watched_directory_removal_hides_descendants_without_deleting_metadata -- --nocapture
```

Expected: 1 passed, 0 failed. The test must still prove page total becomes `0` while `get_wallpaper_by_path` retains metadata.

- [ ] **Step 3: Run the Rust baseline**

Run:

```text
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: formatting PASS; all non-ignored Rust tests PASS.

- [ ] **Step 4: Record the task delta**

Append this subsection to the current Phase 3A changelog entry:

```markdown
### 3A Task 1 — canonical watcher fixture

- Canonicalized the watched root before creating/removing the fixture so the root and stored child share production path provenance.
- The focused removal regression and complete Rust suite pass locally.
- GitHub Windows Actions remains the cross-runner verification source and is not represented as green until the branch is pushed.
```

- [ ] **Step 5: Commit only the fixture and process record**

```text
git add src-tauri/src/main.rs docs/project-docs/CHANGELOG_AI.md
git -c commit.gpgsign=false commit -m "test(watcher): canonicalize removal fixture root"
```

---

### Task 2: Accept the source lifecycle commit-boundary ADR

**Files:**
- Modify: `docs/project-docs/DECISIONS.md`
- Modify: `docs/project-docs/CHANGELOG_AI.md`

**Interfaces:**
- Produces: accepted ADR-029, which gates every later production-code task.
- Consumes: accepted ADR-024, ADR-025, ADR-026, and the user-approved Phase 3 design.
- Preserves: watcher-first import coverage and shutdown rules from ADR-026.

- [ ] **Step 1: Append accepted ADR-029**

Append this exact decision:

```markdown
## ADR-029: Library-source metadata commits precede runtime watcher reconciliation

- **Status**: accepted
- **Decision**:
  1. Source scans and target-folder validation complete outside the SQLite mutex.
  2. Remove commits the source/metadata transaction first, releases SQLite, then takes the watcher from the runtime registry and drops it after the registry lock is released.
  3. Relocate scans the target, commits the source root plus wallpaper path/availability changes atomically, releases SQLite, then removes the old watcher and starts the target watcher. A successful watcher start is followed by an explicit bounded target rescan so changes during the handoff gap are reconciled.
  4. A post-commit watcher failure records `last_error`, leaves the committed source metadata in place, and returns a warning/status that Retry can recover. It is not reported as a rolled-back database mutation.
  5. Add/Retry persists the canonical source before watcher startup. Watcher failure keeps the persisted source offline/error; successful startup precedes the full bounded scan.
- **Context**: SQLite, filesystem scans, and notify watcher handles cannot participate in one transaction. `FolderWatcher::drop()` joins a callback thread that may need SQLite, so replacing or dropping it under the database mutex can deadlock. Pretending a post-commit watcher failure rolled back metadata would also invite unsafe retries.
- **Alternatives**:
  - A. Hold SQLite while replacing watchers — rejected because watcher `Drop` can join a callback waiting for SQLite.
  - B. Start an unregistered target watcher before relocation commits — rejected because callbacks can import against a source root the database does not yet own.
  - C. Roll back SQLite after watcher failure — rejected because runtime resource creation is not transactionally reversible with committed SQLite state.
- **Consequences**: Source commands expose database success separately from watcher warnings. Runtime registry helpers take/drop handles in lock-free phases. Relocation needs an explicit post-handoff rescan. Tests must cover overlap protection, collision rollback, metadata preservation, and watcher-failure status without real desktop or registry mutation.
```

- [ ] **Step 2: Verify the decision gate**

Run:

```text
Select-String -LiteralPath docs/project-docs/DECISIONS.md -Pattern "ADR-029|Status.*accepted|SQLite mutex|post-commit watcher"
```

Expected: ADR-029 and `accepted` appear; no `proposed` line exists.

- [ ] **Step 3: Record and commit the ADR**

Append:

```markdown
### 3A Task 2 — source lifecycle ADR

- Accepted ADR-029 before production changes.
- SQLite commits, filesystem scans, and watcher handoff now have explicit non-transactional boundaries.
```

Then commit:

```text
git add docs/project-docs/DECISIONS.md docs/project-docs/CHANGELOG_AI.md
git -c commit.gpgsign=false commit -m "docs(adr): define library source lifecycle boundaries"
```

---

### Task 3: Add shared path identity, source scan state, summaries, and overlap-safe removal

**Files:**
- Modify: `src-tauri/src/paths.rs`
- Modify: `src-tauri/src/db.rs`
- Modify: `docs/project-docs/CHANGELOG_AI.md`

**Interfaces:**
- Produces: `paths::path_identity_key(&Path) -> String`.
- Produces: `paths::path_is_same_or_descendant(candidate, root) -> bool`.
- Produces: `paths::paths_overlap(left, right) -> bool`.
- Produces: `paths::relative_identity(candidate, root) -> Option<String>`.
- Produces: `WatchedFolderEntry { path, source, last_scan_at, last_error }`.
- Produces: `WatchedFolderSummary { entry, available_count, unavailable_count }`.
- Produces: `SourceRemovalMode::{KeepMetadata, ClearMetadata}`.
- Produces: `SourceRemovalSummary { affected_wallpapers }`.
- Produces: database methods `get_watched_folder_summaries`, `source_removal_impact`, `remove_watched_folder`, `record_watched_folder_scan_success`, and `record_watched_folder_error`.
- Consumes: existing schema version 6 and `wallpapers.file_available`.

- [ ] **Step 1: Write RED path-helper tests**

Add tests in `paths.rs`:

```rust
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
            relative_identity(
                Path::new(r"D:\Walls-B\image.jpg"),
                Path::new(r"D:\Walls-A"),
            ),
            None,
        );
    }
}
```

Move the existing Windows/non-Windows identity and descendant helpers from `db.rs` into `paths.rs`, make them `pub(crate)`, and add:

```rust
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
```

Update `db.rs` imports to consume these helpers instead of keeping a second identity implementation.

- [ ] **Step 2: Run path-helper RED/GREEN**

Run before implementation:

```text
cargo test --manifest-path src-tauri/Cargo.toml paths::tests -- --nocapture
```

Expected RED: missing `paths_overlap` and `relative_identity`.

Run after implementation:

```text
cargo test --manifest-path src-tauri/Cargo.toml paths::tests -- --nocapture
```

Expected GREEN: both path tests pass and the existing watched-removal regression remains green.

- [ ] **Step 3: Write RED database tests for migration, summaries, and removal**

Add focused `db.rs` tests with canonical temp roots:

```rust
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
    let db = Database::new(&db_path).expect("database should initialize");
    let parent = r"D:\PureWall-Test\Walls";
    let child = r"D:\PureWall-Test\Walls\Keep";
    let exclusive = r"D:\PureWall-Test\Walls\exclusive.jpg";
    let covered = r"D:\PureWall-Test\Walls\Keep\covered.jpg";

    db.upsert_watched_folder(parent, "mounted").unwrap();
    db.upsert_watched_folder(child, "imported-folder").unwrap();
    db.upsert_wallpaper(exclusive, "exclusive", "mounted", 10, 10, 1)
        .unwrap();
    db.upsert_wallpaper(covered, "covered", "mounted", 10, 10, 1)
        .unwrap();

    assert_eq!(db.source_removal_impact(parent).unwrap(), 1);
    let summary = db
        .remove_watched_folder(parent, SourceRemovalMode::KeepMetadata)
        .unwrap();
    assert_eq!(summary.affected_wallpapers, 1);
    assert!(db.get_wallpaper_by_path(exclusive).unwrap().is_some());
    assert_eq!(db.get_stats().unwrap().total, 1);
    assert_eq!(
        db.get_wallpapers_page("all", "created", "", 0, 10)
            .unwrap()
            .items[0]
            .path,
        covered,
    );

    drop(db);
    remove_sqlite_files(&db_path);
}

#[test]
fn clear_source_deletes_only_exclusive_metadata() {
    let db_path = unique_temp_db_path("source-clear-remove");
    remove_sqlite_files(&db_path);
    let db = Database::new(&db_path).expect("database should initialize");
    let parent = r"D:\PureWall-Test\Walls";
    let child = r"D:\PureWall-Test\Walls\Keep";
    let exclusive = r"D:\PureWall-Test\Walls\exclusive.jpg";
    let covered = r"D:\PureWall-Test\Walls\Keep\covered.jpg";

    db.upsert_watched_folder(parent, "mounted").unwrap();
    db.upsert_watched_folder(child, "imported-folder").unwrap();
    db.upsert_wallpaper(exclusive, "exclusive", "mounted", 10, 10, 1)
        .unwrap();
    db.upsert_wallpaper(covered, "covered", "mounted", 10, 10, 1)
        .unwrap();

    db.remove_watched_folder(parent, SourceRemovalMode::ClearMetadata)
        .unwrap();
    assert!(db.get_wallpaper_by_path(exclusive).unwrap().is_none());
    assert!(db.get_wallpaper_by_path(covered).unwrap().is_some());

    drop(db);
    remove_sqlite_files(&db_path);
}
```

- [ ] **Step 4: Run database RED**

Run:

```text
cargo test --manifest-path src-tauri/Cargo.toml schema_seven_migrates_existing_watched_folder_scan_columns -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml watched_folder_scan_state_and_counts_round_trip -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml removing_parent_source_protects_wallpapers_covered_by_child_source -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml clear_source_deletes_only_exclusive_metadata -- --nocapture
```

Expected RED: the scan-state columns, summary types, and source removal methods do not exist.

- [ ] **Step 5: Implement schema version 7 and source records**

Set:

```rust
const SCHEMA_VERSION: i64 = 7;
```

Add columns to the create-table definition:

```sql
last_scan_at TEXT,
last_error TEXT,
```

Add migration before indexes:

```rust
for (column, sql_type) in [
    ("last_scan_at", "TEXT"),
    ("last_error", "TEXT"),
] {
    if !Self::column_exists(conn, "watched_folders", column)? {
        conn.execute(
            &format!("ALTER TABLE watched_folders ADD COLUMN {column} {sql_type}"),
            [],
        )?;
    }
}
```

Extend the source record types:

```rust
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
```

Update `get_watched_folders` to select `last_scan_at, last_error`.

- [ ] **Step 6: Implement summary, scan-state, impact, and removal transactions**

Use these exact public signatures:

```rust
pub fn get_watched_folder_summaries(&self) -> Result<Vec<WatchedFolderSummary>>;
pub fn record_watched_folder_scan_success(&self, path: &str) -> Result<()>;
pub fn record_watched_folder_error(&self, path: &str, message: &str) -> Result<()>;
pub fn source_removal_impact(&self, path: &str) -> Result<usize>;
pub fn remove_watched_folder(
    &self,
    path: &str,
    mode: SourceRemovalMode,
) -> Result<SourceRemovalSummary>;
```

`record_watched_folder_scan_success` executes:

```sql
UPDATE watched_folders
SET last_scan_at = datetime('now'), last_error = NULL
WHERE path = ?1
```

`record_watched_folder_error` executes:

```sql
UPDATE watched_folders SET last_error = ?2 WHERE path = ?1
```

For both impact and removal:

1. Load all source roots and reject an unknown exact root.
2. Load wallpaper `(id, path, file_available)` rows once.
3. Select rows under the removed root.
4. Exclude any row under a different remaining root.
5. In one `BEGIN`/`COMMIT`, delete the source and either update selected IDs to `file_available = 0` or delete the selected wallpaper rows.
6. On any error, execute `ROLLBACK`.

Do not touch the filesystem or call `trash`.

- [ ] **Step 7: Run GREEN and the existing database suite**

Run:

```text
cargo test --manifest-path src-tauri/Cargo.toml db::tests -- --nocapture
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: all database tests pass; formatting and check pass.

- [ ] **Step 8: Document and commit**

Append:

```markdown
### 3A Task 3 — source database lifecycle

- Centralized Windows source-path identity in `paths.rs`.
- Migrated SQLite to schema 7 with `last_scan_at` and `last_error`.
- Added source summaries and overlap-safe keep/clear removal transactions; neither path touches original files.
```

Commit:

```text
git add src-tauri/src/paths.rs src-tauri/src/db.rs docs/project-docs/CHANGELOG_AI.md
git -c commit.gpgsign=false commit -m "feat(library): add source removal lifecycle"
```

---

### Task 4: Add atomic relative-path relocation

**Files:**
- Modify: `src-tauri/src/db.rs`
- Modify: `docs/project-docs/CHANGELOG_AI.md`

**Interfaces:**
- Produces: `ScannedWallpaperRecord`.
- Produces: `SourceRelocationSummary { matched, imported, unavailable }`.
- Produces: `relocate_watched_folder(old_root, new_root, source, scanned)`.
- Consumes: Task 3 path identity helpers and schema 7 source state.
- Preserves: wallpaper IDs, ratings, custom titles, tags, collections, and play history.

- [ ] **Step 1: Write RED relocation tests**

Add:

```rust
#[test]
fn relocate_source_preserves_ids_and_metadata_by_relative_path() {
    let db_path = unique_temp_db_path("source-relocate");
    remove_sqlite_files(&db_path);
    let db = Database::new(&db_path).unwrap();
    let old_root = r"D:\PureWall-Test\Old";
    let new_root = r"E:\PureWall-Test\New";
    let old_path = r"D:\PureWall-Test\Old\Nature\lake.jpg";
    let new_path = r"E:\PureWall-Test\New\Nature\lake.jpg";

    db.upsert_watched_folder(old_root, "mounted").unwrap();
    db.upsert_wallpaper(old_path, "old", "mounted", 10, 10, 1)
        .unwrap();
    db.set_rating(old_path, 1).unwrap();
    let before = db.get_wallpaper_by_path(old_path).unwrap().unwrap();

    let result = db
        .relocate_watched_folder(
            old_root,
            new_root,
            "mounted",
            &[ScannedWallpaperRecord {
                path: new_path.to_string(),
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
    let after = db.get_wallpaper_by_path(new_path).unwrap().unwrap();
    assert_eq!(after.id, before.id);
    assert_eq!(after.rating, 1);
    assert_eq!(after.width, 1920);
    assert!(db.get_wallpaper_by_path(old_path).unwrap().is_none());
    assert_eq!(db.get_watched_folders().unwrap()[0].path, new_root);

    drop(db);
    remove_sqlite_files(&db_path);
}

#[test]
fn relocate_source_keeps_unmatched_metadata_unavailable_and_imports_new_files() {
    let db_path = unique_temp_db_path("source-relocate-partial");
    remove_sqlite_files(&db_path);
    let db = Database::new(&db_path).unwrap();
    let old_root = r"D:\PureWall-Test\Old";
    let new_root = r"E:\PureWall-Test\New";
    let missing_old = r"D:\PureWall-Test\Old\missing.jpg";
    let added_new = r"E:\PureWall-Test\New\added.jpg";

    db.upsert_watched_folder(old_root, "mounted").unwrap();
    db.upsert_wallpaper(missing_old, "missing", "mounted", 10, 10, 1)
        .unwrap();

    let result = db
        .relocate_watched_folder(
            old_root,
            new_root,
            "mounted",
            &[ScannedWallpaperRecord {
                path: added_new.to_string(),
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
    assert!(db.get_wallpaper_by_path(missing_old).unwrap().is_some());
    assert_eq!(db.get_stats().unwrap().total, 1);
    assert_eq!(
        db.get_wallpapers_page("all", "created", "", 0, 10)
            .unwrap()
            .items[0]
            .path,
        added_new,
    );

    drop(db);
    remove_sqlite_files(&db_path);
}

#[test]
fn relocate_collision_rolls_back_source_and_wallpaper_paths() {
    let db_path = unique_temp_db_path("source-relocate-collision");
    remove_sqlite_files(&db_path);
    let db = Database::new(&db_path).unwrap();
    let old_root = r"D:\PureWall-Test\Old";
    let new_root = r"E:\PureWall-Test\New";
    let old_path = r"D:\PureWall-Test\Old\same.jpg";
    let occupied = r"E:\PureWall-Test\New\same.jpg";

    db.upsert_watched_folder(old_root, "mounted").unwrap();
    db.upsert_wallpaper(old_path, "old", "mounted", 10, 10, 1)
        .unwrap();
    db.upsert_wallpaper(occupied, "occupied", "mounted", 10, 10, 1)
        .unwrap();

    let error = db
        .relocate_watched_folder(
            old_root,
            new_root,
            "mounted",
            &[ScannedWallpaperRecord {
                path: occupied.to_string(),
                hash: "new".to_string(),
                width: 20,
                height: 10,
                file_size: 2,
            }],
        )
        .expect_err("collision must abort");
    assert!(error.to_string().contains("relocation collision"));
    assert_eq!(db.get_watched_folders().unwrap()[0].path, old_root);
    assert!(db.get_wallpaper_by_path(old_path).unwrap().is_some());
    assert!(db.get_wallpaper_by_path(occupied).unwrap().is_some());

    drop(db);
    remove_sqlite_files(&db_path);
}
```

- [ ] **Step 2: Run relocation RED**

Run each test by name. Expected RED: `ScannedWallpaperRecord`, `SourceRelocationSummary`, and `relocate_watched_folder` do not exist.

- [ ] **Step 3: Add the relocation records and method**

Add:

```rust
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

pub fn relocate_watched_folder(
    &self,
    old_root: &str,
    new_root: &str,
    source: &str,
    scanned: &[ScannedWallpaperRecord],
) -> Result<SourceRelocationSummary>;
```

The implementation must execute this exact sequence:

1. Load all source roots and reject any remaining root for which `paths_overlap(new_root, remaining_root)` is true.
2. Load every wallpaper `(id, path)` and map old descendants by `relative_identity(old_path, old_root)`.
3. Map scanned target rows by `relative_identity(new_path, new_root)` and reject duplicates.
4. Before `BEGIN`, detect any scanned identity already owned by a wallpaper outside the old-source match; return an error containing `relocation collision`.
5. Inside one transaction, update the source root, update matched wallpaper rows in place, mark unmatched old IDs unavailable, and insert unmatched scanned rows.
6. Set `last_scan_at = datetime('now')` and `last_error = NULL` on the relocated source.
7. Commit and return counts; roll back every database mutation on error.

Use parameterized SQL only. Do not call scanner or filesystem APIs from `db.rs`.

- [ ] **Step 4: Run relocation GREEN and complete Rust checks**

Run:

```text
cargo test --manifest-path src-tauri/Cargo.toml relocate_source -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml relocate_collision_rolls_back_source_and_wallpaper_paths -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml db::tests -- --nocapture
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: all focused and database tests pass.

- [ ] **Step 5: Document and commit**

Append:

```markdown
### 3A Task 4 — atomic source relocation

- Added relative-path relocation that preserves wallpaper IDs and metadata.
- New target files import, unmatched old rows remain unavailable, and path/source collisions roll back the complete transaction.
```

Commit:

```text
git add src-tauri/src/db.rs docs/project-docs/CHANGELOG_AI.md
git -c commit.gpgsign=false commit -m "feat(library): relocate sources by relative path"
```

---

### Task 5: Expose source commands and coordinate runtime watchers

**Files:**
- Create: `src-tauri/src/library_sources.rs`
- Create: `src-tauri/src/library_source_tests.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `docs/project-docs/CHANGELOG_AI.md`

**Interfaces:**
- Produces Tauri commands:
  - `list_library_sources() -> CommandResult<Vec<LibrarySource>>`
  - `rescan_library_source(path) -> CommandResult<ImportResult>`
  - `retry_library_source(path) -> CommandResult<ImportResult>`
  - `preview_remove_library_source(path) -> CommandResult<RemoveLibrarySourceImpact>`
  - `remove_library_source(path, mode) -> CommandResult<LibrarySourceMutation>`
  - `relocate_library_source(path, new_path) -> CommandResult<LibrarySourceMutation>`
- Produces stable error codes `SOURCE_OFFLINE`, `SOURCE_PATH_CONFLICT`, `SOURCE_PERMISSION_DENIED`, `SOURCE_INVALID_PATH`, and `SOURCE_RELOCATE_COLLISION`.
- Produces source status `online | offline | scanning | error`.
- Consumes: Tasks 3–4 database methods, existing bounded scanner, `AppState.folder_watchers`, `folder-changed`, and `operation-failed`.

- [ ] **Step 1: Create DTOs and RED status tests**

Create `library_sources.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LibrarySourceStatus {
    Online,
    Offline,
    Scanning,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct LibrarySource {
    pub path: String,
    pub source: String,
    pub status: LibrarySourceStatus,
    pub available_count: usize,
    pub unavailable_count: usize,
    pub last_scan_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoveLibrarySourceMode {
    KeepMetadata,
    ClearMetadata,
}

impl From<RemoveLibrarySourceMode> for crate::db::SourceRemovalMode {
    fn from(value: RemoveLibrarySourceMode) -> Self {
        match value {
            RemoveLibrarySourceMode::KeepMetadata => Self::KeepMetadata,
            RemoveLibrarySourceMode::ClearMetadata => Self::ClearMetadata,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RemoveLibrarySourceImpact {
    pub affected_wallpapers: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct LibrarySourceMutation {
    pub affected_wallpapers: usize,
    pub matched_wallpapers: usize,
    pub imported_wallpapers: usize,
    pub unavailable_wallpapers: usize,
    pub watcher_warning: Option<String>,
}

pub fn status_for(
    path: &str,
    scanning_paths: &HashSet<String>,
    last_error: Option<&str>,
) -> LibrarySourceStatus {
    if scanning_paths.contains(path) {
        LibrarySourceStatus::Scanning
    } else if !Path::new(path).is_dir() {
        LibrarySourceStatus::Offline
    } else if last_error.is_some() {
        LibrarySourceStatus::Error
    } else {
        LibrarySourceStatus::Online
    }
}

pub struct SourceOperationGuard<'a> {
    path: String,
    operations: &'a Mutex<HashSet<String>>,
}

impl<'a> SourceOperationGuard<'a> {
    pub fn begin(
        operations: &'a Mutex<HashSet<String>>,
        path: &str,
    ) -> Result<Self, &'static str> {
        let mut active = operations.lock().map_err(|_| "source operations unavailable")?;
        if !active.insert(path.to_string()) {
            return Err("source operation already running");
        }
        Ok(Self {
            path: path.to_string(),
            operations,
        })
    }
}

impl Drop for SourceOperationGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut active) = self.operations.lock() {
            active.remove(&self.path);
        }
    }
}
```

Add tests proving scanning wins over error, missing wins over error, and `SourceOperationGuard` removes the path after success and unwind.

- [ ] **Step 2: Register the module and run RED**

Add:

```rust
mod library_sources;
#[cfg(test)]
mod library_source_tests;
```

Add `source_operations: Mutex<HashSet<String>>` to `AppState` and initialize it in setup.

Run:

```text
cargo test --manifest-path src-tauri/Cargo.toml library_sources -- --nocapture
```

Expected RED before the implementation is complete; GREEN after the DTO/status/guard code is registered.

- [ ] **Step 3: Add application-boundary regression tests**

Create `library_source_tests.rs` with tests for:

```rust
use crate::library_sources::{status_for, LibrarySourceStatus};
use std::collections::HashSet;

#[test]
fn source_status_prioritizes_scanning_then_offline_then_error() {
    let path = std::env::temp_dir()
        .canonicalize()
        .expect("temp root should canonicalize")
        .to_string_lossy()
        .to_string();
    let mut scanning = HashSet::new();
    scanning.insert(path.clone());
    assert_eq!(
        status_for(&path, &scanning, Some("old error")),
        LibrarySourceStatus::Scanning,
    );

    scanning.clear();
    assert_eq!(
        status_for(&path, &scanning, Some("old error")),
        LibrarySourceStatus::Error,
    );
    assert_eq!(
        status_for(
            r"Z:\purewall-source-that-does-not-exist",
            &scanning,
            Some("old error"),
        ),
        LibrarySourceStatus::Offline,
    );
}
```

Also test a generic watcher-map take helper:

```rust
#[test]
fn taking_a_watcher_removes_only_the_exact_canonical_root() {
    let mut watchers = std::collections::HashMap::from([
        ("D:\\Walls".to_string(), ("mounted".to_string(), 1)),
        (
            "D:\\Walls-Other".to_string(),
            ("imported-folder".to_string(), 2),
        ),
    ]);
    let removed = crate::take_runtime_folder_watcher(&mut watchers, "D:\\Walls");
    assert_eq!(removed, Some(("mounted".to_string(), 1)));
    assert_eq!(watchers.len(), 1);
    assert!(watchers.contains_key("D:\\Walls-Other"));
}
```

Implement:

```rust
pub(crate) fn take_runtime_folder_watcher<T>(
    watchers: &mut HashMap<String, (String, T)>,
    folder_path: &str,
) -> Option<(String, T)> {
    watchers.remove(folder_path)
}
```

- [ ] **Step 4: Implement list and source-operation commands**

Use `CommandError::new` with the exact uppercase codes.

`list_library_sources`:

1. Lock SQLite only long enough to call `get_watched_folder_summaries`.
2. Clone the active scan set under its own lock.
3. Release both locks.
4. Derive filesystem status and DTOs without a SQLite guard.

For rescan/retry:

1. Validate that the exact source exists in SQLite.
2. Start `SourceOperationGuard`.
3. Validate/canonicalize the persisted path.
4. Ensure watcher registration outside SQLite.
5. Run `scanner::scan_folder` outside SQLite.
6. Lock SQLite, reconcile/import, record success, release.
7. Emit `folder-changed`.
8. On error, record `last_error` in a separate short SQLite scope and return the stable source error.

Refactor the existing add/startup paths to the same contract:

- `persist_and_ensure_folder_watcher` persists the canonical source in a short SQLite scope, releases SQLite, then starts/inserts the watcher.
- `set_wallpaper_folder`, `import_wallpaper_folder`, and `synchronize_persisted_folder` call `record_watched_folder_scan_success` only after a complete bounded snapshot persists.
- Scan or watcher-start failures call `record_watched_folder_error` for the exact source path.
- `synchronize_watched_paths` receives the owning root path as well as the source label; an incremental synchronization error records `last_error` for that root before emitting `operation-failed`.

For remove:

```rust
#[tauri::command]
fn preview_remove_library_source(
    state: tauri::State<AppState>,
    path: String,
) -> CommandResult<library_sources::RemoveLibrarySourceImpact>;

#[tauri::command]
fn remove_library_source(
    state: tauri::State<AppState>,
    app: tauri::AppHandle,
    path: String,
    mode: library_sources::RemoveLibrarySourceMode,
) -> CommandResult<library_sources::LibrarySourceMutation>;
```

The remove command commits the database operation in one lexical block, then takes the watcher in a second block:

```rust
let summary = {
    let db = state.db.lock().map_err(|_| {
        CommandError::new("database_unavailable", "Could not access the wallpaper database.")
    })?;
    db.remove_watched_folder(&path, mode.into())
        .map_err(CommandError::from_display)?
};

let removed_watcher = {
    let mut watchers = state.folder_watchers.lock().map_err(|_| {
        CommandError::new(
            "watcher_registry_unavailable",
            "Could not access the folder watcher registry.",
        )
    })?;
    take_runtime_folder_watcher(&mut watchers, &path)
};
drop(removed_watcher);
```

No database guard or watcher-registry guard may exist at `drop`.

- [ ] **Step 5: Implement relocate coordination**

`relocate_library_source` performs:

1. Exact old-source lookup.
2. `validate_existing_folder(new_path)` and target scan outside SQLite.
3. Convert every `scanner::ImageInfo` to `db::ScannedWallpaperRecord`.
4. Call the atomic database relocation in one short lock scope.
5. Take/drop the old watcher outside SQLite and outside the watcher-registry lock.
6. Start/register the new watcher.
7. If watcher startup fails, persist the error on the new source and return `LibrarySourceMutation` with `watcher_warning`.
8. If watcher succeeds, call the same bounded rescan helper once more to close the handoff gap.
9. Emit `folder-changed`.

Map overlap to `SOURCE_PATH_CONFLICT`, invalid/offline target to `SOURCE_INVALID_PATH`/`SOURCE_OFFLINE`, permission failures to `SOURCE_PERMISSION_DENIED`, and database path collision to `SOURCE_RELOCATE_COLLISION`.

- [ ] **Step 6: Register all six commands**

Add to `tauri::generate_handler!`:

```rust
list_library_sources,
rescan_library_source,
retry_library_source,
preview_remove_library_source,
remove_library_source,
relocate_library_source,
```

- [ ] **Step 7: Run focused and complete backend verification**

Run:

```text
cargo test --manifest-path src-tauri/Cargo.toml library_source -- --nocapture
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: focused tests and complete Rust gate pass; no real watcher, registry, or wallpaper operation is performed by tests.

- [ ] **Step 8: Document and commit**

Append:

```markdown
### 3A Task 5 — Tauri source commands and watcher handoff

- Added list/rescan/retry/remove/relocate commands with stable source error codes.
- Scans occur outside SQLite; watcher handles are taken and dropped after database locks are released.
- Post-commit watcher failures persist retryable source errors instead of misreporting database rollback.
```

Commit:

```text
git add src-tauri/src/library_sources.rs src-tauri/src/library_source_tests.rs src-tauri/src/main.rs docs/project-docs/CHANGELOG_AI.md
git -c commit.gpgsign=false commit -m "feat(library): expose source lifecycle commands"
```

---

### Task 6: Add Pinia source state and command routing

**Files:**
- Modify: `src/stores/wallpapers.ts`
- Create: `src/stores/librarySources.test.ts`
- Modify: `docs/project-docs/CHANGELOG_AI.md`

**Interfaces:**
- Produces frontend types `LibrarySource`, `LibrarySourceStatus`, `RemoveLibrarySourceMode`, `RemoveLibrarySourceImpact`, and `LibrarySourceMutation`.
- Produces refs `librarySources`, `librarySourceBusy`, and `librarySourceErrors`.
- Produces methods `loadLibrarySources`, `rescanLibrarySource`, `retryLibrarySource`, `previewRemoveLibrarySource`, `removeLibrarySource`, and `relocateLibrarySource`.
- Consumes: Task 5 command names and snake-case payloads.
- Preserves: failed-operation selection/state; no global infinite loading state.

- [ ] **Step 1: Write RED store routing tests**

Create `librarySources.test.ts` with hoisted mocks for `invoke`, `listen`, and `getCurrentWebviewWindow`, then add:

```ts
it("loads and stores exact source DTOs", async () => {
  invokeMock.mockResolvedValueOnce([
    {
      path: "D:\\Walls",
      source: "mounted",
      status: "online",
      available_count: 12,
      unavailable_count: 2,
      last_scan_at: "2026-07-28 10:00:00",
      last_error: null,
    },
  ]);
  const store = useWallpaperStore();

  await store.loadLibrarySources();

  expect(invokeMock).toHaveBeenCalledWith("list_library_sources");
  expect(store.librarySources).toHaveLength(1);
  expect(store.librarySources[0].available_count).toBe(12);
});

it("routes remove preview and keep-metadata confirmation exactly", async () => {
  invokeMock.mockImplementation(async (command: string) => {
    if (command === "preview_remove_library_source") {
      return { affected_wallpapers: 4 };
    }
    if (command === "remove_library_source") {
      return {
        affected_wallpapers: 4,
        matched_wallpapers: 0,
        imported_wallpapers: 0,
        unavailable_wallpapers: 4,
        watcher_warning: null,
      };
    }
    if (command === "list_library_sources") return [];
    if (command === "get_wallpapers_page") {
      return { items: [], total: 0, offset: 0, limit: 96, has_more: false };
    }
    if (command === "get_stats") return emptyStats;
    return undefined;
  });
  const store = useWallpaperStore();

  await expect(store.previewRemoveLibrarySource("D:\\Walls")).resolves.toEqual({
    affected_wallpapers: 4,
  });
  await store.removeLibrarySource("D:\\Walls", "keep_metadata");

  expect(invokeMock).toHaveBeenCalledWith("remove_library_source", {
    path: "D:\\Walls",
    mode: "keep_metadata",
  });
  expect(store.librarySourceBusy.has("D:\\Walls")).toBe(false);
});

it("keeps the row error and clears busy state when relocate fails", async () => {
  invokeMock.mockRejectedValueOnce({
    code: "SOURCE_RELOCATE_COLLISION",
    message: "Target wallpaper path is already owned.",
  });
  const store = useWallpaperStore();

  await expect(
    store.relocateLibrarySource("D:\\Walls", "E:\\Walls"),
  ).rejects.toMatchObject({ code: "SOURCE_RELOCATE_COLLISION" });
  expect(store.librarySourceBusy.has("D:\\Walls")).toBe(false);
  expect(store.librarySourceErrors.get("D:\\Walls")).toContain(
    "Target wallpaper path is already owned.",
  );
});
```

Define `emptyStats` in the test and return empty values for incidental store commands.

- [ ] **Step 2: Run frontend RED**

Run:

```text
npx vitest run src/stores/librarySources.test.ts
```

Expected RED: source interfaces, refs, and methods do not exist.

- [ ] **Step 3: Add types and per-row operation state**

Add:

```ts
export type LibrarySourceStatus = "online" | "offline" | "scanning" | "error";
export type RemoveLibrarySourceMode = "keep_metadata" | "clear_metadata";
export type LibrarySourceOperation = "rescan" | "retry" | "relocate" | "remove";

export interface LibrarySource {
  path: string;
  source: "mounted" | "imported-folder";
  status: LibrarySourceStatus;
  available_count: number;
  unavailable_count: number;
  last_scan_at: string | null;
  last_error: string | null;
}

export interface RemoveLibrarySourceImpact {
  affected_wallpapers: number;
}

export interface LibrarySourceMutation {
  affected_wallpapers: number;
  matched_wallpapers: number;
  imported_wallpapers: number;
  unavailable_wallpapers: number;
  watcher_warning: string | null;
}
```

Inside the store:

```ts
const librarySources = ref<LibrarySource[]>([]);
const librarySourceBusy = reactive<Map<string, LibrarySourceOperation>>(new Map());
const librarySourceErrors = reactive<Map<string, string>>(new Map());
```

Add:

```ts
function commandMessage(error: unknown): string {
  if (error && typeof error === "object" && "message" in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string" && message.trim()) return message;
  }
  return error instanceof Error ? error.message : String(error);
}

async function runLibrarySourceOperation<T>(
  path: string,
  operation: LibrarySourceOperation,
  run: () => Promise<T>,
): Promise<T> {
  librarySourceBusy.set(path, operation);
  librarySourceErrors.delete(path);
  try {
    return await run();
  } catch (error) {
    librarySourceErrors.set(path, commandMessage(error));
    reportFailure(`Library source ${operation} failed`, error);
    throw error;
  } finally {
    librarySourceBusy.delete(path);
  }
}
```

- [ ] **Step 4: Add exact command methods and refresh rules**

Implement:

```ts
async function loadLibrarySources() {
  librarySources.value = await invoke<LibrarySource[]>("list_library_sources");
}

async function refreshAfterLibrarySourceMutation() {
  await Promise.all([loadLibrarySources(), loadWallpapers(), loadStats()]);
}

async function rescanLibrarySource(path: string) {
  return runLibrarySourceOperation(path, "rescan", async () => {
    const result = await invoke<ImportResult>("rescan_library_source", { path });
    await refreshAfterLibrarySourceMutation();
    return result;
  });
}

async function retryLibrarySource(path: string) {
  return runLibrarySourceOperation(path, "retry", async () => {
    const result = await invoke<ImportResult>("retry_library_source", { path });
    await refreshAfterLibrarySourceMutation();
    return result;
  });
}

async function previewRemoveLibrarySource(path: string) {
  return invoke<RemoveLibrarySourceImpact>("preview_remove_library_source", { path });
}

async function removeLibrarySource(path: string, mode: RemoveLibrarySourceMode) {
  return runLibrarySourceOperation(path, "remove", async () => {
    const result = await invoke<LibrarySourceMutation>("remove_library_source", {
      path,
      mode,
    });
    await refreshAfterLibrarySourceMutation();
    return result;
  });
}

async function relocateLibrarySource(path: string, newPath: string) {
  return runLibrarySourceOperation(path, "relocate", async () => {
    const result = await invoke<LibrarySourceMutation>("relocate_library_source", {
      path,
      newPath,
    });
    await refreshAfterLibrarySourceMutation();
    return result;
  });
}
```

Update the `folder-changed` listener to run:

```ts
await Promise.all([loadWallpapers(), loadStats(), loadLibrarySources()]);
```

Update the `import-complete` listener to include `loadLibrarySources()` before showing success. Export every new ref/method from the store return object.

- [ ] **Step 5: Run frontend GREEN and complete store tests**

Run:

```text
npx vitest run src/stores/librarySources.test.ts
npm run test:unit
npx vue-tsc --noEmit
```

Expected: focused source tests, complete Vitest suite, and TypeScript check pass.

- [ ] **Step 6: Document and commit**

Append:

```markdown
### 3A Task 6 — Pinia source lifecycle

- Added typed source state and exact Tauri command routing.
- Source operations own per-row busy/error state; failures retain actionable row errors and always clear busy state.
- Import completion refreshes source status alongside gallery data.
```

Commit:

```text
git add src/stores/wallpapers.ts src/stores/librarySources.test.ts docs/project-docs/CHANGELOG_AI.md
git -c commit.gpgsign=false commit -m "feat(library): add source state and command routing"
```

---

### Task 7: Build the Settings Library Sources card

**Files:**
- Create: `src/components/librarySourceModel.ts`
- Create: `src/components/librarySourceModel.test.ts`
- Create: `src/components/LibrarySourcesSettings.vue`
- Modify: `src/components/InspectorPanel.vue`
- Modify: `docs/project-docs/CHANGELOG_AI.md`

**Interfaces:**
- Consumes: Task 6 store types and methods.
- Produces: `primarySourceAction(status) -> "rescan" | "retry"`.
- Produces: `removeSourceCopy(impact)` with an unconditional no-original-file-deletion statement.
- Produces: Settings rows with status, counts, last scan/error, Rescan/Retry, Relocate, and Remove.
- Preserves: Gallery Folder/Import folder as the existing add-source shortcut.

- [ ] **Step 1: Write RED presentation-model tests**

Create:

```ts
import { describe, expect, it } from "vitest";
import { primarySourceAction, removeSourceCopy } from "./librarySourceModel";

describe("library source presentation model", () => {
  it("uses retry only for offline and error sources", () => {
    expect(primarySourceAction("online")).toBe("rescan");
    expect(primarySourceAction("scanning")).toBe("rescan");
    expect(primarySourceAction("offline")).toBe("retry");
    expect(primarySourceAction("error")).toBe("retry");
  });

  it("states the clear impact and original-file safety invariant", () => {
    const copy = removeSourceCopy(7);
    expect(copy.keepMetadata).toContain("Keep ratings, tags, collections");
    expect(copy.clearMetadata).toContain("7");
    expect(copy.safety).toBe(
      "Neither option deletes, moves, or recycles original image files.",
    );
  });
});
```

- [ ] **Step 2: Run model RED**

Run:

```text
npx vitest run src/components/librarySourceModel.test.ts
```

Expected RED: the model file and functions do not exist.

- [ ] **Step 3: Implement the pure model**

```ts
import type { LibrarySourceStatus } from "../stores/wallpapers";

export function primarySourceAction(
  status: LibrarySourceStatus,
): "rescan" | "retry" {
  return status === "offline" || status === "error" ? "retry" : "rescan";
}

export function removeSourceCopy(affectedWallpapers: number) {
  return {
    keepMetadata:
      "Keep ratings, tags, collections, and titles. Exclusive wallpapers become unavailable and can recover when a covering folder is added again.",
    clearMetadata: `Clear metadata for ${affectedWallpapers.toLocaleString()} wallpapers that are exclusive to this source.`,
    safety: "Neither option deletes, moves, or recycles original image files.",
  };
}
```

Run the focused test again. Expected: 2 tests pass.

- [ ] **Step 4: Create `LibrarySourcesSettings.vue`**

The component must:

- call `store.loadLibrarySources()` on mount;
- use `@tauri-apps/plugin-dialog.open({ directory: true, multiple: false })` for Add and Relocate;
- render `Online`, `Offline`, `Scanning`, or `Error`;
- show `available_count`, `unavailable_count`, `last_scan_at`, and `last_error`;
- disable only the row whose path exists in `librarySourceBusy`;
- call Rescan for online/scanning rows and Retry for offline/error rows;
- show an in-app confirmation surface with Keep metadata and Clear metadata;
- show the exact safety copy from `removeSourceCopy`;
- keep the dialog open and row state intact when removal fails;
- close the dialog only after a successful removal.

Use semantic tokens already defined in `styles.css`. Keep component styles scoped; do not add a new global design system.

The remove confirmation state is:

```ts
const removeCandidate = ref<LibrarySource | null>(null);
const removeImpact = ref<RemoveLibrarySourceImpact | null>(null);
const removeLoading = ref(false);

async function prepareRemove(source: LibrarySource) {
  removeCandidate.value = source;
  removeLoading.value = true;
  try {
    removeImpact.value = await store.previewRemoveLibrarySource(source.path);
  } catch {
    removeCandidate.value = null;
    removeImpact.value = null;
  } finally {
    removeLoading.value = false;
  }
}

async function confirmRemove(mode: RemoveLibrarySourceMode) {
  const source = removeCandidate.value;
  if (!source) return;
  try {
    await store.removeLibrarySource(source.path, mode);
    removeCandidate.value = null;
    removeImpact.value = null;
  } catch {
    // Store retains the row error and visible notification.
  }
}
```

Relocate uses:

```ts
async function relocate(source: LibrarySource) {
  const selected = await open({ directory: true, multiple: false });
  if (typeof selected !== "string") return;
  try {
    await store.relocateLibrarySource(source.path, selected);
  } catch {
    // Store retains the row error and visible notification.
  }
}
```

Add source reuses `store.importFolder(selected)` and relies on `import-complete` to refresh the source list.

- [ ] **Step 5: Mount the component under Settings**

Import `LibrarySourcesSettings` in `InspectorPanel.vue` and add it after the interface-mode card and before Focus Pause:

```vue
<LibrarySourcesSettings />
```

Do not alter Sidebar navigation or Gallery import buttons.

- [ ] **Step 6: Run frontend verification**

Run:

```text
npx vitest run src/components/librarySourceModel.test.ts
npm run test:unit
npx vue-tsc --noEmit
npm run build
```

Expected: focused and complete tests pass, typecheck passes, production build passes with only already documented third-party annotation warnings.

- [ ] **Step 7: Document and commit**

Append:

```markdown
### 3A Task 7 — Settings Library Sources UI

- Added source rows, per-row busy/error states, Rescan/Retry, Relocate, and explicit two-mode Remove confirmation.
- Both removal choices state that original files are never deleted, moved, or recycled.
- The existing Gallery/TitleBar folder action remains the add-source shortcut.
```

Commit:

```text
git add src/components/librarySourceModel.ts src/components/librarySourceModel.test.ts src/components/LibrarySourcesSettings.vue src/components/InspectorPanel.vue docs/project-docs/CHANGELOG_AI.md
git -c commit.gpgsign=false commit -m "feat(settings): add library source management"
```

---

### Task 8: Complete Phase 3A verification and project memory

**Files:**
- Modify: `docs/project-docs/ARCHITECTURE.md`
- Modify: `docs/project-docs/FEATURE_PLAYBOOK.md` only if execution changes the reusable feature workflow.
- Modify: `docs/project-docs/CHANGELOG_AI.md`
- Modify: `docs/project-docs/AI_DIARY.md` only for new pitfalls.

**Interfaces:**
- Consumes: all Tasks 1–7.
- Produces: Phase 3A completion evidence and explicit manual QA residuals.
- Preserves: Phase 3B/3C remain unimplemented and separately planned.

- [ ] **Step 1: Update architecture**

Append a `Library source lifecycle` addendum documenting:

- schema 7 `last_scan_at`/`last_error`;
- shared Windows path identity in `paths.rs`;
- overlap-safe keep/clear removal;
- relative-path relocation with ID preservation and collision rollback;
- scans outside SQLite;
- database commit before watcher take/drop;
- post-commit watcher warning and Retry recovery;
- no original-file or registry/system mutation.

- [ ] **Step 2: Run the complete automated gate**

Run in order:

```text
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
npx vue-tsc --noEmit
npm run test:unit
npm run build
git diff --check
```

Expected: every command PASS. Record exact test counts and any pre-existing build warnings.

- [ ] **Step 3: Run bounded manual QA only with explicit desktop approval**

Use a dedicated temporary folder and test database to verify:

1. Add source and see Online.
2. Remove the folder externally and see Offline.
3. Restore it and Retry.
4. Relocate to a second temporary folder and preserve rating/tag/collection metadata.
5. Remove with Keep metadata and confirm original files remain.
6. Re-add, then Remove with Clear metadata and confirm only metadata disappears.
7. Verify 800×600, common Windows scaling, keyboard focus, and remove-dialog reachability.

Do not touch a real user library, registry, autostart, context menu, Windows wallpaper, or system setting. If desktop approval is unavailable, record these items as NOT RUN.

- [ ] **Step 4: Final changelog closure**

Append:

```markdown
### Phase 3A completion gate

- Source listing, status, rescan/retry, overlap-safe removal, and relative-path relocation are implemented.
- Original wallpaper files and Windows/registry state remain outside source lifecycle mutations.
- Complete automated verification results are recorded below with exact counts.
- Manual Windows QA is recorded as PASS only for steps actually executed; all others remain explicit residual items.
- Phase 3B batch completion is the next separately planned slice.
```

Add exact command results beneath that text.

- [ ] **Step 5: Review scope and commit**

Run:

```text
git status --short
git diff --check
git diff --stat origin/main...HEAD
```

Verify every changed file belongs to Phase 3A. Then:

```text
git add docs/project-docs/ARCHITECTURE.md docs/project-docs/CHANGELOG_AI.md
git add docs/project-docs/FEATURE_PLAYBOOK.md docs/project-docs/AI_DIARY.md
git -c commit.gpgsign=false commit -m "docs(phase3): record source lifecycle completion"
```

Only stage `FEATURE_PLAYBOOK.md` or `AI_DIARY.md` if they actually changed.
