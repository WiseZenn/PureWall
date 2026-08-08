# PureWall Phase 2 Final Unwind-Test Fix Report

- **Baseline:** `97a92aa`
- **Review input:** `.superpowers/sdd/phase-2-final-minor-review.md`
- **Review finding:** `0 Critical / 0 Important / 1 Minor`
- **Fix commit:** `test(phase2): authenticate cleanup unwind` (SHA reported after commit)
- **Final disposition:** `0 Critical / 0 Important / 0 Minor` outstanding for this follow-up

## Result

The cleanup regression no longer accepts setup failure as proof of RAII cleanup. All setup and precondition checks run outside `catch_unwind`; the captured closure unwinds only after moving the guard and raising a fixed sentinel. The captured payload is authenticated before the five cleanup postconditions are evaluated.

## Finding and genuine RED

The old test performed guard construction and all five writes inside `catch_unwind`, accepted only `unwind.is_err()`, then checked absence. A setup `.expect` panic could satisfy both observations without reaching the intended cleanup panic.

The RED patch added the missing exact-payload contract while the old panic remained unchanged.

Command:

`cargo test playback_entrypoint_tests::temp_playback_files_remove_database_sidecars_and_images_during_unwind -- --nocapture`

Observed result: FAIL, 0 passed / 1 failed. The assertion reported:

- left: `"exercise panic-safe temporary playback cleanup"`
- right: `"purewall-temp-playback-cleanup-sentinel"`

This failure demonstrates that the previous test authenticated no intended panic; it merely caught whichever panic occurred first.

## Minimal GREEN

Final test name:

`temp_playback_files_remove_precreated_artifacts_during_expected_unwind`

The corrected sequence is:

1. Construct unique database, `db-shm`, `db-wal`, and two image paths.
2. Construct the `TempPlaybackFiles` guard outside `catch_unwind`.
3. Write all five artifacts outside the catch and assert every path exists.
4. Move the guard into the catch closure.
5. Raise only `panic_any(CLEANUP_PANIC_SENTINEL)` inside that closure.
6. Require `expect_err`, downcast either `&str` or `String`, and compare the payload exactly to the sentinel.
7. Assert all five paths are absent after the authenticated unwind.

The local `_files` owner is initialized before the sentinel panic and therefore runs `Drop` during that unwind. The separate partial-display regression still declares the same guard before its later-created `Database`, preserving the database-before-files reverse drop order on assertions and panics.

Focused GREEN:

- Authenticated unwind cleanup regression: PASS, 1/1.
- Original partial-display regression: PASS, 1/1.

## New project memory

Appended `#testing-001` to `AI_DIARY.md`: `catch_unwind` establishes only that some panic happened. Setup failures must escape the catch, and a deliberate panic must carry an exact authenticated sentinel before cleanup absence counts as evidence.

## Full completion gate

Run after the test and diary correction:

- Pre-flight `cargo check` — PASS.
- Pre-flight `npx vue-tsc --noEmit` — PASS.
- `cargo fmt -- --check` — PASS.
- `cargo check` — PASS.
- `cargo clippy --all-targets -- -D warnings` — PASS.
- `cargo test` — PASS: 76 passed, 0 failed, 1 ignored manual performance benchmark.
- `npx vue-tsc --noEmit` — PASS.
- `npm run test:unit` — PASS: 7 files, 26 tests.
- `npm run build` — PASS: 158 modules transformed; only the documented third-party `@vueuse/core` Rolldown `INVALID_ANNOTATION` warnings.
- `git diff --check` — PASS before documentation and rerun against the complete staged and post-commit whole-range diffs.

## Scope, safety, and runtime disposition

- Test code only: `src-tauri/src/playback_entrypoint_tests.rs`.
- Append-only memory/evidence: `AI_DIARY.md`, `CHANGELOG_AI.md`, and this report.
- No production Rust, Vue/TypeScript, dependency, capability, SQLite schema, selector, event vocabulary, ADR, or architecture file changed.
- All edits used checked unified diffs under `C:\tmp`, applied with `git apply --check --recount` followed by `git apply --recount`.
- No registry/system-setting/context-menu/autostart action, real wallpaper apply, COM injection, Tauri desktop session, WebView lifecycle manipulation, source mutation, or application database mutation occurred.
- No real desktop QA was run. The prior user-approved Windows integration and visual smoke items remain unchanged and are not claimed as PASS.
