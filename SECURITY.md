# Security Policy

PureWall is a local Windows desktop application that handles user-selected files, Windows wallpaper APIs, PureWall-owned HKCU registry entries, local SQLite data, installers, and future signed update artifacts. Reports at those boundaries may place user files or system state at risk and must be handled privately.

## Supported versions

Security fixes target the current `main` branch and any published release explicitly listed as supported in its release notes. An arbitrary local build, draft artifact, fork, or older release is not implicitly supported or verified by this policy.

## Report privately

GitHub Private Vulnerability Reporting, exposed through the [GitHub private security advisory](https://github.com/WiseZenn/PureWall/security/advisories/new) channel, is a mandatory release prerequisite for PureWall's first public release and every later release. Before publishing, maintainers must confirm that the link accepts private reports.

If GitHub reports that the private advisory channel is unavailable, the release process is **BLOCKED**: maintainers must not publish, and reporters must not disclose the finding publicly. Maintainers must enable or restore the channel before release or private reporting can proceed.

Do **not** post exploit details, proof-of-concept code, private paths, secrets, destructive test evidence, or unredacted logs in a public issue, discussion, pull request, or safety form.

Always report these categories privately:

- destructive or unexpected file operations,
- registry writes outside PureWall-owned HKCU keys,
- installer or uninstaller cleanup behavior,
- path traversal, UNC/network-path bypass, or unsafe destination replacement,
- command execution, shell escaping, or Tauri/WebView IPC exposure,
- SQLite corruption or backup/restore boundary bypass,
- updater signature, Authenticode, checksum, provenance, or release-pipeline bypass.

## What to include

In the private advisory, include:

- affected PureWall release, commit, or branch,
- Windows version and relevant install type,
- impact and the safety boundary crossed,
- the smallest reproduction that is safe to share privately,
- whether user files, registry state, installer state, signatures, or secrets can be affected,
- redacted logs or synthetic test artifacts,
- any suggested mitigation or disclosure constraint.

Use disposable files, a disposable Windows account or runner, and synthetic data whenever reproduction could mutate native state. Do not test against another person's system or real wallpaper library.

## Intended safety boundaries

PureWall is designed around these boundaries:

- no HKLM writes,
- no Windows policy edits,
- no disabling the Windows 11 context menu,
- no recursive deletion of user-selected directories,
- user-confirmed wallpaper deletion goes through the Windows Recycle Bin,
- context-menu and autostart writes stay inside documented PureWall-owned HKCU entries,
- wallpaper libraries use local paths; UNC and mapped network paths are rejected,
- signing material and release secrets never enter repository files or logs,
- updater and Windows installer signatures are separate controls and must both be verified before a release claims them.

The current source tree does not by itself prove that a binary is signed, provenance-attested, lifecycle-tested, or safe to install. Only evidence from an actually run, matching release may make those claims.

## Public safety questions

Use the public safety form only for non-sensitive policy clarification or documentation corrections. Regular setup questions belong in [GitHub Discussions](https://github.com/WiseZenn/PureWall/discussions), and reproducible non-security defects belong in the bug form. See [SUPPORT.md](SUPPORT.md).
