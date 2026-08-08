# PureWall Phase 4 Interaction and Layout Implementation Plan

> **Execution:** Directly authorized by the user on 2026-07-30. Use test-driven development,
> the existing isolated worktree, and the project Pre-/Post-Flight checklists.

**Goal:** Make first-run import, state feedback, playback hierarchy, keyboard focus, semantic images,
and minimum-window behavior clear and consistent without changing PureWall's backend contracts.

**Architecture:** Keep the accepted Living Gallery/AppShell/Inspector architecture. Add only focused
frontend presentation helpers and tests. Do not add a router, backend command, database migration,
dependency, registry behavior, or system-setting operation.

**Branch:** `codex/purewall-phase-4`

**Worktree:** `D:\Desktop\PureWall\.worktrees\purewall-phase-3`

**Phase 3 carry-forward:** Native source/backup QA and GitHub Windows CI remain incomplete. Phase 4
work must not rewrite those statuses or claim they passed.

## Design contract

- First run has one dominant **Choose wallpaper folder** action, a concise local-only statement, and
  a lower-emphasis individual-image action.
- Library bootstrap loading, empty library, and filtered-empty results remain distinct states.
- Success/info notifications are polite status messages; failures are assertive alerts.
- The stage dock contains Next, Like, Dislike, Pause/Resume, and rotation interval. Display mode,
  Focus Pause, and Insights stay in their existing system sections.
- Opening an inspector remembers its opener; closing restores focus to that opener with the main
  workspace as a fallback.
- System panels use semantic headings and receive focus when opened.
- Wallpaper accessible labels prefer a user-defined display title, then a tag/generic fallback.
- The virtual gallery exposes grid/row/cell semantics.
- Compact-height rules complement width breakpoints; required actions remain reachable at 800×600.
- Source and backup paths wrap under enlarged text instead of relying only on single-line ellipsis.
- Browser zoom/device-scale stress is labeled simulated and is never reported as real Windows DPI.

---

### Task 1: First-run and application-state contract — COMPLETE

**Files:**

- Create: `src/components/workspacePresentation.ts`
- Create: `src/components/workspacePresentation.test.ts`
- Modify: `src/components/AppShell.vue`
- Modify: `src/stores/wallpapers.ts`
- Modify: `src/components/EmptyState.vue`
- Modify: `src/components/NotificationCenter.vue`
- Modify: `src/components/appShell.test.ts`
- Modify: `src/styles.css`

**RED**

- Add pure tests proving bootstrap loading is distinct from an empty library.
- Add SSR assertions for the local-only statement, one dominant folder action, busy semantics, and
  tone-specific live-region behavior.
- Run the focused tests and confirm they fail for the missing contract.

**GREEN**

- Add the smallest presentation helper for `loading | empty | gallery`.
- Render a stable loading surface before first-load completion.
- Update first-run copy and import busy states without adding tutorial steps.
- Use polite `status` semantics for success/info and assertive `alert` semantics only for errors.
- Add only the styles required by the state surface.

**Verify**

```powershell
npx vitest run src/components/workspacePresentation.test.ts src/components/appShell.test.ts
npx vue-tsc --noEmit
```

---

### Task 2: Restore playback-control dominance — COMPLETE

**Files:**

- Create: `src/components/currentWallpaperPanel.test.ts`
- Modify: `src/components/CurrentWallpaperPanel.vue`

**RED**

- Add a contract test that the stage exposes the four primary playback actions plus rotation interval,
  while display mode and Focus Pause remain system-panel concerns.
- Confirm the focused test fails against the current stage composition.

**GREEN**

- Remove display-mode and Focus Pause controls from the stage dock.
- Keep the existing rotation interval command and all four playback commands unchanged.
- Remove now-unused imports and compact obsolete stage-focus styles without broad CSS cleanup.

**Verify**

```powershell
npx vitest run src/components/stageControlModel.test.ts
npx vue-tsc --noEmit
```

---

### Task 3: Keyboard focus and semantic wallpaper coverage — COMPLETE

**Files:**

- Create: `src/composables/useInspector.test.ts`
- Modify: `src/composables/useInspector.ts`
- Modify: `src/components/Sidebar.vue`
- Modify: `src/components/TitleBar.vue`
- Modify: `src/components/WallpaperCard.vue`
- Modify: `src/components/WallpaperGrid.vue`
- Modify: `src/components/InspectorPanel.vue`
- Modify: `src/styles.css`
- Modify: `src/utils/wallpaperPresentation.ts`
- Create: `src/utils/wallpaperPresentation.test.ts`
- Create: `src/components/semanticSurfaces.test.ts`

**RED**

- Add tests for remembered opener, focus restoration, fallback focus, display-title-first accessible
  labels, and generic fallback labels.
- Add SSR assertions for semantic system headings and grid/row/cell structure.
- Confirm the focused tests fail for the intended missing behavior.

**GREEN**

- Extend the inspector composable with opener capture and bounded focus restoration.
- Pass actual triggering elements from sidebar and wallpaper cards.
- Focus system inspectors when opened and restore focus when closed.
- Replace visual-only system titles with semantic headings.
- Add grid/row/cell roles and accessible card labels without nesting new buttons.
- Prefer the display title in `wallpaperAccessibleLabel`.

**Verify**

```powershell
npx vitest run src/composables/useInspector.test.ts src/utils/wallpaperPresentation.test.ts src/components/appShell.test.ts
npx vue-tsc --noEmit
```

---

### Task 4: Minimum-window, scaling, and long-path resilience — COMPLETE

**Files:**

- Modify: `src/styles.css`
- Modify: `src/components/LibrarySourcesSettings.vue`
- Modify: `src/components/LibraryBackupSettings.vue`
- Create: `src/components/responsiveContract.test.ts`

**RED**

- Add contract assertions for compact-height rules, required-action preservation, and wrapping source/
  backup path content.
- Confirm the focused test fails before the style changes.

**GREEN**

- Add a compact-height breakpoint aligned with the 800×600 minimum.
- Preserve playback and import-folder actions while allowing secondary metadata to simplify.
- Keep side rail and system drawers independently scrollable.
- Replace single-line ellipsis with bounded wrapping for user-visible filesystem paths.
- Keep reduced-motion behavior intact.

**Verify**

```powershell
npx vitest run src/components/responsiveContract.test.ts
npx vue-tsc --noEmit
npm run build
```

---

### Task 5: Browser QA — COMPLETE

**Artifacts:** ignored local `output/playwright/phase4/`

- Start one owned Vite process only if port 1420 is free.
- Test 1200×800 and 800×600 in dark and light themes.
- Stress a fixed 800×600 physical target at 1.25 and 1.5 simulated scale by reducing the CSS viewport to 640×480 and 533×400; compare CSS boxes directly with CSS viewport bounds.
- Verify onboarding/loading/filter-empty surfaces without invoking native dialogs.
- Verify Tab, Shift+Tab, Enter, Space, arrow navigation, Escape, focus entry, and focus return wherever
  the browser harness can exercise the behavior.
- Record console/network limitations caused by the absent Tauri bridge.
- Stop only the owned Vite/Playwright processes.

**Acceptance**

- No required playback or import-folder action is clipped.
- No horizontal app-shell overflow at the tested viewports.
- System drawers and long paths remain reachable by scrolling.
- Success/info messages are polite; errors are assertive.
- With no committed baseline, visual regression status remains INCONCLUSIVE.

---

### Task 6: Full gate, review, and process documentation — COMPLETE

**Files:**

- Modify: `docs/project-docs/CHANGELOG_AI.md`
- Modify: `docs/project-docs/AI_DIARY.md` only for genuinely new reusable pitfalls
- Modify: this plan's execution status

**Full verification**

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npx vue-tsc --noEmit
npm run test:unit
npm run build
git diff --check
git status --short
```

**Review and closeout**

- Review only `Phase 4 base..HEAD`.
- Classify Critical, Important, and Minor findings.
- Fix verified Critical/Important findings with focused RED/GREEN tests.
- Record any unrun native Windows DPI evidence as NOT RUN/INCONCLUSIVE, never PASS.
- Append exact test counts and browser evidence to `CHANGELOG_AI.md`.
- Append new pitfalls to `AI_DIARY.md` only when discovered.
- Keep Phase 3 native/remote blockers unchanged.
- Do not merge, push, create a PR, remove a worktree, or modify system settings without separate scope.

**Execution status (2026-08-01)**

- Fresh full gate: PASS — Rust formatting/check/test/Clippy, TypeScript, frontend unit tests,
  frontend production build, and Git whitespace/status checks all passed.
- Independent review covered `d16f541..f416be6` and both subsequent fix increments. The initial
  0 Critical / 0 Important / 3 Minor findings were all corrected; final incremental re-review
  reports 0 Critical / 0 Important / 0 Minor. Task 6 is complete.
- Browser QA: mocked browser-shell matrix PASS 9/9, including a fixed 800×600 physical target
  simulated as 640×480 CSS at 1.25x and 533×400 CSS at 1.5x. This is not real Windows DPI.
- Carry-forward evidence is unchanged: real Windows DPI NOT RUN; screenshot regression INCONCLUSIVE
  without a committed baseline; Phase 3 native source/backup QA BLOCKED/NOT RUN; GitHub Windows CI
  NOT RUN until an authorized push.
