# PureWall Roadmap

This roadmap describes direction, not a promise of dates. A milestone is complete only when its implementation and verification evidence are recorded in the repository.

## Completed foundation

- Phase 1: local wallpaper library, Windows wallpaper control, weighted random playback, ratings, and local persistence.
- Phase 2: one typed playback loop shared by the main window, widget, tray, CLI, and timer, with explicit completion ownership and durable side-effect boundaries.
- Phase 3: tags, collections, hidden items, batch operations, local source lifecycle, pagination, availability tracking, and portable metadata backup/restore.
- Phase 4: display-aware playback, focus auto-pause, yearly insights, accessibility/responsive hardening, and full local verification gates.

## Phase 5: trustworthy open-source release loop

Completed in the repository:

- release trust boundary and executable version/tag contract,
- public onboarding, support, security, contribution, issue, pull-request, and repository-artifact policies.

Still requiring implementation or external evidence before a release can claim them:

- tag-driven draft-release automation,
- Authenticode-signed Windows artifacts, Tauri updater signatures, checksums, and provenance,
- an explicit signature-verified update experience,
- disposable clean-install, upgrade, and uninstall lifecycle coverage,
- final independent release-candidate review and maintainer publication decision.

Automation may prepare a draft. Publishing remains a deliberate maintainer action, and documentation must not treat an unrun workflow as release evidence.

## Phase 6: targeted maintainability

After the release loop is proven, Phase 6 may make narrow internal improvements supported by measurements and tests, including:

- reducing remaining command/orchestration concentration in `main.rs`,
- strengthening boundary and failure-path tests around release, media, source, and backup workflows,
- improving contributor diagnostics and reproducible performance checks,
- retiring compatibility paths only after migration evidence shows they are no longer needed.

Phase 6 is not a broad rewrite. Existing local-library, database, playback, and Windows safety contracts remain the compatibility baseline.

## Explicitly deferred

- **PureWall-X** is a later product built after the ordinary PureWall foundation and release loop are complete. Online feeds, recommendation services, or PureWall-X-only behavior do not belong in current PureWall feature requests.
- **Shared-core extraction** is deferred until PureWall-X creates demonstrated, stable duplication. PureWall will not introduce a speculative shared framework before two real consumers define the boundary.
