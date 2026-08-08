# PureWall Phase 5 Open-Source Release Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close PureWall's open-source Windows release loop with a trustworthy public repository, tag-driven signed artifacts, checksums and provenance, a signature-verified updater, and repeatable installer lifecycle evidence.

**Architecture:** Keep ordinary pull-request CI separate from privileged release automation. A pushed SemVer tag may create only a draft GitHub Release, and only after version synchronization, tests, updater signing, Authenticode signing, checksum generation, and provenance attestation succeed. Runtime update checks are explicit user actions routed through a narrow Rust module; the release build receives its public updater configuration without committing private material.

**Tech Stack:** Tauri 2, Vue 3, TypeScript, Rust, PowerShell, GitHub Actions, GitHub Releases, Tauri updater signatures, Windows Authenticode.

## Global Constraints

- Windows is the only release target in this phase.
- PureWall-X, shared-core extraction, online wallpaper feeds, accounts, cloud sync, and recommendation work remain out of scope.
- No private key, PFX content, certificate password, token, or generated signing config may be committed.
- Release automation must create a draft release; publishing remains an explicit maintainer action.
- The updater must use HTTPS and Tauri's mandatory signature verification. Insecure transport and unsigned fallback paths are forbidden.
- Windows installers must be Authenticode-signed from repository secrets before a draft release can be considered releasable.
- Release artifacts must include SHA-256 checksums and GitHub build-provenance attestations.
- Installer smoke tests may run only on disposable GitHub-hosted Windows runners unless the user separately authorizes local native installation.
- Registry verification is read-only and limited to the documented PureWall-owned HKCU entries; no HKLM, Windows policy, or Windows 11 context-menu-mode change is permitted.
- Every implementation task appends its scope and evidence to `docs/project-docs/CHANGELOG_AI.md`; `AI_DIARY.md` is append-only and changes only for a new reusable pitfall.
- The existing identifier `com.purewall.app` remains unchanged in Phase 5 because it determines the current app-data location. Identifier migration requires a separate accepted ADR and data migration.

---

## File Map

- `docs/project-docs/DECISIONS.md`: accepted release/update trust-boundary ADR.
- `docs/project-docs/ARCHITECTURE.md`: release pipeline, updater, and lifecycle verification boundaries.
- `docs/project-docs/CHANGELOG_AI.md`: task-by-task implementation and verification evidence.
- `.gitignore`: local agents, generated evidence, caches, signing residue, and build artifacts.
- `docs/REPOSITORY_POLICY.md`: public policy for generated images, local agent configuration, caches, and artifacts.
- `.github/ISSUE_TEMPLATE/*.yml`: public bug, feature, and safety intake.
- `.github/pull_request_template.md`, `SUPPORT.md`, `ROADMAP.md`: contributor and support surfaces.
- `README.md`, `SECURITY.md`, `CONTRIBUTING.md`: quick start, visual preview, FAQ, troubleshooting, private reporting, and release instructions.
- `scripts/verify-release-contract.mjs`: version/tag/config/repository contract verifier.
- `.github/workflows/release.yml`: privileged tag-driven draft-release workflow.
- `src-tauri/tauri.conf.json`: updater artifact generation and public package metadata.
- `src-tauri/src/app_updates.rs`: pending-update ownership and explicit fetch/install commands.
- `src-tauri/src/main.rs`: updater plugin initialization and command registration only.
- `src/components/UpdateSettings.vue`: explicit check, review, download/install, and progress UI.
- `src/components/InspectorPanel.vue`: mounts the update surface in Settings.
- `src/stores/appUpdates.ts`: frontend update state machine and typed command adapter.
- `scripts/verify-install-lifecycle.ps1`: clean install, optional upgrade, uninstall, and ownership assertions.
- `.github/workflows/installer-smoke.yml`: disposable-runner lifecycle workflow.

---

### Task 1: Accept the release trust boundary and add an executable release contract

**Files:**
- Create: `scripts/verify-release-contract.mjs`
- Modify: `package.json`
- Modify: `docs/project-docs/DECISIONS.md`
- Modify: `docs/project-docs/ARCHITECTURE.md`
- Modify: `docs/project-docs/CHANGELOG_AI.md`

**Interfaces:**
- Consumes: `package.json.version`, `src-tauri/Cargo.toml` package version, `src-tauri/tauri.conf.json.version`, optional `PUREWALL_RELEASE_TAG`.
- Produces: `npm run verify:release`, which exits non-zero on version drift, a malformed tag, a tag/version mismatch, an insecure updater endpoint, or forbidden release configuration.

- [x] **Step 1: Append accepted ADR-031 before changing the release architecture**

Record these decisions exactly:

```text
Tag: v<SemVer>, equal to all three application version declarations.
Release state: draft only; publishing is manual.
Update trust: Tauri signature verification plus HTTPS latest.json.
Installer trust: Authenticode is mandatory for releasable artifacts.
Secrets: GitHub Actions secrets only; never repository files or logs.
Lifecycle tests: disposable Windows runners only by default.
Identifier: preserve com.purewall.app during Phase 5.
```

- [x] **Step 2: Write the failing release-contract verifier**

Create an ESM script that:

```js
import { readFileSync } from "node:fs";

const packageJson = JSON.parse(readFileSync("package.json", "utf8"));
const tauriConfig = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
const cargoToml = readFileSync("src-tauri/Cargo.toml", "utf8");
const cargoVersion = cargoToml.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
const versions = [packageJson.version, cargoVersion, tauriConfig.version];
const semver = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/;
const failures = [];

if (versions.some((version) => version !== versions[0])) failures.push(`version mismatch: ${versions.join(", ")}`);
if (!semver.test(versions[0] ?? "")) failures.push(`invalid SemVer: ${versions[0]}`);

const tag = process.env.PUREWALL_RELEASE_TAG;
if (tag && tag !== `v${versions[0]}`) failures.push(`tag ${tag} does not match v${versions[0]}`);

const updater = tauriConfig.plugins?.updater;
if (updater) {
  if (tauriConfig.bundle?.createUpdaterArtifacts !== true) failures.push("updater artifacts are not enabled");
  if (!Array.isArray(updater.endpoints) || updater.endpoints.some((url) => !url.startsWith("https://"))) failures.push("updater endpoints must use HTTPS");
  if (updater.dangerousInsecureTransportProtocol === true) failures.push("insecure updater transport is forbidden");
}

if (failures.length) {
  console.error(failures.join("\n"));
  process.exit(1);
}
console.log(`PureWall release contract OK for ${versions[0]}`);
```

- [x] **Step 3: Run RED with a mismatched tag**

Run:

```powershell
$env:PUREWALL_RELEASE_TAG = 'v9.9.9'
npm run verify:release
Remove-Item Env:PUREWALL_RELEASE_TAG
```

Expected: FAIL with `tag v9.9.9 does not match v0.1.0`.

- [x] **Step 4: Add the package script and run GREEN**

Add:

```json
"verify:release": "node scripts/verify-release-contract.mjs"
```

Run `npm run verify:release` with no tag. Expected: PASS and one version line.

- [x] **Step 5: Update architecture and internal changelog**

Document the privileged draft-release boundary, signature separation (Tauri updater signature versus Authenticode), secret ownership, disposable installer runners, and retained identifier. Record no native app, installer, registry, or release action was executed.

- [x] **Step 6: Verify and commit**

Run:

```powershell
npm run verify:release
cargo check --manifest-path src-tauri/Cargo.toml
npx vue-tsc --noEmit
git diff --check
```

Commit: `docs(phase5): define release trust boundary`.

---

### Task 2: Complete the public repository and community experience

**Files:**
- Modify: `.gitignore`
- Create: `docs/REPOSITORY_POLICY.md`
- Create: `.github/ISSUE_TEMPLATE/bug.yml`
- Create: `.github/ISSUE_TEMPLATE/feature.yml`
- Create: `.github/ISSUE_TEMPLATE/safety.yml`
- Create: `.github/ISSUE_TEMPLATE/config.yml`
- Create: `.github/pull_request_template.md`
- Create: `SUPPORT.md`
- Create: `ROADMAP.md`
- Modify: `README.md`
- Modify: `SECURITY.md`
- Modify: `CONTRIBUTING.md`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `docs/project-docs/CHANGELOG_AI.md`

**Interfaces:**
- Consumes: existing public docs and committed `design/PureWall-Living-Gallery-Fusion.png` plus `design/purewall-icon-motion.gif`.
- Produces: complete public onboarding, support, security, roadmap, contribution, and repository-artifact policies.

- [x] **Step 1: Write the failing repository contract**

Extend `scripts/verify-release-contract.mjs` with required paths:

```js
const requiredPublicFiles = [
  ".github/ISSUE_TEMPLATE/bug.yml",
  ".github/ISSUE_TEMPLATE/feature.yml",
  ".github/ISSUE_TEMPLATE/safety.yml",
  ".github/ISSUE_TEMPLATE/config.yml",
  ".github/pull_request_template.md",
  "SUPPORT.md",
  "ROADMAP.md",
  "docs/REPOSITORY_POLICY.md",
];
for (const path of requiredPublicFiles) {
  if (!existsSync(path)) failures.push(`missing public repository file: ${path}`);
}
```

Import `existsSync` from `node:fs`. Run `npm run verify:release`; expected: FAIL listing the missing files.

- [x] **Step 2: Add scoped ignore rules and repository policy**

Add these rules without ignoring approved committed assets:

```gitignore
# Local agent configuration
.agents/
.codex/
skills-lock.json

# Language/tool caches
**/__pycache__/
*.py[cod]

# Generated QA and signing residue
output/
*.pfx
*.pem
*.key
src-tauri/tauri.release.generated.conf.json
```

`docs/REPOSITORY_POLICY.md` must state that curated release images live under `design/` or `docs/images/`, raw generated candidates and QA captures stay under ignored `output/`, local agent configuration is never required to build PureWall, caches/build artifacts are ignored, and secrets/signing material are never committed.

- [x] **Step 3: Add issue forms and pull-request template**

The bug form must request Windows version, PureWall version/commit, reproduction, expected/actual behavior, and logs with personal paths redacted. The feature form must ask which existing PureWall workflow improves and reject PureWall-X/online-feed scope. The safety form must not collect public exploit details; it directs destructive-file, registry, installer, traversal, and updater-signature reports to GitHub private security advisories. Disable blank issues in `config.yml` and link support/security pages. The PR template must include behavior evidence, screenshots when UI changes, the full verification gate, registry/system-setting declaration, and documentation updates.

- [x] **Step 4: Add support and roadmap pages**

`SUPPORT.md` routes usage questions to Discussions, reproducible bugs to the bug form, and security issues to private advisories. `ROADMAP.md` lists the completed PureWall foundation milestones, Phase 5 release work, Phase 6 targeted maintainability, and explicitly defers PureWall-X and shared-core extraction.

- [x] **Step 5: Complete README, FAQ, quick start, and troubleshooting**

Embed both committed visuals with honest labels (product interface preview and icon motion), add a five-step local-library quick start, a first-build contributor quick start, checksum/provenance verification instructions, FAQ entries for local-only data, supported paths, Recycle Bin behavior, SmartScreen/signing, and updates, plus troubleshooting for WebView2, unsupported/network paths, missing files, registry integrations, and build-cache permission failures. Remove the obsolete macOS wording from `tauri.conf.json.longDescription` in this task because it is public package metadata.

- [x] **Step 6: Run GREEN and verify docs**

Run:

```powershell
npm run verify:release
npx vue-tsc --noEmit
cargo check --manifest-path src-tauri/Cargo.toml
git diff --check
```

Expected: PASS. Append the exact files and evidence to `CHANGELOG_AI.md`.

- [x] **Step 7: Commit**

Commit: `docs(phase5): complete public repository experience`.

---

### Task 3: Add tag-driven signed draft releases, checksums, and provenance

**Files:**
- Create: `.github/workflows/release.yml`
- Create: `scripts/prepare-release-config.ps1`
- Create: `scripts/collect-release-artifacts.ps1`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `scripts/verify-release-contract.mjs`
- Modify: `README.md`
- Modify: `CONTRIBUTING.md`
- Modify: `docs/project-docs/CHANGELOG_AI.md`
- Append if needed: `docs/project-docs/AI_DIARY.md`

**Interfaces:**
- Consumes secrets `WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_PASSWORD`, `TAURI_SIGNING_PRIVATE_KEY`, optional `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, and repository variable `TAURI_SIGNING_PUBLIC_KEY`.
- Produces a draft release for tag `v<SemVer>`, signed MSI/NSIS artifacts, updater signatures and `latest.json`, `SHA256SUMS.txt`, and GitHub artifact attestations.

- [ ] **Step 1: Make the release contract fail until privileged workflow controls exist**

Require `.github/workflows/release.yml` and assert its text contains all of:

```text
tags:
PUREWALL_RELEASE_TAG
TAURI_SIGNING_PRIVATE_KEY
TAURI_SIGNING_PUBLIC_KEY
WINDOWS_CERTIFICATE
releaseDraft: true
actions/attest@v4
SHA256SUMS.txt
```

Run `npm run verify:release`; expected: FAIL because the workflow is missing.

- [ ] **Step 2: Enable updater artifacts without weakening transport**

Set `bundle.createUpdaterArtifacts` to `true`. Do not add a fake public key or an incomplete updater block to the checked-in config: `scripts/prepare-release-config.ps1` must merge the real `TAURI_SIGNING_PUBLIC_KEY` and the stable endpoint `https://github.com/WiseZenn/PureWall/releases/latest/download/latest.json` into an ignored generated config consumed only by the release build.

- [ ] **Step 3: Implement secret-safe release config preparation**

`scripts/prepare-release-config.ps1` must:

```text
1. Require all four signing inputs before writing any file.
2. Decode WINDOWS_CERTIFICATE into $env:RUNNER_TEMP only.
3. Import it into Cert:\CurrentUser\My and capture the imported certificate thumbprint.
4. Reject an empty or non-HTTPS timestamp URL.
5. Write src-tauri/tauri.release.generated.conf.json as UTF-8 without BOM.
6. Include only certificateThumbprint, sha256, timestampUrl, createUpdaterArtifacts=true,
   updater pubkey, and the fixed WiseZenn/PureWall HTTPS latest.json endpoint.
7. Never print certificate content, passwords, private keys, or the generated JSON.
```

Use `ConvertFrom-Json` in a focused script test with dummy non-secret inputs and `-ConfigOnly` so local verification exercises validation without importing a certificate.

- [ ] **Step 4: Implement artifact collection and checksum generation**

`scripts/collect-release-artifacts.ps1` takes `-BundleRoot` and `-OutputDirectory`, rejects roots outside the workspace, copies only `.msi`, setup `.exe`, `.sig`, and `latest.json`, sorts by file name, computes SHA-256 with `Get-FileHash`, and writes `SHA256SUMS.txt` using `UTF8Encoding(false)`. It fails if MSI, NSIS, updater signatures, or `latest.json` are absent.

- [ ] **Step 5: Add the tag-driven workflow**

The Windows job must:

```text
trigger only on v* SemVer tags or manual dry-run dispatch;
use contents: write, id-token: write, attestations: write;
checkout, set up Node 22 and stable Rust with rustfmt/clippy;
npm ci, full npm audit, typecheck, unit tests, build, Rust fmt/check/test/clippy;
validate PUREWALL_RELEASE_TAG against all version declarations;
prepare the secret-backed config;
run tauri-apps/tauri-action@v1 with releaseDraft: true and the generated config;
verify Authenticode status Valid for every EXE/MSI before release promotion;
collect artifacts and SHA256SUMS.txt;
upload the checksum to the same draft release;
attest the collected release-artifacts directory with actions/attest@v4.
```

Manual dispatch must build and attest workflow artifacts but must not create a GitHub Release unless `publishDraft` is explicitly true.

- [ ] **Step 6: Document maintainer provisioning and consumer verification**

Document exact secret/variable names, private-key rotation risk, certificate expiry/timestamp requirements, draft review, `Get-FileHash` checksum verification, and `gh attestation verify <artifact> --repo WiseZenn/PureWall`. Never document real secret values.

- [ ] **Step 7: Verify locally without secrets or a release**

Run:

```powershell
npm run verify:release
powershell -ExecutionPolicy Bypass -File scripts/prepare-release-config.ps1 -ConfigOnly -UpdaterPublicKey 'LOCAL_TEST_PUBLIC_KEY' -CertificateThumbprint '0000000000000000000000000000000000000000' -TimestampUrl 'https://timestamp.test.invalid'
powershell -ExecutionPolicy Bypass -File scripts/collect-release-artifacts.ps1 -SelfTest
cargo check --manifest-path src-tauri/Cargo.toml
npx vue-tsc --noEmit
git diff --check
```

Expected: all local/dry-run checks PASS; no certificate import, registry write, release, tag, push, or installer execution occurs.

- [ ] **Step 8: Security review and commit**

Review workflow permissions, secret logging, fork behavior, endpoint scheme, artifact allowlist, path containment, draft-only state, and generated-config ignore coverage. Append evidence to the changelog and any new signing/PowerShell pitfall to the diary. Commit: `ci(phase5): add trusted Windows release pipeline`.

---

### Task 4: Add an explicit signature-verified update path

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Create: `src-tauri/src/app_updates.rs`
- Modify: `src-tauri/src/main.rs`
- Create: `src/stores/appUpdates.ts`
- Create: `src/stores/appUpdates.test.ts`
- Create: `src/components/UpdateSettings.vue`
- Create: `src/components/updateSettings.test.ts`
- Modify: `src/components/InspectorPanel.vue`
- Modify: `src/styles.css`
- Modify: `docs/project-docs/ARCHITECTURE.md`
- Modify: `docs/project-docs/CHANGELOG_AI.md`

**Interfaces:**
- Consumes: Tauri release configuration and `tauri-plugin-updater = "2"`.
- Produces commands `fetch_update() -> Result<Option<UpdateMetadata>, String>` and `install_update(on_event: Channel<DownloadEvent>) -> Result<(), String>` plus a Settings UI state machine.

- [ ] **Step 1: Write failing Rust tests for pending-update ownership**

In `app_updates.rs`, test a pure `PendingUpdateState<T>` helper so that a successful fetch replaces the previous pending value, no-update clears it, install takes it exactly once, and a second install returns `NO_PENDING_UPDATE`. Run `cargo test app_updates`; expected: compile failure because the module does not exist.

- [ ] **Step 2: Implement the narrow Rust updater module**

Follow Tauri's documented pending-update pattern. Keep `Update` behind `Mutex<Option<Update>>`, expose only version/currentVersion/date/body metadata, emit typed Started/Progress/Finished channel events, and take the pending update before installation so retries cannot install stale state. Serialize stable user-safe errors; do not return URLs, signatures, headers, or internal filesystem paths.

- [ ] **Step 3: Register the updater plugin and commands**

Add the updater dependency, initialize `tauri_plugin_updater::Builder::new().build()`, manage pending state, and register only `fetch_update` and `install_update`. Do not auto-check at startup and do not add JavaScript updater permissions.

- [ ] **Step 4: Write failing frontend state-machine tests**

Cover:

```text
idle -> checking -> current
idle -> checking -> available
available -> downloading(progress) -> readyToRestart/completed
any operation -> error with retry enabled
concurrent check/install requests are ignored
```

Mock only the two Tauri commands and the event channel. Expected RED: missing store/module.

- [ ] **Step 5: Implement store and explicit Settings UI**

The UI displays current version, a `Check for updates` button, release version/notes when available, explicit `Download and install`, byte progress when known, and a terminal error with retry. It must warn that Windows exits PureWall while the installer runs. No update is downloaded or installed merely by opening Settings.

- [ ] **Step 6: Verify signature/config boundary**

Extend `npm run verify:release` to require `createUpdaterArtifacts: true`, require the generated-config script to pin the fixed HTTPS endpoint and forbid insecure transport, require Rust updater dependency/plugin registration, and reject updater private-key material in tracked files. The public key remains injected into the release config by Task 3.

- [ ] **Step 7: Run focused and full gates**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml app_updates
npx vitest run src/stores/appUpdates.test.ts src/components/updateSettings.test.ts
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npx vue-tsc --noEmit
npm run test:unit
npm run build
npm run verify:release
git diff --check
```

Expected: all PASS with non-zero focused test counts. Do not contact the update endpoint or run the native app during automated local verification.

- [ ] **Step 8: Update docs, review, and commit**

Document explicit user initiation, signature enforcement, pending-update ownership, Windows exit semantics, offline/error behavior, and secret separation. Perform security review of endpoint pinning, error exposure, and retry state. Commit: `feat(phase5): add verified application updates`.

---

### Task 5: Add disposable clean-install, upgrade, and uninstall smoke coverage

**Files:**
- Create: `scripts/verify-install-lifecycle.ps1`
- Create: `.github/workflows/installer-smoke.yml`
- Modify: `scripts/verify-release-contract.mjs`
- Modify: `CONTRIBUTING.md`
- Modify: `docs/project-docs/CHANGELOG_AI.md`
- Append if needed: `docs/project-docs/AI_DIARY.md`

**Interfaces:**
- Consumes: a current signed MSI, optional previous signed MSI, expected `PureWall` product name, and the documented PureWall-owned HKCU key allowlist.
- Produces: structured lifecycle evidence for clean install, optional upgrade, uninstall, Authenticode, executable presence/version, and registry ownership.

- [ ] **Step 1: Write a failing script self-test**

The script must expose `-SelfTest` and test pure helpers for path containment, MSI extension validation, version ordering, allowed registry identities, and redacted command reporting. Run it before implementation; expected: FAIL because the script is absent.

- [ ] **Step 2: Implement fail-fast lifecycle verification**

The script must:

```text
require Windows and an explicit -DisposableRunner switch for mutation;
verify Authenticode before installation;
snapshot only the documented PureWall-owned HKCU keys and Run value;
install the previous MSI when supplied, otherwise perform a clean current install;
verify the installed PureWall executable exists and reports the expected file version without launching it;
install the current MSI as an upgrade when a previous MSI was supplied;
uninstall using msiexec and the exact MSI path;
verify the installed binary is gone;
compare the PureWall-owned registry snapshot and report residue;
never enumerate/delete arbitrary directories and never write HKLM or policy keys;
stop on every non-zero msiexec exit code except documented reboot-required success codes.
```

- [ ] **Step 3: Add a disposable GitHub runner workflow**

`installer-smoke.yml` uses `workflow_dispatch` inputs for current tag and optional previous tag, downloads MSI plus `SHA256SUMS.txt` from draft/published releases with `gh release download`, verifies checksums and attestations, and runs the script with `-DisposableRunner`. It requests only `contents: read`, `id-token: write`, and `attestations: read`; it never publishes or modifies a release.

- [ ] **Step 4: Add static release-contract checks**

Require the workflow, `workflow_dispatch`, `-DisposableRunner`, `SHA256SUMS.txt`, `gh attestation verify`, and absence of `Remove-Item -Recurse`, `HKLM`, `Policies`, and the Win11 CLSID. Run `npm run verify:release`; expected: PASS.

- [ ] **Step 5: Run safe local verification only**

Run:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/verify-install-lifecycle.ps1 -SelfTest
npm run verify:release
cargo check --manifest-path src-tauri/Cargo.toml
npx vue-tsc --noEmit
git diff --check
```

Expected: PASS. Do not pass `-DisposableRunner` locally; no MSI execution, native app, registry write, uninstall, or system-setting action occurs.

- [ ] **Step 6: Document evidence boundary and commit**

Document that the workflow is executable coverage but remains NOT RUN until a signed draft release and optional prior version exist. Record the exact allowed registry scope and that real lifecycle evidence must come from the disposable runner. Commit: `test(phase5): add installer lifecycle smoke coverage`.

---

### Task 6: Final Phase 5 review, full gate, and release-candidate handoff

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `docs/project-docs/OPEN_SOURCE_RELEASE_REVIEW.md`
- Modify: `docs/project-docs/CHANGELOG_AI.md`
- Append if needed: `docs/project-docs/AI_DIARY.md`

**Interfaces:**
- Consumes: Tasks 1-5.
- Produces: a reviewed Phase 5 branch with explicit local, CI, secret-provisioning, tag, and native-smoke evidence boundaries.

- [ ] **Step 1: Run an independent whole-branch review**

Review from the Phase 5 base commit for Critical/Important/Minor findings. Require explicit checks for secret leakage, updater downgrade/unsigned paths, workflow privilege, release draft state, checksum coverage, attestation subject paths, PowerShell path containment, installer mutation guards, registry allowlist, user-visible updater failure states, and documentation truthfulness. Fix every Critical/Important finding and either fix or record Minor findings.

- [ ] **Step 2: Run the fresh full local gate**

Run:

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npx vue-tsc --noEmit
npm run test:unit
npm audit
npm audit --omit=dev
npm run build
npm run verify:release
powershell -ExecutionPolicy Bypass -File scripts/verify-install-lifecycle.ps1 -SelfTest
git diff --check
```

Expected: all PASS. Record executed test counts and unchanged third-party warnings precisely.

- [ ] **Step 3: Record release boundaries honestly**

Mark these separately:

```text
LOCAL STATIC/UNIT GATE: PASS or FAIL.
GITHUB CI: NOT RUN until pushed.
SIGNING SECRETS: PROVISIONED or BLOCKED.
TAG-DRIVEN DRAFT RELEASE: NOT RUN until an authorized tag push.
AUTHENTICODE/UPDATER ARTIFACT VERIFICATION: NOT RUN until the draft release job runs.
INSTALL/UPGRADE/UNINSTALL SMOKE: NOT RUN until invoked on a disposable runner.
PUBLIC RELEASE: NOT CREATED/PUBLISHED by Phase 5 implementation work.
```

- [ ] **Step 4: Update public and internal release status**

Keep `CHANGELOG.md` under `Unreleased` unless the user supplies a release version and separately authorizes tagging. Update the release review with completed engineering gates, retained identifier warning, secret provisioning instructions, and any external-run blockers. Append the final evidence to `CHANGELOG_AI.md` and diary only for new pitfalls.

- [ ] **Step 5: Commit final closeout**

Commit: `docs(phase5): close open-source release loop`.

---

## Plan Self-Review

- **Spec coverage:** Tag automation is Task 3; signed installers, checksums, and provenance are Task 3; signature-verified updates are Task 4; clean install/upgrade/uninstall coverage is Task 5; screenshots/quick start/FAQ/troubleshooting and community files are Task 2; generated assets/local agents/caches/build artifacts policy is Task 2; contributor clean-clone commands and the final gate are Tasks 1, 2, and 6.
- **Safety coverage:** Secrets never enter tracked files; releases remain drafts; native installer mutations are guarded to disposable runners; updater transport is HTTPS and signature verification is mandatory; registry checks use the PureWall allowlist only.
- **Boundary coverage:** Actual certificate/key provisioning, tag push, GitHub release execution, and installer lifecycle execution are external evidence gates, not falsely represented by local source validation.
- **Completeness scan:** No deferred or fill-later steps remain. External secret names and exact verification states are defined.
- **Type consistency:** `UpdateMetadata`, `DownloadEvent`, `fetch_update`, `install_update`, and the pending-update state names are consistent across Rust and frontend tasks.
