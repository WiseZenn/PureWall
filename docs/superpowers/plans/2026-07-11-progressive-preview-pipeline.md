# Progressive Active Preview Pipeline Implementation Plan

> **Execution rule:** Complete tasks in order. For every behavior change, write the named failing test first, run it and observe the expected failure, then make the smallest implementation pass. Do not batch RED tests with unrelated production edits.

**Goal:** Make PureWall restore and prepare the real current wallpaper preview before gallery startup, show a stable thumbnail fallback on the first cold load, eliminate same-source duplicate decoding, and accelerate Windows derivatives with WIC.

**Architecture:** A bootstrap command owns current-wallpaper restoration and initial cache status. A path-keyed media queue merges thumbnail/preview needs and exposes a reserved preview lane. One decoder boundary prefers WIC target-sized decode on Windows and falls back to `image`. Vue consumes one explicit active-media state across all large-preview surfaces.

**Tech stack:** Rust, Tauri 2, Windows Imaging Component via `windows` 0.60, Vue 3, Pinia, TypeScript, Vitest.

**Design:** `docs/superpowers/specs/2026-07-11-progressive-preview-pipeline-design.md`
**Decision:** `docs/project-docs/DECISIONS.md` ADR-023

---

## Task 1: Add a testable frontend active-media state

**Files:**
- Modify: `package.json`
- Modify: `package-lock.json`
- Create: `src/stores/activeMedia.ts`
- Create: `src/stores/activeMedia.test.ts`

1. Add Vitest as a development dependency and a `test:unit` script.
2. RED: test that a cold current wallpaper immediately exposes its thumbnail, switches only after the matching preview reports loaded, ignores a late event for the previous path, and keeps the thumbnail after preview failure.
3. Run `npm run test:unit -- activeMedia.test.ts` and confirm it fails because the state reducer does not exist.
4. Implement a small framework-independent state reducer with `idle | thumbnail | preview | error` phases and a monotonically increasing generation token.
5. Re-run the focused test and `npx vue-tsc --noEmit`.

## Task 2: Restore the persisted current wallpaper before gallery pagination

**Files:**
- Create: `src-tauri/src/active_preview.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src/stores/wallpapers.ts`
- Modify: `src/views/Home.vue`
- Modify: `src/stores/activeMedia.test.ts`

1. RED (Rust): test a bootstrap helper for valid persisted path + preview hit, valid path + thumbnail-only hit, and stale/missing persisted path.
2. Run `cargo test active_preview` and confirm the helper is missing.
3. Implement serializable `ActiveWallpaperBootstrap { path, thumbnail_path, preview_path, preview_state }` and a command that validates through the DB/cache, returns immediately, and enqueues a preview miss without decoding.
4. RED (frontend): test/apply a startup function where bootstrap resolves before the gallery response and the gallery first row cannot replace the restored path.
5. Update `Home.vue` startup order: install listeners, bootstrap active wallpaper, then run gallery/tags/collections/stats/runtime queries concurrently.
6. Remove the implicit first-row override when a valid bootstrap path exists outside the current page.
7. Run `cargo test active_preview`, `npm run test:unit -- activeMedia.test.ts`, and `npx vue-tsc --noEmit`.

## Task 3: Prewarm the large preview after every wallpaper change

**Files:**
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/widget.rs`
- Modify: `src-tauri/src/media_queue.rs`
- Modify: `src-tauri/src/active_preview.rs`

1. Extract a single `prewarm_active_preview(path, queue, cache)` boundary.
2. RED: test that a cache hit creates no job and a miss enqueues an active-preview request exactly once.
3. Call the boundary only after a successful wallpaper change from manual set, next/rotation, widget, tray/single-instance, and CLI action paths.
4. Ensure prewarming never holds the SQLite mutex during decode or encoding.
5. Run focused Rust tests, then `cargo check`.

## Task 4: Merge media work by normalized source path

**Files:**
- Modify: `src-tauri/src/media_queue.rs`

1. RED: replace kind-keyed tests with path-keyed tests covering queued thumbnail upgraded to preview, preview plus thumbnail merge, running-job pending needs, duplicate idempotence, newest active preview priority, backlog cap, and shutdown.
2. Run `cargo test media_queue` and observe failures against the old `(MediaJobKind, path)` implementation.
3. Implement `MediaNeeds`, `MediaPriority`, path-keyed queued/running state, upgrade/move-to-front behavior, and a `finish` result that indicates whether pending needs require another pass.
4. Keep path normalization at the command validation boundary so queue identity is stable and case-insensitive on Windows.
5. Run `cargo test media_queue` and `cargo clippy --all-targets -- -D warnings`.

## Task 5: Reserve execution capacity for active previews

**Files:**
- Modify: `src-tauri/src/media_queue.rs`
- Modify: `src-tauri/src/main.rs`

1. RED: test that a preview-only consumer does not claim thumbnail work and that general consumers prefer preview work.
2. Introduce one `PreviewReserved` worker and two `General` workers; concentrate the counts in named constants.
3. Change the worker loop to process merged needs and emit derivative-specific events from one job.
4. Add timing fields for queue wait, decode/resize/encode, and total work in debug logs without holding DB locks.
5. Run `cargo test media_queue`, `cargo check`, and `cargo clippy --all-targets -- -D warnings`.

## Task 6: Decode once and produce all requested derivatives

**Files:**
- Modify: `src-tauri/src/thumbnails.rs`
- Modify: `src-tauri/src/main.rs`

1. RED: test that a combined thumbnail+preview request opens the source once through an injectable decoder seam and produces both correctly bounded JPEGs.
2. Add `generate_derivatives(source, needs)` which checks each cache hit, performs at most one source decode for remaining `image`-backend outputs, writes each file atomically, and returns per-derivative outcomes.
3. Preserve 512/q88 and 1440/q92 visual profiles.
4. Delete the frontend fallback behavior that independently requests a thumbnail after a preview miss; a single request must express both needs.
5. Run `cargo test thumbnails`, the frontend unit tests, and `npx vue-tsc --noEmit`.

## Task 7: Add Windows WIC target-sized decode with fallback

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/image_decoder.rs`
- Create: `src-tauri/src/windows_image_decoder.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/thumbnails.rs`

1. Add only the required WIC/COM `windows` features.
2. RED: define decoder contract tests for target bounds, portrait orientation, and explicit fallback when the primary decoder reports unsupported input.
3. Implement the portable `image` backend first behind the contract and keep tests green.
4. Implement WIC decoder creation, `IWICBitmapSourceTransform` target-size request, scaler fallback, pixel format conversion, JPEG encoding, and COM lifetime handling in the Windows-only module.
5. Ensure any WIC error is logged with stage information and automatically retries through `image` without exposing platform error codes as the only user message.
6. Run `cargo fmt --check`, `cargo test`, `cargo check`, and `cargo clippy --all-targets -- -D warnings`.

## Task 8: Unify all large-preview surfaces and avoid blank swaps

**Files:**
- Modify: `src/stores/wallpapers.ts`
- Modify: `src/components/CurrentWallpaperPanel.vue`
- Modify: `src/components/InspectorPanel.vue`
- Modify: `src/components/QuietCanvas.vue`
- Modify: `src/styles.css`
- Modify: `src/stores/activeMedia.test.ts`

1. RED: cover the store adapter that maps bootstrap/events/`img.load`/`img.error` to active-media state.
2. Make all three surfaces consume the same state and URL; none may call the backend independently.
3. Use a real `<img>` for the primary large image. Preload the next URL and commit it only on its matching `load` event.
4. Keep dimensions/aspect container stable during thumbnail-to-preview replacement; preserve thumbnail on error with a compact non-blocking message.
5. Run frontend unit tests, `npx vue-tsc --noEmit`, and `npm run build`.

## Task 9: Add bounded speculative warming and cache compatibility

**Files:**
- Modify: `src-tauri/src/active_preview.rs`
- Modify: `src-tauri/src/media_queue.rs`
- Modify: `src-tauri/src/thumbnails.rs`

1. RED: test a speculative backlog cap of two, cancellation/eviction by a new active request, protected current/active cache entries, and old-format fallback selection.
2. Warm at most two likely-next images only after the active preview is ready; never scan the full library at startup.
3. Add stale-while-revalidate lookup for a valid previous preview format while the current format regenerates.
4. Run focused and full Rust tests.

## Task 10: Performance verification, documentation, and full post-flight

**Files:**
- Modify: `docs/project-docs/ARCHITECTURE.md`
- Modify: `docs/project-docs/CHANGELOG_AI.md`
- Append if a new pitfall occurred: `docs/project-docs/AI_DIARY.md`
- Create or modify: `docs/project-docs/PERFORMANCE_PREVIEW_REVIEW.md`

1. Measure release derivative generation on one normal 20–25 MP image and at least two 90–112 MP images. Record cache hit, cold WIC, and forced `image` fallback timings.
2. Confirm steady-state startup returns the cached current preview without decode and first-ever cold startup immediately returns a thumbnail/queued state.
3. Run:
   - `cargo fmt --check`
   - `cargo test`
   - `cargo check`
   - `cargo clippy --all-targets -- -D warnings`
   - `npm run test:unit`
   - `npx vue-tsc --noEmit`
   - `npm run build`
   - `git diff --check`
4. Update architecture, changelog, performance evidence, and append-only diary entries. List any unrendered desktop QA or hardware-dependent uncertainty under unresolved items.
