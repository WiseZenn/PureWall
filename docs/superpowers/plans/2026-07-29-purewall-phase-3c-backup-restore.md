# PureWall Phase 3C Backup and Restore Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a versioned, bounded PureWall metadata backup that can be exported atomically, previewed safely, and merged transactionally without copying, moving, deleting, or embedding original wallpaper files.

**Execution status (2026-07-29):** Tasks 1–7 completed on `codex/purewall-phase-3`; Phase 3C is complete. Phase 3D desktop QA/external CI/final review, merge, and push remain separate and pending.

**Architecture:** A focused Rust `library_backup` module owns the v1 JSON contract, validation, size limits, path normalization, preview DTOs, and atomic file replacement. `Database` owns consistent snapshot reads and one-transaction merge semantics. `main.rs` remains the Tauri orchestration boundary: it maps stable errors, releases SQLite before filesystem/watcher work, reapplies imported runtime settings, and restores reachable source watchers after commit. Pinia owns command busy/error/refresh behavior; a dedicated Settings component owns native file pickers, preview confirmation, and simultaneous display of database success plus watcher warnings.

**Tech Stack:** Rust 2021, Tauri 2, rusqlite/SQLite, serde/serde_json, chrono, Vue 3, TypeScript, Pinia, Vitest, Tauri dialog plugin.

## Global Constraints

- Work only in `D:\Desktop\PureWall\.worktrees\purewall-phase-3` on `codex/purewall-phase-3`.
- PureWall-X, cloud sync, accounts, remote libraries, binary wallpaper backup, caches, play history, registry, autostart, context-menu, Windows wallpaper, and system settings remain out of scope.
- Original wallpaper files are read only for existence/canonical identity. Export/import must never copy, move, delete, recycle, or rewrite them.
- Reuse accepted ADR-029: SQLite metadata commits first; watcher restoration happens after the database guard is released, and failures are returned as warnings rather than transaction failures.
- Reuse the existing `CommandResult`/`CommandError` boundary. Do not add a second global error system or any new dependency.
- Use a 32 MiB input/output ceiling, 256 sources, 100,000 wallpapers, 10,000 tags, 10,000 collections, 256 tag/collection names per wallpaper, 32,767-character paths, 512-character titles, and 128-character tag/collection names.
- Import is always two-step: `preview_backup_import` validates and summarizes; `import_backup` re-reads and re-validates the selected file before committing.
- Existing local-only rows, sources, tags, collections, and settings are retained. Backup conflicts override rating, hidden state, custom title, and complete tag/collection association sets for the same normalized path.
- Existing paths are canonicalized. Missing absolute paths receive deterministic lexical Windows normalization and remain unavailable. Ambiguous identity is never guessed.
- Only these v1 settings are accepted: `rotationSecs`, `displayMode`, `focusModeEnabled`, `paused`, `theme`, and `workspaceMode`. Missing keys do not change local state; unknown JSON keys are ignored by serde but never persisted.
- Every implementation step follows RED → GREEN → focused verification. After any failed patch check or application, stop immediately and inspect `git status --short`.
- Every code delta updates `docs/project-docs/CHANGELOG_AI.md`; append `AI_DIARY.md` only for a genuinely new pitfall. Keep unresolved manual QA and CI items explicit.

---

### Task 1: Define and validate the bounded v1 backup contract

**Files:**
- Create: `src-tauri/src/library_backup.rs`
- Create: `src-tauri/src/library_backup_tests.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `docs/project-docs/CHANGELOG_AI.md`

**Interfaces:**

```rust
pub(crate) const BACKUP_SCHEMA_VERSION: u32 = 1;
pub(crate) const MAX_BACKUP_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupDocumentV1 {
    pub app: String,
    pub schema_version: u32,
    pub exported_at: String,
    pub sources: Vec<BackupSource>,
    pub wallpapers: Vec<BackupWallpaper>,
    pub tags: Vec<BackupNamedEntity>,
    pub collections: Vec<BackupNamedEntity>,
    pub settings: BackupSettings,
}

pub(crate) fn parse_and_normalize(bytes: &[u8])
    -> Result<NormalizedBackup, BackupError>;
pub(crate) fn read_bounded(path: &Path) -> Result<Vec<u8>, BackupError>;
```

- [ ] Add crate-level RED tests for the exact `app = "PureWall"` and `schemaVersion = 1` gates, the 32 MiB read limit, absolute paths, valid ratings, bounded arrays/strings, duplicate normalized wallpaper identities, duplicate names, and undefined relation names.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml library_backup_tests -- --nocapture`. Expected RED: the `library_backup` module and DTOs do not exist.
- [ ] Add `mod library_backup;` and `#[cfg(test)] mod library_backup_tests;` to `main.rs`.
- [ ] Implement serde DTOs with camelCase field names, the fixed limits, stable `BackupError` variants, full-document validation, existing-path canonicalization, and offline absolute-path lexical normalization using the shared `paths::path_identity_key` boundary.
- [ ] Ensure normalization trims user-facing names, preserves display-title nullability, deduplicates relation names in first-occurrence order, and rejects duplicate source/tag/collection/wallpaper identities rather than silently choosing one.
- [ ] Rerun the focused test command. Expected GREEN: all contract/limit tests pass.
- [ ] Run `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` and `cargo check --manifest-path src-tauri/Cargo.toml`.
- [ ] Append Task 1 RED/GREEN evidence and limits to `CHANGELOG_AI.md`.
- [ ] Commit: `git -c commit.gpgsign=false commit -m "feat(phase3): define bounded backup schema"`

### Task 2: Add consistent snapshot export and atomic destination replacement

**Files:**
- Modify: `src-tauri/src/library_backup.rs`
- Modify: `src-tauri/src/library_backup_tests.rs`
- Modify: `src-tauri/src/db.rs`
- Modify: `docs/project-docs/CHANGELOG_AI.md`

**Interfaces:**

```rust
impl Database {
    pub(crate) fn export_backup_snapshot(
        &self,
        client: BackupClientSettings,
        exported_at: String,
    ) -> Result<BackupDocumentV1>;
}

pub(crate) fn serialize_bounded(document: &BackupDocumentV1)
    -> Result<Vec<u8>, BackupError>;
pub(crate) fn write_atomically(destination: &Path, bytes: &[u8])
    -> Result<(), BackupError>;
```

- [ ] Add RED tests proving the snapshot contains sources, wallpaper user metadata, tag/collection definitions and associations, and only allowlisted settings; it must omit play history, derivative/cache paths, internal maintenance keys, registry/autostart state, and secrets.
- [ ] Add RED tests proving deterministic ordering, pretty JSON, output-size rejection before destination mutation, cleanup of the operation-owned temp file on failure, and replacement of an existing destination without exposing a partial JSON file.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml library_backup_tests -- --nocapture`. Expected RED: snapshot/serialization/atomic-writer APIs are missing.
- [ ] Implement a read transaction for a consistent metadata snapshot. Query wallpaper relations by names, not database IDs; sort every exported collection deterministically.
- [ ] Map SQLite `display_title = ""` to JSON `null`; include scanner metadata for offline restoration, but do not export `play_count`, `last_played`, `play_events`, or unapproved settings.
- [ ] Serialize to memory, reject output larger than 32 MiB, create one same-directory operation-owned temp file with `create_new`, write/flush/sync it, then atomically install it. On Windows, replace an existing destination through the existing `windows` crate file API; on failure remove only that exact temp file.
- [ ] Rerun focused tests. Expected GREEN: snapshot and file-safety tests pass.
- [ ] Run `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` and `cargo check --manifest-path src-tauri/Cargo.toml`.
- [ ] Append Task 2 evidence to `CHANGELOG_AI.md`.
- [ ] Commit: `git -c commit.gpgsign=false commit -m "feat(phase3): export metadata backups atomically"`

### Task 3: Implement preview and one-transaction metadata merge

**Files:**
- Modify: `src-tauri/src/library_backup.rs`
- Modify: `src-tauri/src/library_backup_tests.rs`
- Modify: `src-tauri/src/db.rs`
- Modify: `docs/project-docs/CHANGELOG_AI.md`

**Interfaces:**

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupImportPreview {
    pub source_count: usize,
    pub wallpaper_count: usize,
    pub tag_count: usize,
    pub collection_count: usize,
    pub setting_count: usize,
    pub new_wallpapers: usize,
    pub overwritten_wallpapers: usize,
    pub missing_paths: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupMergeResult {
    pub added_wallpapers: usize,
    pub updated_wallpapers: usize,
    pub added_sources: usize,
    pub created_tags: usize,
    pub created_collections: usize,
    pub updated_settings: usize,
    pub client_settings: BackupClientSettings,
}

impl Database {
    pub(crate) fn preview_backup_import(
        &self,
        backup: &NormalizedBackup,
    ) -> Result<BackupImportPreview>;
    pub(crate) fn merge_backup(
        &mut self,
        backup: &NormalizedBackup,
    ) -> Result<BackupMergeResult>;
}
```

- [ ] Add RED tests for preview counts/warnings, backup-priority rating/hidden/title replacement, null-title clearing, complete tag/collection set replacement, name reuse without duplicate definitions, local-only preservation, missing-file insertion as unavailable, source reuse, allowlist-only settings, and idempotent second import.
- [ ] Add a RED rollback test that calls the database merge boundary with an invalid relation after transaction start and proves no tag/source/wallpaper/settings mutation survives.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml library_backup_tests -- --nocapture`. Expected RED: preview/merge APIs are missing.
- [ ] Implement preview using normalized path identities and current database identities without mutating SQLite or starting scans/watchers.
- [ ] Implement one SQLite transaction that reuses/creates tag and collection names, reuses/adds watched sources, updates matching wallpaper user metadata, inserts missing wallpaper metadata with real `file_available`, replaces the backup wallpaper's relation sets, and writes only present allowlisted settings.
- [ ] Preserve local-only metadata and scanner-derived fields on matching local rows. For new rows use backed-up source/hash/dimensions/file size, and compute availability from the current filesystem.
- [ ] On any error, roll back the entire merge. Return counts only after commit.
- [ ] Rerun focused tests. Expected GREEN: preview, conflict, idempotence, missing-path, and rollback tests pass.
- [ ] Run the complete database tests plus `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` and `cargo check --manifest-path src-tauri/Cargo.toml`.
- [ ] Append Task 3 evidence to `CHANGELOG_AI.md`.
- [ ] Commit: `git -c commit.gpgsign=false commit -m "feat(phase3): merge backup metadata transactionally"`

### Task 4: Expose Tauri commands and reconcile runtime state after commit

**Files:**
- Modify: `src-tauri/src/library_backup.rs`
- Modify: `src-tauri/src/library_backup_tests.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `docs/project-docs/CHANGELOG_AI.md`

**Interfaces:**

```rust
#[tauri::command]
fn export_library_backup(
    state: tauri::State<AppState>,
    destination: String,
    client_settings: BackupClientSettings,
) -> CommandResult<BackupExportResult>;

#[tauri::command]
fn preview_backup_import(
    state: tauri::State<AppState>,
    path: String,
) -> CommandResult<BackupImportPreview>;

#[tauri::command]
fn import_library_backup(
    state: tauri::State<AppState>,
    app: tauri::AppHandle,
    path: String,
) -> CommandResult<BackupImportResult>;
```

- [ ] Add RED tests for stable error mapping (`BACKUP_TOO_LARGE`, `BACKUP_INVALID_APP`, `BACKUP_UNSUPPORTED_SCHEMA`, `BACKUP_INVALID_DATA`, `BACKUP_WRITE_FAILED`) and a pure post-commit source reconciliation helper whose injected watcher start can fail without changing the already merged database.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml library_backup_tests -- --nocapture`. Expected RED: command mapping/runtime reconciliation helpers are missing.
- [ ] Implement export so SQLite snapshot collection finishes before destination IO. Validate the user-selected path and return the final byte count.
- [ ] Implement preview as a read-only bounded parse plus database comparison.
- [ ] Implement import by re-reading/re-validating, committing through `&mut Database`, releasing the database mutex, applying imported rotation/display/focus/pause values to `AppState`, then restoring only reachable persisted sources through `ensure_runtime_folder_watcher`.
- [ ] Return missing/offline sources and watcher start failures as warnings alongside an explicit committed database result. Persist source-level watcher errors; never turn a post-commit watcher failure into an import transaction error.
- [ ] Register all three commands in `tauri::generate_handler!`.
- [ ] Rerun focused tests. Expected GREEN: error and post-commit warning tests pass.
- [ ] Run `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`, `cargo test --manifest-path src-tauri/Cargo.toml`, and `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.
- [ ] Append Task 4 evidence to `CHANGELOG_AI.md`.
- [ ] Commit: `git -c commit.gpgsign=false commit -m "feat(phase3): expose backup restore commands"`

### Task 5: Add Pinia routing and refresh ownership

**Files:**
- Create: `src/stores/libraryBackup.test.ts`
- Modify: `src/stores/wallpapers.ts`
- Modify: `docs/project-docs/CHANGELOG_AI.md`

**Interfaces:**

```ts
export interface BackupImportPreview { /* exact camelCase command DTO */ }
export interface BackupImportResult {
  committed: boolean;
  merge: BackupMergeResult;
  warnings: string[];
}

const isBackupBusy = ref(false);
const backupError = ref("");

async function exportLibraryBackup(
  destination: string,
  clientSettings: BackupClientSettings,
): Promise<BackupExportResult>;
async function previewBackupImport(path: string): Promise<BackupImportPreview>;
async function importLibraryBackup(path: string): Promise<BackupImportResult>;
```

- [ ] Add RED Vitest cases for exact command names/payloads, one in-flight guard, failure state release, preview error retention, post-import refresh of sources/wallpapers/tags/collections/stats/runtime settings, and simultaneous committed-success plus warning notification.
- [ ] Run `npx vitest run src/stores/libraryBackup.test.ts`. Expected RED: DTOs/state/methods are absent.
- [ ] Implement typed DTOs and one shared backup-operation runner. Reject overlapping export/preview/import operations, clear stale errors at start, preserve actionable errors on failure, and always release busy state.
- [ ] After confirmed import, refresh library sources, gallery, tags, collections, stats, display mode, focus mode, and pause state. Do not clear unrelated Gallery selection on preview or failed import.
- [ ] Show export/import success through existing notifications; show warnings without reclassifying the committed import as failed.
- [ ] Rerun focused tests. Expected GREEN: all backup routing tests pass.
- [ ] Run `npx vue-tsc --noEmit` and `npm run test:unit`.
- [ ] Append Task 5 evidence to `CHANGELOG_AI.md`.
- [ ] Commit: `git -c commit.gpgsign=false commit -m "feat(phase3): route backup restore state"`

### Task 6: Add the Settings export/preview/confirm workflow

**Files:**
- Create: `src/components/LibraryBackupSettings.vue`
- Create: `src/components/backupPresentationModel.ts`
- Create: `src/components/backupPresentationModel.test.ts`
- Modify: `src/components/AppIcon.vue`
- Modify: `src/components/InspectorPanel.vue`
- Modify: `docs/project-docs/CHANGELOG_AI.md`

- [ ] Add RED model tests for preview summary rows, singular/plural missing-path copy, commit count copy, and rendering database success together with offline/watcher warnings.
- [ ] Run `npx vitest run src/components/backupPresentationModel.test.ts`. Expected RED: the presentation model does not exist.
- [ ] Implement the pure presentation model and rerun the focused test. Expected GREEN.
- [ ] Add a `LibraryBackupSettings` card immediately after `LibrarySourcesSettings` in Settings. Reuse the Tauri dialog plugin `save` for `purewall-backup-v1.json` and `open` with a JSON filter for import.
- [ ] Pass current `theme` and `workspaceMode` into export. After committed import, apply only returned valid client settings through `setTheme` and `setWorkspaceMode`.
- [ ] Present a modal preview with counts for sources/wallpapers/tags/collections/settings, new/overwritten/missing paths, warnings, Cancel, and explicit `Import backup`. Reuse the source-dialog keyboard contract: semantic buttons, dialog focus, Escape, focus trap, focus return, visible focus rings, reduced motion, and no confirm action until preview succeeds.
- [ ] After confirmation, keep one success surface that states metadata was committed and lists offline/watcher warnings. Never suggest that original images were restored or changed.
- [ ] Add official Fluent upload/download icons to `AppIcon.vue`; do not add a dependency or hand-write SVG.
- [ ] Run focused model tests, `npx vue-tsc --noEmit`, `npm run test:unit`, and `npm run build`.
- [ ] Append Task 6 evidence to `CHANGELOG_AI.md`.
- [ ] Commit: `git -c commit.gpgsign=false commit -m "feat(phase3): add backup restore settings flow"`

### Task 7: Synchronize architecture/process docs and run the Phase 3C gate

**Files:**
- Modify: `docs/project-docs/ARCHITECTURE.md`
- Modify: `docs/project-docs/FEATURE_PLAYBOOK.md` only if the implemented workflow changes the reusable development playbook
- Modify: `docs/project-docs/CHANGELOG_AI.md`
- Modify: `docs/project-docs/AI_DIARY.md` only for genuinely new pitfalls
- Modify: `docs/superpowers/specs/2026-07-28-purewall-phase-3-library-management-design.md`

- [ ] Add the implemented v1 schema/limits, snapshot/atomic-write boundary, preview/confirm boundary, merge rules, allowlist settings, and post-commit watcher warning ownership to `ARCHITECTURE.md`.
- [ ] Mark the Phase 3 design status consistently with the already recorded user approval; mark 3C complete without claiming 3D.
- [ ] Record exact final evidence and unresolved manual/external checks in `CHANGELOG_AI.md`. Append `AI_DIARY.md` only if implementation discovered a new reusable pitfall.
- [ ] Run `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`.
- [ ] Run `cargo check --manifest-path src-tauri/Cargo.toml`.
- [ ] Run `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml`. Expected: all automated Rust tests pass; the existing manual release benchmark remains ignored.
- [ ] Run `npx vue-tsc --noEmit`.
- [ ] Run `npm run test:unit`.
- [ ] Run `npm run build`. Record third-party warnings separately from errors.
- [ ] Run `git diff --check` and `git status --short`.
- [ ] Do not run live Tauri import/export against the real app database. Record isolated automated coverage as PASS and real Windows desktop QA as NOT RUN unless separately authorized with a dedicated test database.
- [ ] Review the complete branch diff for original-file, registry, autostart, system-setting, secret, unbounded-input, path-identity, transaction, and SQLite-lock/watcher-drop violations.
- [ ] Commit: `git -c commit.gpgsign=false commit -m "docs(phase3): record backup restore completion"`

## Plan self-review

- Spec coverage: versioned schema, bounds, atomic export, preview, confirmation, backup-priority merge, full relation replacement, local-only preservation, missing paths, allowlist settings, idempotence, rollback, and post-commit watcher warnings each have an implementation owner and an automated test.
- Safety coverage: the plan never invokes or modifies wallpaper binaries, caches, Recycle Bin, registry, autostart, Windows wallpaper, policy, or user app data.
- Architecture coverage: no new schema or dependency is required; accepted ADR-029 governs watcher restoration after merge commit.
- Type consistency: Rust and TypeScript DTOs use the same camelCase serialized fields; database IDs never appear in the backup file, while tag/collection names are the stable relation keys.
- Placeholder scan: no TODO, “implement later,” ellipsis-based production body, or unresolved design choice remains in the executable tasks. The TypeScript comment in the interface sketch is descriptive only; the implementation task requires the exact command DTO fields.
- Completion boundary: Phase 3C can finish on this branch, but Phase 3D manual Windows QA, external CI, final review, merge, and push remain explicit later work.
