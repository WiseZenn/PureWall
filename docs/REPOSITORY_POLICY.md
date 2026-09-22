# Repository Artifact Policy

PureWall keeps its public repository reproducible, reviewable, and free of local-only or sensitive residue.

## Tracked public artifacts

- Application source, build configuration, tests, and maintained documentation belong in the repository.
- Curated release and documentation images live under `design/` or `docs/images/`.
- An image is promoted into one of those tracked directories only after it has a clear public purpose, contains no private user data, and has been reviewed for licensing and attribution.
- Release binaries, installers, checksums, signatures, and provenance are distributed through a matching GitHub release when that release workflow exists; they are not committed as source files.

## Local and generated artifacts

- Raw generated design candidates, QA screenshots, traces, recordings, and other review captures stay under the ignored `output/` directory.
- `.agents/`, `.codex/`, and `skills-lock.json` are local agent configuration. They are never required to build, test, or contribute to PureWall.
- Dependency directories, compiler output, build caches, Python bytecode, IDE state, and operating-system metadata remain ignored because they can be reproduced or are machine-specific.

To promote a useful QA or design image, first remove personal paths and user content, confirm its origin and license, then copy the reviewed final asset into `design/` or `docs/images/` and reference that tracked path from public documentation.

## Secrets and signing material

Secrets and signing material are never committed. This includes private updater keys, certificate exports, `*.pfx`, `*.pem`, `*.key`, generated release-signing configuration, tokens, passwords, and secret-bearing logs.

Privileged release credentials belong in GitHub Actions secrets. Public keys, certificates, checksums, signatures, or provenance may be published only through the reviewed release process and must not be presented as completed evidence before that process has actually run.
