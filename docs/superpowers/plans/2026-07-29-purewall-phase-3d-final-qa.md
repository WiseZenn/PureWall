# PureWall Phase 3D Final QA and Review Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the local Phase 3 quality gate with CI-equivalent automation, bounded responsive/keyboard QA, an isolated real-Tauri source/backup smoke, an independent full-branch review, and exact evidence without touching the user's real library or Windows settings.

**Architecture:** Verification is split by evidence owner. Static/unit/release commands prove build contracts; Playwright stresses the frontend shell without claiming native coverage; a separately launched release executable was planned to use dedicated temporary app data and a WebView profile for native evidence. The WebView profile isolated, but Windows Known Folder resolution ignored the process-scoped `APPDATA`; the native run was stopped and marked BLOCKED before interaction. A fresh read-only reviewer audits `origin/main@199c7dc..HEAD`; only verified Critical/Important findings may change production code.

**Tech Stack:** Rust 2021, Cargo, Tauri 2, Vue 3, TypeScript, Vitest, Vite 8, Playwright CLI, Windows WebView2, PowerShell, SQLite.

**Execution Status (2026-07-30): INCOMPLETE / BLOCKED**

- Local automated/release gate and independent code review: PASS.
- Real-Tauri source/backup acceptance journeys: BLOCKED/NOT RUN because the planned data-root isolation failed.
- GitHub Windows CI: NOT RUN. Phase 3D and Phase 3 must remain incomplete.

## Global Constraints

- Work only in `D:\Desktop\PureWall\.worktrees\purewall-phase-3` on `codex/purewall-phase-3`.
- Do not merge, push, create a PR, remove the worktree, or alter the dirty main worktree.
- Do not kill or reuse an already running user PureWall process. If one exists, mark the real-Tauri step blocked and continue independent gates.
- Do not run a QA executable against the user's real `%APPDATA%\com.purewall.app`; launch it with a new dedicated `APPDATA` root under `C:\tmp`.
- Also isolate WebView browser state with a new `WEBVIEW2_USER_DATA_FOLDER` under the same QA root.
- QA fixtures may copy only repository-owned icon assets into the dedicated QA source folders. Record SHA-256 before and after every source lifecycle operation.
- Do not invoke registration, autostart, context-menu, Recycle Bin, Windows wallpaper, policy, display-scaling, or any other system-setting command.
- Do not change Windows display scaling. Test the current real DPI once and use CSS zoom/browser viewport stress only as clearly labeled simulations.
- Native GUI automation must act only on the process ID launched by this plan and its owned file dialogs. Never use blind desktop-wide clicks.
- Preserve failed-run artifacts until evidence is captured. Cleanup may remove only the resolved QA root under `C:\tmp` after the launched process is closed.
- A browser screenshot without a committed baseline is visual evidence but not a regression PASS; report comparison as INCONCLUSIVE.
- GitHub Actions remains NOT RUN unless the user separately authorizes a push. A successful local CI mirror is not labeled as remote CI.

---

### Task 1: Run the local CI and release-packaging gate

**Files:**
- Read: `.github/workflows/ci.yml`
- Read: `src-tauri/tauri.conf.json`
- Modify: `docs/project-docs/CHANGELOG_AI.md` only in Task 5

**Interfaces:**
- Consumes: the clean Phase 3C branch at `38a2c71`.
- Produces: exact exit codes, test counts, bundle paths, hashes, dependency-audit status, and third-party warning text for Task 5.

- [x] Confirm the worktree is clean and the Playwright prerequisite exists.

```powershell
git status --short
Get-Command npx | Select-Object -ExpandProperty Source
```

Expected: no Git status entries and one resolved `npx` path.

- [x] Run the frontend gate with fail-fast checks.

```powershell
npx vue-tsc --noEmit
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
npm run test:unit
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
npm run build
```

Expected: zero type errors, every Vitest test passes, and Vite emits production assets. Record dependency warnings separately.

- [x] Run the Rust gate with the exact CI warning policy.

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo check --manifest-path src-tauri/Cargo.toml
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo test --manifest-path src-tauri/Cargo.toml
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo build --manifest-path src-tauri/Cargo.toml --release
```

Expected: formatting/check/Clippy pass, all automated tests pass with only the declared manual performance benchmark ignored, and `target/release/purewall.exe` exists.

- [x] Run the full Tauri bundler and inspect artifacts without cleaning Cargo output.

```powershell
npm run tauri build
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Get-ChildItem -LiteralPath 'src-tauri/target/release/bundle' -Recurse -File |
  Select-Object FullName, Length
Get-ChildItem -LiteralPath 'src-tauri/target/release/bundle' -Recurse -File |
  Get-FileHash -Algorithm SHA256 |
  Select-Object Path, Hash
```

Expected: release executable and configured Windows bundle artifacts exist with non-zero lengths and SHA-256 values.

- [x] Mirror the CI production dependency audit.

```powershell
npm audit --omit=dev
```

Expected: exit 0. A registry/credential/network failure is recorded as external infrastructure failure, not converted into a source-code PASS.

---

### Task 2: Run bounded browser layout, theme, and keyboard QA

**Files:**
- Read: `src/components/InspectorPanel.vue`
- Read: `src/components/LibrarySourcesSettings.vue`
- Read: `src/components/LibraryBackupSettings.vue`
- Artifact root: `output/playwright/phase3d/` (ignored/local only)

**Interfaces:**
- Consumes: the built frontend and Playwright CLI through `npx`.
- Produces: screenshots, accessibility snapshots, console/network evidence, focus-order observations, and an explicit visual-baseline status.

- [x] Start one owned Vite process on port 1420 and record its PID. If the port is occupied before launch, do not stop the owner; report the browser task blocked.

```powershell
Get-NetTCPConnection -LocalPort 1420 -State Listen -ErrorAction SilentlyContinue
$phase3dVite = Start-Process -FilePath 'npm.cmd' -ArgumentList @('run','dev','--','--host','127.0.0.1') -WorkingDirectory (Get-Location) -WindowStyle Hidden -PassThru
$phase3dVite.Id
```

Expected: no pre-existing listener and one owned process ID.

- [x] Open `http://127.0.0.1:1420` in a named Playwright session, take a fresh snapshot, and record console/network output before interaction.

```powershell
npx --yes --package @playwright/cli playwright-cli --session purewall-phase3d open http://127.0.0.1:1420
npx --yes --package @playwright/cli playwright-cli --session purewall-phase3d snapshot
npx --yes --package @playwright/cli playwright-cli --session purewall-phase3d console
npx --yes --package @playwright/cli playwright-cli --session purewall-phase3d network
```

Expected: the app shell renders. Tauri-bridge errors in a plain browser are classified as expected harness limitations; unrelated render exceptions are failures.

- [x] Resize to 1200×800 and 800×600, navigate to Settings from fresh snapshot refs, and capture dark/light screenshots plus the Library Sources and Library Backup cards.

```powershell
npx --yes --package @playwright/cli playwright-cli --session purewall-phase3d resize 1200 800
npx --yes --package @playwright/cli playwright-cli --session purewall-phase3d snapshot
npx --yes --package @playwright/cli playwright-cli --session purewall-phase3d resize 800 600
npx --yes --package @playwright/cli playwright-cli --session purewall-phase3d snapshot
npx --yes --package @playwright/cli playwright-cli --session purewall-phase3d screenshot
```

Expected: no horizontal overflow, clipped primary actions, inaccessible Settings content, or overlap at the configured minimum window size. Because there is no committed image baseline, visual-regression comparison is INCONCLUSIVE.

- [x] Stress the 800×600 Settings surface at CSS zoom 1.25 and 1.5, then restore zoom to 1.

```powershell
npx --yes --package @playwright/cli playwright-cli --session purewall-phase3d eval "document.documentElement.style.zoom='1.25'"
npx --yes --package @playwright/cli playwright-cli --session purewall-phase3d screenshot
npx --yes --package @playwright/cli playwright-cli --session purewall-phase3d eval "document.documentElement.style.zoom='1.5'"
npx --yes --package @playwright/cli playwright-cli --session purewall-phase3d screenshot
npx --yes --package @playwright/cli playwright-cli --session purewall-phase3d eval "document.documentElement.style.zoom='1'"
```

Expected: controls remain reachable through scrolling and labels do not overlap. This is layout stress, not a claim that Windows 125%/150% DPI was changed.

- [x] Use sequential `Tab`, `Shift+Tab`, `Enter`, and `Escape` from fresh snapshots to verify visible focus, logical order, Settings controls, and modal focus return wherever the plain-browser harness can reach the behavior.

Expected: every semantic control is keyboard reachable; unavailable native-dialog flows are deferred to Task 3 rather than reported PASS.

- [x] Close the Playwright session and stop only the recorded Vite process.

```powershell
npx --yes --package @playwright/cli playwright-cli --session purewall-phase3d close
Stop-Process -Id $phase3dVite.Id
```

Expected: the owned process exits and no unrelated process is stopped.

---

### Task 3: Run the isolated real-Tauri source and backup smoke

**Files:**
- Read-only fixture source: `src-tauri/icons/icon.png`
- Temporary runtime root: a new `C:\tmp\purewall-phase3d-qa-<timestamp>\`
- Temporary app data: `<qa-root>\appdata\com.purewall.app\`
- Temporary WebView profile: `<qa-root>\webview\`
- Temporary source/relocation/backup artifacts: `<qa-root>\fixtures\`

**Interfaces:**
- Consumes: `src-tauri/target/release/purewall.exe` from Task 1.
- Produces: launched PID, app-window-only screenshots, source-file hashes, test database/backup evidence, per-journey PASS/FAIL/NOT RUN, and current-window DPI.

- [x] Check for any running `purewall` process. If found, do not stop it and mark this task BLOCKED.

```powershell
Get-Process -Name purewall -ErrorAction SilentlyContinue |
  Select-Object Id, ProcessName, MainWindowTitle
```

Expected: no process rows before isolated launch.

- [x] Create a unique QA root under `C:\tmp`, two source folders, one relocation target, and repository-owned PNG fixtures. Resolve the root and assert it starts with `C:\tmp\` before any later cleanup.

- [x] Record SHA-256 for every fixture image, then launch only the release executable with process-scoped `APPDATA` and `WEBVIEW2_USER_DATA_FOLDER` pointing into the QA root.

**BLOCKED (2026-07-30):** the WebView profile was isolated, but the native window rendered the real library and no database appeared below the QA app-data target. PureWall's Tauri/`dirs` path resolution follows the Windows Known Folder result, so process-scoped `APPDATA` is not an isolation contract. Owned PID 39992 was stopped before any UI interaction; the app-window screenshot was deleted because it contained real thumbnails. The remaining native journeys are NOT RUN. See `CHANGELOG_AI.md` and `AI_DIARY.md #windows-data-dir-001`.

Recorded partial native evidence: actual Tauri window handle 9375588 and `GetDpiForWindow = 120` (Windows 125%). The full capture/resize criterion below remains incomplete because retaining the screenshot would have preserved user data.

- [ ] Capture only the launched PureWall window by its `MainWindowHandle`; record its current `GetDpiForWindow` result and set the window to 800×600 through its own Tauri title-bar controls or Win32 window bounds without changing system DPI.

- [ ] Through visible, screenshot-guided interaction, add the first source using the native directory picker; verify source status/counts and the test database exists only below the isolated `APPDATA`.

- [ ] Copy one additional repository-owned PNG into the watched folder; verify watcher-driven metadata/count refresh without restarting.

- [ ] Make the source temporarily unavailable by moving only the QA fixture directory within the QA root, verify Offline/Error plus Retry behavior, restore the owned directory, and verify Retry recovery.

- [ ] Relocate the source to the QA relocation target. Verify matched metadata survives and every original QA fixture hash remains unchanged.

- [ ] Exercise both Remove choices on QA-only sources: Keep metadata and Clear metadata. Verify the affected source image files still exist and retain their SHA-256 hashes after each choice.

- [ ] Export `purewall-backup-v1.json` into the QA root, inspect its bounded v1 envelope, preview it, confirm import, and verify the committed-success surface can coexist with warnings. Never select a real user path.

- [ ] Verify keyboard behavior in the real backup/remove dialogs: initial focus, Tab/Shift+Tab trap, Escape cancel, explicit confirmation, and focus return.

- [ ] Close only the launched PID. Record final fixture hashes and the resolved database/backup paths. Remove the QA root only after confirming the process exited and the resolved root remains below `C:\tmp`.

Expected: every completed journey mutates only the isolated database/runtime and leaves QA source images byte-identical. Any GUI automation limitation is recorded per step as NOT RUN rather than inferred.

---

### Task 4: Obtain and resolve an independent full-branch review

**Files:**
- Read: `docs/superpowers/specs/2026-07-28-purewall-phase-3-library-management-design.md`
- Read: Phase 3A/3B/3C plans under `docs/superpowers/plans/`
- Review range: `199c7dc60f9e26096d711c0e6f16b0fe7ea38463..HEAD`

**Interfaces:**
- Consumes: the complete Phase 3 branch and QA evidence from Tasks 1–3.
- Produces: a read-only reviewer verdict with file:line findings separated into Critical, Important, and Minor.

- [x] Resolve and record `BASE_SHA` and `HEAD_SHA`, then dispatch one fresh reviewer with no conversation history using the `requesting-code-review` template.

- [x] Require the reviewer to inspect plan alignment, original-file/system-setting safety, schema migration, path identity, transaction rollback, watcher lock/drop order, input bounds, selection/error behavior, backup UI semantics, and real-vs-mocked test coverage.

- [x] Validate every reviewer finding against the exact source and existing tests. Reject incorrect findings with concrete code/test evidence.

- [x] For each confirmed Critical or Important issue, stop the closeout, add a focused RED test, verify RED for the intended reason, implement the smallest correction, run focused GREEN plus the complete affected gate, update `CHANGELOG_AI.md`, and commit the fix separately.

- [x] Record Minor findings as explicit deferred items unless the correction is documentation-only and cannot broaden risk.

Review evidence (2026-07-30):

- `BASE_SHA=199c7dc60f9e26096d711c0e6f16b0fe7ea38463`; initially reviewed `HEAD_SHA=5b90a55e2755b3d0d1970d2a1270edf81e2db603`.
- One fresh read-only reviewer reported one Critical code defect, one Important code defect, one Important evidence gap, and one Minor documentation inconsistency.
- Critical confirmed: native export accepted any absolute destination and the Windows atomic install could replace a registered wallpaper. The backend now requires `.json` and rejects canonical targets matching registered wallpapers or residing below PureWall app data.
- Important code defect confirmed: confirmation re-read a path without proving it matched previewed bytes. Preview now returns a SHA-256 `contentDigest`; confirmation must send `expectedDigest`, verified before parse/merge.
- Important evidence gap retained: Task 3 native journeys remain BLOCKED/NOT RUN and are not treated as a code fix or PASS.
- Minor documentation inconsistency resolved by synchronizing the architecture and design safety contracts with the implemented destination/digest guards.
- Focused RED failed for the intended missing APIs/parameter. Focused GREEN passed 19 Rust backup tests and 10 frontend backup tests; the complete local automated/release gate also passed after the production correction.
- The same reviewer re-reviewed the uncommitted correction and reported no Critical or Important code finding; both original defects are resolved.
- Deferred Minor: canonicalizing every registered wallpaper can add substantial filesystem work at the 100,000-row bound.
- Deferred Minor evidence: destination tests do not yet enumerate Windows aliases, and no command-level spy directly proves digest mismatch skips `merge_backup`; helper/wiring tests plus source order cover the implemented boundary.
- Result: zero unresolved code-level Critical/Important findings. Task 3's Important evidence gap remains a release blocker.

Expected: zero unresolved Critical/Important findings before Task 5.

---

### Task 5: Synchronize Phase 3 status and commit local QA evidence

**Files:**
- Modify: `docs/project-docs/CHANGELOG_AI.md`
- Modify: `docs/project-docs/AI_DIARY.md` only for a genuinely new reusable pitfall
- Modify: `docs/project-docs/ARCHITECTURE.md` only if a confirmed review fix changes an architectural boundary
- Modify: `docs/superpowers/specs/2026-07-28-purewall-phase-3-library-management-design.md`
- Modify: this plan's execution status

**Interfaces:**
- Consumes: exact Task 1–4 evidence.
- Produces: an auditable local Phase 3D status that distinguishes PASS, FAIL, BLOCKED, INCONCLUSIVE, and NOT RUN.

- [x] Append exact commands, counts, artifact paths/hashes, third-party warnings, browser limitations, real-Tauri journey results, reviewer findings, and safety evidence to `CHANGELOG_AI.md`.

- [x] Mark Phase 3D complete only if every local required gate and real-Tauri acceptance journey passes with zero unresolved Critical/Important findings. Otherwise name the exact remaining gate and keep Phase 3D/Phase 3 incomplete.

- [x] Keep GitHub Windows CI as NOT RUN until a push creates a remote run. Do not equate the local mirror with acceptance criterion 8.

- [x] Run the fresh final automated gate from Task 1 after any production fix; for documentation-only closeout, rerun `git diff --check` and inspect `git status --short`.

- [x] Commit only the reviewed Phase 3D documentation delta.

Final status:

- PASS — fresh local Rust/frontend/security/release gate; 121 Rust tests passed (1 ignored manual benchmark), 49 frontend tests passed, both npm audits reported 0 vulnerabilities, and final MSI/NSIS bundles were produced.
- PASS — mocked browser-shell layout/theme/keyboard journeys at 1200×800, 800×600, and 375×667.
- INCONCLUSIVE — no committed screenshot baseline; CSS zoom stress clipped and is not evidence of real Windows DPI behavior.
- PASS — independent full-branch review after fixes has zero unresolved code-level Critical/Important findings; three Minor items are deferred.
- BLOCKED / NOT RUN — native source add/watcher/offline-retry/relocate/remove, backup round-trip, dialog keyboard, and source-hash journeys.
- NOT RUN — GitHub Windows CI, merge, push, and PR.
- Overall — Phase 3D and Phase 3 remain incomplete.

```powershell
git add -- docs/project-docs/CHANGELOG_AI.md docs/project-docs/AI_DIARY.md docs/project-docs/ARCHITECTURE.md docs/superpowers/specs/2026-07-28-purewall-phase-3-library-management-design.md docs/superpowers/plans/2026-07-29-purewall-phase-3d-final-qa.md
git diff --cached --check
git -c commit.gpgsign=false commit -m "docs(phase3): record final local QA"
```

Expected: commit succeeds, the worktree is clean, no merge/push occurs, and remaining remote/manual gates are explicit.

## Plan self-review

- Spec coverage: automated gate, release packaging, source lifecycle, both removal modes, Retry, Relocate, watcher refresh, backup export/preview/import, 800×600, scale stress, keyboard/focus, docs, and independent review each have an evidence owner.
- Safety coverage: all mutable QA paths are new and bounded below `C:\tmp`; real user app data, source images, registry, autostart, context menu, Windows wallpaper, Recycle Bin, policies, and display settings are excluded.
- Evidence coverage: browser simulation, real Tauri, local CI mirror, and GitHub CI use distinct status labels.
- Type consistency: the plan reuses `workbench | quiet`, the v1 backup contract, accepted ADR-029, and the existing command/UI vocabulary.
- Placeholder scan: every executable step has a concrete command, boundary, expected result, or explicit finding-driven RED/GREEN protocol.
- Completion boundary: local Phase 3D evidence can be committed without claiming remote CI, merge readiness, push, or PureWall-X completion.
