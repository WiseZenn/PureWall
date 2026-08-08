# PureWall Windows 11 Fluent UI Refactor Plan

## Summary
Refactor the existing Phase 5 UI into a more native Windows 11 Fluent/Mica-inspired desktop app while preserving the current information architecture and Pinia/Tauri behavior. Treat the provided image as the accepted visual reference. This is a frontend-focused refactor: no Rust commands, SQLite schema, registry behavior, or wallpaper playback logic should change.

Implementation will be split into 4 phases:
1. Design tokens, app shell, theme foundation
2. Icon system, sidebar, current wallpaper panel, grid
3. Inspector panel, chart, accessibility/interactions
4. Verification and documentation write-up

## Key Changes
- Add semantic theme tokens in `src/styles.css` for `data-theme="dark"` and `data-theme="light"` using the exact token values from the request.
- Replace legacy tokens like `--bg-primary`, `--surface`, `--border`, and macOS/SF-oriented styling with the requested Windows-oriented token names.
- Refactor shell into stable desktop layout:
  - `AppShell`: 240px sidebar, flexible main, 340px inspector
  - `TitleBar`: about 56px
  - `StatusBar`: about 36px
  - main content remains left sidebar, top search/filter toolbar, current wallpaper preview, wallpaper grid, right inspector, bottom status bar.
- Split current monolithic UI sections into reusable components:
  - `SidebarItem`, `SearchToolbar`, `CurrentWallpaperPanel`, `WallpaperControls`, `SettingRow`
  - `WallpaperGrid`, `WallpaperCard`
  - `InspectorPanel`, `MetadataRow`, `ColorSwatchList`, `TagPill`, `YearlyInsightsChart`
  - `StatusBar`, `IconButton`, `ToggleSwitch`
- Replace `AppIcon.vue` with a unified custom SVG outline system:
  - 18/20px optical size
  - `stroke-width="1.75"`
  - rounded caps/joins
  - `currentColor`
  - includes all required icons and a custom `PureWallLogo` that combines a wallpaper frame, landscape silhouette, and subtle P-like negative-space cue.
- Keep all existing store actions and app behavior:
  - search/filter/sort
  - next/like/dislike/pause
  - interval/display mode/focus pause
  - wallpaper selection, batch actions, tags, hidden state
  - yearly stats display
  - Tauri titlebar window controls
- Do not add macOS traffic-light controls, dashboard-style hero sections, or heavy chart dependencies.

## Phase Plan
**Phase 1: Tokens + Shell**
- Add `[data-theme="dark"]` and `[data-theme="light"]` token blocks in `src/styles.css`.
- Default theme to dark in app startup, with switching supported by setting `document.documentElement.dataset.theme = "dark" | "light"`.
- Create/refactor `AppShell.vue`, `TitleBar.vue`, and `StatusBar.vue`.
- Move status bar markup out of `Home.vue`.
- Use `Segoe UI Variable`, `Segoe UI`, system fallback. Remove `-apple-system`, `SF Pro Text`, and “macOS-style” glass comments/classes.

**Phase 2: Sidebar + Main Gallery**
- Refactor sidebar to match the requested navigation:
  - Library, Liked, Hidden, Tags
  - right-aligned counts
  - selected state with accent left line and active background
  - quieter System section
  - compact Storage section
  - bottom Ready indicator.
- Build `SearchToolbar`, `CurrentWallpaperPanel`, `WallpaperControls`, and `SettingRow` from existing `Gallery.vue` behavior.
- Convert gallery to `WallpaperGrid` + `WallpaperCard`:
  - 3-column adaptive grid
  - 16:9 thumbnails
  - subtle hover overlay
  - selected accent border
  - small heart badge
  - no layout shift on hover actions.

**Phase 3: Inspector + Chart + A11y**
- Rebuild inspector as compact tool panel:
  - selected preview
  - file name
  - metadata rows for resolution, file size, added date, folder path
  - color swatches
  - compact tags
  - yearly insights chart
  - compact stats
  - View Full Year Stats button.
- Implement `YearlyInsightsChart` with div/SVG bars only:
  - muted inactive bars
  - teal active month
  - small month labels
  - no grid, gradients, or chart library.
- Add interaction states:
  - hover, pressed, active, selected, disabled, focus-visible.
- Add `aria-label` to all icon-only buttons and ensure selected states use more than color alone.

**Phase 4: Verification + Docs**
- Run required pre/post checks:
  - `cargo check`
  - `npx vue-tsc --noEmit`
  - `npm run build`
- Use rendered visual QA against the provided reference image, with mocked Tauri IPC if needed.
- Verify:
  - desktop layout stability
  - narrow-width behavior
  - search/filter/sort still work
  - wallpaper card selection still works
  - next/like/dislike/pause still call existing store actions
  - interval/display/focus pause controls still call existing handlers.
- Update documentation:
  - `docs/project-docs/CHANGELOG_AI.md`: append UI refactor scope, changed files, verification evidence, unresolved items.
  - `docs/project-docs/AI_DIARY.md`: append only if a new pitfall appears.
  - `docs/project-docs/ARCHITECTURE.md`: update Phase 5 UI addendum to mention tokenized Fluent shell and component split.
  - `docs/project-docs/DECISIONS.md`: no new ADR unless implementation changes information architecture or backend contracts.

## Public Interfaces / Types
- No Rust command changes.
- No SQLite schema changes.
- No registry/autostart behavior changes.
- Frontend component interface additions:
  - `IconButton`: `icon`, `label`, `active?`, `disabled?`
  - `ToggleSwitch`: `checked`, `label`, emits update/change
  - `TagPill`: `label`, `active?`, `color?`, `removable?`
  - `MetadataRow`: `icon`, `label`, `value`, optional `title`
  - `YearlyInsightsChart`: monthly stats array plus active month.
- Theme interface:
  - root attribute: `data-theme="dark"` or `data-theme="light"`.

## Test Plan
- `cargo check` must pass with zero errors; existing unused warnings may remain.
- `npx vue-tsc --noEmit` must pass.
- `npm run build` must pass.
- Visual QA:
  - compare rendered desktop screenshot to `D:\Desktop\PureWall\ChatGPT Image 2026年6月3日 20_18_39.png`
  - inspect desktop and narrow viewport
  - confirm no text overlap, no macOS-like window controls, no heavy dashboard styling.
- Functional QA:
  - search narrows wallpapers
  - sidebar filters change visible list
  - grid card click changes active inspector wallpaper
  - double-click/set action still sets wallpaper
  - like/dislike/reset states update
  - next/pause controls still work
  - interval/display/focus controls still call existing logic
  - tags assign/unassign still work.

## Assumptions
- The provided image is the accepted visual reference.
- This task is UI-only unless a real blocker appears.
- Bottom `Auto-start` should remain non-mutating visual/status chrome unless explicitly wired later.
- Metadata currently unavailable from the store, such as real resolution/file size/color extraction, should be shown using existing available fields or conservative placeholders until backend metadata exists.
- Documentation updates happen after code changes, following the project append-only rules.
