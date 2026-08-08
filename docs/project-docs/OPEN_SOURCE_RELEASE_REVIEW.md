# PureWall Open Source Release Review

> Date: 2026-07-01
> Scope: safety, uninstall/delete behavior, registry behavior, release packaging, icon assets, dependency posture, and open-source readiness.
> Rule of engagement: review and documentation only. No production code was changed.

---

## 1. Executive Conclusion

PureWall is much safer than the earlier June review baseline. I did not find any implementation that recursively deletes an install directory, a drive root, a user profile, or arbitrary folders during normal app use or packaging. The dangerous class of bug you mentioned, such as an uninstaller deleting the current disk, is not present in the reviewed source.

The current deletion path is scoped to individual wallpaper files that must be valid, local, supported image files and already registered in SQLite. On Windows, deletion uses the Recycle Bin through `trash::delete`, not permanent recursive removal.

However, PureWall is not yet ready for a polished open-source release. The main blockers are release packaging, missing public project documents, no real automated safety tests, failed strict Clippy, and a Tauri bundle icon configuration gap.

---

## 2. Verification Evidence

| Check | Result | Notes |
|---|---:|---|
| `cargo check` | PASS | Rust compile baseline clean. |
| `npx vue-tsc --noEmit` | PASS | TypeScript/Vue type baseline clean. |
| `npm audit --omit=dev` | PASS | 0 vulnerabilities. |
| `npm audit` | PASS | First sandbox run hit socket hang up; elevated rerun found 0 vulnerabilities. |
| `npm run build` | PASS | Ordinary sandbox hit known Vite/Rolldown `spawn EPERM`; elevated rerun passed. |
| `cargo test` | PASS, but weak | 0 tests executed. This is a release risk. |
| `cargo clippy --all-targets -- -D warnings` | FAIL | 8 style/lint failures, no security finding. |
| `npm run tauri build` | PARTIAL FAIL | Release exe built, MSI bundling failed: `Couldn't find a .ico icon`. |

Release build evidence:

- Built exe: `src-tauri/target/release/purewall.exe`, size about 16.96 MB.
- Installer/bundle output: not produced because MSI bundling failed at icon lookup.
- Tauri warning: `identifier` is `com.purewall.app`, and Tauri warns against identifiers ending in `.app`.

---

## 3. Critical Safety Review

### 3.1 No "Delete Whole Disk" Pattern Found

I searched for destructive file operations and reviewed the deletion chain. There is no `remove_dir_all`, no installer/uninstaller cleanup script, no recursive delete over the current directory, and no command that deletes a folder tree chosen from user input.

Current delete flow:

- Frontend queues delete with a 7-second undo window before invoking the backend.
- Backend validates the path with `require_registered_wallpaper_file`.
- Validation requires:
  - non-empty path,
  - local path policy,
  - existing file,
  - supported image extension,
  - row already registered in SQLite.
- Windows deletion uses `trash::delete(&path)`.
- SQLite row removal happens only after file deletion succeeds.

Evidence:

- `src-tauri/src/main.rs:183` validates existing image files.
- `src-tauri/src/main.rs:202` requires DB registration before file operations.
- `src-tauri/src/main.rs:760` single delete command.
- `src-tauri/src/main.rs:878` batch delete command.
- `src/stores/wallpapers.ts` uses `DELETE_UNDO_MS = 7000` and pending delete UI.

### 3.2 Registry Boundary Is Currently Good

The registry implementation is constrained to PureWall-owned HKCU entries:

- Context menu: `HKCU\Software\Classes\Directory\Background\shell\PureWall` and historical `PW*` cleanup keys.
- Autostart: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\PureWall`.
- No HKLM writes found.
- No Win11 global shell policy / CLSID menu-disabling write found in current code.
- Registry calls use direct `reg.exe` process arguments, not generated PowerShell strings.

Evidence:

- `src-tauri/src/context_menu.rs:32` registers the PureWall context menu.
- `src-tauri/src/context_menu.rs:65` unregisters current and historical PureWall-owned entries.
- `src-tauri/src/context_menu.rs:93`, `:108`, `:123` call `reg` directly.
- `src-tauri/src/autostart.rs:5` defines the Run key.
- `src-tauri/src/autostart.rs:34`, `:57`, `:74` call `reg.exe` directly.

### 3.3 Path Scan Boundaries Are Good

Folder import has meaningful guardrails:

- max scan depth: 24,
- max images: 20,000,
- max directory entries: 100,000,
- rejects UNC and extended UNC paths,
- rejects Windows mapped network drives,
- skips symlinks during recursion.

Evidence:

- `src-tauri/src/scanner.rs:18` to `:20` scan limits.
- `src-tauri/src/scanner.rs:43` folder scanning entry.
- `src-tauri/src/scanner.rs:64` explicit file import entry.
- `src-tauri/src/scanner.rs:121` symlink skip.
- `src-tauri/src/scanner.rs:160` local path policy.
- `src-tauri/src/scanner.rs:174` UNC detection.
- `src-tauri/src/scanner.rs:183` remote drive rejection.

---

## 4. Release Blockers

### P0-1: Installer Packaging Fails Because Bundle Icon Config Only Lists PNG

`npm run tauri build` compiled the release exe but failed in MSI bundling:

```text
Built application at: D:\Desktop\PureWall\src-tauri\target\release\purewall.exe
failed to bundle project `Couldn't find a .ico icon`
```

The updated icon assets exist, including `src-tauri/icons/icon.ico`, but `tauri.conf.json` currently lists only `icons/icon.png`:

- `src-tauri/tauri.conf.json:48`

Release action:

- Wire the regenerated `src-tauri/icons/icon.ico` into the Tauri bundle icon list.
- Re-run `cargo check`, `npm run build`, and `npm run tauri build`.
- Pay attention to the historical `#icon-001` diary entry: verify the ICO is accepted by Windows resource compilation.

### P0-2: No Public Open-Source Project Documents

Root-level public project files are missing:

- no `README.md`,
- no `LICENSE`,
- no `SECURITY.md`,
- no `CONTRIBUTING.md`,
- no public `CHANGELOG.md`.

Release action:

- Add a user-facing README with screenshots, install instructions, safety promise, feature list, and roadmap.
- Pick and add a license before publishing. For a desktop app, MIT or Apache-2.0 are common permissive choices; GPL is a deliberate copyleft choice.
- Add `SECURITY.md` with vulnerability reporting guidance.
- Add `CONTRIBUTING.md` with local setup, verification commands, and registry safety rules.
- Add a public changelog separate from internal `CHANGELOG_AI.md`.

### P0-3: No Automated Safety Regression Tests

`cargo test` passed but ran 0 tests. This is the biggest safety-process gap for the kind of app you want PureWall to become.

Minimum release tests should cover:

- delete refuses unregistered paths,
- delete refuses directories,
- delete refuses network/UNC paths,
- delete only calls file deletion after DB registration succeeds,
- scanner stops at depth/count/entry budgets,
- scanner skips symlinks,
- registry unregister only targets PureWall-owned keys,
- autostart command constructs only the `PureWall` Run value,
- SQLite migrations preserve existing wallpaper/tag/play rows.

### P0-4: Strict Clippy Fails

`cargo clippy --all-targets -- -D warnings` failed on small issues:

- `src-tauri/src/autostart.rs:12` uses `&PathBuf` where `&Path` is enough.
- `src-tauri/src/db.rs:403` can use `std::iter::repeat_n`.
- `src-tauri/src/scanner.rs:246` has an identity `map`.
- `src-tauri/src/wallpaper.rs:39`, `:41`, `:44`, `:61`, `:75` have needless `return`.

These are not safety bugs, but open-source projects look healthier when strict lint is green and enforced in CI.

### P0-5: Tauri Identifier Warning

Tauri warns that `com.purewall.app` ends with `.app`, which can conflict with macOS bundle extension conventions:

- `src-tauri/tauri.conf.json:5`

PureWall targets Windows today, so this is not urgent for Windows-only distribution. Still, before open-source release, choose a durable identifier such as `io.github.wisezenn.purewall` or `com.wisezenn.purewall`.

---

## 5. Safety Hardening Before Public Release

### P1-1: Add Explicit Uninstall Cleanup for PureWall-Owned Registry Entries

No dangerous uninstaller script exists, which is good. But without a cleanup strategy, users may uninstall while leaving context menu or autostart registry entries behind.

Release action:

- Add a documented uninstall cleanup path that removes only:
  - `HKCU\Software\Classes\Directory\Background\shell\PureWall`,
  - historical `PWNext` / `PWLike` / `PWDislike` / `PWPause`,
  - `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\PureWall`,
  - PureWall-owned menu icon file under app data if appropriate.
- Do not touch Win11 global menu policy or HKLM.
- Add a regression test or dry-run script for the exact registry keys.

### P1-2: Make Batch Delete Atomic or Return Per-File Results

Current batch deletion validates all paths first, then loops through `trash::delete` and DB removal per file. If item N fails after earlier files succeeded, the frontend catch path restores all UI snapshots, but earlier files may already be in the Recycle Bin and removed from SQLite.

This is not an over-delete risk, but it can create confusing partial state.

Release action:

- Return per-file delete results to the frontend, or
- make DB row deletion a transaction after successful trash moves, or
- present partial success clearly in the notification.

### P1-3: Add a "Safety Promise" Section to README

PureWall should publicly state its boundaries:

- never modifies HKLM,
- never disables Windows 11 system context menu,
- deletes only individual registered wallpaper files,
- uses Recycle Bin on Windows,
- refuses network paths,
- app data lives under `%APPDATA%\com.purewall.app`,
- original wallpaper files are referenced, not copied into app data.

This will make the project feel trustworthy.

---

## 6. Icon and Motion Asset Readiness

The redesigned icon and motion assets are present and should be used in the release story:

Static assets:

- `design/purewall-icon.svg`
- `src-tauri/icons/16x16.png`
- `src-tauri/icons/24x24.png`
- `src-tauri/icons/32x32.png`
- `src-tauri/icons/128x128.png`
- `src-tauri/icons/128x128@2x.png`
- `src-tauri/icons/icon.png`
- `src-tauri/icons/icon.ico`

Motion assets:

- `design/purewall-icon-motion.html`
- `design/purewall-icon-motion.apng`
- `design/purewall-icon-motion.webp`
- `design/purewall-icon-motion.gif`
- `design/purewall-icon-motion-strip.png`

Recommended usage:

- Use `icon.png` and `icon.ico` for app/bundle icons after fixing Tauri bundle config.
- Use APNG or WebP in README and release pages as the high-quality motion preview.
- Keep GIF as a compatibility fallback only.
- Use the motion strip in design documentation or release notes.
- Add a short note in README that the icon represents a wallpaper card stack plus "next" action.

---

## 7. Open-Source Roadmap Toward an Excellent Wallpaper Manager

### Release Foundation

1. Fix Tauri bundle icon config and produce a real installer.
2. Add README, LICENSE, SECURITY, CONTRIBUTING, and public CHANGELOG.
3. Add GitHub Actions CI:
   - `npm ci`,
   - `npx vue-tsc --noEmit`,
   - `npm run build`,
   - `cargo check`,
   - `cargo test`,
   - `cargo clippy --all-targets -- -D warnings`,
   - `npm audit --omit=dev`.
4. Add release artifacts:
   - portable exe,
   - installer exe or MSI,
   - SHA256 checksums,
   - release notes.

### Safety and Trust

1. Add regression tests for delete, scanner, registry, and migrations.
2. Add an uninstall cleanup command or documented cleanup flow.
3. Add a diagnostics export button for logs/settings/environment.
4. Add a "dry run" or confirmation surface for registry/context-menu registration.

### Product Quality

1. Add first-run onboarding that explains import, Recycle Bin delete, and local-only behavior.
2. Add missing-file cleanup UX for files deleted outside PureWall.
3. Add backup/export/import for tags and preferences.
4. Add image metadata credits display and filtering.
5. Add performance benchmarks for 1k, 5k, and 20k libraries.

### Community Health

1. Publish a clear roadmap with labels: `good first issue`, `help wanted`, `safety`, `windows`.
2. Add issue templates for bug report, feature request, and safety concern.
3. Add screenshots and motion preview to GitHub social preview/release notes.
4. Keep internal AI docs, but make public docs concise and user-centered.

---

## 8. Final Release Gate

PureWall should not be tagged as a public v1.0 until these gates are green:

- [x] Installer/bundle builds successfully.
- [x] Bundle uses the new `.ico` and PNG icon assets.
- [x] README/LICENSE/SECURITY/CONTRIBUTING/CHANGELOG exist.
- [x] `cargo clippy --all-targets -- -D warnings` passes.
- [x] Safety tests exist and pass.
- [ ] A clean clone can run the documented setup.
- [ ] Manual smoke test confirms:
  - import folder,
  - set wallpaper,
  - next/like/dislike/pause,
  - context menu register/unregister,
  - autostart enable/disable,
  - delete to Recycle Bin,
  - uninstall cleanup behavior.

Current status after the 2026-07-01 implementation pass: release-candidate engineering gates are green, but public release should still wait for manual Windows smoke testing, identifier/app-data migration decision, and optional checksum/signing work.

---

## 9. Phase 5 Open-Source Release Loop Review (2026-08-08)

> Scope: Phase 5 Tasks 1-6 — release trust boundary (ADR-031), executable release contract, public repository experience, tag-driven signed draft releases with checksums/provenance, signature-verified updates, installer lifecycle smoke coverage, and the independent whole-branch security review.

### 9.1 Phase 5 evidence summary

| Gate | Result | Notes |
|---|---:|---|
| `cargo fmt -- --check` | PASS | — |
| `cargo check` | PASS | 0 errors (cargo 1.95.0). |
| `cargo test` | PASS | 129 passed, 1 ignored (updater retry tests included). |
| `cargo clippy --all-targets -- -D warnings` | PASS | — |
| `npx vue-tsc --noEmit` | PASS | 0 errors. |
| `npm run test:unit` | PASS | 51 files / 151 tests. |
| `npm audit` + `npm audit --omit=dev` | PASS | 0 vulnerabilities (nanoid 3.3.16 -> 3.3.18 resolved). |
| `npm run build` | PASS | Only upstream pure-annotation warnings. |
| `npm run verify:release` + `--self-test` | PASS | Strict SemVer, YAML structure, public files, ignore scope, release + smoke workflow contracts. |
| `prepare-release-config.ps1 -ConfigOnly` | PASS | Dry-run only; no certificate import. |
| `collect-release-artifacts.ps1 -SelfTest` | PASS | Containment, allowlist, required set, duplicate-basename rejection. |
| `verify-install-lifecycle.ps1 -SelfTest` | PASS | Path containment (reparse-aware), MSI validation, version ordering, registry allowlist, redacted commands. |
| `git diff --check` | PASS | — |

### 9.2 Independent whole-branch security review (2026-08-08)

Reviewed the full Phase 5 diff (base `d3b1829` -> HEAD). Critical: **none**. Verdict: **CONDITIONAL** -> resolved.

Findings and disposition:

| Severity | Finding | Disposition |
|---|---|---|
| Important | `release.yml` manual `publishDraft` compared as string `'true'`; supplied tag not verified to exist or checked out before building. | Fixed: boolean comparison `inputs.publishDraft`; added `Resolve and verify release tag` step (strict `v<SemVer>` pattern, `git ls-remote` existence check, checkout of the exact tag before build). |
| Important | `app_updates.rs` consumed the pending update before download, so a failed install could not be retried (`NO_PENDING_UPDATE`) while the frontend still offered Retry. | Fixed: `restore_after_failed_install` restores the pending value on failure (unless a newer fetch replaced it); 2 new Rust tests cover retry and newer-fetch precedence. |
| Important | `collect-release-artifacts.ps1` duplicate basenames from different subdirectories overwrite each other in the flat output dir; stale pre-existing files would be attested. | Fixed: duplicate-basename rejection + refuse unexpected pre-existing output files. |
| Important | `verify-install-lifecycle.ps1` MSI containment was lexical only; a junction/reparse point could resolve outside the temp root. | Fixed: resolve the final filesystem target (reparse-following) before containment checks. |
| Minor | README claimed no completed updater experience although the branch ships the updater plugin, commands, store, and Settings UI. | Fixed: README now states the updater path exists, is unit-tested, and remains unverified until a signed release provides endpoint/signature evidence. |
| Minor | `installer-smoke.yml` requested `id-token: write` unnecessarily. | Fixed: removed; only `contents: read` + `attestations: read`. |

### 9.3 Release boundaries (honest status)

```text
LOCAL STATIC/UNIT GATE: PASS.
GITHUB CI: NOT RUN until pushed.
SIGNING SECRETS: BLOCKED (must be provisioned in GitHub Actions secrets; never committed).
TAG-DRIVEN DRAFT RELEASE: NOT RUN until an authorized tag push.
AUTHENTICODE/UPDATER ARTIFACT VERIFICATION: NOT RUN until the draft release job runs.
INSTALL/UPGRADE/UNINSTALL SMOKE: NOT RUN until invoked on a disposable runner against a signed release.
PUBLIC RELEASE: NOT CREATED/PUBLISHED by Phase 5 implementation work.
```

### 9.4 Identifier warning

Tauri warns that identifiers ending in `.app` (`com.purewall.app`) are discouraged. Phase 5 deliberately preserves `com.purewall.app` because it determines the current app-data location; identifier migration requires a separate accepted ADR plus a data migration.

### 9.5 Remaining release prerequisites

- Provision `WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_PASSWORD`, `WINDOWS_CERTIFICATE_TIMESTAMP_URL`, `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, and `TAURI_SIGNING_PUBLIC_KEY` as GitHub Actions secrets/variables.
- Push `main` (or a release branch) so GitHub Windows CI produces its first green run.
- Tag `v0.1.0` after user authorization to trigger the draft-release workflow, review the draft, verify checksums/attestations, and publish.
- Run `installer-smoke` on a disposable runner against the signed draft release.
