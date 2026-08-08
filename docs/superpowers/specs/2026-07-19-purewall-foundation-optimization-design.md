# PureWall Foundation Optimization Design

**Date:** 2026-07-19
**Status:** Approved
**Scope:** PureWall only; PureWall-X is explicitly deferred

## 1. Product Definition

PureWall is an open-source Windows wallpaper manager with the interaction model of a local music player:

1. Import one or more local wallpaper folders.
2. Browse the library or start random rotation.
3. Use Next, Like, Dislike, and Pause as the primary controls.
4. Return to liked wallpapers, tags, and collections for lightweight organization.

PureWall succeeds by being understandable, reliable, safe, and responsive. It does not need a recommendation engine or a novel content-discovery model.

### 1.1 Included capabilities

PureWall keeps the capabilities already present in the product:

- local folder import, persistent watchers, and paginated gallery browsing;
- automatic and manual random playback;
- Like, Dislike, Hide, restore, Recycle Bin deletion, and undo flows;
- search, tags, and collections as separate organization axes;
- multi-display playback modes;
- system tray, PureWall-owned context-menu entries, and the optional floating widget;
- lightweight playback statistics;
- dark and light themes;
- local SQLite metadata and backward-compatible migrations.

### 1.2 Explicit non-goals

PureWall will not add the following as part of this optimization program:

- AI recommendation, recommendation explanations, mood analysis, or automatic semantic classification;
- online wallpaper feeds, accounts, cloud synchronization, or social features;
- an image editor, marketplace, or plugin store;
- a live/video/web wallpaper engine;
- placeholder modules or speculative APIs for PureWall-X;
- a one-time rewrite of the existing application.

## 2. PureWall and PureWall-X Boundary

PureWall and the future PureWall-X will both be open source. The intended long-term relationship is two independent applications sharing a reusable core.

That relationship is a future direction, not a current deliverable. The present program must:

- optimize and release PureWall as the only application;
- preserve stable domain boundaries that could later be extracted;
- avoid creating a PureWall-X repository, binary, workspace member, feature flag, or extension SDK;
- defer the actual shared-core extraction until PureWall behavior and data contracts are stable;
- require a separate accepted ADR before any future shared-core extraction changes repository or package boundaries.

PureWall-X must not influence the current UI, feature list, release schedule, or acceptance criteria.

## 3. Architecture Strategy

The architecture strategy is "extractable, but not extracted." PureWall remains one Tauri application while its responsibilities are made clearer incrementally.

```text
Vue UI
  -> application state and use cases
  -> typed Tauri command/event boundary
  -> library, playback, rating, collection, and settings domains
  -> SQLite, file-system, and Windows adapters
```

### 3.1 Current boundaries

- Vue components own presentation and direct user interaction.
- Pinia application state coordinates gallery, playback, settings, and media presentation.
- Tauri commands and events are the typed process boundary.
- Rust domain modules own library, playback, rating, collection, and safety rules.
- SQLite, Windows COM, registry, filesystem watchers, and Recycle Bin integration are platform adapters.

### 3.2 Incremental decomposition

Large files may be split only when the active task already changes their responsibility:

- `src-tauri/src/main.rs`: gradually move command groups and platform orchestration into focused modules;
- `src-tauri/src/db.rs`: separate migrations, gallery queries, taxonomy, and playback history behind stable database interfaces;
- `src/stores/wallpapers.ts`: separate gallery, playback, settings, and media state while retaining a stable consumer-facing facade;
- `src/styles.css`: group existing tokens, layout, components, and responsive rules without redesigning the visual language.

No phase is allowed to combine a behavior change with an unrelated large refactor.

## 4. User-Facing Behavior Contract

### 4.1 Primary loop

The primary user journey is:

```text
Import folder
  -> browse or start random playback
  -> Like / Dislike
  -> Next
  -> automatic rotation
  -> review liked wallpapers
```

The current wallpaper plus Next, Like, Dislike, and Pause remain the highest-priority controls in the main experience.

### 4.2 Rating and random-selection semantics

- A normal available wallpaper has one random-selection ticket.
- A liked available wallpaper has two tickets.
- A disliked wallpaper is excluded from automatic random playback.
- A disliked wallpaper remains visible in the Disliked view and can be restored.
- Hidden, unavailable, and pending-deletion wallpapers are excluded from playback.
- Multi-display selection samples without replacement whenever enough unique eligible wallpapers exist.
- Manual Next and automatic rotation use the same eligibility and weighting contract.

The user-facing term is **Disliked**. Existing internal names such as `Passed`, `blacklisted`, or negative rating values may remain implementation details only where changing them would create unnecessary migration risk.

### 4.3 Organization semantics

- Tags describe wallpapers.
- Collections are user-curated sets.
- Folders are source locations.
- Ratings influence playback.

These concepts must not be silently represented by the same database or UI operation.

## 5. Optimization Program

### Phase 1: Correct existing P0 behavior

Deliverables:

- fix collection creation so it calls the collection path rather than tag creation;
- expose the Disliked filter and standardize user-facing terminology;
- enforce the approved random-selection contract in backend and UI behavior;
- rename the current Shortcuts surface to Command Line unless real global hotkeys are implemented as a separately approved feature;
- make every navigation and primary control reachable at the configured 800x600 minimum window;
- add behavior-level regression coverage for these flows.

Acceptance:

- Like, Dislike, restore, and collection operations reach the correct backend commands;
- manual and automatic playback share the same candidate rules;
- 800x600 has no unreachable system navigation;
- tests exercise behavior rather than checking only for source-code strings.

### Phase 2: Harden the playback loop

Deliverables:

- route main-window, tray, widget, context-menu, and CLI playback actions through one application action contract;
- verify restart restoration for the current wallpaper, pause state, interval, and display mode;
- handle empty libraries, insufficient candidates, missing files, and bounded transient Windows COM failures consistently;
- verify multi-display uniqueness and play-history accounting.

Acceptance:

- equivalent actions from every entry point produce equivalent state and feedback;
- unavailable or disliked wallpapers never enter automatic playback;
- failures are visible and never become silent no-ops or indefinite loading states.

### Phase 3: Complete local library management

Deliverables:

- show all persisted watched folders;
- remove a watched source without deleting its original images;
- manually rescan a source;
- expose missing or offline sources with a recovery path;
- complete batch rating, visibility, tag, and collection operations;
- export and restore ratings, tags, collections, watched sources, and non-sensitive settings locally.

Acceptance:

- removing one source does not damage unrelated library state;
- a temporarily missing file retains its rating, tags, and collection membership when it returns;
- a backup can restore user-managed metadata into a clean database;
- existing local-path, scan-bound, registry, and Recycle Bin safety rules remain intact.

### Phase 4: Reduce interaction and layout friction

Deliverables:

- keep first-run onboarding focused on selecting a local wallpaper folder;
- add a concise local-only statement without expanding onboarding into a tutorial;
- standardize empty, loading, success, and failure states;
- move low-frequency settings and statistics away from primary playback controls;
- verify 800x600, 125% and 150% Windows scaling, long paths, and enlarged text;
- complete keyboard focus and semantic image coverage for primary flows.

Acceptance:

- a new user can import a folder and start playback without documentation;
- playback controls remain visually and interactively dominant;
- supported minimum size and common Windows scaling do not hide required controls;
- primary flows are keyboard reachable.

### Phase 5: Close the open-source release loop

Deliverables:

- tag-driven Windows release automation;
- signed installers, checksums, and build provenance;
- a signature-verified application update path;
- clean install, upgrade, and uninstall smoke coverage;
- README screenshots, quick start, FAQ, and troubleshooting;
- issue forms, pull-request template, support guidance, security contact, and roadmap;
- explicit repository policy for generated images, local agent configuration, caches, and build artifacts.

Acceptance:

- a new contributor can build and test a clean clone using documented commands;
- public binaries are produced by a traceable CI workflow;
- uninstall cleanup targets only PureWall-owned files and registry entries;
- no public release depends on an undocumented manual build step.

### Phase 6: Perform targeted maintainability work

Deliverables:

- incrementally split the large orchestration, database, store, and stylesheet files along existing responsibilities;
- keep compatibility facades while internal modules move;
- strengthen migration, command-boundary, and public store-behavior tests;
- document each accepted boundary change before implementation.

Acceptance:

- each refactor preserves behavior and Tauri command compatibility;
- no phase performs a big-bang rewrite;
- future work has an obvious module destination instead of growing central files indefinitely;
- PureWall-X and shared-core extraction remain outside this program.

## 6. Error Handling and Safety

- User-triggered failures use the existing typed `CommandResult<T>` boundary and visible operation-failed presentation.
- Background media work has success, bounded retry, and terminal failure states.
- Missing files remain recoverable metadata records rather than being deleted automatically.
- Source removal never deletes original wallpaper files.
- Destructive wallpaper deletion uses the Recycle Bin and preserves the existing undo window.
- PureWall may modify only its documented HKCU registry entries.
- No task may modify HKLM, Windows policy, or the Windows 11 context-menu mode.
- Database migrations remain additive and backward compatible unless a separately accepted ADR defines a safe alternative.

## 7. Verification and Process Documentation

Every implementation task starts with the project Pre-Flight checklist and ends with relevant targeted tests plus the full completion gate:

```text
cargo fmt -- --check
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
npx vue-tsc --noEmit
npm run test:unit
npm run build
git diff --check
```

UI tasks also verify:

- 800x600 and a normal desktop viewport;
- keyboard operation;
- empty, loading, success, and failure states;
- relevant 125% and 150% Windows scaling behavior.

Installer or system-integration tasks also verify:

- clean install, upgrade, and uninstall behavior;
- the exact PureWall-owned registry paths affected;
- absence of HKLM, policy, or unrelated shell changes.

Documentation rules:

- append every completed change and its verification evidence to `docs/project-docs/CHANGELOG_AI.md`;
- append to `docs/project-docs/AI_DIARY.md` only when a new pitfall is discovered;
- update `docs/project-docs/ARCHITECTURE.md` when implemented system boundaries change;
- create and accept an ADR before an architectural boundary changes;
- record unresolved work in the latest `CHANGELOG_AI.md` entry rather than implying completion.

## 8. Implementation Planning Boundary

The detailed implementation plan will decompose the six phases into independently testable plans rather than one large cross-system change. Phase 1 is the first implementation plan. Later phases begin only after the preceding milestone is verified or explicitly reprioritized by the user.
