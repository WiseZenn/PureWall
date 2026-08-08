# PureWall Phase 2 Playback Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Route every primary playback entry point through one tested application action contract, restore persisted playback state consistently, and make playback failures and multi-display accounting verifiable.

**Architecture:** Add a small Rust action vocabulary and pure dispatcher while keeping Tauri command registration and application orchestration in `main.rs` as required by ADR-015. Existing commands remain as compatibility facades, but main-window, tray, widget, context-menu/CLI, and automatic rotation all delegate to the same executor and emit the existing state events. Frontend primary controls consume one typed command; gallery-item rating commands remain separate because they target an explicitly selected file rather than the current playback item.

**Tech Stack:** Tauri 2, Rust, serde, rusqlite, Vue 3, Pinia, TypeScript, Vitest

## Global Constraints

- PureWall only; PureWall-X is explicitly deferred.
- A normal available wallpaper has one random-selection ticket; a liked available wallpaper has two tickets.
- Disliked, hidden, unavailable, and pending-deletion wallpapers are excluded from playback.
- Multi-display selection samples without replacement whenever enough unique eligible wallpapers exist.
- Manual Next and automatic rotation use the same eligibility and weighting contract.
- Preserve all existing public Tauri command names as compatibility facades; do not change the SQLite schema.
- Do not add dependencies, global hotkeys, recommendation behavior, online features, or shared-core scaffolding.
- Do not execute or modify context-menu registration, autostart, HKLM, Windows policy, or the Windows 11 context-menu mode.
- Keep current wallpaper, pause, interval, and display-mode settings backward compatible.
- Every production behavior change follows RED → GREEN → REFACTOR and updates `docs/project-docs/CHANGELOG_AI.md`; append `AI_DIARY.md` only for a genuinely new pitfall.
- Preserve pre-existing dirty tracked and untracked files; stage only the task's intended delta.

---

## File Map

- Create `src-tauri/src/playback_action.rs`: stable action names, serializable outcomes, and a pure closure-driven dispatcher.
- Modify `src-tauri/src/main.rs`: real action execution, compatibility commands, startup restoration, automatic-rotation feedback, and Tauri registration.
- Modify `src-tauri/src/tray.rs`: dispatch tray actions directly through the shared backend contract.
- Modify `src-tauri/src/widget.rs`: keep legacy widget commands as facades over the shared contract.
- Create `src-tauri/src/playback_state_tests.rs`: restart-restoration integration tests against a reopened SQLite database.
- Modify `src-tauri/src/db.rs`: add playback uniqueness and per-display history regression tests only.
- Modify `src/stores/wallpapers.ts`: typed current-playback action method and outcome application.
- Modify `src/stores/wallpaperCommandRouting.test.ts`: frontend RED/GREEN routing and state tests.
- Modify `src/components/WallpaperControls.vue`: primary controls target the current playback item.
- Modify `src/components/QuietCanvas.vue`: reuse the same current-playback action path.
- Modify `src/views/WidgetView.vue`: call the shared command and render failures visibly.
- Modify `docs/project-docs/ARCHITECTURE.md`: document the stable action boundary and restoration flow.
- Modify `docs/project-docs/CHANGELOG_AI.md`: append task scope and verification evidence.
- Replace `.superpowers/sdd/progress.md` content for the Phase 2 ledger after preserving the completed Phase 1 summary.

---

### Task 1: Define the playback action contract

**Files:**
- Create: `src-tauri/src/playback_action.rs`
- Modify: `src-tauri/src/main.rs`

**Interfaces:**
- Produces: `PlaybackAction::{Next, Like, Dislike, TogglePause}`.
- Produces: `PlaybackActionOutcome { action, current_wallpaper_path, rating, paused }` with snake-case serialization.
- Produces: `dispatch_with(action, next, rate, toggle_pause) -> Result<PlaybackActionOutcome, String>`.
- Consumes: no Tauri state; the dispatcher remains unit-testable through closures.

- [ ] **Step 1: Write the failing contract tests**

Create `playback_action.rs` with a test module that imports the not-yet-defined contract and proves exact action parsing, one-and-only-one closure dispatch, stable outcomes, and error propagation:

```rust
#[cfg(test)]
mod tests {
    use super::{dispatch_with, PlaybackAction};

    #[test]
    fn action_names_parse_to_the_stable_playback_vocabulary() {
        assert_eq!(PlaybackAction::parse("next"), Some(PlaybackAction::Next));
        assert_eq!(PlaybackAction::parse("like"), Some(PlaybackAction::Like));
        assert_eq!(PlaybackAction::parse("dislike"), Some(PlaybackAction::Dislike));
        assert_eq!(PlaybackAction::parse("pause"), Some(PlaybackAction::TogglePause));
        assert_eq!(PlaybackAction::parse("toggle_pause"), Some(PlaybackAction::TogglePause));
        assert_eq!(PlaybackAction::parse("unknown"), None);
    }

    #[test]
    fn dispatcher_returns_action_specific_outcomes() {
        let next = dispatch_with(
            PlaybackAction::Next,
            || Ok("D:/walls/next.jpg".to_string()),
            |_| panic!("rating closure must not run"),
            || panic!("pause closure must not run"),
        )
        .expect("next should succeed");
        assert_eq!(next.current_wallpaper_path.as_deref(), Some("D:/walls/next.jpg"));
        assert_eq!(next.rating, None);
        assert_eq!(next.paused, None);

        let liked = dispatch_with(
            PlaybackAction::Like,
            || panic!("next closure must not run"),
            |rating| Ok(("D:/walls/current.jpg".to_string(), rating)),
            || panic!("pause closure must not run"),
        )
        .expect("like should succeed");
        assert_eq!(liked.rating, Some(1));

        let paused = dispatch_with(
            PlaybackAction::TogglePause,
            || panic!("next closure must not run"),
            |_| panic!("rating closure must not run"),
            || Ok(true),
        )
        .expect("pause should succeed");
        assert_eq!(paused.paused, Some(true));
    }

    #[test]
    fn dispatcher_preserves_the_underlying_failure() {
        let error = dispatch_with(
            PlaybackAction::Next,
            || Err("No existing wallpaper files found".to_string()),
            |_| unreachable!(),
            || unreachable!(),
        )
        .expect_err("empty playback must fail");
        assert_eq!(error, "No existing wallpaper files found");
    }
}
```

- [ ] **Step 2: Run RED and inspect the executed test count**

Run: `cargo test playback_action -- --nocapture`

Expected: compilation fails because `PlaybackAction` and `dispatch_with` do not exist; the failure must not be a syntax or module-registration error.

- [ ] **Step 3: Implement the minimal pure contract**

Add the module registration to `main.rs`, then implement:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackAction {
    Next,
    Like,
    Dislike,
    TogglePause,
}

impl PlaybackAction {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "next" => Some(Self::Next),
            "like" => Some(Self::Like),
            "dislike" => Some(Self::Dislike),
            "pause" | "toggle_pause" => Some(Self::TogglePause),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Next => "Next wallpaper",
            Self::Like => "Like current wallpaper",
            Self::Dislike => "Dislike current wallpaper",
            Self::TogglePause => "Toggle rotation pause",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PlaybackActionOutcome {
    pub action: PlaybackAction,
    pub current_wallpaper_path: Option<String>,
    pub rating: Option<i32>,
    pub paused: Option<bool>,
}
```

Implement `dispatch_with` so Next calls only `next`, Like/Dislike call only `rate(1/-1)`, TogglePause calls only `toggle_pause`, and each branch fills only its relevant outcome fields.

- [ ] **Step 4: Run GREEN and formatting**

Run: `cargo test playback_action -- --nocapture`

Expected: 3 focused tests pass and zero focused tests are filtered accidentally.

Run: `cargo fmt -- --check`

Expected: PASS.

- [ ] **Step 5: Commit the task delta only**

```text
git add src-tauri/src/playback_action.rs src-tauri/src/main.rs
git commit -m "feat(playback): define shared action contract"
```

---

### Task 2: Route every backend playback entry point through the contract

**Files:**
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/tray.rs`
- Modify: `src-tauri/src/widget.rs`

**Interfaces:**
- Consumes: Task 1 `PlaybackAction`, `PlaybackActionOutcome`, and `dispatch_with`.
- Produces: Tauri command `run_playback_action(action: PlaybackAction) -> CommandResult<PlaybackActionOutcome>`.
- Produces: `execute_playback_action(&AppState, &AppHandle, PlaybackAction) -> Result<PlaybackActionOutcome, String>`.
- Produces: `report_playback_failure(&AppHandle, source: &str, action, message)` using the existing `operation-failed` event.
- Preserves: `next_wallpaper`, `toggle_pause`, and `widget_*` command names as facades.

- [ ] **Step 1: Extend RED coverage for boundary normalization**

Add tests to `playback_action.rs` proving `pause` and `toggle_pause` normalize to the same enum and `PlaybackAction::label()` supplies non-empty user-facing labels for every variant. Add a failing `main.rs` child-module test file `src-tauri/src/playback_entrypoint_tests.rs` that expects `cli_action_from_args` to return `PlaybackAction` instead of `String`:

```rust
use crate::{cli_action_from_args, playback_action::PlaybackAction};

#[test]
fn cli_actions_use_the_shared_playback_vocabulary() {
    let args = vec!["purewall.exe".to_string(), "--action".to_string(), "like".to_string()];
    assert_eq!(cli_action_from_args(&args), Some(PlaybackAction::Like));
}

#[test]
fn unknown_cli_actions_are_rejected_before_execution() {
    let args = vec!["purewall.exe".to_string(), "--action".to_string(), "shuffle".to_string()];
    assert_eq!(cli_action_from_args(&args), None);
}
```

Register the test module under `#[cfg(test)]` before running RED.

- [ ] **Step 2: Run RED**

Run: `cargo test playback_entrypoint_tests -- --nocapture`

Expected: type/assertion failure because `cli_action_from_args` still returns an arbitrary `String`.

- [ ] **Step 3: Implement one real executor and compatibility facades**

In `main.rs`:

1. Change `cli_action_from_args` to parse through `PlaybackAction::parse`.
2. Add `execute_playback_action` using `dispatch_with` and the existing `advance_wallpaper`, `set_current_wallpaper_rating`, and pause helpers.
3. Emit `auto-rotated`, `wallpaper-rating-changed`, and `pause-changed` only from the shared underlying operations; do not add parallel event names.
4. Add `run_playback_action` as the typed Tauri command.
5. Make `next_wallpaper` and `toggle_pause` delegate to the executor and return their legacy scalar results.
6. Harden `set_current_wallpaper_rating`: load the persisted current path, validate it with `require_registered_wallpaper_file`, then update rating and emit the existing event.
7. Make automatic rotation call `execute_playback_action(..., PlaybackAction::Next)`. On error, call `report_playback_failure` so an empty/missing library is not silently swallowed.
8. Classify known candidate/current-path failures into a stable `playback_unavailable` `CommandError`; preserve `operation_failed` for platform/COM errors.

In `tray.rs`, replace per-action logic and `tray-like`/`tray-dislike` frontend relays with one call to `execute_playback_action`. On failure call `report_playback_failure(app, "Tray", action, message)`.

In `widget.rs`, keep `widget_next`, `widget_like`, and `widget_dislike`, but make each delegate to `execute_playback_action` and extract its legacy return value.

In CLI forwarding, pass the parsed enum into the same executor, keep `purewall-cli.log`, and emit the existing visible failure event on failure. Context-menu actions automatically follow this path because their registered command line remains `--action next|like|dislike|pause`; do not run or modify registry registration.

- [ ] **Step 4: Run focused GREEN and backend verification**

Run: `cargo test playback_action -- --nocapture`

Expected: all focused action-contract tests pass with non-zero executed counts.

Run: `cargo test playback_entrypoint_tests -- --nocapture`

Expected: all focused contract and CLI parser tests pass with non-zero executed counts.

Run: `cargo check`

Expected: PASS.

Run: `cargo clippy --all-targets -- -D warnings`

Expected: PASS.

- [ ] **Step 5: Commit the task delta only**

```text
git add src-tauri/src/main.rs src-tauri/src/tray.rs src-tauri/src/widget.rs src-tauri/src/playback_action.rs src-tauri/src/playback_entrypoint_tests.rs
git commit -m "fix(playback): unify backend action entry points"
```

---

### Task 3: Route primary frontend controls and show widget failures

**Files:**
- Modify: `src/stores/wallpapers.ts`
- Modify: `src/stores/wallpaperCommandRouting.test.ts`
- Modify: `src/components/WallpaperControls.vue`
- Modify: `src/components/QuietCanvas.vue`
- Modify: `src/views/WidgetView.vue`

**Interfaces:**
- Consumes: `run_playback_action` and Task 1 outcome shape.
- Produces: TypeScript `PlaybackAction` and `PlaybackActionOutcome` types.
- Produces: store methods `runPlaybackAction`, `likeCurrentWallpaper`, and `dislikeCurrentWallpaper`.
- Preserves: `nextWallpaper` and `togglePause` store methods as UI compatibility facades.
- Preserves: path-targeted `like(path)`, `dislike(path)`, and `resetRating(path)` for gallery and inspector item management.

- [ ] **Step 1: Write the failing store routing tests**

Extend `wallpaperCommandRouting.test.ts`:

```ts
it("routes current playback actions through one typed command", async () => {
  invokeMock.mockImplementation(async (command: string) => {
    if (command === "run_playback_action") {
      return {
        action: "like",
        current_wallpaper_path: "D:/walls/current.jpg",
        rating: 1,
        paused: null,
      };
    }
    if (command === "get_stats") return emptyStats;
    return undefined;
  });
  const store = useWallpaperStore();

  await store.likeCurrentWallpaper();

  expect(invokeMock).toHaveBeenCalledWith("run_playback_action", { action: "like" });
  expect(store.currentWallpaperPath).toBe("D:/walls/current.jpg");
});

it("applies pause outcomes from the shared action command", async () => {
  invokeMock.mockResolvedValue({
    action: "toggle_pause",
    current_wallpaper_path: null,
    rating: null,
    paused: true,
  });
  const store = useWallpaperStore();

  await store.togglePause();

  expect(store.isPaused).toBe(true);
  expect(invokeMock).toHaveBeenCalledWith("run_playback_action", { action: "toggle_pause" });
});
```

- [ ] **Step 2: Run RED**

Run: `npm run test:unit -- src/stores/wallpaperCommandRouting.test.ts`

Expected: fails because the current-playback methods do not exist and pause still invokes `toggle_pause`.

- [ ] **Step 3: Implement the typed store action**

Add exact frontend types:

```ts
export type PlaybackAction = "next" | "like" | "dislike" | "toggle_pause";

interface PlaybackActionOutcome {
  action: PlaybackAction;
  current_wallpaper_path: string | null;
  rating: number | null;
  paused: boolean | null;
}
```

Implement `runPlaybackAction(action)` to invoke the shared command and apply every non-null field. For a returned current path, update current/active paths, request preview/metadata, refresh the row, and refresh stats/history. For a returned rating, patch the current row. For a returned pause state, update `isPaused`. Keep failure reporting through `reportFailure`.

Make `nextWallpaper`, `togglePause`, `likeCurrentWallpaper`, and `dislikeCurrentWallpaper` thin calls into `runPlaybackAction`. Remove the obsolete `tray-like` and `tray-dislike` listeners; tray rating completion arrives through `wallpaper-rating-changed`.

Update `WallpaperControls.vue` and `QuietCanvas.vue` so playback Like/Dislike targets the current item through the new methods rather than whichever gallery row is selected. Keep card/inspector path-targeted rating behavior unchanged.

Change `WidgetView.vue` to invoke `run_playback_action` with `{ action }`. Render a compact `role="status"` error message from the serialized command error; clear it on the next successful action. Keep `hide_widget` separate because it is a window action, not playback.

- [ ] **Step 4: Run GREEN and frontend verification**

Run: `npm run test:unit -- src/stores/wallpaperCommandRouting.test.ts`

Expected: all store command-routing tests pass.

Run: `npx vue-tsc --noEmit`

Expected: PASS.

Run: `npm run build`

Expected: PASS; only previously documented upstream annotation warnings may remain.

- [ ] **Step 5: Commit the task delta only**

```text
git add src/stores/wallpapers.ts src/stores/wallpaperCommandRouting.test.ts src/components/WallpaperControls.vue src/components/QuietCanvas.vue src/views/WidgetView.vue
git commit -m "fix(ui): share current playback action path"
```

---

### Task 4: Verify restart restoration through one loader

**Files:**
- Modify: `src-tauri/src/main.rs`
- Create: `src-tauri/src/playback_state_tests.rs`

**Interfaces:**
- Produces: `RestoredPlaybackState { current_wallpaper_path, manual_paused, rotation_secs, display_mode }`.
- Produces: `load_restored_playback_state(&db::Database) -> Result<RestoredPlaybackState, String>`.
- Preserves: default interval `600`, maximum interval `86_400`, default display mode `all`, and legacy pause-file migration.

- [ ] **Step 1: Write the failing reopened-database tests**

Create a child test module that writes settings, drops/reopens the database, and expects the loader to restore them:

```rust
use crate::{db::Database, load_restored_playback_state};

#[test]
fn restart_restores_current_pause_interval_and_display_mode() {
    let (db_path, wallpaper_path) = create_registered_temp_wallpaper("restart-state");
    {
        let db = Database::new(&db_path).expect("database should initialize");
        db.upsert_wallpaper(&wallpaper_path, "hash", "test", 100, 100, 10).unwrap();
        db.set_setting("current_wallpaper", &wallpaper_path).unwrap();
        db.set_setting("paused", "true").unwrap();
        db.set_setting("rotation_secs", "1800").unwrap();
        db.set_setting("display_mode", "independent").unwrap();
    }

    let reopened = Database::new(&db_path).expect("database should reopen");
    let restored = load_restored_playback_state(&reopened).expect("settings should restore");
    assert_eq!(restored.current_wallpaper_path.as_deref(), Some(wallpaper_path.as_str()));
    assert!(restored.manual_paused);
    assert_eq!(restored.rotation_secs, 1800);
    assert_eq!(restored.display_mode, "independent");
}
```

Add a second test proving malformed values fall back to `600`/`all` and an oversized interval clamps to `86_400`. The helper must delete only its own temporary database and image files.

- [ ] **Step 2: Run RED**

Run: `cargo test playback_state_tests -- --nocapture`

Expected: compilation fails because the restored-state type and loader do not exist.

- [ ] **Step 3: Implement and integrate the loader**

Move `MAX_ROTATION_SECS` beside the playback setting constants. Implement one loader that trims a non-empty current path, parses pause with `parse_bool_setting`, parses/clamps interval with the existing defaults, and validates display mode with `is_valid_display_mode`.

Use the loaded struct in `.setup()` for protected-cache sources and `AppState` initialization. Preserve the existing legacy `paused.txt` migration after the database value is loaded, and persist `true` when that migration fires. Do not reapply or delete a missing wallpaper at startup; `bootstrap_active_wallpaper` remains responsible for returning no active row when the source is unavailable.

- [ ] **Step 4: Run GREEN and regression checks**

Run: `cargo test playback_state_tests -- --nocapture`

Expected: restart restoration tests pass with non-zero counts.

Run: `cargo test active_preview_tests -- --nocapture`

Expected: restart tests and existing current-wallpaper bootstrap tests pass with non-zero counts.

Run: `cargo check`

Expected: PASS.

- [ ] **Step 5: Commit the task delta only**

```text
git add src-tauri/src/main.rs src-tauri/src/playback_state_tests.rs
git commit -m "test(playback): verify restart restoration"
```

---

### Task 5: Lock multi-display uniqueness and play-history accounting

**Files:**
- Modify: `src-tauri/src/db.rs`

**Interfaces:**
- Consumes: existing `get_next_wallpapers`, `record_play_for_display`, and `get_yearly_stats`.
- Produces: regression evidence only; no schema or public database API changes.

- [ ] **Step 1: Add failing precision tests**

Add two database tests:

1. Insert three available files and request three paths; assert length `3` and unique-set length `3`. Then retain only two eligible files, request three paths, and assert length `3` with unique-set length `2`.
2. Insert two wallpapers, call `record_play_for_display` once for `DISPLAY-A` and once for `DISPLAY-B`, then assert the current year's `YearlyStats.total_plays == 2`, `unique_wallpapers == 2`, each top row has one play, and a private test query sees two distinct non-null display IDs.

Use real temporary files because selection intentionally performs `Path::exists()` as a defensive guard.

- [ ] **Step 2: Run the focused tests and establish RED or characterization evidence**

Run: `cargo test multi_display -- --nocapture`

Expected: if an assertion exposes a bug, retain the expected failure before the minimal fix; if existing behavior already passes, record the tests as characterization coverage and do not change production SQL unnecessarily.

- [ ] **Step 3: Make only the minimal correction if RED exposed one**

Do not change the approved 2x ticket weights. Do not add per-monitor duplicate suppression outside `get_next_wallpapers`; uniqueness belongs at selection. Do not record history until the corresponding Windows wallpaper application succeeds. If the characterization tests are already GREEN, skip production changes.

- [ ] **Step 4: Run GREEN plus the existing eligibility regressions**

Run: `cargo test multi_display -- --nocapture`

Expected: the two new multi-display tests pass with non-zero counts.

Run: `cargo test next_wallpapers_exclude_negative_hidden_and_unavailable_rows -- --nocapture`

Expected: the persisted eligibility regression passes.

Run: `cargo test playback_sampling_excludes_disliked -- --nocapture`

Expected: all focused tests pass with non-zero counts.

- [ ] **Step 5: Commit the task delta only**

```text
git add src-tauri/src/db.rs
git commit -m "test(playback): lock multi-display accounting"
```

---

### Task 6: Complete Phase 2 verification and process documentation

**Files:**
- Modify: `docs/project-docs/ARCHITECTURE.md`
- Modify: `docs/project-docs/CHANGELOG_AI.md`
- Modify only if needed: `docs/project-docs/AI_DIARY.md`
- Modify: `.superpowers/sdd/progress.md`

**Interfaces:**
- Documents: one action vocabulary, compatibility facades, event feedback, restart restoration, and play-history guarantees.
- Produces: final verification evidence and unresolved runtime items.

- [ ] **Step 1: Update architecture and the durable SDD ledger**

Document that:

- `playback_action.rs` owns stable action names and the pure dispatcher;
- `main.rs` remains the Tauri/orchestration boundary and owns the real executor;
- main UI, tray, widget, CLI/context menu, and timer share the executor;
- `auto-rotated`, `wallpaper-rating-changed`, `pause-changed`, and `operation-failed` remain the cross-window event contract;
- restart loading restores current path, manual pause, bounded interval, and valid display mode;
- gallery-item rating remains path-targeted and separate from current-playback actions.

Initialize the Phase 2 ledger with branch, merge base, baseline test counts, and Task 1–6 status. Preserve a one-line pointer that Phase 1 completed and merged at `b61353a`.

- [ ] **Step 2: Run the full completion gate**

Run in this order:

```text
cargo fmt -- --check
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
npx vue-tsc --noEmit
npm run test:unit
npm run build
git diff --check
```

Expected: every command exits `0`; inspect Rust and Vitest executed test counts instead of relying on exit codes alone.

- [ ] **Step 3: Perform runtime-oriented QA without changing external state**

Use the production frontend/browser harness or an interactive dev build to verify:

- main-window Next/Like/Dislike/Pause reaches the shared command;
- an empty library produces visible failure feedback;
- widget failures render inside the widget rather than only in DevTools;
- 800x600 and normal desktop sizes keep all primary controls reachable;
- no registry, autostart, Windows policy, wallpaper source file, or system setting is modified by QA.

If Tauri runtime testing would actually change the Windows wallpaper, do not automate it without explicit scope; retain the Rust/store/event tests and record the manual runtime item as unresolved.

- [ ] **Step 4: Append the final CHANGELOG entry**

Record task commits, action-entry coverage, restart settings, multi-display evidence, full gate outputs, runtime QA availability, and these safety statements:

- no registry command executed;
- no PureWall-owned registry entry changed;
- no HKLM/policy/system setting changed;
- no SQLite migration or wallpaper source deletion occurred.

Append `AI_DIARY.md` only if implementation revealed a new, reusable pitfall not already covered by existing entries.

- [ ] **Step 5: Final review and branch completion**

Generate a whole-branch review package from the true merge base, dispatch the final code reviewer, fix every Critical/Important finding in one fix wave, rerun covering tests, and re-review. When clean, use `finishing-a-development-branch` and present the standard integration options; do not push or merge without the user's selected option.

