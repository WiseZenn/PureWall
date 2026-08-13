# Changelog

All notable public changes to PureWall will be documented in this file.

## Unreleased

- Hardened watched-folder synchronization with pure `EventKind` classification, component-aware root containment, source-owned bounded coalescing, unique-path capacity accounting, two-worker fairness, and deterministic shutdown. Ordinary directory metadata changes stay incremental; notify loss and per-source path saturation promote to a bounded snapshot, while new roots beyond the global admission limit fail closed. Per-admission generation tokens preserve one in-flight slot across remove/re-admit races, and stale completions cannot release replacement work. Startup, import, manual, relocation-handoff, and watcher full snapshots atomically authorize ownership, mark absent descendants unavailable, upsert images, and persist scan state without deleting metadata or relationships. Stale source work and outside-root missing paths are no-ops, stale work emits no success refresh, Windows traversal rejects reparse roots and descendants, and source-identified failures refresh the UI once per unresolved episode. Shutdown drops native watchers before joining the reconciliation queue and checkpoints SQLite last.
- Hardened wallpaper deletion: only exact registered local fixed-volume files may proceed through a project-owned Windows Shell Recycle Bin operation; reparse/remote/unsupported-volume paths and unsupported platforms fail closed, Recycle Bin evidence is required before metadata cleanup, and PureWall never auto-confirms permanent deletion. The Windows Shell path-bound same-user replacement residual remains explicitly documented rather than claimed eliminated.
- Hardened SQLite startup recovery: older supported databases pass `quick_check`, receive a retained WAL-safe adjacent migration backup, and migrate transactionally with fail-closed collision handling; PureWall never auto-restores or deletes safety copies.
- Preparing PureWall for its first open-source Windows release.
- Added release-readiness documentation and a staged implementation plan.
- Added Windows MSI and NSIS packaging verification.
- Added initial safety regression tests for scanner, SQLite, context-menu, and autostart boundaries.
- Added public project documentation: README, license, security policy, contributing guide, and changelog.
- Added a Windows GitHub Actions CI workflow for typecheck, build, Rust fmt/check/test/clippy, and production dependency audit.
- Documented safe uninstall cleanup scope for PureWall-owned HKCU registry entries.
- The release collector covers MSI/NSIS setup artifacts, updater `.sig` artifact(s), and `latest.json`; PureWall does not claim a standalone portable executable.
- Added an executable release contract (`npm run verify:release`) that gates version/tag drift, HTTPS updater endpoints, and release/smoke workflow structure.
- Added a signature-verified update path: Settings can check for updates, review release notes, and explicitly download/install; updates require HTTPS transport and Tauri signature verification.
- Added tag-driven release automation that builds, signs, checksums, and attests Windows artifacts and creates only a draft GitHub Release.
- Added installer lifecycle smoke coverage (clean install, optional upgrade, uninstall, Authenticode, registry ownership) scoped to disposable runners.
- Resolved the last high-severity advisory (nanoid) so `npm audit` reports 0 vulnerabilities.
- Rewrote the public README with product screenshots, feature highlights, a safety promise, and an honest project-status section.
- Added a GitHub social preview banner and reproducible screenshot/marketing generation scripts.
- Added a public engineering architecture doc and a maintainer release guide (`docs/RELEASING.md`).
- Moved internal agent memory, planning docs, and raw design candidates out of public version control (kept locally).
- Pinned GitHub Actions to reviewed commit SHAs and hardened release/installer-smoke workflows (strict tag validation, runner guards, checksum + attestation for upgrade MSI).
- Fixed frontend interaction bugs: icon-button clicks, keyboard activation on nested card controls, and rotation-interval rollback on backend rejection.
- Made play counting transactional and applied the 500-path batch limit to batch deletion.
