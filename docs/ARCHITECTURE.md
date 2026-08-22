# PureWall Engineering Architecture
This document describes the contributor-facing architecture of PureWall, a local-first
Windows wallpaper library and playback application.

## System overview

PureWall combines a Vue desktop interface with a Tauri 2 Rust backend. The application
indexes user-selected local image folders, stores library metadata in SQLite, generates
app-owned image derivatives, and applies wallpapers through Windows APIs. The original
wallpaper files remain in their source locations.

Performance measurement is a separate, contributor-only entry path. The Rust harness is
compiled only with the `performance-harness` Cargo feature and consumes an explicit
`--performance-harness` marker before Tauri setup. Harness execution therefore never
initializes the GUI, watcher, tray, wallpaper integration, production AppData, or a real
library; it uses only marker-owned synthetic temporary roots. See
[PERFORMANCE.md](PERFORMANCE.md) for the run, comparison, and cleanup contract.

The primary flow is:
```text
import or watched-folder change
  → bounded scan and file validation
  → SQLite upsert and availability reconciliation
  → thumbnail/preview cache request
  → Vue gallery or active preview
  → playback action → Windows wallpaper API → persisted history/event
```

## Watched-folder synchronization

Ordinary filesystem changes remain incremental and backend-owned. Native watcher callbacks classify `EventKind` and enqueue per-path intent only; they do not inspect the filesystem or touch SQLite. Ordinary directory data/metadata changes do not trigger subtree scans, while create/rename paths may inspect an existing directory and remove/rename paths may reconcile a missing path independently. A bounded global root-keyed queue admits at most 256 roots and 4,096 unique pending paths per root, coalesces at a 250ms quiet window with a 2-second maximum latency, and uses no more than two reconciliation workers. Per-root path saturation, notify loss, or callback errors promote that source to a bounded snapshot; a new root beyond the 256-root admission limit fails closed and is persisted as unavailable rather than entering a queue that has no slot.

Every work item carries the canonical registered root, source owner, and unique admission generation captured when it is queued. A root identity keeps one in-flight execution slot even if it is removed and immediately re-admitted, so an old completion cannot release or consume the replacement generation's work; later signals coalesce into exactly one follow-up. Component-aware containment is checked before IO and again at the atomic SQLite boundary. Stale-owner work becomes an explicit no-op and does not emit `folder-changed`, while a real applied reconciliation with zero imports still emits its refresh. Outside-root missing paths have no effect, and Windows scans reject symbolic links or `FILE_ATTRIBUTE_REPARSE_POINT` at the root itself and at descendant entries.

Watcher-triggered, startup, initial-import, manual-rescan, and relocation-handoff snapshots all use the same atomic reconciliation transaction. A successful full snapshot marks registered descendants absent from the scan unavailable while retaining their metadata and relationships; registration authorization, availability changes, image upserts, and source scan state commit together before a refresh event. Failed reconciliations roll back and preserve the prior successful state. Native watchers are dropped outside the registry lock before the reconciliation queue is closed and joined; remaining writers stop before SQLite is checkpointed last. Source-identified failures are emitted at most once per unresolved episode, and the frontend refreshes source status before showing that failure.

## Technology stack

| Layer | Technology | Responsibility |
| --- | --- | --- |
| Desktop shell | Tauri 2, WebView2 | Window lifecycle, command IPC, capabilities, asset protocol |
| Frontend | Vue 3, TypeScript, Pinia | Workbench UI, local state, command/event reconciliation |
| Backend | Rust | Filesystem, playback orchestration, media scheduling, Windows integration |
| Persistence | SQLite via `rusqlite` | Library metadata, relationships, settings, playback history |
| Windows integration | `windows` crate | Wallpaper/display APIs and Shell metadata |

## Module map

### Frontend

- `src/main.ts`, `src/router.ts`: Vue bootstrap and routes for the main window and widget.
- `src/views/Home.vue`, `src/views/WidgetView.vue`: top-level window views.
- `src/components/`: workbench, gallery, inspector, media, settings, source-management,
  backup, update, notification, and reusable UI components.
- `src/stores/wallpapers.ts`: the public Pinia facade for library, selection, playback,
  pagination, import, and event state.
- `src/stores/parts/`: focused store implementations for media, playback, taxonomy,
  sources, and shared DTO types.
- `src/stores/activeMedia.ts` and `src/stores/appUpdates.ts`: active-image presentation
  state and updater UI state.
- `src/composables/`: shared theme, inspector, notification, and workspace-mode behavior.
- `src/utils/`: presentation, display-mode, thumbnail scheduling, and retry helpers.

### Rust backend

- `src-tauri/src/main.rs`: application setup, shared state, background-worker lifecycle,
  command registration, and cross-module orchestration.
- `src-tauri/src/commands/*.rs`: command groups for imports, library sources, backup,
  media, playback, and taxonomy.
- `db.rs`, `db_taxonomy.rs`, `db_sources.rs`, `db_backup.rs`: migrations and SQLite
  persistence split by domain.
- `scanner.rs`, `paths.rs`, `library_sources.rs`: bounded local scanning, path identity,
  watched-folder lifecycle, and reconciliation.
- `thumbnails.rs`, `media_queue.rs`, `image_decoder.rs`, `windows_image_decoder.rs`:
  derivative cache, bounded preview-first queue, and target-sized decoding.
- `wallpaper.rs`, `focus.rs`, `playback_action.rs`, `active_preview.rs`: display-aware
  wallpaper application, focus pause, action semantics, and active-preview warming.
- `context_menu.rs`, `autostart.rs`, `tray.rs`, `widget.rs`, `shell_metadata.rs`,
  `app_updates.rs`, and `diagnostics.rs`: Windows integration and operational boundaries.

## Library and media flow

1. Folder, file, and drag/drop imports accept only supported local image paths.
2. The scanner applies recursion, directory-entry, and image-count limits; on Windows it
   rejects a symbolic-link/reparse-point root before traversal and skips descendant entries
   marked as reparse points.
3. Scan results are upserted into SQLite with dimensions, file size, source information,
   and availability state. Watched-folder changes are debounced and reconciled before the
   frontend is notified.
4. Gallery pagination, filtering, search, sorting, collection membership, and availability
   checks run in SQLite. Vue renders only the loaded pages and virtualized visible rows.
5. Thumbnail and preview commands return app-owned derivative-cache paths. The asset
   protocol exposes only that cache, not arbitrary user image folders.
6. A bounded path-keyed media queue deduplicates work, gives active previews priority,
   and emits completion events. The frontend keeps a usable thumbnail visible until the
   higher-resolution preview has decoded in the WebView.

## IPC surface

Tauri commands are registered in `main.rs`; frontend callers use imported Tauri APIs
rather than a global bridge. The surface is organized into these groups:

- library reads: paged/filterable wallpaper queries, metadata, Shell metadata, and stats;
- imports and sources: initial/additive imports, dropped paths, source list/rescan/retry,
  safe source removal/relocation, and backup preview/import/export;
- taxonomy and batch actions: tags, collections, ratings, hidden state, titles, and
  bounded multi-item operations;
- playback: next/set/rate/delete, pause, rotation interval, display mode, focus status,
  and yearly statistics;
- media: thumbnail batches, active-wallpaper bootstrap, preview lookup, and prewarming;
- integration: context-menu status, autostart status, widget controls, updater checks,
  and diagnostic-log reads.

Backend events keep WebView windows synchronized for folder changes, generated media,
playback completion, pause state, widget visibility, and user-visible operation failures.
The command outcome is authoritative for the invoking window; playback completion events
are delivered to other windows to avoid duplicate reconciliation.

## SQLite schema summary

- `wallpapers`: canonical source path, rating, hidden/blacklisted and availability state,
  scanner metadata, display title, play counters, and timestamps.
- `tags` / `wallpaper_tags`: descriptive many-to-many metadata.
- `collections` / `collection_wallpapers`: user-curated many-to-many groups, distinct from
  tags.
- `watched_folders`: persisted local source roots and scan/error status.
- `settings`: persisted playback and application preferences.
- `play_events`: detailed playback telemetry; historical records survive wallpaper removal
  through nullable wallpaper references.

Migrations run at startup. Additive column changes and file-availability backfill are
combined with transactional relationship-table rebuild exceptions when older schemas need
them. Before mutating an existing older database, PureWall consumes the complete SQLite
`quick_check` result and creates a WAL-consistent, adjacent safety copy named
`<db>.pre-migration-v<from>-to-v<to>.bak`. The copy is retained indefinitely, with a
same-directory temporary file published only after online-backup and integrity validation;
an invalid or colliding copy fails closed. Base schema creation, additive migration,
file-availability backfill, relationship rebuilds, index creation, foreign-key validation,
and `user_version` update commit in one transaction. Future schema versions are rejected
before mutation. PureWall never automatically restores or deletes a safety copy. If startup
fails, stop PureWall, preserve both the source database and retained backup, record the full
backend error and backup path, then verify the backup or seek a compatible build/maintainer
guidance; do not replace database files automatically. Files missing from a source remain
recoverable metadata records but are excluded from normal available-library queries and
playback.

Deletion uses a separate bounded safety boundary. PureWall checks exact registration while holding SQLite only,
then releases the mutex before inspecting a local fixed-volume path. On Windows it opens every existing path
component with no-follow reparse flags, rejects any reparse component or directory, captures FILE_ID_INFO
(volume serial plus the full 128-bit file ID) and the handle-resolved path, and revalidates that chain immediately
before invoking a project-owned STA `IFileOperation` wrapper. The wrapper sets `FOFX_RECYCLEONDELETE`,
`FOFX_ADDUNDORECORD`, `FOFX_EARLYFAILURE`, and `FOF_WANTNUKEWARNING`; it never sets `FOF_NO_UI` or
`FOF_NOCONFIRMATION`. PureWall snapshots Recycle Bin item IDs before and after the Shell call and labels an item
`recycled` only when a new matching item is observed. It never auto-confirms permanent deletion; an explicit Windows
permanent-delete choice or missing evidence is `recycle_outcome_unknown`, and metadata is retained. Permanent
delete and non-Windows fallback deletion are forbidden. Because Windows `IFileOperation`/`IShellItem` accepts a
path rather than a handle, an active same-user process can still replace a path after final validation and before
or during Shell execution; PureWall explicitly does not claim complete TOCTOU elimination. Post-effect outcomes
are reported per item, and metadata cleanup failures never cause the UI to restore a file already moved to the
Recycle Bin.

## Playback and media queue

Playback selects eligible available wallpapers through SQLite, using rating-aware weighted
selection. Display-aware application runs through a long-lived Windows STA worker. Rotation
and focus monitoring use condition-variable signaling so idle operation does not poll
unnecessarily. A committed playback action persists its applicable history/current state
before best-effort UI notification.

The media queue is bounded and keyed by normalized source path. It merges thumbnail and
preview needs, reserves capacity for active previews, and drops stale queued thumbnail work
when the cold backlog reaches its limit. Cache keys include source identity and derivative
profile so changed source files and output settings can refresh safely.

## Windows and safety boundaries

- PureWall operates on local, user-selected source paths; UNC, extended UNC, and mapped
  network-drive imports are rejected.
- Deleting a wallpaper uses the project-owned Recycle Bin Shell path; PureWall never auto-confirms permanent deletion and requires observed Recycle Bin evidence before metadata cleanup.
- Context-menu and autostart integration may write only PureWall-owned values under HKCU:
  the PureWall/PW context-menu entries and the `PureWall` Run value.
- The application does not change Windows shell policies, HKLM, or unrelated registry keys.
- The asset protocol is scoped to PureWall-owned derivative cache files; source originals
  are never broadly exposed to the WebView.
- Backup/restore manages PureWall metadata only. It does not copy, move, delete, or rewrite
  original wallpaper files or change Windows wallpaper/system settings.

## Update design

Updates are explicit user actions from the settings UI. Rust owns updater retrieval and
installation through the Tauri updater plugin; only safe version metadata, release notes,
and typed progress cross into the WebView. Release configuration uses HTTPS manifests and
Tauri signature verification. Installable Windows artifacts additionally require
Authenticode signing, while release publication remains a maintainer-controlled step.
