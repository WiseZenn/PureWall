# ARCHITECTURE.md

> 目标：新成员 10 分钟理解 PureWall 项目骨架

## 1. 系统目标与边界

PureWall 是一个 Windows 桌面壁纸管理应用，核心解决：壁纸轮播时无法快速收藏/删除的痛点。

**系统边界**：接管 Windows 壁纸轮播机制 + 提供画廊化管理界面。

## 2. 目录与模块职责

```
PureWall/
├── src/                          # Vue 3 前端（Tauri WebView）
│   ├── App.vue                   # 根布局：TitleBar + Sidebar + Content
│   ├── main.ts                   # Vue 入口 + Pinia 注册
│   ├── stores/wallpapers.ts      # 全局状态（壁纸列表/缩略图/筛选/轮播控制）
│   ├── styles.css                # 毛玻璃/暗色模式/动画
│   └── components/
│       ├── TitleBar.vue          # 自定义无边框标题栏
│       ├── Sidebar.vue           # 筛选/统计/轮播间隔控制
│       ├── Gallery.vue           # 瀑布流容器
│       ├── WallpaperCard.vue     # 单张壁纸卡片（悬浮操作）
│       └── EmptyState.vue        # 首次使用引导
├── src-tauri/                    # Rust 后端
│   ├── src/
│   │   ├── main.rs               # Tauri 入口 + command 边界 + 播放/窗口编排
│   │   ├── wallpaper.rs          # Windows wallpaper/display COM integration
│   │   ├── scanner.rs            # 文件夹/文件扫描 + notify 监听
│   │   ├── db.rs                 # SQLite CRUD + migrations + 加权随机算法
│   │   ├── thumbnails.rs         # 缩略图/预览图缓存
│   │   ├── media_queue.rs        # 预览优先的有界媒体生成队列
│   │   ├── context_menu.rs       # PureWall-owned HKCU context-menu registry keys
│   │   ├── autostart.rs          # PureWall Run key autostart integration
│   │   ├── diagnostics.rs        # CLI diagnostics log tail command
│   │   ├── widget.rs             # Pre-created widget window commands
│   │   ├── focus.rs              # 全屏检测与自动暂停
│   │   └── tray.rs               # 系统托盘菜单
│   └── tauri.conf.json           # 窗口/CSP/权限配置
└── docs/project-docs/            # 项目文档（本目录）
```

## 3. 核心执行流程

```
用户选择初始文件夹 / 追加导入文件夹
  → scanner::start_watcher() 先启动监听，避免长扫描期间丢事件
  → scanner::scan_folder() 在深度/数量/目录项预算内递归收集图片
  → db::upsert_wallpaper() 写入 SQLite（含 width/height/file_size）
  → watcher 将具体路径做 250ms 去重 debounce，只检查变更图片或新目录
  → 后端完成 SQLite upsert 后 emit("folder-changed")
  → 前端并行刷新分页图库与统计；WallpaperGrid 只为可见/邻近行预取缩略图

用户追加导入单张/多张图片
  → scanner::scan_files() 校验受支持图片，并限制单次显式导入数量
  → db::upsert_wallpaper() 写入 SQLite（引用原文件，不复制到 app data）
  → 前端 loadWallpapers() 刷新第一页；后续滚动按页追加

轮播定时器（后台线程）
  → Condvar 等待 rotation interval，pause/focus/interval/shutdown 变化会立即唤醒
  → interval 到期后 db::get_next_wallpaper() 通过 SQLite 加权随机抽样选图
  → wallpaper.rs 持久 STA worker 调用 Windows IDesktopWallpaper API
  → db::record_play() 更新统计
  → emit("auto-rotated") 通知前端更新 UI
```

## 4. 数据流与状态管理

- **Rust 端 AppState**：Mutex<Database>, Mutex<bool> is_paused, AtomicU64 rotation_secs, Mutex<HashMap> thumbnail_cache
- **前端 Pinia store**：wallpapers[] (当前已加载页), wallpaperTotal/hasMoreWallpapers, thumbnails Map, stats, currentWallpaperPath
- **关键设计**：所有操作本地 patchWallpaper() 替换数组触发 Vue 响应式，避免全量重载

## 5. 外部依赖与集成边界

| 层 | 依赖 | 用途 |
|----|------|------|
| Rust | `windows` crate | SystemParametersInfoW 壁纸 API |
| Rust | `rusqlite` (bundled) | SQLite 元数据库 |
| Rust | `notify` | 文件系统监听 |
| Rust | `image` | 缩略图生成 |
| Rust | `fastrand` | 2x 加权无放回随机采样 |
| Rust | `trash` | 回收站删除 |
| 前端 | `@tauri-apps/api` | invoke/event 通信 |
| 前端 | `@tauri-apps/plugin-dialog` | 原生文件夹选择 |
| 前端 | Pinia + Vue 3 | 状态管理 + UI |

## 6. 扩展点

- 新增功能 → 新建 Rust module + Command + 注册到 main.rs handler
- 新增 UI → 新建 component + Pinia store 方法
- 新增筛选 → Sidebar filter 数组 + db.rs 查询方法
- 可失败 Tauri command → 返回 `CommandResult<T>`，错误序列化为 `{ code, message }`；内部 helper 可以继续用 `Result<_, String>`，但不能直接暴露到 WebView command 边界

## 7. 架构级记忆关联

- 缩略图加载坑 → AI_DIARY.md #thumbnail-001
- Vue 响应式不更新坑 → AI_DIARY.md #reactivity-001
- Tauri asset 协议权限坑 → AI_DIARY.md #asset-001

## 8. 当前隐患与重构建议

- `read_image_thumbnail` 单条加载命令目前未使用（批量替代），可清理
- wallpaper.rs 中 `get_current_wallpaper` 和 `is_valid_wallpaper` 未被调用
- `main.rs` 仍承担 command 注册和播放编排；CLI diagnostics 与 widget commands 已按 ADR-015 抽到独立模块，后续可继续拆 command groups
 
## 9. Phase 3 gallery management addendum

- `wallpapers.blacklisted` hides images from the normal gallery and excludes them from weighted playback.
- `tags` stores custom tag metadata; `wallpaper_tags` stores many-to-many wallpaper/tag assignments.
- `collections` stores user-curated collection metadata separately from tags; `collection_wallpapers` stores many-to-many wallpaper/collection assignments.
- SQLite schema migrations are tracked with `PRAGMA user_version`; startup migrations must keep old local databases compatible.
- `wallpaper_tags` and `collection_wallpapers` use foreign keys back to their parent tables with cascade cleanup. `play_events.wallpaper_id` uses `ON DELETE SET NULL` so deleting a wallpaper does not erase historical rotation counts.
- Gallery queries are driven by whitelisted Rust commands. The large-library path uses `get_wallpapers_page(filter, sort, search, offset, limit)` so SQLite owns filtering, sorting, escaped search, total counts, and page boundaries. Collection filters use `collection:<id>` and tag filters use `tag:<id>`.
- Batch metadata actions operate on frontend-selected paths and are exposed as narrow Rust commands: like/dislike/clear rating, tag assign/unassign, collection assign/unassign, and blacklist/restore. `batch_operations.rs` rejects empty or oversized input, caps raw batches at 500 paths, trims and deduplicates in first-occurrence order, and validates rating/relation parameters before the database mutation.
- Each metadata command validates registered source files and related tag/collection existence, applies its writes in one SQLite transaction, and returns `{ affected, refresh }`. The Pinia batch runner blocks overlapping mutations, clears selection only after command success, preserves it on failure, and follows the returned wallpaper/statistics/collection refresh hints.
- Hide/restore is immediately persisted but exposes a frontend Undo action. Delete/batch delete use a 7-second frontend pending-delete window: the item leaves the UI first, Undo restores the row snapshot, and only after the window expires does the backend move files to the Recycle Bin and delete DB rows.

## 10. Phase 4 polish addendum

- Display-aware playback uses Windows `IDesktopWallpaper` through `src-tauri/src/wallpaper.rs`.
- `wallpaper.rs` owns a single long-lived STA worker thread. Display enumeration, all-monitor wallpaper changes, and per-monitor wallpaper changes are queued to that worker instead of creating a new COM thread per operation.
- The STA boundary retries only Windows `E_FAIL` (`0x80004005`) once after 80ms because the desktop wallpaper COM service can fail transiently. Deterministic HRESULTs are returned immediately, and persistent failures include the monitor/file operation context.
- Display modes are persisted in SQLite `settings.display_mode`: `all`, `span`, or `independent`.
- Display-mode changes apply immediately. `all` and `span` reapply the current wallpaper with the selected placement; `independent` selects and applies one wallpaper per detected monitor.
- `independent` mode uses 2x liked-weight ticket sampling without replacement, so one rotation returns distinct wallpapers whenever enough eligible files exist; repetition is only used when the library has fewer eligible paths than displays. One `play_events` row is recorded per display.
- Rotation and focus-monitor background threads are owned by `AppState`: main-window close sets a shared shutdown flag and joins the worker handles.
- Rotation waits on a `Condvar` generation signal instead of polling once per second; interval, manual pause, focus auto-pause, and shutdown changes notify the waiting thread.
- Focus pause is a PureWall-owned background thread in `src-tauri/src/focus.rs`; it detects foreground full-screen windows and only auto-resumes rotations that it paused itself.
- Detailed yearly analytics are stored in `play_events`; older aggregate `wallpapers.play_count` remains the source for total library play counts.
- Phase 4 UI state lives in `src/stores/wallpapers.ts`; Phase 5 presents those controls from the right-side `InspectorPanel.vue`.

## 11. Phase 5 UI workbench addendum

- The main window uses a three-pane workbench in `src/views/Home.vue`: `Sidebar.vue`, `Gallery.vue`, and `InspectorPanel.vue`.
- `Sidebar.vue` is now a narrow navigation rail for Library/Liked/Passed/Hidden filters, tag browsing, tag creation, and total play count.
- `Gallery.vue` owns the image-led wallpaper stage, sort controls, selection toolbar, empty/loading states, and the masonry grid.
- `WallpaperGrid.vue` renders virtualized rows from the currently loaded page. Scrolling near the bottom calls `store.loadMoreWallpapers()` to append the next SQLite page, keeping both DOM/card count and frontend array growth bounded for large libraries.
- `WallpaperCard.vue` selects `store.activeWallpaperPath` on click, supports batch selection mode, and keeps per-card like/pass/set/hide/delete actions visually secondary.
- `InspectorPanel.vue` centralizes selected wallpaper preview/actions, tag assignment, rotation interval, display mode, focus pause, desktop menu, autostart, widget visibility, and compact yearly stats.
- `src/stores/wallpapers.ts` now tracks `activeWallpaperPath`, `activeWallpaper`, and `activeThumbnailUrl` so inspection can follow the selected wallpaper without overwriting `currentWallpaperPath`. Entity patches also update the bootstrapped current-wallpaper row when that row is outside the loaded gallery page.
- Phase 5 initially stayed frontend-only, then added narrow Rust/UI support for preview quality, additive imports, and persistent image metadata. It still does not change registry/autostart behavior or wallpaper playback selection contracts.

## 12. Scan bounds addendum

- Folder scans are intentionally bounded in `src-tauri/src/scanner.rs`: maximum recursion depth, maximum image count, and maximum directory-entry count protect the app from accidentally traversing enormous trees.
- Explicit multi-file imports share the same image-count ceiling.
- Wallpaper library paths must be local. Command validation, folder scanning, explicit file imports, and watcher startup reject UNC paths, extended UNC paths, and Windows mapped network drives.
- Recursive scans skip symbolic-link entries so PureWall does not follow reparse-style paths into network locations or loops.
- When a scan exceeds a bound, the Rust command fails with a user-visible typed command error; PureWall does not silently import a partial library.

## 13. Phase 5 Fluent tokenized shell addendum

- The main UI now uses `AppShell.vue` as the composition root for `TitleBar`, `Sidebar`, `Gallery`, `InspectorPanel`, and `StatusBar`.
- `src/styles.css` defines semantic Fluent-style tokens for `data-theme="dark"` and `data-theme="light"`; components should consume semantic tokens such as `--bg-app`, `--bg-panel`, `--border-subtle`, and `--accent` instead of legacy one-off colors.
- Reusable UI primitives live in `src/components`: `IconButton`, `ToggleSwitch`, `SidebarItem`, `TagPill`, `MetadataRow`, `ColorSwatchList`, `YearlyInsightsChart`, `SearchToolbar`, `CurrentWallpaperPanel`, `WallpaperControls`, `SettingRow`, `WallpaperGrid`, and `StatusBar`.
- `AppIcon.vue` is the single custom outline SVG icon system. Icons use `currentColor`, 18-20px optical sizing, `1.75px` strokes, rounded caps, and rounded joins.
- The Phase 5 Fluent tokenized-shell pass was frontend-only at the time it landed; later Phase 5 repair passes added narrow Rust/SQLite support for preview quality, additive imports, and image metadata while still avoiding registry/autostart writes.

## 14. Two-tier image rendering addendum

- Gallery cards use the 512px JPEG thumbnail cache for fast batch loading.
- Large current-wallpaper and inspector preview surfaces use the separate `load_preview_image` command and a 1440px JPEG preview cache.
- `load_preview_image` returns cached preview paths immediately. Cache misses are generated through the shared `media_queue` workers and refill the frontend through `preview-generated`, so selecting an uncached wallpaper does not block the WebView on 1440px JPEG generation.
- `src-tauri/src/media_queue.rs` prioritizes preview jobs over thumbnail jobs, processes the newest preview first, de-duplicates queued/running paths, and caps queued cold thumbnails so fast scrolling through large libraries cannot create an unbounded backlog of off-screen work.
- Thumbnail and preview disk-cache keys include the source path, derivative profile, file size, and source modification time, so replacing an image at the same path invalidates the old cached JPEG.
- Thumbnail and preview cache hits refresh the derivative file mtime; startup cleanup keeps the most recently used derivative working set instead of deleting frequently viewed cached images.
- The frontend keeps thumbnail and preview asset URLs in separate in-memory maps so selecting a wallpaper upgrades only the active image instead of loading high-resolution previews for the whole library.
- Gallery thumbnail requests are visibility-driven: an 80ms scheduler always submits the complete latest virtual-row path set after layout settles, while the store filters cache hits and in-flight work. A canceled debounce therefore cannot mark a still-visible path as requested before any IPC request was sent.
- Thumbnail lookup prefers the current stable profile key, can immediately reuse the verified historical `DefaultHasher` profile key or original path-only key as a stale fallback, and queues current-profile regeneration so cache migrations do not turn usable derivatives into startup spinners.
- `load_thumbnails_batch` validates local image paths before acquiring SQLite, uses one lightweight registered/available-path query for each frontend batch, releases the database mutex, and only then performs derivative-cache IO or queue work.
- Thumbnail validation and generation failures emit `thumbnail-generation-failed`. The store preserves any usable stale URL, otherwise retries after 250ms and 500ms, then records a terminal per-path error that the visibility scheduler skips until the user explicitly chooses Retry.

- `bootstrap_active_wallpaper` restores the persisted current wallpaper before gallery pagination, returns cached 512px/1440px derivatives, and returns the wallpaper row so the stage remains usable when the current image is outside page 1.
- Every successful wallpaper-change path calls `prewarm_active_preview` after releasing the SQLite lock. Cached previews create no work; cold active requests are idempotent and include both preview and fallback needs.
- Media queue identity is the normalized source path. Thumbnail/preview needs merge into one queued or running state, with one preview-reserved worker and two general workers preventing thumbnail starvation and same-source concurrency.
- A merged media job opens the source once and writes every missing requested derivative from that decoded image; the frontend no longer sends a second thumbnail request after a preview miss.

## 15. Import and metadata addendum

- `set_wallpaper_folder` remains the onboarding/source-folder flow, but returns only `ImportResult` scan/import counts. The frontend reloads gallery page 1 through `get_wallpapers_page` instead of receiving a full `Vec<WallpaperEntry>` from the source-folder command.
- `import_wallpaper_folder` and `import_wallpaper_files` are additive library imports. They reference existing files and do not copy originals into `APPDATA/com.purewall.app/`.
- SQLite `wallpapers` rows now persist `width`, `height`, and `file_size` from scanner results. Existing rows default to zero until refreshed or re-imported.
- `get_image_metadata` refreshes selected-image metadata from the file system when possible and writes successful reads back to SQLite; the frontend falls back to persisted row metadata for immediate inspector rendering.
- Color values are displayed through a top-level popover from `ColorSwatchList.vue`, avoiding clipping inside the scrollable inspector.

## 16. Container-aware workbench layout addendum

- `main-workspace` is a CSS container named `workspace`, so the central gallery can adapt to its actual pane width instead of only the full viewport width.
- At compact central widths, `CurrentWallpaperPanel` uses a narrower image/control split and a 2x2 wallpaper control grid.
- At very narrow central widths, the current wallpaper panel stacks, the central workspace becomes vertically scrollable, and the right inspector no longer overlaps gallery content.
- This mirrors desktop media tools: fixed side panes keep stable affordances, while the central content uses local breakpoints, wrapping toolbars, and constrained minimum widths.

## 17. Windows Shell presentation metadata addendum

- `src-tauri/src/shell_metadata.rs` reads Explorer-compatible properties on demand and writes explicit title edits to the source file's `System.Title` property through a writable Windows Property System store.
- `get_shell_metadata` reads Author, Copyright, and Comment only for active or selected images; it does not scan the full library. Arbitrary file names or Shell titles are not promoted automatically.
- Wallpaper stage titles remain opt-in user metadata. `set_wallpaper_display_title` validates the registered source, writes and commits `System.Title` without holding the SQLite mutex, then persists `wallpapers.display_title` only after the file write succeeds. Read-only or unsupported sources surface an error and retain the previous database title.
- The Living Stage title uses a fixed white foreground with dark shadow because it overlays arbitrary photography; application light/dark theme tokens do not control its contrast.
- Shell metadata remains an in-memory presentation cache and is not persisted to SQLite.

## 18. Drop import and indexed patch addendum

- The main WebView registers Tauri drag/drop events in `AppShell.vue`. Dropping local image files or folders calls `import_dropped_paths` and uses the same local-path and supported-image validation boundaries as explicit imports.
- Dropped folders are scanned through the bounded scanner; dropped files are validated individually; the full mixed drop is deduplicated and capped by the same global image-import limit before being persisted as `source = "dropped"` rows.
- `src/stores/wallpapers.ts` keeps `wallpapers` as the ordered rendering array, but maintains a path-to-index map for single-wallpaper patches. Full collection replacement paths call `setWallpapers(...)` so the index is rebuilt consistently.
- `CurrentWallpaperPanel.vue` renders the Living Stage preview as an actual `<img>` with accessible alt text. CSS controls object-fit and layout only; neither the stage nor the shell ambient layer injects the preview through `background-image`.

## 19. Multi-window theme synchronization addendum

- Theme state is shared across the main window and the pre-created widget WebView through `src/composables/useTheme.ts`.
- Each WebView applies the stored `purewall-theme` preference at startup, listens for `storage` changes, and also uses a `BroadcastChannel` named `purewall-theme` for immediate in-session updates.
- When the preference is `system`, each WebView resolves `prefers-color-scheme`; system theme changes reapply and broadcast the `system` preference so hidden or secondary windows can re-resolve their own token set.

## 20. Progressive active-preview pipeline addendum

- `bootstrap_active_wallpaper` runs before gallery pagination and returns the persisted current wallpaper row plus the best available derivative. A current 1440px preview is `ready`; a thumbnail or compatible stale preview is returned immediately while the current preview is queued.
- `media_queue.rs` owns one path-keyed request state. Thumbnail and preview needs merge, running paths collect missing follow-up needs, and one source is never decoded concurrently for separate derivatives.
- Worker capacity is split into one preview-reserved consumer and two general consumers. Active/normal previews can use the reserved lane; speculative previews are capped at two, run only on general workers, and queued speculative work is evicted when a new active preview arrives.
- Windows derivative decoding goes through `image_decoder.rs`: WIC first requests target-sized pixels through `IWICBitmapSourceTransform`, falls back to a WIC scaler/converter when needed, then falls back to the Rust `image` crate for unsupported inputs or WIC failures.
- Combined derivative generation decodes at most once, keeps the existing 512px/q88 thumbnail and 1440px/q92 preview profiles, and atomically writes each requested JPEG.
- `src/stores/activeMedia.ts` is the shared large-image state machine (`idle | thumbnail | preview | error`). The store keeps the thumbnail visible, preloads the preview with a browser `Image`, and commits the new URL only after the matching generation reports `load`.
- `CurrentWallpaperPanel`, `InspectorPanel`, and `QuietCanvas` consume the same committed store URL. A late load from a previous wallpaper is ignored; a decode error preserves the thumbnail and exposes a compact non-blocking status.
- After an active preview is browser-ready, the frontend selects at most two adjacent paths from the loaded page and calls `prewarm_preview_images`. The backend validates every path against SQLite/local-file boundaries and skips existing current-format cache hits.
- Preview lookup supports stale-while-revalidate for the known 960px/q82 legacy profiles. A valid legacy derivative may be displayed immediately but never suppresses current 1440px generation.
- Startup cache cleanup protects derivatives for the persisted current/active source, including compatible legacy preview paths. Cache hits continue to refresh mtime for LRU-by-mtime behavior.
- Repeatable real-image timing lives in the ignored Windows test `preview_performance_tests::report_real_preview_pipeline_timings`; it must be run in release mode with explicitly supplied local sample paths.

## 21. Incremental watched-folder synchronization addendum

- `scanner.rs` preserves notify event paths and combines repeated events into one sorted, de-duplicated batch after a 250ms quiet window.
- Ordinary directory metadata events are ignored. Supported image paths are inspected incrementally; only directory creation or rename may scan the new subtree. Missing rename/remove paths still trigger a frontend refresh without recursively touching the vanished tree.
- `main.rs` inspects files before acquiring the SQLite mutex, upserts the resulting canonical paths/metadata, and emits `folder-changed` only after persistence succeeds. Failures use the existing `operation-failed` notification channel.
- Both initial source-folder setup and asynchronous additive folder import start their watcher before the bounded initial scan, so filesystem changes during a long scan remain queued.
- External removals do not delete SQLite rows automatically. Watcher reconciliation marks their explicit availability state while preserving user ratings, tags, and collection membership for recoverable paths.

## 22. SQL-consistent source-file availability addendum

- SQLite schema version 5 adds `wallpapers.file_available INTEGER NOT NULL DEFAULT 1` plus an availability/blacklist index. The additive migration checks each legacy row once and backfills availability from the source file.
- Every successful wallpaper upsert restores `file_available = 1`. Watched remove/rename paths mark the exact row and all directory descendants unavailable before `folder-changed` is emitted.
- Windows descendant matching normalizes separators, case, and canonical `\\?\`/`\\?\UNC\` prefixes so ordinary notify paths match canonical scanner paths.
- Gallery filters and `COUNT/LIMIT/OFFSET`, collection badge counts, current library statistics, and rotation candidates all require `file_available = 1` in SQL. Missing rows remain directly addressable for recovery and keep their relational metadata.
- The import boundary rechecks `is_file()` before upsert so an image removed after scanning cannot be revived by stale scan data. Existing-path checks remain a defensive final guard for unobserved external changes, not the pagination model.

## 23. Persisted multi-folder watcher lifecycle addendum

- SQLite schema version 6 adds `watched_folders(path PRIMARY KEY, source, created_at)`. Every accepted canonical local folder is persisted, so watcher ownership matches the additive multi-folder library instead of only the most recently imported source.
- `AppState.folder_watchers` owns one `(source, FolderWatcher)` entry per canonical root. Re-registering the same root/source is idempotent. Per ADR-029, add/re-register persists the canonical source before watcher startup; any existing watcher handle is replaced and dropped without holding SQLite or the watcher-registry lock.
- Startup loads all persisted roots before moving the database into managed state, restores every valid watcher, and registers one tracked background worker that performs bounded folder snapshots sequentially. Snapshot IO finishes before the SQLite mutex is acquired; each snapshot marks absent descendants unavailable and upserts current files.
- Missing or temporarily inaccessible roots remain in `watched_folders` for a later startup retry. Their known descendants are marked unavailable when the root itself is absent, preserving ratings, tags, and collections without presenting stale files as active library members.
- Filesystem callbacks and startup workers stop accepting database work once shutdown begins. Clean shutdown removes and joins every watcher outside the SQLite lock, joins all tracked database-using workers, and checkpoints WAL only after those writes have stopped.
- Startup snapshot results are aggregated into one `folder-changed` event, avoiding one paginated gallery/statistics reload per restored folder.

## 24. Phase 2 playback loop contract addendum

- `src-tauri/src/playback_action.rs` owns the stable action vocabulary: `next`, `like`, `dislike`, and `toggle_pause` (with the legacy CLI `pause` alias). Its `dispatch_with` function is pure and returns an action-specific typed outcome without Tauri state.
- `main.rs` remains the Tauri/orchestration boundary. It owns the real playback executor and its narrow legacy compatibility facades, so the main UI, tray, widget, CLI/context-menu action, and rotation timer all reach one executor rather than implementing their own playback semantics.
- The cross-window contract remains four existing events: `auto-rotated`, `wallpaper-rating-changed`, `pause-changed`, and `operation-failed`. The first three reconcile successful playback state; the last carries visible failures, including unavailable playback candidates.
- Per ADR-027, a WebView caller receives its typed `run_playback_action` outcome as its authoritative completion. The backend emits the matching legacy event to other targets; each playback listener explicitly registers for its own `WebviewWindow` label because Tauri 2.11.2 `Any` listeners cannot be excluded by `emit_filter`.
- Startup reloads the persisted current path, manual pause state, bounded rotation interval, and a valid display mode before playback state is initialized. The legacy pause-file migration remains a compatibility path after that load.
- Current-playback Like/Dislike actions always target the persisted current path. Their outcome/event path is rating-target identity, not a current transition: frontend reconciliation patches that target and refreshes it only when it is still current. Only Next/automatic rotation advances current/active presentation and reloads preview, metadata, and Shell metadata. Gallery and inspector rating APIs remain path-targeted management actions.
- Independent display rotation pairs every successful external monitor apply with its own immediately durable `play_events` record. A successful primary apply also persists current before later monitors are attempted; a later platform failure returns its error while retaining earlier history/current facts.
- Once any playback action's side effects have committed, completion delivery is best-effort. The executor logs emit failures to stderr and still returns the committed typed outcome, preventing unsafe retries of actions that already changed Windows or durable state.

## 25. Library source lifecycle addendum

- SQLite schema version 7 extends `watched_folders` with nullable `last_scan_at` and `last_error`. The source list derives Online/Offline/Scanning/Error status from persisted scan state, local folder availability, and the in-memory operation registry.
- `src-tauri/src/paths.rs` owns source-path canonicalization, identity keys, and overlap/relative-path comparisons. Database transactions and watcher callbacks use the same Windows case-insensitive, canonical-prefix-aware identity boundary.
- Source removal is overlap-safe. Keep metadata marks only wallpapers exclusive to the removed source unavailable; Clear metadata deletes only exclusive wallpaper metadata. Rows still covered by another registered source remain available.
- Source relocation scans the validated target outside SQLite, then atomically rewrites the watched root and matched wallpaper paths by relative path. Wallpaper IDs, ratings, titles, tags, collections, and play history survive; path/source collisions roll back the complete transaction.
- Remove commits its SQLite mutation before taking the watcher handle. Relocate commits before removing the old watcher and starting the target watcher. Watcher handles are always dropped after both the SQLite mutex and watcher-registry lock are released.
- A successful relocation watcher handoff performs one bounded target rescan to reconcile the non-transactional gap. Post-commit watcher or handoff-rescan failures persist `last_error` and return `watcher_warning`; Retry repairs runtime state without pretending the committed SQLite mutation rolled back.
- Add, Retry, and existing folder-import paths persist the canonical source before watcher startup. Complete scans run outside SQLite and persist results only after inspection.
- Queued watcher callbacks carry their owning root/source and revalidate that registration inside the database guard before writes, so stale callbacks cannot revive a removed or relocated source.
- `src-tauri/src/library_sources.rs` defines typed status/removal DTOs and operation guards. `src/stores/wallpapers.ts` owns frontend source state plus row-level busy/error maps; `LibrarySourcesSettings.vue` presents the Settings lifecycle UI and explicit Keep/Clear safety confirmation.
- Source list, rescan, retry, removal, and relocation mutate PureWall metadata and watcher ownership only. They never delete, move, or recycle original wallpaper files and do not read or write registry, autostart, context-menu, Windows wallpaper, policy, or other system-setting state.

## 26. Phase 3C metadata backup/restore addendum

- `src-tauri/src/library_backup.rs` owns the portable `{ app: "PureWall", schemaVersion: 1 }` JSON contract, stable `BACKUP_*` errors, validation, normalization, bounded reads/serialization, atomic file installation, preview DTOs, and post-commit source reconciliation. Database IDs never cross the backup boundary; normalized paths and tag/collection names are the stable identities.
- V1 is capped at 32 MiB, 256 sources, 100,000 wallpapers, 10,000 tags, 10,000 collections, 256 tag/collection relations per wallpaper, 32,767-character paths, 512-character titles, and 128-character tag/collection names. Paths must remain local absolute paths; existing paths use filesystem canonicalization and missing paths use deterministic lexical Windows normalization.
- The exported allowlist is sources, wallpaper scanner/user metadata, tag/collection definitions and name-based associations, plus `rotationSecs`, `displayMode`, `focusModeEnabled`, `paused`, `theme`, and `workspaceMode`. Theme accepts `system | light | dark`; workspace mode reuses the persisted client vocabulary `workbench | quiet`. Play history, caches/derivatives, availability bookkeeping, logs, secrets, registry, autostart, context-menu, and system-setting state are excluded.
- `Database::export_backup_snapshot` reads one consistent SQLite transaction and sorts every portable collection deterministically. Serialization is pretty UTF-8 JSON with a trailing newline. Export rejects oversized output before destination mutation, requires a `.json` target, and rejects canonical targets that match a registered wallpaper or sit below PureWall app data. Accepted destinations use one same-directory operation-owned temp file with create-new/write/flush/sync and atomic installation; failure cleanup removes only that exact temp file.
- Import is explicitly preview/confirm. Preview performs a fresh bounded read, full schema validation, normalized-identity comparison, count/warning construction, and no mutation, then returns a SHA-256 content digest. Confirmation re-reads the file and verifies that digest before parsing or opening the merge transaction; changed bytes return `BACKUP_PREVIEW_CHANGED` and require a fresh preview.
- `Database::merge_backup` owns one SQLite transaction. Backup values replace rating, hidden state, nullable custom title, and complete tag/collection relation sets for matching normalized wallpaper paths; local scanner fields, play history, and local-only rows/sources/definitions/settings remain. New missing-file metadata is inserted unavailable, existing names/sources are reused, identical re-imports report zero changes, and any relation/settings error rolls back the full merge.
- `main.rs` owns Tauri orchestration. It releases the SQLite guard before applying runtime settings or reconciling watchers. Per accepted ADR-029, the metadata commit remains successful when a source is offline or watcher startup fails; those runtime failures are returned together as warnings and persisted source errors rather than misreported as transaction rollback.
- `src/stores/wallpapers.ts` owns one backup busy/error boundary and post-commit refresh routing. `LibraryBackupSettings.vue` owns native save/open pickers, preview confirmation, focus/keyboard behavior, runtime-valid theme/workspace application, and one success surface that can display committed metadata plus watcher/offline warnings simultaneously.
- Backup/restore never copies, moves, deletes, recycles, rewrites, or sets original wallpaper files. It does not mutate the Windows wallpaper, registry, autostart, context-menu, policy, or other system settings.

## 27. Phase 5 release trust boundary addendum

- `scripts/verify-release-contract.mjs` is the local, unprivileged release-contract module. Its `npm run verify:release` interface reads the three application version declarations and optional `PUREWALL_RELEASE_TAG`, rejects version/tag drift, and rejects insecure updater configuration when updater support is present.
- A releasable tag is exactly `v<SemVer>` and must equal `package.json.version`, `src-tauri/Cargo.toml` package version, and `src-tauri/tauri.conf.json.version`.
- Release automation may create draft releases only. Publishing is a deliberate maintainer action outside automated workflows.
- Tauri updater signing and Windows Authenticode signing are separate trust layers: the updater signature authenticates update payloads consumed through HTTPS `latest.json`, while Authenticode authenticates releasable Windows executables/installers. Both are mandatory for releasable artifacts.
- Signing material belongs only in GitHub Actions secrets and must never be committed to repository files or printed to logs. Local contract verification does not consume signing secrets.
- Clean install, upgrade, and uninstall lifecycle tests run on disposable Windows runners by default, not on a maintainer workstation or against real user data.
- Phase 5 preserves the existing `com.purewall.app` identifier so upgrades continue to address the same application identity and app-data boundary.

## 28. Signature-verified application updates

- `src-tauri/src/app_updates.rs` is the updater trust boundary. It uses `tauri-plugin-updater` for HTTPS manifest/package retrieval and mandatory signature verification; no JavaScript updater permissions or automatic startup check are used.
- The Rust command surface is limited to `fetch_update` and `install_update`. `Update` remains behind `Mutex<Option<Update>>`; only version, current version, publish date, and release notes cross into the WebView.
- A successful fetch replaces the pending update, while a no-update response clears stale state. Installation takes the pending value before downloading so an install can never be retried against stale metadata. Download progress uses typed `Started`, `Progress`, and `Finished` channel events.
- Rust updater errors are intentionally stable and user-safe: URLs, signatures, request headers, and internal paths are not returned to the UI.
- `src/stores/appUpdates.ts` owns the explicit Settings state machine. Opening Settings never contacts the update endpoint; users must choose Check for updates and then Download and install. Windows installer execution closes PureWall, which is explained alongside progress and retry states in `UpdateSettings.vue`.
