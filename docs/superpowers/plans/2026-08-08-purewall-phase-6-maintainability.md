# PureWall Phase 6 Maintainability Plan

> **Goal:** Incrementally split the large orchestration, database, store, and stylesheet files along existing responsibilities, keeping compatibility facades while internal modules move. No big-bang rewrite; every refactor preserves behavior and Tauri command compatibility.

**Base:** `01e1056` (Phase 5 closeout) on `main`.
**Strategy (from accepted foundation optimization design §3.2):** large files may be split only when the active task already changes their responsibility; keep stable consumer-facing facades; document each accepted boundary change before implementation.

---

## Current state (measured)

| File | Lines | Notes |
|---|---|---|
| `src-tauri/src/main.rs` | 3684 | 67 `#[tauri::command]` + orchestration; already has 30+ sibling modules |
| `src-tauri/src/db.rs` | 3808 | `Database` impl: migrations, gallery, taxonomy, sources, playback, settings |
| `src/stores/wallpapers.ts` | 2052 | single Pinia store: gallery, media, playback, backup, sources, batch, settings |
| `src/styles.css` | 3944 | tokens, layout, components, responsive — only 1 section marker |

## Phase plan

### Task A: db.rs — extract taxonomy + sources + playback-history modules
- `db.rs` keeps `Database` struct + connection + migrations + core wallpaper CRUD + `WallpaperEntry`/page types.
- Extract into `db_taxonomy.rs` (tags/collections/rating/blacklist/batch ops, ~L1247-1605) and `db_sources.rs` (watched folders + availability reconciliation, ~L470-937) as modules with methods taking `&Database` or a connection; main.rs call sites unchanged via `db::` re-exports.
- Tests: existing `db.rs` tests keep passing; add focused module tests.

### Task B: main.rs — extract command-group modules
- `main.rs` keeps `main()`, watcher orchestration, media workers, timer/focus loop, CLI, and the `invoke_handler` registration.
- Extract:
  - `commands/library_sources_cmds.rs`: list/rescan/retry/preview-remove/remove/relocate source commands + validation helpers (~L449-1239 slice).
  - `commands/import_cmds.rs`: set_wallpaper_folder/import_wallpaper_folder/import_wallpaper_files/import_dropped_paths (~L1239-1466).
  - `commands/taxonomy_cmds.rs`: tags/collections/rating/blacklist/batch commands (~L2019-2359).
  - `commands/playback_cmds.rs`: next/like/dislike/pause/display-mode/focus commands (~L1595-2019 + 2796-3100).
  - `commands/backup_cmds.rs`: export/preview/import backup (~L2605-2796).
  - `commands/media_cmds.rs`: thumbnails/preview/active bootstrap/prewarm (~L2379-2605).
- Each new module keeps the exact `#[tauri::command]` signatures; `invoke_handler` list in main.rs unchanged (register via `commands::*::name` paths). `main_tests` stay in main.rs.
- Requirement: `cargo check` + `cargo test` + clippy `-D warnings` green after each extraction batch, not only at the end.

### Task C: wallpapers.ts — extract media + playback + taxonomy composables
- `stores/wallpapers.ts` keeps store shell, state refs, selection, listeners, and the public facade.
- Extract pure/composable modules in `src/stores/parts/`:
  - `mediaState.ts`: thumbnail/preview cache, retry timers, asset URL helpers, `loadThumbnailsForPaths`/`loadPreview`/active-media presentation.
  - `playbackState.ts`: `runPlaybackAction`, completion reconciliation, pause/display/focus state commands.
  - `taxonomyState.ts`: tags/collections/batch rating/blacklist/delete commands.
  - `sourcesState.ts`: library-source and backup commands.
- Store composes these with a typed internal API; **all public store properties/actions the components use keep identical names/signatures** (facade preserved). `vue-tsc` + full vitest green after each extraction.

### Task D: styles.css — add section structure, no visual change
- Insert `/* === Section: ... === */` markers at existing logical boundaries (tokens, shell, titlebar, sidebar, gallery, inspector, quiet canvas, status bar, responsive, living gallery) without moving rules or changing values.
- Optional: split into `styles/base.css` + `styles/layout.css` + `styles/components.css` + `styles/responsive.css` imported by `main.ts` **only if** the diff stays behavior-neutral and `npm run build` stays green; otherwise keep single file with markers.
- Verify: `npm run build` + existing responsive/semantic unit tests.

### Task E: verification + documentation
- Full gate: cargo fmt/check/test/clippy, vue-tsc, vitest, build, audit, git diff --check.
- Append CHANGELOG_AI.md with per-task evidence; append AI_DIARY.md only for new pitfalls.
- No registry/system changes; no behavior change; no command-name changes; no schema change.

## Safety boundaries
- Tauri command names and `invoke_handler` registration are frozen (ADR-015 incremental extraction).
- Public store facade is frozen; components must not change.
- SQLite schema and migration order unchanged.
- No new dependencies.
- Every batch verified green before the next begins.

## Work assignment (subagents)
- Task A (db.rs) — one worker lane.
- Task B (main.rs) — one worker lane (largest, sequenced internally).
- Task C (wallpapers.ts) — one worker lane.
- Task D (styles.css) — one worker lane.
- Task E (gate + docs) — parent (gpt-5.6-sol) after all lanes merge.

Parallel lanes use isolated worktrees; parent merges, resolves append-only doc conflicts, and runs the final gate.
