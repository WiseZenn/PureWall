# PureWall Roadmap

This roadmap describes direction, not a promise of dates. A milestone is complete only when its implementation and verification evidence are recorded. An unrun workflow never counts as evidence.

## Completed foundation

- **Phase 1:** local wallpaper library, Windows wallpaper control, weighted random playback, ratings, and local persistence.
- **Phase 2:** one typed playback loop shared by the main window, widget, tray, CLI, and timer, with explicit completion ownership and durable side-effect boundaries.
- **Phase 3:** tags, collections, hidden items, batch operations, local source lifecycle, pagination, availability tracking, and portable metadata backup/restore.
- **Phase 4:** display-aware playback, focus auto-pause, yearly insights, accessibility/responsive hardening, and local verification gates.

## Phase 5: trustworthy open-source release loop

### Implemented in the repository

- Release trust boundaries, version/tag contracts, and executable release verification.
- Public onboarding, support, security, contribution, changelog, and repository-artifact documentation.
- Tag-driven draft-release automation, Authenticode/updater signing configuration, checksum generation, and GitHub artifact attestation steps.
- A signature-verified Tauri updater path in the application, covered by unit tests.
- Disposable-runner installer lifecycle smoke coverage for clean install, optional upgrade, and uninstall.

These are implementation facts, not evidence that the external release gates have run.

### Implemented; awaiting external evidence

The remaining Phase 5 work is an authorized release sequence:

1. **GitHub Windows CI:** obtain the first green run of the Windows CI workflow.
2. **Signing secrets:** provision the required GitHub Actions certificate and updater-key secrets/variables without committing values.
3. **Authorized tag:** push the authorized `v0.1.0` tag after its version declarations and review gate match.
4. **Draft release:** let the tag workflow build the signed Windows artifacts and create a draft release; verify checksums and attestation for the draft artifacts.
5. **Disposable-runner smoke:** run the installer lifecycle workflow against the draft (and a prior release where an upgrade path is being tested).
6. **Maintainer publication:** review the evidence and publish the draft deliberately.

Until this sequence produces evidence attached to a matching release, PureWall must not claim that CI, signing, release automation, updater delivery, provenance, installer smoke, or publication has run. A workflow definition, local dry-run, existing bundle, or passing unit test is not a substitute for the corresponding external evidence. See [docs/RELEASING.md](docs/RELEASING.md).

## Phase 6: targeted maintainability — complete

Phase 6 is complete. The implementation extracted focused responsibilities without changing the public store facade, SQLite schema/migration order, command names, or safety contracts:

- Database persistence was split into store/backend responsibility modules.
- Rust command groups were extracted from `main.rs`.
- Wallpaper store state was split into focused parts behind the existing facade.
- `styles.css` gained responsibility markers without visual behavior changes.
- The hardened ADR-034 gate records **182 passed Rust tests (1 ignored)** and **88 frontend unit tests across 28 files**, with strict `cargo clippy --all-targets -- -D warnings`, typecheck, production build, release-contract, and dependency-audit gates green. Real native watcher/junction and large-source lifecycle tests remain disposable-runner work.

Phase 6 remains a completed maintainability milestone, not a broad rewrite. Existing local-library, database, playback, and Windows safety contracts remain the compatibility baseline.

## Phase 7A: evidence-led large-library performance — Standard optimization complete

Phase 7A is building an isolated, repeatable performance evidence loop before changing production behavior:

- A feature-gated pre-Tauri harness uses deterministic synthetic data and production scanner, database, playback-selection, thumbnail, preview, and queue paths without touching PureWall AppData or a real wallpaper library.
- Windows CI runs only the 120-item contract fixture. The local Standard scale is 10,000 items in one root; Stress is 100,000 items across ten roots and requires a separate explicit opt-in.
- Reports preserve raw samples, median/P95, peak working set, correctness evidence, environment and dataset fingerprints, and fail-closed comparison results.
- The first untouched Standard baseline is recorded and self-compares as stable. Its ranking selected `scan.full` as the only production hotspot for targeted optimization.
- The optimized scanner canonicalizes a safe root once, reuses verified Windows entry metadata, and caches decoded dimensions for hard-linked physical files within one scan while preserving separate path records and hashes.
- Two same-commit Standard confirmations improved the selected `scan.full` median by 88.57% and 90.22%. Their machine comparisons still reported `regressed` because changing unrelated scenarios crossed the 5% guardrail while the host sustained background load. The maintainer explicitly accepted that documented environment exception; the original reports remain unchanged.
- The separately gated 100,000-item Stress workflow has not run, so no Stress stability or performance result is claimed.

See [docs/PERFORMANCE.md](docs/PERFORMANCE.md) for the contributor workflow and safety boundary.

## Phase 7B: Windows native watcher lifecycle — implemented; awaiting external evidence

- Real Windows notify coverage is isolated behind the `watcher-lifecycle` Cargo feature and ignored tests. Default builds and CI never start an OS watcher.
- The manual `watcher-lifecycle` workflow requires a GitHub-hosted Windows runner, a unique marker-owned root directly under `RUNNER_TEMP`, read-only repository permission, serial execution, and a fixed timeout.
- The native scenarios cover nested create/modify/rename/remove delivery through the production root/source queue, watcher-drop quiescence, and an actual Windows junction that production scanning must not traverse.
- Local verification compiles the feature and runs only pure guard tests. Phase 7B cannot claim real lifecycle success until the manual workflow completes on a matching commit.

## Explicitly deferred

- **PureWall-X** is a later product built after the ordinary PureWall foundation and release loop are complete. Online feeds, recommendation services, or PureWall-X-only behavior do not belong in current PureWall feature requests.
- **Shared-core extraction** is deferred until PureWall-X creates demonstrated, stable duplication. PureWall will not introduce a speculative shared framework before two real consumers define the boundary.
