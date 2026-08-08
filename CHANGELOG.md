# Changelog

All notable public changes to PureWall will be documented in this file.

## Unreleased

- Preparing PureWall for its first open-source Windows release.
- Added release-readiness documentation and a staged implementation plan.
- Added Windows MSI and NSIS packaging verification.
- Added initial safety regression tests for scanner, SQLite, context-menu, and autostart boundaries.
- Added public project documentation: README, license, security policy, contributing guide, and changelog.
- Added a Windows GitHub Actions CI workflow for typecheck, build, Rust fmt/check/test/clippy, and production dependency audit.
- Documented safe uninstall cleanup scope for PureWall-owned HKCU registry entries.
- Confirmed release artifacts are generated: portable exe, MSI installer, and NSIS setup executable.- Added an executable release contract (`npm run verify:release`) that gates version/tag drift, HTTPS updater endpoints, and release/smoke workflow structure.
- Added a signature-verified update path: Settings can check for updates, review release notes, and explicitly download/install; updates require HTTPS transport and Tauri signature verification.
- Added tag-driven release automation that builds, signs, checksums, and attests Windows artifacts and creates only a draft GitHub Release.
- Added installer lifecycle smoke coverage (clean install, optional upgrade, uninstall, Authenticode, registry ownership) scoped to disposable runners.
- Resolved the last high-severity advisory (nanoid) so `npm audit` reports 0 vulnerabilities.
