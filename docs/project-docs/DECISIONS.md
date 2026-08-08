# DECISIONS.md

> 架构决策记录 (ADR)。仅收录和整体架构生死相关的顶层决策。

---

## ADR-001: Tauri 2 + Vue 3 + TypeScript

- **Status**: accepted
- **Decision**: 使用 Tauri 2 作为应用框架，Rust 后端 + Vue 3 前端
- **Context**: 需要同时满足 Windows 底层 API 控制和 macOS 级 UI 表现
- **Options**:
  - A. Tauri 2 + Vue 3 — 小安装包 (5-10MB)、低内存 (50-100MB)、Rust 调用 Windows API
  - B. Electron + React — 成熟生态但安装包大 (150-200MB)、内存高 (200-400MB)
  - C. C# + WinUI 3 — 原生性能但 Web UI 灵活性受限
- **Why A**: Rust 对 Windows API 原生支持 + Web 前端实现毛玻璃/动画/瀑布流无技术障碍 + 安装包极小
- **Consequences**: 团队需熟悉 Rust，build 工具链较重（需 MSVC）

## ADR-002: base64 缩略图替代 asset 协议

- **Status**: accepted
- **Decision**: 通过 Rust Command 读取图片 → base64 编码 → 前端 data URL 显示
- **Context**: Tauri 2 的 asset 协议权限配置复杂，本地文件加载失败
- **Options**:
  - A. Rust base64 缩略图 — 简单可靠，有内存缓存，但首次加载慢
  - B. Tauri asset 协议 — 性能最优但权限配置复杂，调试困难
  - C. HTTP 静态文件服务 — 需额外线程，增加复杂度
- **Why A**: 实现简单、无额外依赖、256px JPEG 体积小 (~15KB/张)
- **Consequences**: 首次加载全部缩略图需等待；后续缓存命中无感

## ADR-003: 加权随机 2x 而非 3x

- **Status**: accepted
- **Decision**: 喜欢的壁纸权重 2x（而非最初 3x），保持足够随机性
- **Context**: 用户反馈 3x 权重导致随机"不够随机"，喜欢的壁纸过于频繁出现
- **Options**:
  - A. 2x 权重 — 适度推荐，随机感保留
  - B. 3x 权重 — 推荐感强但随机性差
  - C. 等概率 — 完全随机但无推荐效果
- **Why A**: 平衡推荐效果与随机体验。10 普通 + 5 喜欢 → 喜欢每张概率约普通 1.3 倍
- **Consequences**: 用户仍可手动调整权重

## ADR-004: patchWallpaper 数组替换策略

- **Status**: accepted
- **Decision**: 壁纸属性变更时 slice+spread 重建整个数组，而非直接修改对象
- **Context**: Pinia store 中 wallpaper 对象的属性修改不触发 Vue 模板更新
- **Options**:
  - A. 数组替换 — 简单可靠，保证响应式
  - B. Vue reactive() 深度包装 — 性能开销大
  - C. 手动 forceUpdate — 破坏 Vue 一致性
- **Why A**: 最小改动、无额外依赖、性能可接受（<100ms for 500 items）
- **Consequences**: 大量壁纸时可能有性能问题（未验证 1000+ 场景）

## ADR-005: 通知式轮播定时器

- **Status**: accepted
- **Decision**: 轮播定时器使用 `Condvar` + generation 通知模型等待 interval、pause、focus auto-pause 和 shutdown 变化，不再通过 `thread::sleep(1s)` 轮询累计时间。
- **Context**: R-39 核查确认 1 秒轮询会持续唤醒后台线程；PureWall 现在已经有持久后台线程和明确 shutdown 生命周期，适合用通知式等待降低空转并更快响应设置变化。
- **Options**:
  - A. 保留 1 秒轮询 — 实现简单，但后台线程会持续周期性唤醒
  - B. 条件变量 + notify — 复杂度略高，但 interval/pause/shutdown 变化可立即唤醒，空闲时不持续轮询
- **Why B**: 当前代码已经需要响应 pause、focus auto-pause、interval 和 shutdown，通知式等待可以把这些状态变化收敛到一个轻量 signal，减少持续唤醒且不改变播放选择语义。
- **Consequences**: 所有会影响轮播等待状态的写路径必须调用 rotation signal notify；未来新增暂停来源或 interval 来源时也必须维护该通知。
 
## ADR-006: Hash Router single entry + pre-created widget window

- **Status**: accepted
- **Decision**: Use one Vue/Vite entry with Vue Router hash mode. The main window loads `/`, and the floating widget loads `/#/widget` from a statically declared Tauri window.
- **Context**: Dynamic `WebviewWindowBuilder` creation with an external widget HTML file caused the widget to render white and could make the main window stop responding (#widget-005).
- **Options**:
  - A. Single entry + hash route + static widget window
  - B. Keep external widget HTML and create the window at runtime
  - C. Keep the widget only inside the main window
- **Why A**: Keeps Vite/Tauri loading on the known-good app entry, removes runtime webview creation from the hot path, and still allows the widget to be an independent top-level window.
- **Consequences**: Main and widget windows have separate WebView/Pinia instances, so widget actions must keep using Rust commands and Tauri events for synchronization.

## ADR-007: Phase 3 gallery management schema

- **Status**: accepted
- **Decision**: Store custom tags in normalized `tags` and `wallpaper_tags` tables, store blacklist state as a `wallpapers.blacklisted` flag, and expose sorting/filtering through whitelisted Rust query commands.
- **Context**: Phase 3 needs tags, batch operations, blacklist management, and sorting without destabilizing the Phase 1/2 playback path.
- **Options**:
  - A. Normalize tags and add a lightweight blacklist column
  - B. Store tags as JSON text inside `wallpapers`
  - C. Keep tags only in frontend state
- **Why A**: Keeps queries simple, supports many-to-many tagging, lets blacklist apply consistently to gallery and playback, and avoids ad hoc JSON parsing in SQLite.
- **Consequences**: Existing databases need a small additive migration at startup. Normal gallery and playback exclude blacklisted wallpapers by default.

## ADR-008: Phase 4 polish controls and playback telemetry

- **Status**: accepted
- **Decision**: Use the Windows `IDesktopWallpaper` COM API for display-aware wallpaper placement, a PureWall-owned focus monitor thread for full-screen auto-pause, and a SQLite `play_events` table for yearly playback analytics.
- **Context**: Phase 4 needs multi-display polish, game/focus auto-pause, and a yearly stats dashboard while preserving the existing gallery and weighted playback path.
- **Options**:
  - A. Add a lightweight Phase 4 layer around the existing playback engine
  - B. Replace the playback engine with Windows slideshow APIs
  - C. Keep display/focus/statistics as frontend-only state
- **Why A**: It keeps PureWall in control of rating-aware selection, works with current SQLite metadata, and avoids delegating behavior to Windows slideshow state that the app cannot fully inspect.
- **Consequences**: Per-display independent mode records one play event per monitor on each rotation. Changing display mode applies immediately: Same and Span reuse the current wallpaper, while Separate selects one wallpaper per detected display. The yearly dashboard starts tracking detailed history after this migration; older aggregate `play_count` values remain available in the existing stats counters.

## ADR-009: Phase 5 desktop workbench UI information architecture

- **Status**: accepted
- **Decision**: Reorganize the main window into a three-pane desktop workbench: a narrow navigation rail, an image-led gallery workspace, and a right-side inspector for selected wallpaper details, playback controls, system toggles, and compact analytics.
- **Context**: The early PureWall UI exposed the core features but felt fragmented: filtering, playback, display mode, widget, and yearly stats competed in the same sidebar/gallery area. Phase 5 needs a clearer visual hierarchy without changing the backend contract.
- **Options**:
  - A. Three-pane workbench with a focused hero/stage area and inspector panel
  - B. Keep the Phase 4 layout and only restyle colors/spacing
  - C. Build separate pages for gallery, settings, and analytics
- **Why A**: It keeps the first screen usable as the real app, gives wallpapers more visual weight, keeps repeated management actions close to the selected image, and avoids introducing routing or backend changes just for layout polish.
- **Consequences**: `Sidebar.vue` becomes navigation/tag focused, `Gallery.vue` owns the wallpaper stage and masonry list, `InspectorPanel.vue` owns selected-image actions plus playback/system controls, and the store needs an explicit active wallpaper selection separate from the current Windows wallpaper path.

## ADR-010: Persist scanned image metadata in SQLite

- **Status**: accepted
- **Decision**: Persist image width, height, and file size on each wallpaper row during folder/file scans, while retaining an on-demand metadata refresh command for existing records.
- **Context**: Inspector metadata previously depended entirely on reading the selected file again. Missing access, stale paths, or command timing left resolution and file size as `Unknown` even though the scanner had already read them.
- **Options**:
  - A. Persist scanner metadata and refresh on demand
  - B. Read every selected file on demand only
  - C. Read metadata for every row whenever the gallery loads
- **Why A**: It makes inspector rendering immediate and resilient, avoids repeated filesystem work, and keeps old databases compatible through an additive migration.
- **Consequences**: Existing rows start with zero-valued metadata until successfully refreshed or re-imported; new scans populate metadata immediately.

## ADR-011: Dual workspace and Quiet Canvas presentation modes

- **Status**: accepted
- **Decision**: Use the Gallery Studio + Control Deck direction as PureWall's default workbench, and provide Quiet Canvas as a persistent, user-selectable immersive presentation mode.
- **Context**: The full management interface needs high information density and complete controls, while wallpaper appreciation benefits from a visually quiet surface without sidebars, inspectors, or gallery management chrome.
- **Options**:
  - A. One workbench plus a switchable Quiet Canvas mode
  - B. Force the whole application into an immersive layout
  - C. Keep a single dense three-pane interface
- **Why A**: It preserves professional management efficiency while giving PureWall a distinctive, image-first experience. Both modes reuse the same store and backend commands, avoiding duplicated behavior.
- **Consequences**: Workspace mode is frontend-only and persisted independently from the dark/light theme. Quiet Canvas must always provide an obvious route back to the workbench and retain essential playback actions.

## ADR-012: User-visible operation failures

- **Status**: accepted
- **Decision**: Keep Tauri command errors as user-readable messages at the boundary, and surface important runtime/CLI failures through frontend notifications plus the CLI log.
- **Context**: PureWall has multiple invocation paths: the main UI, widget window, tray, right-click CLI action, and single-instance forwarding. Silent failures made wallpaper, registry, and playback problems hard to diagnose.
- **Options**:
  - A. Keep boundary errors user-readable and emit `operation-failed`/notifications for background paths
  - B. Panic/log only in Rust and rely on developer tools
  - C. Block release until every command uses a structured error enum
- **Why A**: It immediately improves user feedback without destabilizing all command signatures. A structured enum migration remains a separate typed-error cleanup.
- **Consequences**: Some commands still return `Result<_, String>`; user-visible coverage is intentional, but R-53 can later replace stringly errors with a typed command error shape.

## ADR-013: Hardened WebView CSP and Tauri exposure

- **Status**: accepted
- **Decision**: Use a restrictive Tauri WebView CSP with `script-src 'self'`, keep `withGlobalTauri` disabled, and allow inline styles only for the current Vue/Vite styling path.
- **Context**: The review identified `unsafe-inline`/`unsafe-eval` script risk and global Tauri exposure. PureWall does not need third-party scripts or `window.__TAURI__` globals for normal operation.
- **Options**:
  - A. Restrict scripts to self and use imported Tauri APIs
  - B. Keep broad CSP for development convenience
  - C. Remove inline styles too and refactor all dynamic style bindings immediately
- **Why A**: It removes the highest-risk script surface while preserving current Vue style bindings and local image/font loading.
- **Consequences**: Future frontend dependencies must work without eval-like script behavior. Removing style `unsafe-inline` requires a dedicated UI styling pass.

## ADR-014: PureWall-owned registry surface only

- **Status**: accepted
- **Decision**: PureWall may modify only its own HKCU registry entries through direct `reg.exe` process calls: context menu keys under PureWall/PW* names and the `PureWall` Run autostart value.
- **Context**: Registry writes are needed for right-click actions and autostart, but system-level changes such as Win11 menu policy tweaks are outside the app's ownership boundary.
- **Options**:
  - A. Limit writes to PureWall-owned HKCU entries and pass arguments directly to `reg.exe`
  - B. Use generated PowerShell scripts for all registry operations
  - C. Allow broader Windows shell policy changes for convenience
- **Why A**: It avoids shell quoting pitfalls, keeps registry side effects reversible, and respects the project rule against automated system setting changes.
- **Consequences**: Context-menu UX stays within the supported PureWall keys. Any non-PureWall registry change requires explicit user risk confirmation outside normal automation.

## ADR-015: Incremental Rust module extraction

- **Status**: accepted
- **Decision**: Keep extracting platform and domain responsibilities into focused Rust modules while `main.rs` remains the command registration and orchestration boundary until a dedicated command-router refactor lands.
- **Context**: PureWall already has modules for autostart, context menu, DB, focus, scanner, shell metadata, thumbnails, tray, wallpaper, and app paths, but `main.rs` still owns many command handlers and shared playback orchestration.
- **Options**:
  - A. Incrementally extract cohesive modules and leave command wiring in `main.rs`
  - B. Rewrite the backend into a full layered architecture now
  - C. Keep all new behavior in `main.rs`
- **Why A**: It reduces risk during review-fix work, keeps Tauri handler registration obvious, and still prevents platform-specific logic from accumulating in the entry file.
- **Consequences**: `main.rs` size remains a tracked P2 follow-up (R-52). New platform APIs should start in dedicated modules, then expose small command-facing functions.

## ADR-016: Hide release console window

- **Status**: accepted
- **Decision**: Use `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` so release builds do not show a console window during normal UI or right-click CLI action use.
- **Context**: PureWall is a Windows desktop app. CLI-style right-click actions are implementation details and should not flash a console for end users.
- **Options**:
  - A. Hide the console in release builds only
  - B. Always keep a console visible for diagnostics
  - C. Hide the console in debug and release builds
- **Why A**: It preserves developer diagnostics during debug runs while keeping release UX clean.
- **Consequences**: Release CLI failures need another feedback path, so PureWall keeps CLI logging and forwards failures to the running app when possible.

## ADR-017: Indexed wallpaper patch updates

- **Status**: accepted
- **Decision**: Keep `wallpapers` as the ordered array consumed by the gallery, but maintain a path-to-index map and patch individual array slots with replacement objects instead of rebuilding the whole array for every item update.
- **Context**: ADR-004 chose full array replacement because mutating fields on an object returned by `find()` did not reliably refresh Vue templates. Review item R-43 identified that the slice/spread strategy copies the full array for frequent single-wallpaper updates after large-library rendering had already been improved.
- **Options**:
  - A. Keep full slice/spread replacement for every patch
  - B. Replace the indexed array slot with a new object and maintain a path index
  - C. Normalize wallpapers into a record plus ordered id list
- **Why B**: It preserves the current array contract for filtering, sorting, rendering, and undo snapshots while avoiding the most wasteful full-array copy on common rating/metadata/play-count updates.
- **Consequences**: Every code path that replaces, restores, or removes the wallpaper array must rebuild or maintain the path index. Direct field mutation of wallpaper objects remains avoided; patches still replace the changed object.
## ADR-018: Asset-backed image delivery and virtualized gallery rendering

- **Status**: accepted
- **Decision**: Replace Base64 image transfer over Tauri IPC with file-backed image delivery through the Tauri asset protocol for PureWall-owned thumbnail and preview cache files, and virtualize gallery rendering so large wallpaper libraries do not create one DOM subtree per wallpaper.
- **Context**: Packaged-app testing showed severe startup and interaction stutter with large or cold-cache libraries. The current path still decodes/generated derivatives on the backend and pushes Base64 strings through IPC, which inflates payload size, increases JS heap pressure, and prevents the WebView from using normal image loading and caching behavior. The gallery also needs a bounded render surface for thousands of wallpapers.
- **Options**:
  - A. Enable the asset protocol for PureWall-owned cache files and convert returned file paths with `convertFileSrc`, then virtualize visible gallery rows
  - B. Keep Base64 IPC and tune batch sizes only
  - C. Expose arbitrary user-selected wallpaper folders directly through the asset protocol
- **Why A**: It removes the largest IPC and JS heap bottleneck while keeping the asset scope narrow. PureWall controls the derivative cache directory, so the WebView can load images without granting broad read access to every selected wallpaper source folder. Virtualized rows keep DOM cost bounded independently of library size.
- **Consequences**: Thumbnail and preview commands should return cache file paths/asset URLs instead of Base64 data URLs. The asset-protocol scope must remain limited to PureWall's app-data derivative cache unless a future persisted-scope design explicitly grants selected source directories. Gallery layout needs predictable row sizing and overscan so rendering stays smooth while still preloading nearby thumbnails. Large-library loading uses a paginated `get_wallpapers_page(filter, sort, search, offset, limit)` command so search/filter/sort semantics stay in SQLite rather than only the currently loaded frontend slice.

## ADR-019: Background thumbnail generation with event refill

- **Status**: accepted
- **Decision**: Keep thumbnails in PureWall's asset-scoped derivative cache, but make cold thumbnail generation asynchronous: thumbnail batch commands return existing cache hits immediately and enqueue missing derivatives on background worker threads that emit `thumbnail-generated` when ready.
- **Context**: Phase 1 removed Base64 IPC and virtualized the gallery, but cache misses can still force a Tauri command to decode, resize, and encode images before returning. On a cold large library this still concentrates media CPU work near visible scrolling.
- **Options**:
  - A. Return cache hits immediately, enqueue misses, and let the frontend fill thumbnails from generation events
  - B. Keep synchronous generation and rely on smaller batches/virtual rows
  - C. Generate every missing thumbnail during startup idle time
- **Why A**: It keeps startup and scroll interactions responsive, avoids broad asset scopes, and lets the UI show lightweight placeholders until individual derivatives are ready. It also preserves the Phase 1 asset URL path and does not require a new database schema.
- **Consequences**: Frontend thumbnail state must listen for `thumbnail-generated` events and ignore stale events for removed wallpapers. Backend needs a bounded in-flight set so repeated visible-row requests do not spawn duplicate generation jobs. The current image crate only supports lossless WebP directly, so lossy WebP should remain a separate encoder-selection task rather than being mixed into the responsiveness change.

## ADR-020: Signal-gated focus monitor

- **Status**: accepted
- **Decision**: Gate the full-screen focus monitor behind a condition-variable signal so the monitor thread blocks completely while focus mode is disabled, and wakes immediately when focus mode changes or the app shuts down.
- **Context**: ADR-005 removed rotation timer polling, but `start_focus_monitor` still slept every 2 seconds even when focus mode was disabled. This is small but unnecessary background wakeup work for a desktop app that should be quiet when idle.
- **Options**:
  - A. Keep the 2-second sleep loop for both enabled and disabled focus mode
  - B. Use the existing signal pattern to block while disabled and timeout only while focus mode is enabled
  - C. Replace the focus monitor with a fully event-driven Win32 foreground-window hook now
- **Why B**: It removes idle polling when focus mode is off while keeping the existing fullscreen detection logic stable. A Win32 hook could be explored later, but it has more platform-specific lifecycle risk.
- **Consequences**: Any command that toggles focus mode must notify the focus signal. Shutdown must also notify it so background thread joins do not hang.

## ADR-021: Separate tags and collections

- **Status**: accepted
- **Decision**: Keep user tags and user collections as separate SQLite concepts. Tags remain in `tags` / `wallpaper_tags`; collections use `collections` / `collection_wallpapers` and their own `collection:<id>` filter key.
- **Context**: The gallery needs both descriptive metadata tags and Apple Music-style user-curated collections. Reusing tags as collections made the UI faster to ship but collapsed two different mental models.
- **Options**:
  - A. Add dedicated collection tables and commands while preserving tags
  - B. Alias tags as collections in the UI
  - C. Store collection names as special prefixed tags
- **Why A**: It preserves existing tag workflows, allows collection-specific counts and future behavior, and avoids hidden taxonomy coupling.
- **Consequences**: Collection creation/filtering/batch assignment has its own commands and store state. Future collection features should not mutate `tags` unless the user explicitly assigns a tag.

## ADR-022: Bounded preview-first media generation queue

- **Status**: accepted
- **Decision**: Route cold thumbnail and preview derivative generation through one bounded media queue. Preview jobs are claimed before thumbnails, newer preview jobs outrank older preview jobs, queued/running paths are de-duplicated, and the cold thumbnail backlog is capped by dropping the oldest not-yet-started thumbnail jobs.
- **Context**: Large wallpaper libraries and fast scrolling can enqueue many cold thumbnail derivatives. At the same time, selecting a wallpaper needs the large preview to appear quickly. Separate per-image spawn paths reduce IPC blocking but still allow stale thumbnail work to crowd out current preview work and grow an unnecessary backlog.
- **Options**:
  - A. Shared bounded priority queue with a small worker pool
  - B. Keep separate ad hoc spawn/semaphore paths for thumbnails and previews
  - C. Generate all missing derivatives eagerly during startup
- **Why A**: It gives the active preview a deterministic path to the front, keeps CPU/IO concurrency bounded, and lets fast scrolling discard off-screen thumbnail work without losing cache hits or current preview responsiveness.
- **Consequences**: Thumbnail and preview commands should only perform validation/cache checks and enqueue misses. Future media derivatives should join the same queue with an explicit priority instead of spawning their own threads.

## ADR-023: Progressive active-preview pipeline with path-level media requests

- **Status**: accepted
- **Decision**: Restore the persisted current wallpaper before gallery pagination, proactively cache its 1440 px preview whenever the wallpaper changes, merge thumbnail and preview work by normalized source path, reserve media capacity for active previews, and use Windows WIC target-sized decoding with the Rust `image` pipeline as a compatibility fallback.
- **Context**: The existing queue prioritizes preview jobs only while they are waiting, but de-duplicates by `(kind, path)`. The same 90–112 MP source can therefore be decoded concurrently for thumbnail and preview, and four already-running thumbnail jobs can still block the active preview. Startup also leaves `currentWallpaperPath` empty and temporarily selects the first paginated row instead of restoring the persisted wallpaper. Real release measurements put a cold 100 MP preview at roughly 2–3.4 seconds with the current full-decode path, while only 39 of 245 wallpapers currently hit the active preview cache profile.
- **Options**:
  - A. Progressive bootstrap, path-level request merging, preview-reserved capacity, proactive preview warming, and WIC scaled decode
  - B. Keep the existing queue and eagerly generate every missing preview at startup
  - C. Expose original user-selected images directly to the WebView and let it decode them
- **Why A**: It makes the steady-state active preview ready before the user opens the page, gives first-ever cold images an immediate thumbnail fallback, prevents duplicate full decodes, and keeps the asset-protocol boundary limited to PureWall-owned derivatives. It also avoids the startup CPU, memory, and disk burst of whole-library pre-generation.
- **Consequences**: Media scheduling state becomes path-centric and needs tests for queued/running upgrades. Every successful wallpaper-change entry point must call the same preview prewarm boundary. Windows builds gain narrowly scoped WIC features and a fallback path; non-Windows or unsupported formats continue to use `image`. The UI needs an explicit `idle | thumbnail | preview | error` active-media state and must replace the displayed image only after the new asset has loaded.

## ADR-024: Backend-owned incremental folder synchronization

- **Status**: accepted
- **Decision**: Preserve filesystem change paths through a short debounce window, inspect only the changed image files or newly created directories, upsert discovered metadata into SQLite before notifying the frontend, and keep missing rows as recoverable metadata rather than deleting ratings, tags, or collection membership automatically.
- **Context**: The current watcher discards every changed path and emits an empty `folder-changed` event. The frontend then reloads an unchanged paginated SQLite result, so newly added images never enter the library. A single filesystem write can also emit several notify events and cause redundant page reloads.
- **Options**:
  - A. Debounce and incrementally synchronize changed paths in the backend, then emit one refresh event
  - B. Keep the frontend-only refresh event
  - C. Rescan the entire watched tree after every event
- **Why A**: It fixes database visibility for new and modified files without repeatedly decoding every image in a large library. It also keeps SQLite as the source of truth for paginated reads and preserves user metadata when a file is temporarily unavailable.
- **Consequences**: The scanner watcher must deliver de-duplicated path batches to a callback. File inspection must finish before taking the SQLite mutex, and a synchronization failure must emit the existing user-visible operation error instead of pretending the folder refresh succeeded. Directory creation may scan that new subtree; whole-library rescans remain explicit import/setup operations.

## ADR-025: Persist source-file availability for SQL-consistent library views

- **Status**: accepted
- **Decision**: Add a `file_available` flag to wallpaper rows, set it on imports and watched-folder changes, and make gallery pagination, filters, collection counts, playback candidates, and current library statistics exclude unavailable rows in SQLite while retaining their metadata and relationships.
- **Context**: PureWall currently applies `LIMIT/OFFSET` and `COUNT(*)` before Rust filters missing paths with `Path::exists()`. Missing files therefore inflate `total`/`has_more` and can consume page slots even though they are not rendered. Deleting rows would discard recoverable ratings, tags, and collection membership; checking every matching path during every page request would make pagination filesystem-bound again.
- **Options**:
  - A. Persist availability and maintain it incrementally
  - B. Delete rows when a source file disappears
  - C. Run filesystem existence checks across every matching row on each page/count query
- **Why A**: It keeps count, offset, filtering, stats, and playback semantics inside one indexed SQL source of truth, preserves user metadata, and makes ordinary watcher updates proportional to the changed paths rather than the full library.
- **Consequences**: Schema version increases additively and existing databases need a one-time availability backfill. Upsert must restore availability. Watcher removal/rename batches must mark exact paths and descendants unavailable before emitting refresh. `Path::exists()` remains a defensive last check for unobserved external changes, not the pagination model.

## ADR-026: Persisted multi-folder watcher registry

- **Status**: accepted
- **Decision**: Persist every accepted local folder source in a dedicated SQLite table, retain one live watcher per canonical root in a runtime registry, restore valid watchers at startup before running bounded background snapshot reconciliation, and keep invalid or temporarily missing roots persisted for a later retry.
- **Context**: `AppState` currently stores one `Option<FolderWatcher>`. Selecting or importing another folder replaces and drops the previous watcher, and no watched roots survive an application restart. SQLite can therefore retain wallpapers from several folders while only the latest folder receives live availability/import updates; changes made while PureWall is closed are also missed.
- **Options**:
  - A. Persist canonical roots and retain a runtime watcher map with startup snapshot reconciliation
  - B. Replace only the runtime `Option` with an in-memory vector/map
  - C. Watch one broad common ancestor for every imported source
- **Why A**: It aligns watcher lifetime with the additive multi-folder library model, avoids duplicate watchers for the same canonical root, survives restarts, and reconciles changes that occurred while the app was not running without watching unrelated directories.
- **Consequences**: The schema gains an additive watched-folder table and startup reads it before registering watchers. Folder scans remain bounded and run outside the SQLite mutex; snapshot persistence marks missing descendants unavailable and upserts current files before notifying the frontend. A watcher-start failure is logged but does not delete the persisted root, so a later startup can retry. The registry owns all watcher handles and dropping application state stops every watcher.

## ADR-027: Caller-excluded playback completion events

- **Status**: accepted
- **Decision**: For `run_playback_action` invoked by a WebView window, treat the typed command outcome as that caller's authoritative completion and emit the existing playback completion event to every target except the invoking window. Tray, CLI, timer, and other non-WebView entry points continue broadcasting the existing events.
- **Context**: Phase 2 initially let the invoking frontend apply both the command outcome and the broadcast completion event. Without a correlation id, action-name/FIFO matching cannot distinguish concurrent same-action calls from tray, widget, CLI, or timer events, causing duplicate refreshes, dropped external completions, stale writes, and pending-intent leaks.
- **Options**:
  - A. Filter the existing event away from the invoking window and use one authoritative completion channel per window
  - B. Add correlation ids to command outcomes and every existing event payload
  - C. Infer event identity in the frontend from action names, ordering, or timing
- **Why A**: Tauri 2.11.2 already provides `emit_filter`; filtering preserves existing event names and payloads, keeps external entry points synchronized, avoids timing/FIFO heuristics, and does not add a dependency or database change.
- **Consequences**: Playback event emission accepts an optional excluded WebView label. The invoking window applies the typed outcome exactly once; other windows apply the existing event exactly once. Frontend pending-intent inference is removed. Tests must prove caller exclusion, non-WebView broadcast behavior, main-window outcome application, widget-to-main event delivery, and current-row metadata outside gallery pagination.
- **Listener requirement (2026-07-22)**: Playback completion listeners that rely on caller exclusion must register with an explicit `WebviewWindow` target for the current label. Tauri's default JavaScript `listen(...)` target is `Any`, and Tauri 2.11.2 delivers `Any` listeners before consulting `emit_filter`, so an unscoped listener cannot be excluded.
- **Test consequence**: Frontend tests must assert the explicit target option used by production listeners; Rust predicate-only tests are necessary but not sufficient to prove caller exclusion.

## ADR-028: Playback side-effect commit boundaries

- **Status**: accepted
- **Decision**:
  1. Independent-display playback commits each successful external display application separately. Immediately after a display apply succeeds, PureWall durably records that display's history; when the successful display is primary, it also persists the current wallpaper before attempting later displays. A later display failure returns that platform error without erasing or withholding facts already committed for earlier displays.
  2. Only `Next` and automatic rotation are current-wallpaper transitions. Like/Dislike outcomes and completion payload paths identify the rating target; they do not move current or active presentation state. Pause outcomes change only pause state.
  3. After an action's external or durable side effects have committed, completion-event emission is best-effort. An emit failure is logged for observability but must not turn the committed action into an apparently retryable failure.
- **Context**: Final Phase 2 review found three related commit-boundary bugs: independent mode applied every display before writing any history, delayed rating completions reused a generic path reconciler and could roll the UI back to an older wallpaper, and Next propagated a completion emit error after Windows and SQLite had already changed. These paths need one explicit distinction between committed domain facts, target identity, presentation transitions, and notification delivery.
- **Alternatives**:
  - A. Apply all displays, then record all history — preserves an all-or-nothing-looking database result but loses already-successful external display facts when a later apply fails.
  - B. Action-agnostic reconciliation — treats every payload path as a current transition, but identity-only rating payloads can overwrite newer playback state.
  - C. Hard completion-emit failure — reports post-commit delivery errors as action failures, inviting unsafe retries that can advance or mutate state twice.
- **Consequences**: Independent playback orchestration needs a production-used injectable seam so tests can simulate partial platform failure without COM. Frontend reconciliation branches by action and refreshes a rating target without transitioning paths. Completion delivery failures remain observable through stderr logging, while callers still receive the committed outcome. This changes no public Tauri command/event name, dependency, SQLite schema, registry boundary, or selection weight.

## ADR-029: Library-source metadata commits precede runtime watcher reconciliation

- **Status**: accepted
- **Decision**:
  1. Source scans and target-folder validation complete outside the SQLite mutex.
  2. Remove commits the source/metadata transaction first, releases SQLite, then takes the watcher from the runtime registry and drops it after the registry lock is released.
  3. Relocate scans the target, commits the source root plus wallpaper path/availability changes atomically, releases SQLite, then removes the old watcher and starts the target watcher. A successful watcher start is followed by an explicit bounded target rescan so changes during the handoff gap are reconciled.
  4. A post-commit watcher failure records `last_error`, leaves the committed source metadata in place, and returns a warning/status that Retry can recover. It is not reported as a rolled-back database mutation.
  5. Add/Retry persists the canonical source before watcher startup. Watcher failure keeps the persisted source offline/error; successful startup precedes the full bounded scan.
- **Context**: SQLite, filesystem scans, and notify watcher handles cannot participate in one transaction. `FolderWatcher::drop()` joins a callback thread that may need SQLite, so replacing or dropping it under the database mutex can deadlock. Pretending a post-commit watcher failure rolled back metadata would also invite unsafe retries.
- **Alternatives**:
  - A. Hold SQLite while replacing watchers — rejected because watcher `Drop` can join a callback waiting for SQLite.
  - B. Start an unregistered target watcher before relocation commits — rejected because callbacks can import against a source root the database does not yet own.
  - C. Roll back SQLite after watcher failure — rejected because runtime resource creation is not transactionally reversible with committed SQLite state.
- **Consequences**: Source commands expose database success separately from watcher warnings. Runtime registry helpers take/drop handles in lock-free phases. Relocation needs an explicit post-handoff rescan. Tests must cover overlap protection, collision rollback, metadata preservation, and watcher-failure status without real desktop or registry mutation.

## ADR-030: Transactional wallpaper-title sync to Windows file metadata

- **Status**: accepted
- **Decision**: Treat a user-edited wallpaper title as one command-level operation that writes the source file's Windows `System.Title` property first and updates SQLite only after the file property commit succeeds. Return write failures to the frontend instead of reporting a database-only save as complete.
- **Context**: PureWall currently stores `display_title` in SQLite and attempts a best-effort Shell Property System write. The writer opens `GPS_DEFAULT`, which is read-only, so `IPropertyStore::SetValue` returns `STG_E_ACCESSDENIED`; the error is logged but the inspector still shows “Saved”. The stage also derives overlay title color from the application theme even though the text sits on arbitrary image content.
- **Options**:
  - A. Require file metadata success before updating SQLite and surface unsupported/read-only file errors
  - B. Keep SQLite authoritative and continue best-effort file writes
  - C. Rename the source file instead of using Windows metadata
- **Why A**: It matches the user's explicit expectation that the title is written into the image, prevents silent divergence between Explorer and PureWall, preserves the stable source path, and makes failure actionable.
- **Consequences**: Title edits can fail for read-only files or formats whose Windows property handler does not support `System.Title`; those failures remain visible and leave the previous SQLite title unchanged. The Shell property store must be opened with `GPS_READWRITE`, values must use the canonical single-string type, and the image-overlay title must use image-safe contrast independent of the light/dark application theme.

## ADR-031: Phase 5 release and update trust boundary

- **Status**: accepted
- **Decision**:
  - Tag: v<SemVer>, equal to all three application version declarations.
  - Release state: draft only; publishing is manual.
  - Update trust: Tauri signature verification plus HTTPS latest.json.
  - Installer trust: Authenticode is mandatory for releasable artifacts.
  - Secrets: GitHub Actions secrets only; never repository files or logs.
  - Lifecycle tests: disposable Windows runners only by default.
  - Identifier: preserve com.purewall.app during Phase 5.
- **Context**: Phase 5 turns the existing local packaging path into an open-source release loop. Release automation, updater delivery, installer signing, and lifecycle tests cross privileged boundaries that need one explicit trust model before implementation.
- **Consequences**: Local verification may validate version and configuration contracts without secrets. Only tag-matched builds may enter the privileged draft-release path; publishing remains a maintainer action. Tauri updater signatures and Windows Authenticode signatures are separate mandatory checks, signing material stays in GitHub Actions secrets, and installer lifecycle evidence comes from disposable Windows runners rather than the maintainer workstation.
