# PureWall Open Source Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn PureWall from a promising Windows wallpaper manager into a safe, buildable, trustable open-source release candidate.

**Architecture:** Keep changes incremental and release-gated. Start with packaging and lint gates, then add safety tests around deletion/scanning/registry boundaries, then add public-facing repository documents and CI. Do not change registry behavior or system settings unless the task explicitly says so and verification is local/dry-run.

**Tech Stack:** Tauri 2, Vue 3, TypeScript, Rust, SQLite, Windows HKCU registry integration, GitHub Actions.

## Global Constraints

- Windows is the primary release platform.
- PureWall must never modify non-PureWall registry entries automatically.
- PureWall may only modify its own HKCU entries: context menu keys under `PureWall`/historical `PW*` and the `PureWall` Run autostart value.
- Destructive file behavior must remain scoped to registered wallpaper files; Windows deletion must use the Recycle Bin.
- Every code change must update `docs/project-docs/CHANGELOG_AI.md`.
- Append new pitfalls only to `docs/project-docs/AI_DIARY.md`; never rewrite history.
- Before each implementation batch, run `cargo check` and `npx vue-tsc --noEmit`.
- After each implementation batch, run the narrow verification for that task plus `cargo check` and `npx vue-tsc --noEmit`.

---

## File Map

- `src-tauri/tauri.conf.json`: Tauri release bundle metadata and icon list.
- `src-tauri/src/autostart.rs`: PureWall-owned HKCU Run key integration; needs Clippy cleanup and testable argument construction later.
- `src-tauri/src/db.rs`: SQLite schema/query module; needs Clippy cleanup and migration tests.
- `src-tauri/src/scanner.rs`: Local path policy and bounded scanning; needs Clippy cleanup and safety tests.
- `src-tauri/src/wallpaper.rs`: Windows wallpaper integration; needs Clippy cleanup only.
- `src-tauri/src/context_menu.rs`: PureWall-owned context menu registry integration; needs testable/dry-run registry key coverage later.
- `README.md`: Public project landing page.
- `LICENSE`: Open-source license.
- `SECURITY.md`: Vulnerability reporting and safety policy.
- `CONTRIBUTING.md`: Local development and verification guide.
- `CHANGELOG.md`: Public release notes, separate from internal AI changelog.
- `.github/workflows/ci.yml`: Public CI gate.
- `docs/project-docs/CHANGELOG_AI.md`: Internal change log for each task.
- `docs/project-docs/AI_DIARY.md`: Append-only pitfall memory.

---

### Task 1: Unblock Tauri Windows Packaging Icons

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: `docs/project-docs/CHANGELOG_AI.md`
- Append if needed: `docs/project-docs/AI_DIARY.md`

**Interfaces:**
- Consumes: regenerated icons under `src-tauri/icons/`.
- Produces: a Tauri bundle config that includes `icons/icon.ico` for Windows MSI packaging.

- [x] **Step 1: Add ICO to bundle icon list**

Change:

```json
"icon": ["icons/icon.png"]
```

to:

```json
"icon": [
  "icons/icon.png",
  "icons/icon.ico"
]
```

- [x] **Step 2: Verify Rust resource compilation**

Run:

```powershell
cargo check
```

from `src-tauri`.

Expected: PASS. If Windows RC fails with `RC2176`, stop and regenerate the ICO using the existing System.Drawing path in `scripts/generate-icons.ps1`.

- [x] **Step 3: Verify frontend build**

Run:

```powershell
npm run build
```

from repo root.

Expected: PASS. If normal sandbox hits Vite/Rolldown `spawn EPERM`, rerun the same command elevated and record that in `CHANGELOG_AI.md`.

- [x] **Step 4: Verify release packaging**

Run:

```powershell
npm run tauri build
```

Expected: release exe plus installer bundle succeeds. If bundling still fails, record the exact error and keep this task open.

- [x] **Step 5: Update docs**

Append `CHANGELOG_AI.md` with changed files and verification evidence. Append `AI_DIARY.md` only if a new packaging pitfall appears.

---

### Task 2: Make Strict Rust Clippy Green

**Files:**
- Modify: `src-tauri/src/autostart.rs`
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/scanner.rs`
- Modify: `src-tauri/src/wallpaper.rs`
- Modify: `docs/project-docs/CHANGELOG_AI.md`

**Interfaces:**
- Consumes: existing behavior.
- Produces: `cargo clippy --all-targets -- -D warnings` PASS without behavior changes.

- [x] **Step 1: Fix `autostart.rs` pointer argument lint**

Use `Path` instead of `PathBuf` for `quoted_exe_value`:

```rust
use std::path::{Path, PathBuf};

fn quoted_exe_value(exe: &Path) -> Result<String> {
    let exe_str = exe.to_string_lossy();
    if exe_str.contains('"') {
        anyhow::bail!("Executable path contains an unsupported quote character");
    }
    Ok(format!("\"{}\"", exe_str))
}
```

- [x] **Step 2: Fix `db.rs` placeholder iterator lint**

Change the placeholder builder to:

```rust
let placeholders = std::iter::repeat_n("?", ids.len())
    .collect::<Vec<_>>()
    .join(",");
```

- [x] **Step 3: Fix `scanner.rs` identity map lint**

Change:

```rust
image::image_dimensions(path).ok().map(|(w, h)| (w, h))
```

to:

```rust
image::image_dimensions(path).ok()
```

- [x] **Step 4: Fix `wallpaper.rs` needless returns**

Keep the behavior identical while returning expressions from cfg blocks:

```rust
match set_wallpaper_all_com(path_str.clone(), placement) {
    Ok(()) => Ok(()),
    Err(err) if placement == WallpaperPlacement::Fill => {
        set_wallpaper_system_parameters(&path_str)
            .with_context(|| format!("IDesktopWallpaper failed first: {err}"))
    }
    Err(err) => Err(err),
}
```

and:

```rust
sta_worker().set_monitor(monitor_id.to_string(), path_str)
```

```rust
sta_worker().get_displays()
```

- [x] **Step 5: Verify**

Run:

```powershell
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo check
npx vue-tsc --noEmit
```

Expected: all PASS.

- [x] **Step 6: Update docs**

Append `CHANGELOG_AI.md`. No `AI_DIARY.md` entry is needed unless a new tooling pitfall appears.

---

### Task 3: Add Safety Regression Tests

**Files:**
- Modify: `src-tauri/src/scanner.rs`
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/autostart.rs`
- Modify: `src-tauri/src/context_menu.rs`
- Modify: `docs/project-docs/CHANGELOG_AI.md`

**Interfaces:**
- Consumes: current path validation, SQLite migration, and registry key constants.
- Produces: meaningful `cargo test` coverage for release-critical safety boundaries.

- [x] **Step 1: Add scanner policy tests**

Add tests under `src-tauri/src/scanner.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn scan_files_rejects_too_many_paths_before_touching_disk() {
        let paths = (0..=MAX_SCAN_IMAGES)
            .map(|i| format!(r"C:\purewall-test\{i}.jpg"))
            .collect::<Vec<_>>();

        let err = scan_files(&paths).expect_err("oversized imports must fail");
        assert!(
            err.to_string().contains("Too many files selected"),
            "unexpected error: {err}"
        );
    }

    #[cfg(windows)]
    #[test]
    fn ensure_local_path_rejects_unc_paths() {
        let err = ensure_local_path(Path::new(r"\\server\share\wallpaper.jpg"))
            .expect_err("UNC paths must be rejected");
        assert!(
            err.to_string().contains("Network paths are not supported"),
            "unexpected error: {err}"
        );
    }
}
```

- [x] **Step 2: Add SQLite migration smoke tests**

Add tests under `src-tauri/src/db.rs` using a unique temp DB path. The test should create `Database::new`, insert a wallpaper, create a tag, assign it, record a play, and verify stats are non-zero.

- [x] **Step 3: Extract testable registry constants or argument builders**

Add pure helpers that do not call `reg.exe`, for example:

```rust
fn purewall_context_menu_root() -> &'static str {
    r"HKCU\Software\Classes\Directory\Background\shell\PureWall"
}
```

and a test asserting every unregister target starts with a PureWall-owned prefix.

- [x] **Step 4: Extract testable autostart target helper**

Add a pure helper returning the Run value identity:

```rust
fn autostart_value_name() -> &'static str {
    APP_NAME
}
```

and test that it is exactly `PureWall`.

- [x] **Step 5: Verify**

Run:

```powershell
cargo test
cargo clippy --all-targets -- -D warnings
cargo check
npx vue-tsc --noEmit
```

Expected: tests run with non-zero test count and all PASS.

- [x] **Step 6: Update docs**

Append `CHANGELOG_AI.md`. Add `AI_DIARY.md` only if test isolation hits a new Windows/Tauri issue.

---

### Task 4: Add Public Open-Source Project Documents

**Files:**
- Create: `README.md`
- Create: `LICENSE`
- Create: `SECURITY.md`
- Create: `CONTRIBUTING.md`
- Create: `CHANGELOG.md`
- Modify: `docs/project-docs/CHANGELOG_AI.md`

**Interfaces:**
- Consumes: `docs/project-docs/OPEN_SOURCE_RELEASE_REVIEW.md` and icon/motion assets.
- Produces: a repository that looks understandable and trustworthy to first-time visitors.

- [x] **Step 1: Create README**

Include:

```markdown
# PureWall

PureWall is a Windows desktop wallpaper manager built with Tauri, Vue, TypeScript, Rust, and SQLite.

## Safety Promise

- PureWall only writes PureWall-owned HKCU registry entries.
- PureWall never disables the Windows 11 system context menu.
- PureWall deletes only registered wallpaper files selected by the user.
- On Windows, deletes go to the Recycle Bin.
- Network wallpaper paths are rejected.
```

Also include setup commands:

```powershell
npm ci
npx vue-tsc --noEmit
npm run build
cd src-tauri
cargo check
cargo test
```

- [x] **Step 2: Choose and add license**

Default to MIT unless the project owner chooses another license:

```text
MIT License

Copyright (c) 2026 PureWall contributors
```

- [x] **Step 3: Add SECURITY.md**

Document:

```markdown
# Security Policy

Please report destructive file-operation, registry, installer, or path traversal issues privately before public disclosure.
```

- [x] **Step 4: Add CONTRIBUTING.md**

Document required pre-flight and post-flight commands, including:

```powershell
cargo check
npx vue-tsc --noEmit
cargo test
cargo clippy --all-targets -- -D warnings
npm run build
```

- [x] **Step 5: Add public CHANGELOG.md**

Start with an unreleased section:

```markdown
# Changelog

## Unreleased

- Preparing PureWall for its first open-source Windows release.
```

- [x] **Step 6: Verify**

Run:

```powershell
git diff --check -- README.md LICENSE SECURITY.md CONTRIBUTING.md CHANGELOG.md
cargo check
npx vue-tsc --noEmit
```

- [x] **Step 7: Update docs**

Append `CHANGELOG_AI.md`.

---

### Task 5: Add CI Release Gates

**Files:**
- Create: `.github/workflows/ci.yml`
- Modify: `docs/project-docs/CHANGELOG_AI.md`

**Interfaces:**
- Consumes: green local build/test/lint commands.
- Produces: GitHub Actions workflow for pull requests and pushes.

- [x] **Step 1: Add CI workflow**

Use Windows because PureWall targets Windows APIs:

```yaml
name: CI

on:
  push:
  pull_request:

jobs:
  verify:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: npm
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - run: npm ci
      - run: npx vue-tsc --noEmit
      - run: npm run build
      - run: cargo fmt --check
        working-directory: src-tauri
      - run: cargo check
        working-directory: src-tauri
      - run: cargo test
        working-directory: src-tauri
      - run: cargo clippy --all-targets -- -D warnings
        working-directory: src-tauri
      - run: npm audit --omit=dev
```

- [x] **Step 2: Verify workflow syntax locally as text**

Run:

```powershell
git diff --check -- .github/workflows/ci.yml
```

- [x] **Step 3: Verify normal local gates**

Run:

```powershell
cargo check
npx vue-tsc --noEmit
npm run build
cargo test
cargo clippy --all-targets -- -D warnings
```

- [x] **Step 4: Update docs**

Append `CHANGELOG_AI.md`.

---

### Task 6: Document and Implement Safe Uninstall Cleanup

**Files:**
- Modify: `src-tauri/src/context_menu.rs`
- Modify: `src-tauri/src/autostart.rs`
- Modify: `README.md`
- Modify: `CONTRIBUTING.md`
- Modify: `docs/project-docs/CHANGELOG_AI.md`
- Append if needed: `docs/project-docs/AI_DIARY.md`

**Interfaces:**
- Consumes: PureWall-owned registry boundary from ADR-014.
- Produces: a documented cleanup path that removes only PureWall-owned entries.

- [x] **Step 1: Keep cleanup scoped**

Do not add any HKLM operation. Do not add any Win11 menu policy operation. The cleanup scope is only:

```text
HKCU\Software\Classes\Directory\Background\shell\PureWall
HKCU\Software\Classes\Directory\Background\shell\PWNext
HKCU\Software\Classes\Directory\Background\shell\PWLike
HKCU\Software\Classes\Directory\Background\shell\PWDislike
HKCU\Software\Classes\Directory\Background\shell\PWPause
HKCU\Software\Classes\PureWall_Commands
HKCU\Software\Microsoft\Windows\CurrentVersion\Run\PureWall
```

- [x] **Step 2: Prefer existing unregister functions**

Use `context_menu::unregister()` and `autostart::disable()` as the cleanup surface. Do not invent a script that recursively deletes directories.

- [x] **Step 3: Document manual cleanup**

README should include exact registry paths, but tell users to prefer PureWall's UI cleanup where available.

- [x] **Step 4: Verify**

Run dry checks only unless the user explicitly asks to alter the local registry:

```powershell
cargo test
cargo clippy --all-targets -- -D warnings
cargo check
npx vue-tsc --noEmit
```

- [x] **Step 5: Update docs**

Append `CHANGELOG_AI.md`; append `AI_DIARY.md #registry` only if registry behavior changes.

---

### Task 7: Final Release Candidate Verification

**Files:**
- Modify: `docs/project-docs/OPEN_SOURCE_RELEASE_REVIEW.md`
- Modify: `CHANGELOG.md`
- Modify: `docs/project-docs/CHANGELOG_AI.md`

**Interfaces:**
- Consumes: all prior tasks.
- Produces: a release-candidate checklist with artifact locations and known limitations.

- [x] **Step 1: Run full verification**

Run:

```powershell
cargo fmt --check
cargo check
cargo test
cargo clippy --all-targets -- -D warnings
npx vue-tsc --noEmit
npm audit --omit=dev
npm run build
npm run tauri build
```

- [x] **Step 2: Record artifacts**

Record:

```text
src-tauri/target/release/purewall.exe
src-tauri/target/release/bundle/**
```

- [x] **Step 3: Manual smoke checklist**

Before tagging a release, manually verify:

```text
Import folder
Set wallpaper
Next / Like / Dislike / Pause
Context menu register / unregister
Autostart enable / disable
Delete selected wallpaper to Recycle Bin
Widget show / hide / actions
Light / dark theme
```

- [x] **Step 4: Update public changelog**

Move `CHANGELOG.md` `Unreleased` notes into the chosen version section.

- [x] **Step 5: Update internal docs**

Append `CHANGELOG_AI.md` with the final release-candidate status and unresolved items.
