# Contributing to PureWall

Thanks for helping improve PureWall. Contributions should strengthen the existing local Windows wallpaper workflow; PureWall-X, online wallpaper feeds, and speculative shared-core extraction are outside the current project scope.

## Set up a clean Windows build

Requirements:

- Windows 10 or 11,
- Node.js `20.19+` or `22.12+`,
- Rust stable with the MSVC toolchain,
- Microsoft Edge WebView2 Runtime,
- Git.

```powershell
git clone https://github.com/WiseZenn/PureWall.git
cd PureWall
npm ci
npm run test:release-contract
npm run verify:release
npx vue-tsc --noEmit
cargo check --manifest-path src-tauri/Cargo.toml
```

Run the frontend/native development process only when interactive verification is needed:

```powershell
npm run tauri dev
```

That command launches a native app and uses PureWall's normal app-data boundary. Use a disposable Windows account and synthetic wallpaper library for integration or destructive-flow testing.

## Make a focused change

1. Start from a clean branch and confirm the baseline checks pass.
2. Add a focused failing test or executable contract for the intended behavior when practical.
3. Make the smallest implementation that closes that RED evidence without unrelated refactoring.
4. Run focused tests, then the complete gate below.
5. Update public documentation and `docs/project-docs/CHANGELOG_AI.md`; append to `AI_DIARY.md` only for a new reusable pitfall.

Do not describe signing, CI, updater delivery, installer lifecycle, native smoke, or publication as complete unless that exact action ran and its evidence is available.

## Full verification gate

Run from the repository root:

```powershell
npm run test:release-contract
npm run verify:release
npx vue-tsc --noEmit
npm run test:unit
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
git diff --check
```

For a packaging change, also run `npm run tauri build` in a controlled local or disposable environment. A successful package build is not a substitute for Authenticode, updater signing, remote CI, provenance, or clean install/upgrade/uninstall evidence.

Before any public release, confirm that [GitHub Private Vulnerability Reporting](https://github.com/WiseZenn/PureWall/security/advisories/new) accepts private reports. If that channel is unavailable, the release is blocked: maintainers must not publish, and reporters must not disclose security findings publicly.

If the Windows sandbox blocks Cargo, npm, or Vite build-cache writes with access denied, rerun the same command in a normal developer PowerShell and report that environment boundary. Do not change application source, delete user app data, or modify system policy to work around a compiler-cache error.

## Native safety rules

- Do not add automatic HKLM writes or Windows policy edits.
- Do not disable the Windows 11 context menu.
- Do not add recursive deletion of user-selected directories.
- Keep file deletion scoped to registered wallpaper files and the Windows Recycle Bin.
- Keep registry operations scoped to documented PureWall-owned HKCU entries.
- Prefer existing cleanup functions (`context_menu::unregister()` and `autostart::disable()`) over installer scripts or shell snippets.
- Never test destructive, installer, registry, wallpaper, display, or updater behavior against real user data without explicit authorization and isolation.

PureWall-owned registry entries are:

```text
HKCU\Software\Classes\Directory\Background\shell\PureWall
HKCU\Software\Classes\Directory\Background\shell\PWNext
HKCU\Software\Classes\Directory\Background\shell\PWLike
HKCU\Software\Classes\Directory\Background\shell\PWDislike
HKCU\Software\Classes\Directory\Background\shell\PWPause
HKCU\Software\Classes\PureWall_Commands
HKCU\Software\Microsoft\Windows\CurrentVersion\Run\PureWall
```

Any proposed cleanup outside that list requires an accepted ADR and explicit user confirmation before it is ever run locally.

## Public documentation and assets

- Update `README.md`, `SUPPORT.md`, `SECURITY.md`, or `ROADMAP.md` when user-visible behavior, support routing, security boundaries, or project scope changes.
- Keep curated public images under `design/` or `docs/images/`.
- Keep raw generated candidates, QA captures, traces, and recordings under ignored `output/`.
- Do not commit `.agents/`, `.codex/`, `skills-lock.json`, caches, private keys, certificate exports, release secrets, or generated signing configuration.
- Follow [docs/REPOSITORY_POLICY.md](docs/REPOSITORY_POLICY.md) before promoting an image or release artifact.

UI evidence must use synthetic or fully redacted data. Screenshots are required for material UI changes unless the pull request explains why they cannot be produced safely.

## Pull requests

Use the pull-request template. Include behavior evidence, focused RED/GREEN results, the full verification gate, any native safety impact, documentation changes, and honest unresolved evidence boundaries. A check that did not run stays unchecked.

For security-sensitive findings, do not open a public pull request before coordinating through [GitHub private security advisories](https://github.com/WiseZenn/PureWall/security/advisories/new).

## Release automation and signing

A pushed `v<SemVer>` tag that matches all three version declarations (`package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`) triggers `.github/workflows/release.yml`, which builds, signs, checksums, and attests the Windows artifacts and creates **only a draft** GitHub Release. Publishing is a manual maintainer action. Manual `workflow_dispatch` runs are dry-runs: they build and attest without creating a release unless `publishDraft` is explicitly enabled.

Maintainers must provision these GitHub Actions **secrets** (names only, never values): `WINDOWS_CERTIFICATE` (base64 PFX), `WINDOWS_CERTIFICATE_PASSWORD`, `WINDOWS_CERTIFICATE_TIMESTAMP_URL` (HTTPS), `TAURI_SIGNING_PRIVATE_KEY`, and optional `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; plus the repository **variable** `TAURI_SIGNING_PUBLIC_KEY`. `scripts/prepare-release-config.ps1` imports the certificate into `Cert:\CurrentUser\My` on the runner and writes the ignored `src-tauri/tauri.release.generated.conf.json` with the captured thumbprint, the HTTPS timestamp URL, the updater public key, and the fixed WiseZenn/PureWall `latest.json` endpoint. `scripts/collect-release-artifacts.ps1` copies only MSI, setup EXE, `.sig`, and `latest.json` into `release-artifacts/` and writes `SHA256SUMS.txt`.

Rotation and expiry: losing `TAURI_SIGNING_PRIVATE_KEY` or letting the code-signing certificate expire blocks future signed releases. Rotate the updater key and certificate together with a published release, and always use an HTTPS timestamp URL so Authenticode signatures remain valid after certificate expiry.

Local dry-run verification never requires secrets and never imports a certificate:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/prepare-release-config.ps1 `
  -ConfigOnly -UpdaterPublicKey 'LOCAL_TEST_PUBLIC_KEY' `
  -CertificateThumbprint '0000000000000000000000000000000000000000' `
  -TimestampUrl 'https://timestamp.test.invalid'
powershell -ExecutionPolicy Bypass -File scripts/collect-release-artifacts.ps1 -SelfTest
```

Real signing, updater, provenance, and release evidence comes from the tag-driven GitHub workflow, never from a local dry-run.

## Installer lifecycle smoke boundary

`.github/workflows/installer-smoke.yml` runs only on a disposable `windows-latest` GitHub runner and only via `workflow_dispatch` with a current release tag (and an optional previous tag for the upgrade path). It downloads the MSI and `SHA256SUMS.txt` with `gh release download`, verifies the SHA-256 checksum and `gh attestation verify` provenance, then runs `scripts/verify-install-lifecycle.ps1 -DisposableRunner` for clean install, optional upgrade, and uninstall.

The lifecycle script requires Windows plus an explicit `-DisposableRunner` switch for any mutation, verifies Authenticode before installing, snapshots only the PureWall-owned HKCU keys and the `Run\PureWall` value, verifies the installed executable and its file version without launching it, uninstalls with `msiexec` and the exact MSI path, and stops on every non-zero msiexec exit except the documented reboot-required codes (3010, 1641). It never enumerates or deletes arbitrary directories and never writes HKLM, Windows policy, or Windows 11 context-menu-mode keys.

The smoke workflow requests only `contents: read`, `id-token: write`, and `attestations: read` permissions and never publishes, edits, or deletes a release. It is executable coverage that remains **NOT RUN** until a signed draft or published release (and an optional prior version) exists; real lifecycle evidence must come from that disposable runner, never from a local `-DisposableRunner` invocation.
