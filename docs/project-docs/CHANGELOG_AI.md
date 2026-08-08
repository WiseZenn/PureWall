# CHANGELOG_AI.md

> AI 维护的精确代码变更日志。仅记录已执行改动。

---

## 2026-05-28 — Phase 1 MVP 完整搭建

### 变更目标
从零搭建 PureWall 壁纸管理应用的 Phase 1 MVP。

### 代码范围
- 新增 `src-tauri/` 整个 Rust 后端（5 个源文件 + Cargo.toml + tauri.conf.json）
- 新增 `src/` 整个 Vue 前端（7 个源文件 + 配置文件）
- 新增 `.gitignore`, `index.html`, `tailwind.config.js`, `postcss.config.js`

### 架构与依赖影响
- 引入 Tauri 2 框架（Rust + WebView2）
- 引入 SQLite (rusqlite bundled)、文件监听 (notify)、图片处理 (image)、回收站 (trash)
- 引入 Vue 3 + Pinia + Tailwind CSS

### 关键实现点

**Rust 后端 (src-tauri/src/)**
- `wallpaper.rs` L12-L32: SystemParametersInfoW 调用，UTF-16 宽字符转换
- `db.rs` L151-L187: 加权随机算法，喜欢 2x 权重，屏蔽排除
- `main.rs` L265-L293: 后台轮播定时器线程，1 秒轮询 + 间隔触发
- `main.rs` L136-L175: 缩略图批量加载 + HashMap 缓存

**前端 (src/)**
- `stores/wallpapers.ts` L106-L112: patchWallpaper() 数组替换触发 Vue 响应式
- `stores/wallpapers.ts` L188-L207: 事件监听（tray/自动轮播/文件夹变更）
- `components/WallpaperCard.vue`: 缩略图从 store.thumbnails Map 取值

### 踩坑记录
详见 AI_DIARY.md 所有条目（#scaffold-001 至 #emitter-001）

---

## 2026-05-28 (续) — 定时轮播 + 项目文档体系

### 变更目标
1. 补充 Phase 1 遗漏的定时轮播功能
2. 按 prompt 链建立完整的项目文档体系
3. 基于实际使用经验优化 prompt 链

### 代码范围
- `src-tauri/src/main.rs`: 新增 `AtomicU64 rotation_secs` + `start_rotation_timer()` 后台线程 + `set_rotation_interval`/`get_rotation_interval` commands
- `src/stores/wallpapers.ts`: 新增 `auto-rotated` 事件监听（更新 play_count + stats）
- `src/components/Sidebar.vue`: 新增轮播间隔选择器（1m/5m/10m/30m）
- 新增 `docs/project-docs/` 5 个核心文档
- 更新 `prompt/00-router.md`, `01-onboard.md`, `05-feature-dev.md`, `08-changelog.md`
- 新增 `prompt/09-debug.md`

---

## 2026-05-28 (续) — Phase 2: 交互强化

### 变更目标
实现 Phase 2 三个功能：桌面右键菜单、悬浮挂件、开机自启

### 代码范围
- 新增 `src-tauri/src/context_menu.rs`: PowerShell 写注册表（HKCU shell entries）+ register/unregister/is_registered
- 新增 `src-tauri/src/autostart.rs`: 注册表 Run 键实现开机自启
- `src-tauri/src/main.rs`: CLI 参数处理（--action next/like/dislike/pause）+ save_current_wallpaper() + 5 个新 Command
- `src/components/Sidebar.vue`: 右键菜单开关 + 开机自启开关 + 悬浮挂件按钮
- 新增 `public/widget.html`: 透明置顶悬浮窗口（❤️/⏭️/👎）
- 更新 `src-tauri/capabilities/default.json`: webview/window/event 权限

### 关键实现点
- `main.rs` L293-L303: CLI 参数拦截 — `--action` 存在时直接执行命令退出，不启动 Tauri 窗口
- `context_menu.rs`: 通过 PowerShell 写注册表 `HKCU\Software\Classes\Directory\Background\shell\PureWall`
- `main.rs` L68-L74: save_current_wallpaper() 保存当前壁纸路径到文件，供 CLI action 引用
- Widget 通过 Tauri 事件（widget-like/widget-dislike）与主窗口通信

### 验证证据
- `cargo check` — PASS（2 warnings）
- `vue-tsc --noEmit` — PASS

---

## 2026-06-01 — Phase 2 悬浮挂件修复

### 变更目标
修复悬浮挂件未达到预期的问题：独立 widget 窗口授权、圆角透明视觉、按钮事件同步、关闭后 Sidebar 状态同步。

### 代码范围
- `src-tauri/src/main.rs`：新增 `widget_next` / `hide_widget` commands；`toggle_widget` 增加 `.transparent(true)`、显隐错误传播和 `widget-visibility-changed` 事件。
- `public/widget.html`：页面根背景改为 `transparent`；“下一张”改调 `widget_next`；关闭按钮改调 `hide_widget`。
- `src/components/Sidebar.vue`：监听 `widget-visibility-changed`，同步 `widgetVisible`。
- `src-tauri/capabilities/default.json`：`windows` 增加 `"widget"`，让运行时创建的独立窗口获得已声明权限。

### 架构与依赖影响
- 无新增依赖。
- 不改变主窗口布局、数据库结构、右键菜单注册表逻辑。
- 继续沿用已接受的 Rust 端 widget 生命周期管理方案。

### 关键实现点
- widget 独立窗口必须被写入 capabilities，否则独立页面内 Tauri API 权限可能不完整。
- 透明效果需要 Rust 窗口 `.transparent(true)` 和 HTML 根背景 `transparent` 同时满足。
- widget 的“下一张”走 `perform_action("next")`，复用 `auto-rotated` 事件，主窗口 store 能更新当前壁纸和统计。
- widget 隐藏统一由 Rust command 发出 `widget-visibility-changed`，避免 Sidebar 本地状态漂移。

### 踩坑记录
- 已追加 `AI_DIARY.md`：#widget-004、#verify-001。

### 验证证据
- `cargo check` — PASS（提权后通过；仅 2 个既有 unused warning：`get_current_wallpaper` / `is_valid_wallpaper`）。
- `npx vue-tsc --noEmit` — PASS。
- `npm run build` — PASS（提权后通过；Vite 36 modules transformed，`dist/widget.html` 存在）。

### 未解决项
- 尚未在真实 Tauri 窗口中手动拖拽/点击验证视觉和事件效果；建议运行 `npm run tauri dev` 后验证显示、下一张、喜欢、不喜欢、关闭再打开。

### 踩坑记录
详见 AI_DIARY.md #permission-001, #widget-001

### 未解决项
- 悬浮挂件独立窗口白屏（#widget-005），多次尝试均失败
- 轮播定时器 1 秒轮询有微量 CPU 消耗

---

## 2026-06-01 — 悬浮挂件白屏排查 + 代码回退

### 变更目标
修复悬浮挂件白屏问题，排查 7 种方案均失败，最终回退到窗口内 Vue 组件方案（临时）

### 尝试记录
1. 移除 `transparent: true`，CSS 改不透明背景 → 白屏
2. 移除 `background_color` 调用 → 白屏
3. 移除 `window-state` 插件 → 白屏
4. `WebviewUrl::App("widget.html")` → 白屏
5. `WebviewUrl::External("file://...")` → 白屏
6. 窗口内 FloatingWidget.vue → 可用但无实际意义

### 当前状态
- 悬浮挂件暂时是主窗口内浮动 Vue 组件（功能正常但仅在主窗口内可见）
- widget_content.html 保留备用
- 需要调研 Tauri 2 多窗口白屏根因

### 未解决项
- 悬浮挂件独立窗口白屏（详见 AI_DIARY #widget-005）

---

## 2026-05-28 (续) — Phase 2 Bug 修复

### 修复内容
1. **轮播定时器**：重写，累加 elapsed 至 rotation_secs 才轮播（之前硬编码 60 秒）
2. **右键菜单**：改用 .ps1 脚本文件 + PowerShell 字符串拼接，恢复子菜单结构
3. **Widget like/dislike**：改为 emit tray-like/tray-dislike 复用已有事件链路
4. **CLI action 路径**：统一为 APPDATA/com.purewall.app/，新增 app_data_dir() helper

### 代码范围
- `src-tauri/src/main.rs`：重写 start_rotation_timer()，新增 app_data_dir()，修复 handle_cli_action() 路径
- `src-tauri/src/context_menu.rs`：完全重写，.ps1 文件方案，子菜单结构
- `src-tauri/src/main.rs`：widget_like/widget_dislike 改发 tray-like/tray-dislike

### 验证证据
- `cargo check` — PASS
- 手动执行 register_menu.ps1 — 注册表条目正确创建，子菜单结构完整
- `purewall.exe --action next` — 成功切换壁纸并保存路径

### 验证证据
- `cargo check` — PASS（2 warnings）
- `vue-tsc --noEmit` — PASS（零 error）

### 未解决项
- 轮播定时器 1 秒轮询有微量 CPU 消耗（ADR-005 proposed）
- prompt 链改进需在实际后续开发中验证效果

### 验证证据
- `cargo check` — Rust 编译通过，仅 2 个 unused warning
- `vue-tsc --noEmit` — TypeScript 检查通过
- `vite build` — 前端构建成功 (12.67 KB CSS + 100.40 KB JS)
- `npm run tauri dev` — 应用启动成功，壁纸切换功能可用

### 未解决项
- 轮播定时器使用 1 秒轮询，轻度 CPU 消耗
- 缩略图首次加载全部壁纸时可能较慢（需遍历磁盘）
- `read_image_thumbnail` 单条命令冗余（已被 batch 替代）

### 下一步
- Phase 2: 桌面右键菜单注入、悬浮挂件、开机自启

---

## 2026-05-29 — Phase 2 修复 + 优化

### 变更目标
1. 右键菜单重写（SubCommands + reg.exe）
2. 缩略图磁盘缓存优化启动速度
3. 悬浮窗位置记忆 + 显示/隐藏修复

### 代码范围
- `src-tauri/src/context_menu.rs`：完全重写 4 次，最终方案 reg.exe + .bat + SubCommands
- `src-tauri/src/thumbnails.rs`：新增磁盘缓存模块（替换内存 HashMap）
- `src-tauri/src/main.rs`：移除 read_image_thumbnail/base64_encode，改用 ThumbnailCache；添加 windows_subsystem 属性
- `src/components/Sidebar.vue`：悬浮窗 toggleWidget 改用 hide/show
- `src-tauri/Cargo.toml`：新增 tauri-plugin-window-state
- `src-tauri/capabilities/default.json`：新增 window-state/webview-hide/show 权限

### 关键实现点
- `context_menu.rs`：MUIVerb + SubCommands + inline shell\，用 reg.exe add /ve 写入默认值（New-ItemProperty -Name "(default)" 无效）
- `thumbnails.rs`：ThumbnailCache 用路径哈希做缓存键，app_data_dir/thumbnails/*.jpg
- `main.rs`：#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] 隐藏 release 版控制台

### 踩坑记录
详见 AI_DIARY.md #registry-001 (子菜单不可行原因), #registry-002 (违规修改系统设置), #registry-003 (恢复方式)

### 验证证据
- `cargo check` — PASS（2 warnings）
- `vue-tsc --noEmit` — PASS
- 手动测试右键菜单 — SubCommands 子菜单成功展开

### 未解决项
- 悬浮窗 hide 后 show 失效（#widget-002，已重构为 Rust 端管理待验证）
- Win11 右键菜单在"显示更多选项"中（平台限制）

---

## 2026-05-29 (续) — Widget 生命周期重构 + Single-Instance + 文档约束

### 变更目标
1. Widget 生命周期彻底修复（Rust 端管理 + Alt+F4 拦截）
2. Single-instance 插件（右键菜单不再启动新进程）
3. 统一 perform_action 三部曲（DB → API → emit）
4. CLAUDE.md 添加约束 7（每次改动必须更新文档）+ Post-Flight Checklist

### 代码范围
- `src-tauri/src/main.rs`：新增 toggle_widget Command（show-or-create + CloseRequested 拦截），perform_action 统一入口，single-instance 插件注册
- `src-tauri/src/tray.rs`：重写，tray 操作统一走 perform_action
- `src/components/Sidebar.vue`：toggleWidget 简化为 invoke("toggle_widget")
- `public/widget.html`：新增 ✕ 关闭按钮，getCurrentWindow().hide()
- `src-tauri/Cargo.toml`：新增 tauri-plugin-single-instance
- `src-tauri/capabilities/default.json`：新增 window-hide, window-get-all-windows 等权限
- `CLAUDE.md`：新增约束 7 + Post-Flight Checklist

### 验证证据
- `cargo check` — PASS（2 warnings）
- `vue-tsc --noEmit` — PASS

## 2026-06-01 (Phase 2 Fix) - Hover Menu Experience & Empty State Fix

### 变更目标
1. 修复由于空滤镜或删除最后一张壁纸导致的全局 EmptyState 劫持问题 (The Fallback / Empty State Hijack)。现在只有当数据库中总壁纸数(store.stats.total)为0时才显示初始化提示。
2. 在画廊界面 (Gallery) 中为过滤后的空结果（如当前分类无喜欢的壁纸）添加正确的占位符展示。
3. 补齐缺失的 👎 不喜欢 (Dislike) 按钮，与 Widget 功能保持一致。
4. 修正了壁纸卡片图片因为 \group-hover\ 的瞬间缩放丢失问题 (Transition Desync)，改成绑定 \isHovered\ 并加上 \duration-300\ 平滑过渡。

### 代码范围
- \src/App.vue\：修改了路由判断，使用 \store.stats.total === 0\ 决定是否展示 Onboarding 的 EmptyState。
- \src/components/Gallery.vue\：添加内部的空数据占位符 (\<div v-else-if="store.wallpapers.length === 0">\)。
- \src/components/WallpaperCard.vue\：添加 dislike 按钮；重构了 img 组件样式以解决缩放动画回弹突兀的问题。

### 验证证据
- \cargo check\ -> PASS
- \ue-tsc --noEmit\ -> PASS
 
## 2026-06-02 - Widget single-entry routing + pre-created window

### Change goal
Fix #widget-005 by removing runtime widget WebView creation and external widget HTML loading. The widget now uses the same Vue/Vite entry as the main app and is pre-created by Tauri at startup.

### Code scope
- `package.json` / `package-lock.json`: added `vue-router@4`.
- `src/router.ts`: added Vue Router hash routes for `/` and `/widget`.
- `src/App.vue`: reduced root component to `RouterView`.
- `src/views/Home.vue`: moved the previous main app layout and startup loading logic out of `App.vue`.
- `src/views/WidgetView.vue`: added independent widget view that invokes Rust commands for like/next/dislike/hide.
- `src/styles.css`: added transparent widget body mode and compact widget styling.
- `src/components/Sidebar.vue`: listens for `widget-visibility-changed` so the button state follows widget-side hide.
- `src/stores/wallpapers.ts`: listens for `wallpaper-rating-changed` from widget commands.
- `src-tauri/tauri.conf.json`: added explicit `main` label and static hidden `widget` window at `index.html#/widget`.
- `src-tauri/src/main.rs`: rewrote widget control to show/hide the pre-created window; added `hide_widget`, `widget_next`, `widget_like`, and `widget_dislike`; removed runtime `WebviewWindowBuilder` path.
- Removed obsolete `widget_content.html` and unused `src/components/FloatingWidget.vue`.
- `docs/project-docs/DECISIONS.md`: added accepted ADR-006 for hash router single entry + pre-created widget window.

### Verification evidence
- `cargo check` PASS after elevated rerun; normal sandbox hit known target write permission issue. Remaining warnings are the existing unused `get_current_wallpaper` / `is_valid_wallpaper`.
- `npx vue-tsc --noEmit` PASS.
- `npm run build` PASS after elevated rerun; normal sandbox hit known esbuild `spawn EPERM`.
- `npm run tauri dev -- --no-watch --exit-on-panic --verbose` reached long-running GUI/dev-process state with no immediate config error captured, then was stopped by timeout. Leftover Vite/cargo/PureWall processes from the smoke test were cleaned up; port 1420 was confirmed released.

### Unresolved items
- Needs manual visual verification in the real Tauri window: show widget, click next/like/dislike, hide from widget, re-show from Sidebar, and confirm no white screen or main-window freeze.

## 2026-06-02 - Phase 3 gallery management

### Change goal
Implement the Phase 3 management layer: custom tags, blacklist management, batch actions, and gallery sorting.

### Code scope
- `docs/project-docs/DECISIONS.md`: added accepted ADR-007 for normalized tags, `wallpapers.blacklisted`, and whitelisted filter/sort queries.
- `docs/project-docs/ARCHITECTURE.md`: added Phase 3 gallery management architecture addendum.
- `src-tauri/src/db.rs`: added `TagEntry`, `WallpaperEntry.tags`, `WallpaperEntry.blacklisted`, `Stats.blacklisted`, additive SQLite migration for `blacklisted`, `tags` and `wallpaper_tags` tables, tag CRUD, blacklist updates, batch operations, and filter/sort gallery queries.
- `src-tauri/src/main.rs`: exposed Phase 3 commands for filtered wallpaper loading, tags, blacklist updates, and batch rating/tag/blacklist/delete operations.
- `src/stores/wallpapers.ts`: expanded Pinia state for tags, sort mode, selection mode, selected paths, batch actions, and blacklist-aware filtering.
- `src/components/Sidebar.vue`: rebuilt the sidebar controls with All/Liked/Disliked/Hidden filters, tag creation/deletion, stats, rotation controls, and existing Phase 2 settings.
- `src/components/Gallery.vue`: added sticky sort controls and selection toolbar for batch like/dislike/tag/hide/restore/delete.
- `src/components/WallpaperCard.vue`: added selection affordance, tag pills, hidden badge, and per-card hide/restore action.
- `src/styles.css`: added reusable batch/action button styles.

### Verification evidence
- `npx vue-tsc --noEmit` PASS.
- `cargo check` PASS after elevated rerun; normal sandbox hit the known target write permission issue. Remaining warnings are the existing unused `get_current_wallpaper` / `is_valid_wallpaper`.
- `npm run build` PASS after elevated rerun; normal sandbox hit the known esbuild `spawn EPERM`.

### Unresolved items
- Needs manual Tauri window verification for tag creation/deletion, tag filtering, batch actions, blacklist restore flow, and sort ordering against a real wallpaper library.

## 2026-06-02 - Repository bootstrap

### Change goal
Prepare PureWall for publishing to a private GitHub repository.

### Code scope
- No application code changes.
- `.gitignore`: excluded local `.claude/` assistant settings and accidental `src-tauri/$c/` registry residue from the first commit.
- `docs/project-docs/AI_DIARY.md`: recorded stale GitHub CLI token pitfall.
- Local Git repository initialization and first commit are part of the release workflow, not runtime behavior.

### Verification evidence
- `cargo check` PASS with existing unused warnings.
- `npx vue-tsc --noEmit` PASS.
- Created private GitHub repository `WiseZenn/PureWall`.
- Pushed `main` to `origin/main` with initial commit `9afa19d`.

### Unresolved items
- None for repository bootstrap.

## 2026-06-02 - Phase 4 polish layer

### Change goal
Complete Phase 4 baseline: multi-display wallpaper modes, game/focus auto-pause, and yearly wallpaper statistics.

### Code scope
- `docs/project-docs/DECISIONS.md`: added accepted ADR-008 for Phase 4 display/focus/telemetry architecture.
- `docs/project-docs/ARCHITECTURE.md`: added Phase 4 addendum.
- `src-tauri/Cargo.toml`: enabled `Win32_System_Com` and `Win32_UI_Shell`.
- `src-tauri/src/wallpaper.rs`: added `IDesktopWallpaper` display discovery, all-display fill/span placement, and per-monitor wallpaper setting with COM STA helper.
- `src-tauri/src/focus.rs`: added foreground full-screen detection and focus mode status model.
- `src-tauri/src/db.rs`: added `settings`, `play_events`, yearly stats queries, and multi-wallpaper weighted selection for independent displays.
- `src-tauri/src/main.rs`: persisted Phase 4 settings, routed next/rotation/CLI actions through display modes, added focus monitor thread, and exposed display/focus/yearly stats commands.
- `src-tauri/src/tray.rs`: routed tray next/pause through the unified Phase 4 playback/pause path.
- `src/stores/wallpapers.ts`: added display mode, monitor list, focus status, yearly stats, and Phase 4 event handling.
- `src/components/Sidebar.vue`: added display mode controls and focus pause toggle.
- `src/components/InsightsPanel.vue`, `src/components/Gallery.vue`, `src/styles.css`, `src/views/Home.vue`: added the yearly dashboard and initialization hook.

### Verification evidence
- `cargo check` PASS after elevated rerun; normal sandbox hit the known target write permission issue. Remaining warnings are the existing unused `get_current_wallpaper` / `is_valid_wallpaper`.
- `npx vue-tsc --noEmit` PASS.
- `npm run build` PASS after elevated rerun; normal sandbox hit the known esbuild `spawn EPERM`.

### Unresolved items
- Needs manual Tauri window verification on real hardware: All/Span/Each display next wallpaper, focus pause while a fullscreen game/app is foreground, and yearly dashboard updates after real rotations.

## 2026-06-02 - Phase 4 Tauri dev smoke test

### Test goal
Run a real Tauri development smoke test for the Phase 4 build.

### Verification evidence
- `cargo check` PASS; remaining warnings are the existing unused `get_current_wallpaper` / `is_valid_wallpaper`.
- `npx vue-tsc --noEmit` PASS.
- `npm run tauri dev -- --no-watch --exit-on-panic --verbose` launched the long-running dev stack and was stopped by the 60s smoke-test timeout.
- During the smoke test, `purewall.exe` was running from `src-tauri/target/debug/purewall.exe`.
- `http://localhost:1420` returned HTTP `200`, confirming the Vite dev server was serving the app.
- After the smoke test, leftover Node/Cargo/PureWall processes were stopped and port 1420 was confirmed clear.

### Unresolved items
- Full visual interaction still needs human confirmation inside the opened Tauri window: display mode switching, focus pause with a real fullscreen app, widget show/hide, and yearly stats changes after real wallpaper rotations.

## 2026-06-03 - Phase 5 UI redesign plan and first workbench pass

### Change goal
Start Phase 5 as a frontend-first UI redesign. The goal is to make PureWall feel like a usable wallpaper management assistant instead of an early prototype: stronger wallpaper preview, clearer navigation, fewer overloaded panels, and selected-image actions grouped in one predictable inspector.

### Phase 5 plan
- Information architecture: keep the first screen as the real app, not a landing page; use a three-pane workbench with navigation, gallery, and inspector.
- Visual direction: dark graphite desktop surface, larger image-led stage, calmer controls, compact repeated actions, and less card-heavy decoration.
- Interaction model: click a wallpaper to inspect it, double-click/card action to set it, keep batch selection explicit, and keep playback/system controls available without burying the gallery.
- Near-term polish queue: real-window visual QA, responsive width pass, icon consistency pass, hover/focus states, empty/loading state refinement, and cleanup of obsolete Phase 4 dashboard wiring if no longer needed.

### Code scope already changed
- `docs/project-docs/DECISIONS.md`: added accepted ADR-009 for the Phase 5 three-pane workbench information architecture.
- `docs/project-docs/ARCHITECTURE.md`: added the Phase 5 UI workbench addendum and updated the Phase 4 UI note to point at the inspector.
- `docs/project-docs/AI_DIARY.md`: added #tool-001 for the Windows sandbox `rg.exe` access-denied fallback.
- `src/views/Home.vue`: changed the main window layout to `Sidebar` + `Gallery` + `InspectorPanel`.
- `src/components/Sidebar.vue`: rebuilt the sidebar as a lean navigation/tag rail with library counts and total plays.
- `src/components/Gallery.vue`: replaced the previous dashboard-heavy gallery with an image-led wallpaper stage, sort tabs, selection bar, and masonry grid.
- `src/components/WallpaperCard.vue`: updated card behavior and visuals around active selection, hover actions, ratings, tags, and hidden state.
- `src/components/InspectorPanel.vue`: added the new right-side inspector for selected wallpaper details, tag assignment, playback controls, display mode, focus pause, desktop menu, autostart, widget visibility, and mini yearly stats.
- `src/components/TitleBar.vue`: restyled the custom title bar to match the darker Phase 5 shell.
- `src/stores/wallpapers.ts`: added `activeWallpaperPath`, `activeWallpaper`, `activeThumbnailUrl`, and active-selection maintenance around load, remove, rotate, and set-wallpaper flows.
- `src/styles.css`: added the Phase 5 graphite workbench tokens and layout styles while retaining existing widget styles.

### Verification evidence
- `cargo check` PASS on 2026-06-03; remaining warnings are the existing unused `get_current_wallpaper` / `is_valid_wallpaper`.
- `npx vue-tsc --noEmit` PASS on 2026-06-03.
- `npm run build` PASS after elevated rerun on 2026-06-03; normal sandbox hit the known esbuild `spawn EPERM`.

### Unresolved items
- Real Tauri window visual verification is still needed with an actual wallpaper library: desktop width, narrow width, active selection, inspector actions, batch selection, display controls, and widget toggle.
- The generated concept direction has not yet been compared against a rendered app screenshot.

## 2026-06-03 - Phase 5 concept-aligned UI polish pass

### Change goal
Continue Phase 5 against the provided concept image `D:\Desktop\PureWall\ig_0ad02e45ef04d6c1016a1f09d518d08191bb1afcb8afab9987.png`: remove visible glyph corruption, tighten the three-pane workbench, add search, and verify the rendered layout with screenshots.

### Code scope
- `src/components/AppIcon.vue`: added a small inline SVG icon component for consistent app chrome controls.
- `src/stores/wallpapers.ts`: added `searchQuery` and `visibleWallpapers` for frontend search without changing the Rust/SQLite query contract.
- `src/components/Gallery.vue`: rebuilt the Phase 5 gallery topbar, search field, current wallpaper stage, four-action stage controls, sort bar, selection actions, and visible-wallpaper rendering.
- `src/components/Sidebar.vue`: replaced corrupted text glyphs with `AppIcon`, cleaned the navigation rail, tag list, and tag creation controls.
- `src/components/WallpaperCard.vue`: replaced corrupted hover/selection/rating/action glyphs with SVG icons and kept active/selected behavior intact.
- `src/components/InspectorPanel.vue`: cleaned selected wallpaper preview, metadata, actions, playback, system toggles, and yearly insight controls; removed corrupted separators/glyphs.
- `src/components/EmptyState.vue`: replaced mojibake onboarding copy with a clean Phase 5 empty state.
- `src/styles.css`: added search/topbar, icon, inspector metadata, empty state, and narrow-width responsive polish.

### Verification evidence
- Pre-flight `cargo check` PASS; final `cargo check` PASS. Remaining warnings are the existing unused `get_current_wallpaper` / `is_valid_wallpaper`.
- Pre-flight `npx vue-tsc --noEmit` PASS; final `npx vue-tsc --noEmit` PASS.
- `npm run build` PASS after elevated rerun; normal sandbox hit the known esbuild `spawn EPERM`.
- Source glyph scan PASS: no `src/` matches for the known mojibake glyph patterns.
- Rendered QA PASS using bundled Playwright package with system Chrome fallback and mocked Tauri IPC:
  - Desktop screenshot: `C:\tmp\purewall-phase5-desktop.png`
  - Search screenshot: `C:\tmp\purewall-phase5-search.png`
  - Narrow screenshot: `C:\tmp\purewall-phase5-narrow.png`
  - QA JSON: `C:\tmp\purewall-phase5-qa.json`
  - Checks: app title `PureWall`, library content present, search narrows to `1 visible`, four stage actions present, no relevant console errors.

### Unresolved items
- Needs manual real Tauri verification against an actual wallpaper library because screenshot QA used mocked IPC and generated data URLs for layout proof.
- The rendered implementation is structurally aligned with the concept, but concept-grade photo fidelity depends on real wallpaper thumbnails from the user's library.

## 2026-06-03 - Phase 5 concept replica tightening pass

### Change goal
Further reduce the gap between the rendered PureWall UI and the concept image. The main visual target was to match the concept's desktop workbench structure: titlebar search, left navigation rail without duplicate branding, central current-wallpaper workbench with playback/settings controls, and right inspector focused on selected wallpaper details plus yearly insights.

### Code scope
- `src/components/TitleBar.vue`: moved the search field and filter button into the top titlebar so the gallery content starts higher like the concept.
- `src/components/AppIcon.vue`: added desktop/filter/settings/sliders/tag icons for the concept-style navigation and controls.
- `src/components/Sidebar.vue`: removed the duplicate brand block, changed the main nav to Library/Liked/Hidden/Tags, added System links and a storage meter, and simplified tag rows.
- `src/components/Gallery.vue`: moved playback interval, display mode, and focus pause into the central current-wallpaper stage; changed the stage to a concept-like image + controls/settings layout; kept search filtering wired through the titlebar.
- `src/components/InspectorPanel.vue`: removed duplicated Playback/System sections and rebuilt the right panel around selected wallpaper preview, metadata, color swatches, tags, and yearly insights.
- `src/styles.css`: tightened workbench column widths, titlebar search, sidebar density, stage sizing, large action buttons, uniform 3-column gallery grid, inspector swatches, and yearly stats button styling.

### Verification evidence
- `cargo check` PASS. Remaining warnings are the existing unused `get_current_wallpaper` / `is_valid_wallpaper`.
- `npx vue-tsc --noEmit` PASS.
- `npm run build` PASS after elevated rerun; normal sandbox hit the known esbuild `spawn EPERM`.
- Rendered QA PASS using bundled Playwright package with system Chrome fallback and mocked Tauri IPC:
  - Concept inspected: `D:\Desktop\PureWall\ig_0ad02e45ef04d6c1016a1f09d518d08191bb1afcb8afab9987.png`
  - Latest desktop screenshot: `C:\tmp\purewall-phase5-replica-desktop-v2.png`
  - QA JSON: `C:\tmp\purewall-phase5-replica-qa-v2.json`
  - Checks: app title `PureWall`, no relevant console errors, stage settings rendered, and the screenshot was visually compared with the concept.

### Remaining differences
- Browser QA still uses mocked IPC and generated placeholder thumbnails; real Tauri verification with the user's wallpaper library is needed for final image-crop fidelity.
- The concept includes a few non-functional decorative/right-toolbar controls and bottom status/autostart chrome that are not fully implemented yet.

## 2026-06-03 - Phase 5 render-fidelity tightening pass

### Change goal
Respond to the remaining mismatch between the live PureWall UI and the concept render. This pass focused on spatial fidelity rather than new behavior: move the current-wallpaper label/image/buttons into the same visual rhythm as the reference, restore the right inspector's top tool strip, add the bottom status chrome, and reduce excess central spacing.

### Code scope
- `src/components/Gallery.vue`: moved `CURRENT WALLPAPER` back above the stage image, removed the count block from the control area, added the stage overflow menu, and moved visible wallpaper count into the gallery toolbar.
- `src/components/InspectorPanel.vue`: added the concept-style inspector toolbar above the selected wallpaper panel.
- `src/views/Home.vue`: added a bottom status bar matching the reference layout.
- `src/components/Sidebar.vue`: formatted navigation and footer counts with locale thousands separators.
- `src/styles.css`: tightened gallery top/right padding, stage height/padding, inspector toolbar height, central stage surface styling, toolbar count alignment, and bottom status bar styling.

### Verification evidence
- Pre-flight `cargo check` PASS; final `cargo check` PASS. Remaining warnings are the existing unused `get_current_wallpaper` / `is_valid_wallpaper`.
- Pre-flight `npx vue-tsc --noEmit` PASS; final `npx vue-tsc --noEmit` PASS.
- `npm run build` PASS after elevated rerun; normal sandbox hit the known esbuild `spawn EPERM`.
- Rendered QA PASS using local Chrome CDP fallback because Browser plugin was unavailable and bundled Playwright was missing `playwright-core`:
  - Desktop screenshot: `C:\tmp\purewall-phase5-tight-desktop.png`
  - Search screenshot: `C:\tmp\purewall-phase5-tight-search.png`
  - Checks: app title `PureWall`, stage rendered, inspector toolbar rendered, status bar rendered, search narrowed to `1 wallpapers`, no framework overlay, no captured console errors.

### Unresolved items
- Real Tauri verification with the user's actual wallpaper library is still needed for final crop fidelity and native window chrome feel.
- Bottom `Auto-start On` status is currently visual chrome only; it intentionally does not change registry/autostart behavior in this UI-only pass.

## 2026-06-03 - Phase 5 Windows 11 Fluent tokenized UI refactor

### Change goal
Refactor the accepted Phase 5 concept into a more native-feeling Windows 11 Fluent/Mica-inspired desktop UI while preserving the existing PureWall information architecture and Pinia/Tauri behavior.

### Code scope
- `src/styles.css`: replaced legacy visual tokens with semantic `data-theme="dark"` / `data-theme="light"` tokens, stable 240px/340px shell layout, Segoe UI Variable typography, compact Fluent panels, focus-visible states, card states, and responsive rules.
- `src/views/Home.vue`: delegates UI composition to `AppShell.vue` and sets the default root theme to `data-theme="dark"`.
- `src/components/AppShell.vue`, `StatusBar.vue`, `SearchToolbar.vue`, `CurrentWallpaperPanel.vue`, `WallpaperControls.vue`, `SettingRow.vue`, `WallpaperGrid.vue`: split the app shell, title/search/status/current-wallpaper/gallery responsibilities into smaller components.
- `src/components/IconButton.vue`, `ToggleSwitch.vue`, `SidebarItem.vue`, `TagPill.vue`, `MetadataRow.vue`, `ColorSwatchList.vue`, `YearlyInsightsChart.vue`: added reusable UI primitives for consistent interaction states and accessibility.
- `src/components/AppIcon.vue`: replaced the earlier mixed icon set with a custom outline SVG system, including the new PureWall logo mark.
- `src/components/TitleBar.vue`, `Sidebar.vue`, `Gallery.vue`, `WallpaperCard.vue`, `InspectorPanel.vue`, `EmptyState.vue`: refactored to consume the new component system and semantic tokens while keeping existing store actions.
- `docs/project-docs/ARCHITECTURE.md`: documented the tokenized Fluent shell and component split.
- `docs/project-docs/AI_DIARY.md`: appended the new frontend QA/CDP pitfall.

### Verification evidence
- Pre-flight `cargo check` PASS; final `cargo check` PASS. Remaining warnings are the existing unused `get_current_wallpaper` / `is_valid_wallpaper`.
- Pre-flight `npx vue-tsc --noEmit` PASS; final `npx vue-tsc --noEmit` PASS.
- `npm run build` PASS after elevated rerun; normal sandbox hit the known esbuild `spawn EPERM`.
- Source scan PASS for removed macOS/SF/legacy token strings: no `src/` matches for `macOS`, `SF Pro`, `-apple-system`, `--bg-primary`, `--surface`, or `glass`.
- Rendered QA PASS using system Chrome through CDP because the Browser plugin was unavailable and bundled Playwright lacked `playwright-core`:
  - Reference inspected: `D:\Desktop\PureWall\ChatGPT Image 2026年6月3日 20_18_39.png`
  - Desktop screenshot: `C:\tmp\purewall-fluent-desktop.png`
  - Search screenshot: `C:\tmp\purewall-fluent-search.png`
  - Narrow screenshot: `C:\tmp\purewall-fluent-narrow.png`
  - QA JSON: `C:\tmp\purewall-fluent-qa.json`
  - Checks: page title `PureWall`, no Vite overlay, search narrowed to `1 wallpapers`, clearing search restored `9 wallpapers`, clicking the third card updated the inspector to `Aurora Lake.jpg`.

### Preserved functionality
- Existing store actions and Tauri command contracts remain unchanged: search/filter/sort, next/like/dislike/pause, interval/display mode/focus pause, selection/batch actions, tags, hidden state, and yearly stats.
- No Rust command, SQLite schema, registry operation, autostart behavior, or wallpaper playback logic was changed in this pass.

### Unresolved items
- Real Tauri window verification with the user's actual wallpaper library is still needed for final photo crop fidelity, native WebView rendering, and live Windows wallpaper actions.
- Resolution/file size/color extraction still uses conservative UI placeholders where backend metadata is unavailable.

## 2026-06-03 - Phase 5 UI optimization second pass

### Change goal
Continue the Windows 11 Fluent UI optimization after the first refactor pass. This pass focused on reducing static/fake controls and tightening interaction feedback without changing Rust commands, SQLite schema, registry behavior, or wallpaper playback logic.

### Code scope
- `src/stores/wallpapers.ts`: added frontend-only `wallpaperViewMode` state with `grid`, `list`, and `compact` modes.
- `src/components/Gallery.vue`, `src/components/WallpaperGrid.vue`, `src/components/WallpaperCard.vue`: wired the view-mode buttons to real rendered states, added list-card details, and preserved existing selection/rating/set-wallpaper actions.
- `src/components/SearchToolbar.vue`: changed the filter icon into an active clear-search control while search text is present.
- `src/components/StatusBar.vue`: replaced the hardcoded `Auto-start On` display with a read-only `is_autostart_enabled` status and a disabled switch, avoiding registry mutation in this UI-only pass.
- `src/components/IconButton.vue`: added `aria-pressed` for active icon buttons.
- `src/components/Sidebar.vue`, `src/styles.css`: removed duplicate Ready status from the sidebar and added grid/list/compact responsive layout polish.

### Verification evidence
- Pre-flight `cargo check` PASS; final `cargo check` PASS. Remaining warnings are the existing unused `get_current_wallpaper` / `is_valid_wallpaper`.
- Pre-flight `npx vue-tsc --noEmit` PASS; final `npx vue-tsc --noEmit` PASS.
- `npm run build` PASS after elevated rerun; normal sandbox hit the known esbuild `spawn EPERM`.
- Rendered QA PASS using system Chrome through CDP because Browser plugin was unavailable:
  - Desktop screenshot: `C:\tmp\purewall-round2-desktop.png`
  - List-mode screenshot: `C:\tmp\purewall-round2-list.png`
  - Filtered screenshot: `C:\tmp\purewall-round2-filtered.png`
  - Narrow screenshot: `C:\tmp\purewall-round2-narrow.png`
  - QA JSON: `C:\tmp\purewall-round2-qa.json`
  - Checks: page title `PureWall`, no framework overlay, no captured console errors, list mode rendered, search cleared back to `9 wallpapers`, clicking the third card updated inspector to `Northern Lake.jpg`, autostart status rendered as read-only `Off` in the mock.

### Unresolved items
- Real Tauri window verification with the user's actual wallpaper library is still needed for native WebView rendering, live wallpaper actions, and actual autostart status.
- System section links and several overflow/toolstrip controls remain presentational until a later settings/navigation pass wires them to real panels.

## 2026-06-08 - Phase 5 UI optimization third pass

### Change goal
Continue the Fluent UI optimization plan by reducing remaining presentational chrome. This pass wires the sidebar System links, status-bar Settings button, and inspector toolbar actions to real frontend state while preserving the UI-only boundary: no Rust command changes, SQLite schema changes, registry writes, or autostart mutation.

### Code scope
- `src/stores/wallpapers.ts`: added frontend-only `workspaceSection` state for `library`, `displays`, `settings`, `shortcuts`, `advanced`, and `insights`; selecting wallpapers or filters returns the inspector to the library view.
- `src/components/Sidebar.vue`: made System links active/selectable and connected them to `workspaceSection`.
- `src/components/StatusBar.vue`: made the Settings icon open the right-side Settings panel while keeping autostart read-only.
- `src/components/InspectorPanel.vue`: wired inspector grid/list/insights toolbar buttons, added compact Displays/Settings/Shortcuts/Advanced panels, and kept existing display/focus/pause commands on their current store paths.
- `src/styles.css`: added compact system-panel styling and fixed the 800px minimum-window layout so the inspector remains reachable by hiding the side rail between 700-920px instead of hiding the inspector.
- `docs/project-docs/AI_DIARY.md`: appended `#responsive-001` for the min-width inspector visibility pitfall.

### Verification evidence
- Pre-flight `cargo check` PASS; final `cargo check` PASS. Remaining warnings are the existing unused `get_current_wallpaper` / `is_valid_wallpaper`.
- Pre-flight `npx vue-tsc --noEmit` PASS; final `npx vue-tsc --noEmit` PASS.
- `npm run build` PASS after elevated rerun; normal sandbox hit the known esbuild `spawn EPERM`.
- Rendered QA PASS using system Chrome headless with a local `dist/` server and mocked Tauri IPC because Browser plugin was unavailable and bundled Playwright lacks `playwright-core`:
  - Desktop Displays screenshot: `C:\tmp\purewall-system-panel-desktop.png`
  - Desktop Settings screenshot: `C:\tmp\purewall-system-panel-settings.png`
  - Minimum-width Settings screenshot after responsive fix: `C:\tmp\purewall-system-panel-minwidth-v2.png`
  - QA JSON: `C:\tmp\purewall-system-panel-qa.json`
  - Checks: page rendered as PureWall, Settings panel contained `Focus Pause` and `Pause State`, no Vite/framework overlay in DOM, and screenshots were produced successfully.

### Unresolved items
- Real Tauri window verification with the user's actual wallpaper library is still needed for native WebView rendering and live Windows wallpaper actions.
- Shortcuts/Advanced panels are informational only in this UI-only pass; no registry or system setting mutation was added.

## 2026-06-08 - Phase 5 workflow and scaling repair pass

### Change goal
Fix the usability regressions reported after the Fluent refactor: settings needed an obvious close path, folder/wallpaper actions needed to be visible again during normal library use, preview images were too blurry, color swatches needed copyable HEX values, and narrow scaling needed stricter layout rules to avoid clipped controls.

### Code scope
- `src-tauri/src/thumbnails.rs`: added a separate 960px / JPEG 82% preview generator alongside the existing 256px thumbnail generator.
- `src-tauri/src/main.rs`: added and registered `load_preview_image` for active-wallpaper preview loading.
- `src/stores/wallpapers.ts`: added `previews`, `activePreviewUrl`, `loadPreview`, and active-preview loading on selection/current-wallpaper changes.
- `src/components/CurrentWallpaperPanel.vue`: switched the large current preview to `activePreviewUrl` and changed the dead more button into a real Choose Folder action using the Tauri dialog plugin.
- `src/components/Gallery.vue`: restored a visible Folder action in the gallery toolbar for changing the wallpaper library after onboarding.
- `src/components/InspectorPanel.vue`: switched preview to `activePreviewUrl`, added close buttons for settings/system/insights panels, added explicit `Set Wallpaper`, `Open Folder`, and `Copy Path` actions for the selected wallpaper, and made compact top-color swatches copyable.
- `src/components/ColorSwatchList.vue`: made color swatches clickable buttons that copy HEX values and show temporary copied feedback.
- `src/components/AppIcon.vue`: added a `copy` icon for file/path actions.
- `src/styles.css`: added responsive button/grid constraints for inspector actions, color swatches, wrapped toolbars, system-panel close buttons, and 340px inspector button readability.
- `docs/project-docs/ARCHITECTURE.md`: documented the two-tier thumbnail/preview image path.
- `docs/project-docs/AI_DIARY.md`: appended `#preview-001`.

### Verification evidence
- Pre-flight `cargo check` PASS; final `cargo check` PASS. Remaining warnings are the existing unused `get_current_wallpaper` / `is_valid_wallpaper`.
- Pre-flight `npx vue-tsc --noEmit` PASS; final `npx vue-tsc --noEmit` PASS.
- `npm run build` PASS after elevated rerun; normal sandbox hit the known esbuild `spawn EPERM`.
- Rendered QA PASS using system Chrome headless with a local `dist/` server and mocked Tauri IPC because Browser plugin was unavailable and bundled Playwright lacks `playwright-core`:
  - Main/actions screenshot: `C:\tmp\purewall-actions-main.png`
  - Settings screenshot: `C:\tmp\purewall-actions-settings.png`
  - Closed-settings screenshot: `C:\tmp\purewall-actions-closed.png`
  - Minimum-width screenshot: `C:\tmp\purewall-actions-minwidth.png`
  - Final action-layout screenshot: `C:\tmp\purewall-actions-final.png`
  - QA JSON: `C:\tmp\purewall-actions-qa.json`
  - Checks: `Folder`, `Set Wallpaper`, `Copy Path`, and copyable `#111418` color controls were present, settings close returned to the library inspector, no Vite/framework overlay appeared, and screenshots were produced successfully.

### Unresolved items
- Real Tauri window verification with the user's actual wallpaper library is still needed for native WebView rendering, real folder opening, and real clipboard behavior.
- The app now has a more stable responsive fallback, but the long-term professional layout direction should be explicit breakpoints: full three-pane desktop, two-pane compact desktop, then single-pane narrow mode rather than arbitrary viewport scaling.

## 2026-06-09 - Phase 5 import, metadata, and color picker pass

### Change goal
Continue the reported workflow repairs after clarifying that the missing operation is import, not export. This pass adds explicit additive folder/image imports, fixes selected-image resolution/file-size availability, and redesigns color swatches as compact circular color-picker controls with copyable values.

### Code scope
- `docs/project-docs/DECISIONS.md`: added accepted ADR-010 for persisting scanned image metadata in SQLite before applying the schema change.
- `src-tauri/src/scanner.rs`: added `scan_files()` for explicit image imports and `image_info()` for selected-file metadata refreshes.
- `src-tauri/src/db.rs`: added `wallpapers.width`, `wallpapers.height`, and `wallpapers.file_size` additive migrations; persisted metadata through `upsert_wallpaper()` and added metadata update/read helpers.
- `src-tauri/src/main.rs`: added and registered `import_wallpaper_folder`, `import_wallpaper_files`, and `get_image_metadata`; import commands add to the existing library instead of replacing it.
- `src/stores/wallpapers.ts`: added import state/actions, active metadata fallback from wallpaper rows, and metadata refresh/write-through after selected image reads.
- `src/components/Gallery.vue` and `src/components/CurrentWallpaperPanel.vue`: wired `Import Folder` and `Import Images` through the Tauri dialog plugin.
- `src/components/InspectorPanel.vue`: displays real resolution/file size and uses the color picker component for compact top colors.
- `src/components/ColorSwatchList.vue`: redesigned swatches as circular dots; hover/focus/click opens a top-level popover with HEX/RGB/HSL rows and copy controls.
- `src/styles.css`: added import toolbar status styling, circular swatches, color popover rows, and compact color-row constraints.
- `docs/project-docs/ARCHITECTURE.md`: documented additive imports, persisted image metadata, and the color popover clipping boundary.
- `docs/project-docs/AI_DIARY.md`: appended `#metadata-001` and `#popover-001`.

### Verification evidence
- Pre-flight `cargo check` PASS; final `cargo check` PASS. Remaining warnings are the existing unused `get_current_wallpaper` / `is_valid_wallpaper`.
- Pre-flight `npx vue-tsc --noEmit` PASS; final `npx vue-tsc --noEmit` PASS.
- `npm run build` PASS after elevated rerun.
- Rendered QA PASS using system Chrome headless with mocked Tauri IPC because Browser plugin was unavailable and bundled Playwright lacks a managed browser:
  - Desktop/color-popover screenshot: `C:\tmp\purewall-import-desktop.png`
  - Narrow layout screenshot: `C:\tmp\purewall-import-narrow.png`
  - QA JSON: `C:\tmp\purewall-import-qa.json`
  - Checks: `Import Folder` and `Import Images` rendered, inspector showed `2560x1440` and `7.00 MB`, color popover rendered HEX/RGB/HSL rows, no framework overlay appeared, and no console errors were captured.

### Unresolved items
- Real Tauri window verification with the user's actual wallpaper library is still needed for native dialogs, native clipboard writes, and real file-path metadata failures.
- Palette extraction is still mocked/static in the UI; this pass improves copy/display behavior but does not implement real dominant-color extraction from images.

## 2026-06-09 - Phase 5 container-aware scaling repair

### Change goal
Fix the scaling/overlap shown in the user's screenshot where the middle current-wallpaper panel and toolbar extended underneath the right inspector at Windows-scaled desktop widths.

### Code scope
- `src/styles.css`: made `.main-workspace` a CSS container and added container queries keyed to the central pane width rather than only viewport width.
- `src/styles.css`: constrained `.current-wallpaper-panel` with `min-width: 0` and `overflow: hidden`, then switched compact central panes to a narrower image/control split.
- `src/styles.css`: changed compact wallpaper controls to a 2x2 grid, hid the optional setting note in that tight control column, wrapped the gallery toolbar, and added a single-column fallback for very narrow central panes.
- `docs/project-docs/ARCHITECTURE.md`: documented the container-aware workbench layout behavior.
- `docs/project-docs/AI_DIARY.md`: appended `#responsive-002`.

### Verification evidence
- Pre-flight `cargo check` PASS; final `cargo check` PASS. Remaining warnings are the existing unused `get_current_wallpaper` / `is_valid_wallpaper`.
- Pre-flight `npx vue-tsc --noEmit` PASS; final `npx vue-tsc --noEmit` PASS.
- `npm run build` PASS after elevated rerun.
- Rendered QA PASS using system Chrome headless with mocked Tauri IPC because Browser plugin was unavailable:
  - Screenshot at screenshot-like width: `C:\tmp\purewall-phase5-tight-desktop.png`
  - Search-state screenshot: `C:\tmp\purewall-phase5-tight-search.png`
  - Width checks:
    - 1448px viewport: `mainScrollWidth == mainClientWidth` (868), `currentScrollWidth == currentClientWidth` (826), toolbar width matched, current panel right edge stayed before inspector.
    - 1200px viewport: `mainScrollWidth == mainClientWidth` (620), `currentScrollWidth == currentClientWidth` (578), toolbar width matched.
    - 800px viewport: `mainScrollWidth == mainClientWidth` (520), `currentScrollWidth == currentClientWidth` (494), toolbar width matched.

### Unresolved items
- Real Tauri WebView verification on the user's exact Windows display scale is still useful, but the rendered browser QA now exercises the same CSS constraints that caused the overlap.

## 2026-06-13 - Project skill reinstall and restrained UI simplification

### Change goal
Reinstall the requested project-level design skills, then apply their audit guidance to make the PureWall Windows workbench quieter and more direct without changing backend commands, SQLite, registry behavior, autostart mutation, or wallpaper playback.

### Skill scope
- `.agents/skills/design-taste-frontend`: reinstalled Taste Skill v2 (`design-taste-frontend`) from `Leonxlnx/taste-skill`.
- `.codex/skills/ui-ux-pro-max`: reinstalled UI UX Pro Max for Codex through `uipro-cli`.
- `.agents/skills/figma-implement-design`: installed the Codex official Figma implementation skill.
- `.agents/skills/winui-app`: installed the Codex official Windows app / WinUI guidance skill.
- Applied design read: restrained Windows media manager, Fluent-oriented, low motion, medium density, one accent, minimal decorative chrome.

### Code scope
- `src/components/Sidebar.vue`: removed fabricated storage-capacity UI and added a real Insights destination.
- `src/components/SearchToolbar.vue`: removed the inert filter affordance; the secondary action now appears only when search can be cleared.
- `src/components/Gallery.vue`: replaced four sort tabs with one native sort select while preserving import, selection, count, and view controls.
- `src/components/CurrentWallpaperPanel.vue`: removed the duplicate folder-import action.
- `src/components/InspectorPanel.vue`: removed the fake view-size slider, duplicate view controls, redundant close-details action, always-open compact analytics, and static/mock color palette display.
- `src/components/StatusBar.vue`: reduced the footer to read-only readiness and autostart status, removing duplicate settings and disabled switch chrome.
- `src/components/WallpaperCard.vue`: removed the always-visible duplicate favorite badge so wallpaper imagery remains primary.
- `src/components/WallpaperControls.vue`: added accessible labels/titles for compact icon-only behavior.
- `src/styles.css`: removed the decorative app gradient, reduced redundant panel borders, simplified section-label typography, opened the current/inspector surfaces, and added a minimum-width compact command-row layout.
- `docs/project-docs/AI_DIARY.md`: appended `#responsive-003`.

### Verification evidence
- Pre-flight `cargo check` PASS; final `cargo check` PASS. Only the existing unused `get_current_wallpaper` / `is_valid_wallpaper` warnings remain.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- Final `npm run build` PASS after elevated rerun; normal sandbox first hit the known `esbuild spawn EPERM`.
- Rendered QA PASS using system Chrome through Playwright with mocked Tauri IPC because the Browser plugin was unavailable:
  - Wide desktop screenshot: `C:\tmp\purewall-minimal-desktop.png`
  - Insights screenshot: `C:\tmp\purewall-minimal-insights.png`
  - Final 800px minimum-width screenshot: `C:\tmp\purewall-minimal-800-final-v2.png`
  - 1440px: `mainScrollWidth == mainClientWidth` and current-panel scroll width matched.
  - 800px: `mainScrollWidth == mainClientWidth` (`520`), current-panel width matched (`496`), current-panel height matched scroll height (`290`), and all four compact actions rendered with accessible labels.
  - Search interaction narrowed `9` wallpapers to `1`; Clear Search restored `9`; Insights opened from the sidebar.
  - No framework overlay or relevant app console error appeared; the only browser log was the local static server's missing favicon `404`.

### Unresolved items
- Real Tauri WebView verification with the user's wallpaper library is still needed for native image crops, dialogs, shell actions, and exact Windows display scaling.
- The installed skills require a Codex restart before they are auto-discovered in a new session; their guidance was read directly and applied in this session.

## 2026-06-13 - Fluent icon semantics and interaction polish

### Change goal
Use the installed Taste, UI UX Pro Max, and Windows app guidance to make PureWall's icon language clearer and more consistent while preserving the restrained desktop visual direction.

### Code scope
- `src/components/AppIcon.vue`: added distinct compact-view, wallpaper, sort, select, image, keyboard, and advanced-settings symbols; simplified the brand mark and removed misleading icon reuse.
- `src/components/Gallery.vue`: added semantic sort/select icons and replaced the compact-view ellipsis with a compact-grid symbol.
- `src/components/InspectorPanel.vue` and `src/components/WallpaperCard.vue`: changed Set Wallpaper actions from the Next icon to a dedicated wallpaper icon.
- `src/components/Sidebar.vue`: replaced the textual tag-create plus sign with the shared icon component.
- `src/styles.css`: normalized icon sizing, softened the brand mark, clarified active navigation/view states, and added icon-level favorite feedback.
- No backend commands, SQLite behavior, registry entries, autostart behavior, or system settings were changed.

### Verification evidence
- Final `cargo check` PASS. Only the existing unused `get_current_wallpaper` / `is_valid_wallpaper` warnings remain.
- Final `npx vue-tsc --noEmit` PASS.
- Final `npm run build` PASS after elevated rerun; normal sandbox first hit the known `esbuild spawn EPERM`.
- Rendered QA PASS using system Chrome through Playwright with mocked Tauri IPC because the Browser plugin was unavailable:
  - Desktop icon overview: `C:\tmp\purewall-icon-polish-desktop.png`
  - Compact-view icon/state overview: `C:\tmp\purewall-icon-polish-compact.png`
  - Advanced navigation/icon overview: `C:\tmp\purewall-icon-polish-advanced.png`
  - Confirmed 32 rendered icon instances, distinct semantic icon classes, compact-view switching, and Advanced panel navigation.
  - No relevant app console error appeared; the only browser log was the local static server's missing favicon `404`.

### Unresolved items
- Real Tauri WebView verification with the user's wallpaper library is still useful for native rendering and exact Windows display scaling.

## 2026-06-13 - Official Fluent System Icons migration

### Change goal
Replace the remaining hand-drawn icon paths with one consistent Microsoft Fluent System Icons family, including clearer Like and Settings symbols.

### Code scope
- `package.json` and `package-lock.json`: added the official MIT-licensed `@fluentui/svg-icons` package.
- `src/components/AppIcon.vue`: replaced all hand-drawn SVG templates with a centralized mapping to official Fluent 24px Regular SVG assets; preserved existing icon names and aliases so feature components required no logic changes.
- `src/components/AppIcon.vue`: added the official Filled heart asset for active Like/favorite states.
- `src/styles.css`: renders Fluent SVG assets through theme-aware CSS masks and switches active hearts to the Filled asset.
- No backend commands, SQLite behavior, registry entries, autostart behavior, or system settings were changed.

### Verification evidence
- Pre-flight and final `cargo check` PASS. Only the existing unused `get_current_wallpaper` / `is_valid_wallpaper` warnings remain.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- Final `npm run build` PASS after elevated rerun; normal sandbox first hit the known `esbuild spawn EPERM`.
- Rendered QA PASS using system Chrome through Playwright with mocked Tauri IPC:
  - Full Fluent icon overview: `C:\tmp\purewall-fluent-icons-all.png`
  - Settings panel/icon overview: `C:\tmp\purewall-fluent-icons-settings.png`
  - Confirmed 32 rendered icon instances, zero empty icon masks, successful Settings navigation, and Regular-to-Filled heart switching after Like.

### Unresolved items
- `npm install` reported 3 high-severity dependency audit findings. They were not auto-fixed because `npm audit fix --force` may introduce breaking dependency changes and is outside this icon-only task.
- Real Tauri WebView verification with the user's wallpaper library is still useful for native rendering and exact Windows display scaling.

## 2026-06-13 - Frontend build-chain security remediation

### Change goal
Remove the three npm high-severity audit findings introduced through the Vite 6 build chain without using an uncontrolled forced audit fix.

### Security finding
- The three audit entries were one transitive root issue reported across `esbuild`, `vite`, and `@vitejs/plugin-vue`.
- Root advisory: `GHSA-gv7w-rqvm-qjhr`, affecting `esbuild >=0.17.0 <0.28.1`.
- The vulnerable path is esbuild's Deno module, which could execute a malicious downloaded binary when an attacker controls `NPM_CONFIG_REGISTRY`; PureWall does not use Deno, so the direct runtime exposure was low, but the vulnerable build dependency remained in the supply chain.

### Code scope
- `package.json` and `package-lock.json`: upgraded `vite` from `6.4.2` to `8.0.16` and `@vitejs/plugin-vue` from `5.2.4` to `6.0.7`.
- `package.json`: declared the Vite 8-compatible Node requirement `^20.19.0 || >=22.12.0`.
- Vite 8 removed the vulnerable esbuild dependency from PureWall's installed dependency tree.
- No application behavior, backend commands, SQLite behavior, registry entries, autostart behavior, or system settings were changed.

### Verification evidence
- `npm audit --json` PASS: 0 vulnerabilities across all severities.
- `npm ls vite esbuild @vitejs/plugin-vue --all` confirms `vite@8.0.16`, `@vitejs/plugin-vue@6.0.7`, and no installed esbuild dependency.
- `npm run build` PASS with Vite 8 after elevated rerun; 126 modules transformed.
- Pre-flight and final `cargo check` PASS. Only the existing unused `get_current_wallpaper` / `is_valid_wallpaper` warnings remain.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.

### Unresolved items
- Vite 8 requires Node `^20.19.0 || >=22.12.0`; the current environment uses Node `24.15.0` and is compatible.

## 2026-06-13 - Remove unused wallpaper helpers

### Change goal
Remove two unreferenced Rust helpers so compiler warnings remain meaningful instead of permanently carrying known dead code.

### Code scope
- `src-tauri/src/wallpaper.rs`: removed unused `get_current_wallpaper()` and `is_valid_wallpaper()` helpers.
- Repository search confirmed neither helper was called by application code, Tauri commands, or tests.
- Current-wallpaper state continues to use PureWall's existing application/database flow; supported image validation continues to be owned by the scanner/import flow.
- No registry operation, application behavior, SQLite schema, autostart behavior, or system setting was changed.

### Verification evidence
- Final elevated `cargo check` PASS with zero warnings and zero errors. Normal sandbox first hit the known `target` directory access-denied issue.
- Final `npx vue-tsc --noEmit` PASS.
- Repository search confirms no remaining `get_current_wallpaper` or `is_valid_wallpaper` symbols.
- `npm run build` PASS with Vite 8.
- `npm audit --json` PASS with 0 vulnerabilities.

### Unresolved items
- None for this dead-code cleanup.

## 2026-06-13 - Persistent dark and light themes

### Change goal
Make PureWall's existing but unreachable light palette fully usable through an accessible, persistent Dark / Light theme switch.

### Code scope
- `src/composables/useTheme.ts`: added a shared Dark / Light theme state, root `data-theme` application, `color-scheme` synchronization, and guarded `localStorage` persistence.
- `src/main.ts`: restores the saved theme before Vue mounts to avoid applying the wrong theme after the first render.
- `src/views/Home.vue`: removed the old dark-default assignment that bypassed a real theme preference.
- `src/components/InspectorPanel.vue`: added an interactive Appearance section in Settings with Dark and Light segmented controls and current-state ARIA attributes.
- `src/components/AppIcon.vue`: added official Fluent dark-theme and sunny icons for the theme control.
- `src/styles.css`: recalibrated light-mode backgrounds, borders, text, muted text, and accent colors; added the theme-control layout.
- No backend commands, SQLite behavior, registry entries, autostart behavior, or system settings were changed.

### Verification evidence
- `cargo check` PASS with zero warnings and zero errors.
- `npx vue-tsc --noEmit` PASS.
- `npm run build` PASS with Vite 8; 129 modules transformed.
- Rendered QA PASS using system Chrome through Playwright with mocked Tauri IPC:
  - Dark Settings/theme screenshot: `C:\tmp\purewall-theme-dark-settings.png`
  - Light Settings/theme screenshot: `C:\tmp\purewall-theme-light-settings.png`
  - Confirmed Dark is the default when no preference exists, switching to Light updates the root theme immediately, `purewall-theme=light` is stored, and reload restores Light.
  - Light-mode contrast checks: primary text `16.13:1`, secondary text `7.01:1`, muted text `5.24:1`, and accent on white `4.88:1`.
- `npm audit --json` PASS with 0 vulnerabilities.

### Unresolved items
- Real Tauri WebView verification is still useful for exact Windows title-bar rendering in both themes.

## 2026-06-13 - Figma direction selection and Quiet Canvas mode

### Change goal
Adopt the selected Figma product direction: combine Gallery Studio's image-led identity with Control Deck's management efficiency, while making Quiet Canvas a persistent immersive mode.

### Design decision
- Created the editable Figma exploration file: `https://www.figma.com/design/YAfSe61vDuPDYuwLMultpc`.
- User approved the Gallery Studio + Control Deck direction and requested Quiet Canvas as a switchable mode.
- Added accepted ADR-011 for the dual workspace / Quiet Canvas presentation model.

### Code scope
- `src/composables/useWorkspaceMode.ts`: added frontend-only, persistent `workbench` / `quiet` mode state independent from the dark/light theme.
- `src/components/QuietCanvas.vue`: added an image-first immersive surface with current-wallpaper details, Keep, Next, Pause/Resume, and a clear return-to-workspace action.
- `src/components/AppShell.vue`: switches between the complete management workbench and Quiet Canvas while reusing the same Pinia store and backend commands.
- `src/components/TitleBar.vue` and `src/components/InspectorPanel.vue`: added discoverable Quiet Canvas entry points in both the title bar and Settings.
- `src/components/AppIcon.vue`: added official Fluent full-screen enter/exit icons for the mode controls.
- `src/main.ts`, `package.json`, and `package-lock.json`: initialize the saved workspace mode and bundle the Geist + Inter variable font families.
- `src/styles.css`: added the Quiet Canvas presentation layer and updated product/display typography to use the bundled fonts.
- No Rust commands, SQLite behavior, registry entries, autostart behavior, or system settings were changed.

### Verification evidence
- Final `cargo check` PASS with zero warnings and zero errors.
- Final `npx vue-tsc --noEmit` PASS.
- Final `npm run build` PASS after the established elevated rerun for the known Windows Vite `spawn EPERM`; 136 modules transformed and bundled Geist/Inter font assets were emitted.
- `npm audit --json` PASS with 0 vulnerabilities.

### Unresolved items
- Real Tauri WebView visual review is still useful to tune the Quiet Canvas veil and typography against the user's real wallpaper collection.
- Figma Starter-plan MCP call limits prevented capturing screenshots for every concept, but the editable concept frames remain in the Figma file.

## 2026-06-13 - Living Gallery fusion design artifact

### Change goal
Create the selected high-fidelity PureWall design direction directly after Figma MCP limits blocked further canvas writes.

### Design scope
- `design/PureWall-Living-Gallery-Fusion.svg`: added a fully editable 1440x1024 vector mockup combining Living Gallery's immersive surface, Contact Sheet Studio's `PURE WALL` wordmark, and Curatorial Stage's now-playing hierarchy.
- The design makes `Import folder` and `Add images` persistent first-level actions.
- The design introduces a quiet floating navigation rail, an attached command dock, horizontal curated collection rows, and an on-demand slide-over inspector.
- `design/PureWall-Living-Gallery-Fusion.md`: documented typography, visual rules, screen structure, and intended interactions.
- `design/PureWall-Living-Gallery-Fusion.png`: saved the rendered visual reference.
- No application code, backend commands, SQLite behavior, registry entries, autostart behavior, or system settings were changed.

### Verification evidence
- SVG rendered at exactly 1440x1024 in system Chrome through Playwright.
- Visual review confirmed no clipping or horizontal/vertical overflow.
- Import actions, brand wordmark, command dock, curated rows, and slide-over inspector are visibly distinct at the target resolution.

### Unresolved items
- The SVG still needs to be imported into Figma manually or through Figma MCP after its Starter-plan call allowance resets.
- Production implementation and native Tauri visual QA have not started for this direction.

## 2026-06-13 - Living Gallery production interface

### Change goal
Implement the approved Living Gallery fusion direction in the real PureWall Vue/Tauri interface while preserving the existing wallpaper-management behavior and backend contract.

### Code scope
- `src/composables/useInspector.ts`: added shared frontend-only inspector visibility state so the detail/system panel can slide away and reopen when needed.
- `src/components/AppShell.vue`: added active-wallpaper ambient surface styling and changed the inspector from a permanent grid column to an optional overlay.
- `src/components/TitleBar.vue`: introduced the distinctive `PURE WALL` / `Living Gallery` lockup, persistent first-level `Import folder` and `Add images` actions, import feedback, and an inspector toggle.
- `src/components/CurrentWallpaperPanel.vue`: rebuilt the current-wallpaper area as an immersive now-playing stage with real title, category, resolution, play count, attached command dock, interval, display mode, and focus-pause controls.
- `src/components/Gallery.vue` and `src/components/WallpaperGrid.vue`: moved imports out of the secondary toolbar, added a collection heading, and grouped grid-mode wallpapers into horizontal curated rows based on their first tag.
- `src/components/Sidebar.vue`, `src/components/WallpaperCard.vue`, and `src/components/InspectorPanel.vue`: system destinations and wallpaper selection now open the inspector; the inspector can close without changing backend state.
- `src/components/EmptyState.vue`: added separate folder and image import entry points.
- `src/components/WallpaperControls.vue`: renamed the positive preference action to the more curatorial `Keep` label while preserving the existing like behavior.
- `src/styles.css`: implemented the complete Living Gallery visual layer, Geist-first typography, ambient wallpaper color, floating navigation, attached command dock, curated collection tracks, slide-over inspector, responsive behavior, and reduced-motion handling.
- No Rust commands, SQLite behavior, registry entries, autostart mutation, or system settings were changed.

### Verification evidence
- Final `cargo check` PASS with zero warnings and zero errors.
- Final `npx vue-tsc --noEmit` PASS.
- Final `npm run build` PASS after the established elevated rerun for the known Windows Vite `spawn EPERM`; 135 modules transformed.
- Rendered browser QA PASS using system Chrome with mocked Tauri IPC:
  - Desktop Living Gallery: `C:\tmp\purewall-round2-desktop.png`
  - 800px minimum-width layout: `C:\tmp\purewall-round2-narrow.png`
  - List-mode compatibility: `C:\tmp\purewall-round2-list.png`
  - Confirmed nine rendered wallpapers, working search/list interactions, visible `Import folder` at minimum width, selected wallpaper inspector content, and no framework overlay or relevant console errors.

### Unresolved items
- Real Tauri WebView verification with the user's wallpaper library is still needed for native dialog behavior, real image crops, exact Windows display scaling, and the inspector transition.

## 2026-06-13 - Fix Vite dev startup EMFILE failure

### Change goal
Restore `npm run tauri dev` startup after Vite 8 attempted to scan the generated Rust documentation tree as frontend HTML entries.

### Root cause and code scope
- The attached error output showed Vite opening more than 24,000 files under `src-tauri/target/doc/` and failing with `EMFILE: too many open files`.
- Existing `server.watch.ignored: ["**/src-tauri/**"]` only disabled file watching; it did not limit Vite's dependency-entry discovery.
- `vite.config.ts`: added `optimizeDeps.entries: ["index.html"]` so Vite scans only PureWall's actual frontend HTML entry.
- `docs/project-docs/AI_DIARY.md`: appended `#vite-001` with the reusable diagnosis.
- No generated Rust documentation was deleted, and no backend, registry, SQLite, autostart, or system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS with zero warnings and zero errors.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- Final `cargo check` PASS with zero warnings and zero errors.
- Final `npx vue-tsc --noEmit` PASS.
- Vite 8 dev-server smoke test PASS: the server listened on `127.0.0.1:1420`, returned HTTP `200` with the Vite client entry, and did not emit `target/doc` dependency-scan or `EMFILE` errors.
- The verified test Vite process was stopped and port `1420` was confirmed clear.
- Final `npm run build` PASS after the established elevated rerun for the known Windows Vite `spawn EPERM`; 135 modules transformed.
- `git diff --check` PASS for the changed configuration and project-memory files; only expected LF-to-CRLF working-copy notices were reported.

### Unresolved items
- None for this startup fix.

## 2026-06-13 - Non-overlapping inspector and quiet wallpaper labels

### Change goal
Remove the inspector obstruction visible in the real Tauri window and stop long numeric file names from dominating image-first surfaces.

### Code scope
- `src/components/AppShell.vue` and `src/styles.css`: the open inspector now occupies a reserved workbench grid column instead of overlaying the stage and command dock; at narrower widths, duplicate stage quick settings hide before controls can clip.
- `src/utils/wallpaperPresentation.ts`: added one presentation rule for readable wallpaper titles and accessible preview labels.
- `src/components/CurrentWallpaperPanel.vue`, `src/components/InspectorPanel.vue`, `src/components/WallpaperCard.vue`, and `src/components/QuietCanvas.vue`: removed direct file-name display from primary visual surfaces. The first curated tag is used as an optional readable title; untagged wallpapers render without a title.
- Full file paths remain available through Open Folder, Copy Path, and the inspector Folder metadata row.
- No Rust commands, SQLite schema, registry entries, autostart behavior, or system settings were changed.

### Verification evidence
- Pre-flight and final `cargo check` PASS with zero warnings and zero errors.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- `git diff --check` PASS for the changed source files; only expected LF-to-CRLF working-copy notices were reported.
- Static layout verification confirms the inspector is a third grid column when open and the main workspace remains a separate `minmax(0, 1fr)` column.
- Normal sandbox `npm run build` reached the known Vite 8 `spawn EPERM` environment restriction. The established elevated rerun and browser visual QA were unavailable because the current Codex execution allowance was exhausted.

### Unresolved items
- Real Tauri WebView visual confirmation is still needed for the exact window width and Windows display scaling shown in the user's screenshot.
- Reading human-readable place names from image properties is not implemented. GPS metadata provides coordinates, not a reliable offline place name; tags remain the current explicit location/title mechanism.

## 2026-06-14 - Windows Details presentation metadata

### Change goal
Use the same human-readable title and credit fields shown by Windows Properties > Details instead of promoting long file names in PureWall.

### Code scope
- `src-tauri/src/shell_metadata.rs`, `src-tauri/src/main.rs`, and `src-tauri/Cargo.toml`: added a narrow Windows Shell Property System command for Title, Subject, Author, Copyright, and Comment.
- `src/stores/wallpapers.ts`: added an in-memory Shell metadata cache and display-title priority of Title, Subject, first tag, then blank.
- `src/components/CurrentWallpaperPanel.vue`, `src/components/QuietCanvas.vue`, `src/components/InspectorPanel.vue`, and `src/components/WallpaperCard.vue`: use Shell titles on image-first surfaces and expose available credits/details in the inspector.
- Shell metadata is loaded only for active or selected images and is not persisted to SQLite.
- No registry entries, system settings, autostart behavior, or SQLite schema were changed.

### Verification evidence
- Final elevated `cargo check` PASS with zero errors.
- Final elevated `cargo test --no-run` PASS.
- Final `npx vue-tsc --noEmit` PASS.
- Final elevated `npm run build` PASS; 136 modules transformed.
- The real sample `desktop-008_lucerneswitzerland_gettyimages-584814596_3840x2160.jpg` returned Title and Subject `蓝色时刻的桥梁`, its Getty Images author/copyright credits, and its Bing Spotlight comment from the same Windows Shell property source.

### Unresolved items
- Real Tauri WebView visual confirmation remains useful for checking long author/comment wrapping with the user's full library.

## 2026-06-14 - Remove wallpaper titles from visual surfaces

### Change goal
Keep PureWall image-first by removing wallpaper titles that added visual noise even when readable Shell metadata was available.

### Code scope
- `src/components/CurrentWallpaperPanel.vue`, `src/components/QuietCanvas.vue`, `src/components/InspectorPanel.vue`, and `src/components/WallpaperCard.vue`: removed wallpaper titles from the stage, Quiet Canvas, inspector, card hover layer, and list rows.
- `src/stores/wallpapers.ts`, `src-tauri/src/shell_metadata.rs`, and `src/utils/wallpaperPresentation.ts`: removed display-title state and stopped reading Title and Subject while retaining Author, Copyright, Comment, tags, and accessible preview labels.
- `src/styles.css`: removed title-only styling rules.
- No registry entries, system settings, autostart behavior, or SQLite schema were changed.

### Verification evidence
- Pre-flight and final `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- Final elevated `npm run build` PASS; 143 modules transformed.
- Targeted `git diff --check` PASS; only existing Windows LF/CRLF notices were reported.
- Final elevated `cargo check` PASS with zero errors.
- Final `npx vue-tsc --noEmit` PASS.
- Final elevated `npm run build` PASS; 136 modules transformed.
- Title-reference search returned no remaining visual title bindings or title-only style selectors.
- `rustfmt --check src/shell_metadata.rs` and targeted `git diff --check` PASS.

### Unresolved items
- None.

## 2026-06-15 - Preserve navigation before inspector on narrow windows

### Change goal
Keep the global left navigation visible as the window narrows and remove the lower-priority right inspector first.

### Code scope
- `src/styles.css`: from medium viewport widths downward, both open and closed inspector layouts use a persistent left navigation column plus the main gallery; the right inspector is automatically hidden.
- At the smallest breakpoint the left navigation narrows slightly but remains visible.
- Inspector state is preserved, so an inspector that was open returns automatically when the window is widened again.
- No backend, registry, system setting, autostart, or SQLite schema behavior was changed.

### Verification evidence
- Pre-flight and final `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- Final elevated `npm run build` PASS; 143 modules transformed.
- Targeted responsive-rule inspection confirmed the Living Gallery overrides preserve `.side-rail` and hide `.inspector-shell` at 1180px, 920px, and 700px breakpoints.
- Targeted `git diff --check` PASS; only existing Windows LF/CRLF notices were reported.

### Unresolved items
- None.

## 2026-06-15 - Reserve vertical clearance above the stage dock

### Change goal
Prevent stage metadata from being covered by playback controls when side panels narrow the main workspace.

### Code scope
- `src/styles.css`: at narrow main-workspace container widths, anchors stage copy above the command dock with explicit bottom clearance instead of retaining the wide-screen downward top offset.
- Wide workspaces retain the lower visual copy position; only constrained workspaces use collision-safe positioning.
- No backend, registry, system setting, autostart, or SQLite schema behavior was changed.

### Verification evidence
- Pre-flight and final `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- Final elevated `npm run build` PASS; 143 modules transformed.
- Targeted `git diff --check` PASS; only existing Windows LF/CRLF notices were reported.

### Unresolved items
- None.

## 2026-06-15 - Stabilize stage controls across narrow workspaces

### Change goal
Prevent current-wallpaper controls from overlapping or becoming empty blocks as the main workspace narrows, and align the primary actions consistently.

### Code scope
- `src/styles.css`: removed the primary Next button's extra margin, aligned all four actions to the same height and baseline, prevented dock children from shrinking into overlaps, and made container-width rules hide quick settings before hiding primary-action labels.
- `src/styles.css`: retained readable `Next / Like / Dislike / Pause` labels down to a genuinely narrow 360px main workspace and stopped hiding the Add images label at medium viewport widths.
- `src/components/WallpaperControls.vue` and `src/components/QuietCanvas.vue`: renamed Keep to Like and updated the workbench action's accessible label/title.
- No backend, registry, system setting, autostart, or SQLite schema behavior was changed.

### Verification evidence
- Pre-flight and final `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- Targeted source search confirmed no remaining visible Keep labels.
- Targeted `git diff --check` PASS; only existing Windows LF/CRLF notices were reported.

### Unresolved items
- None.

## 2026-06-15 - Unify titlebar action hierarchy across themes

### Change goal
Make the titlebar import actions express the same primary/secondary hierarchy in both Light and Dark themes.

### Code scope
- `src/styles.css`: removed the light-only primary-button override so Import folder consistently uses the accent-soft primary treatment and Add images consistently uses the neutral panel treatment in both themes.
- No backend, registry, system setting, autostart, or SQLite schema behavior was changed.

### Verification evidence
- Pre-flight and final `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- Final elevated `npm run build` PASS; 143 modules transformed.
- Targeted source search confirmed no light-only primary import-button override remains.
- Targeted `git diff --check` PASS; only existing Windows LF/CRLF notices were reported.

### Unresolved items
- None.

## 2026-06-15 - Complete dark-theme titlebar import styling

### Change goal
Remove the remaining bright Import folder button from the dark titlebar while preserving its primary-action emphasis.

### Code scope
- `src/styles.css`: changed the default dark-theme Import folder action from an inverted white fill to an accent-tinted dark surface with accent border, text, and hover treatment. The existing light-theme override remains unchanged.
- No backend, registry, system setting, autostart, or SQLite schema behavior was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.

### Unresolved items
- None.

## 2026-06-15 - Theme-matched command dock and immediate display modes

### Change goal
Make the current-wallpaper controls belong to the active theme and make multi-display switching behavior clear without adding explanatory UI.

### Code scope
- `src/components/CurrentWallpaperPanel.vue` and `src/utils/displayModes.ts`: retained native dropdown menus and renamed the three modes to the concise `Same`, `Span`, and `Separate`.
- `src/components/InspectorPanel.vue`: uses the same concise labels in the existing compact segmented switch.
- `src/styles.css`: added a light frosted command-dock treatment with matching text, separators, hover states, primary action contrast, selects, and focus toggle; dark mode retains the dark image-overlay dock.
- `src-tauri/src/main.rs` and `src/stores/wallpapers.ts`: display mode changes now apply immediately and expose an applying/error state. Same and Span reuse the current wallpaper; Separate immediately assigns one wallpaper per detected display.
- Updated ADR-008 and the architecture memory to document immediate display-mode semantics.
- No registry, system setting, autostart, or SQLite schema changes were made.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- Final elevated `cargo check` PASS with zero errors.
- Final elevated `cargo test --no-run` PASS.
- Final elevated `npm run build` PASS; 137 modules transformed.
- Targeted source search confirmed no detailed display-mode descriptions or custom oversized stage menus remain.
- Targeted `git diff --check` PASS.

### Unresolved items
- None.

## 2026-06-14 - Lower main-stage copy position

### Change goal
Rebalance the current-wallpaper stage after title removal by moving the remaining Now Playing, description, and metadata copy lower on the image.

### Code scope
- `src/styles.css`: moved the desktop Living Gallery stage copy down by roughly 55-80px and the compact workspace copy down by roughly 35px while preserving clearance above the command dock.
- Quiet Canvas and inspector layouts were not changed.
- No backend, registry, system setting, autostart, or SQLite behavior was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- Final `cargo check` PASS.
- Final `npx vue-tsc --noEmit` PASS.
- Final elevated `npm run build` PASS; 136 modules transformed.
- Targeted `git diff --check` PASS.

### Unresolved items
- None.

## 2026-06-15 - Unified collection grid and theme-safe quick controls

### Change goal
Remove inconsistent collection browsing behavior, make compact dropdowns reliable in both themes, and make tagging and theme switching faster.

### Code scope
- `src/components/WallpaperGrid.vue` and `src/styles.css`: replaced per-group horizontal filmstrips with one consistent responsive grid size across Unassigned and tag groups; removed misleading row chevrons and horizontal scrollbars.
- `src/components/CompactDropdown.vue` and `src/components/CurrentWallpaperPanel.vue`: replaced the stage's native interval/display selects with compact theme-controlled dropdown menus while preserving concise dropdown interaction.
- `src/components/TagCombobox.vue` and `src/components/InspectorPanel.vue`: replaced the inspector tag select with an editable substring-matching combobox that can assign existing tags or create and immediately assign a new tag.
- `src/components/Sidebar.vue`: added a persistent bottom quick action for switching between Light and Dark themes.
- `src/components/AppIcon.vue` and `src/components/TitleBar.vue`: replaced the inspector toggle's document-like icon with Fluent `panel_right`.
- `src/styles.css`: made the light-theme Import folder action use the same light panel material instead of a black fill.
- No backend, registry, system setting, autostart, or SQLite schema behavior was changed.

### Verification evidence
- Pre-flight and final `cargo check` PASS with zero errors.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- Final elevated `npm run build` PASS; 143 modules transformed.
- Targeted `git diff --check` PASS; only existing Windows LF/CRLF notices were reported.
- Targeted source search confirmed the inspector tag select and stage interval/display native selects were removed.

### Unresolved items
- Real Tauri WebView visual confirmation remains useful for final compact-dropdown placement at the smallest supported window width.

## 2026-06-15 - Lower stage copy and surface active tags

### Change goal
Improve the visual balance of the current-wallpaper stage and make its metadata more useful without adding a wallpaper title.

### Code scope
- `src/components/CurrentWallpaperPanel.vue`: added up to three active wallpaper tags after resolution and play count, with a compact `+N` overflow indicator.
- `src/styles.css`: moved the desktop stage copy roughly 50px lower, slightly lowered the compact breakpoint copy, and added truncated colored-dot tag pills.
- No backend, registry, system setting, autostart, or SQLite schema behavior was changed.

### Verification evidence
- Pre-flight and final `cargo check` PASS with zero errors.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- Final elevated `npm run build` PASS; 143 modules transformed.
- Targeted `git diff --check` PASS; only the existing Windows LF/CRLF notice was reported.

### Unresolved items
- None.

## 2026-06-16 - Status theme switch, floating widget entry, and app icon polish

### Change goal
Move appearance switching to the real bottom-left status area, restore the floating widget entry, make compact stage controls degrade predictably, and replace the temporary blue-square app icon.

### Code scope
- `src/composables/useTheme.ts`, `src/components/StatusBar.vue`, and `src/components/InspectorPanel.vue`: added Dark / Light / System theme preference support, persisted the selected preference, followed OS theme changes in System mode, and exposed a compact status-bar cycle button next to Ready.
- `src/components/Sidebar.vue` and `src/components/AppIcon.vue`: removed the sidebar theme switch and restored a Fluent-styled Floating widget action that calls the existing `toggle_widget` command.
- `src/components/TitleBar.vue` and `src/styles.css`: made window minimize/maximize/hide calls explicit async operations, ensured window controls are outside the drag region, unified titlebar import button materials, and hid lower-priority stage quick settings before primary command labels can overlap.
- `src-tauri/src/tray.rs`: removed emoji from tray menu labels for a cleaner Windows utility menu.
- `scripts/generate-icons.ps1` and `src-tauri/icons/*`: added a reproducible PureWall icon generator and regenerated PNG/ICO app assets with a dark gallery mark and teal accent.
- No registry, system setting, autostart, or SQLite schema behavior was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- Final elevated `cargo check` PASS with zero errors after the sandbox blocked writes to `src-tauri/target`.
- Final elevated `npm run build` PASS; 144 modules transformed.
- Generated icon assets visually inspected at `src-tauri/icons/icon.png`.

### Unresolved items
- Real Tauri WebView visual confirmation remains useful for the smallest supported widths and for confirming the OS tray/taskbar icon after a fresh rebuild.

## 2026-06-17 - Larger minimal app icon and context-menu icon path

### Change goal
Make the PureWall app icon feel visually balanced beside other Windows app icons, and ensure the desktop right-click menu stops showing the old blue-square executable icon.

### Code scope
- `scripts/generate-icons.ps1`: replaced the busier dark illustration icon with a simpler high-contrast mark: light rounded tile, dark image frame, and one teal wall/gallery accent; reduced outer padding and enlarged the internal mark for taskbar and context-menu legibility.
- `src-tauri/icons/*`: regenerated `icon.png`, `icon.ico`, and small PNG sizes including 16px and 24px outputs.
- `src-tauri/src/context_menu.rs`: exports the embedded `icon.ico` to `APPDATA/com.purewall.app/purewall-menu.ico` during PureWall-owned context-menu registration, and writes the registry `Icon` value to that stable `.ico` file instead of the executable path.
- Current user registry state was updated only for PureWall's own `HKCU\Software\Classes\Directory\Background\shell\PureWall\Icon` value; no system-level registry entries were touched.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- Final elevated `cargo check` PASS after sandbox target-cache access denial.
- Final elevated `npm run build` PASS; 144 modules transformed.
- `reg query HKCU\Software\Classes\Directory\Background\shell\PureWall /v Icon` confirmed `C:\Users\zhong\AppData\Roaming\com.purewall.app\purewall-menu.ico`.

### Unresolved items
- Windows Explorer may keep showing a cached old context-menu icon until the menu is reopened, Explorer refreshes its icon cache, or the app is rebuilt/restarted.

## 2026-06-18 - Review report verification and fix plan

### Change goal
Verify whether the issues raised in `REVIEW_REPORT.md` are present in the current source tree and capture a prioritized repair plan without changing runtime behavior.

### Code scope
- Added `docs/project-docs/REVIEW_FIX_PLAN.md` with per-issue verdicts, source evidence, priority, and phased remediation plan.
- Reviewed backend CLI/playback/database/watcher/autostart paths, frontend store/component accessibility patterns, CSP configuration, and performance hot spots.
- No Rust, Vue, registry, autostart, SQLite schema, or system setting behavior was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- `DECISIONS.md` reviewed; ADR-005 remains proposed and is called out as a gate for timer-mechanism changes.
- Source searches confirmed the main reported issues and identified a few wording corrections in the original report.

### Unresolved items
- The new plan is documentation only; no reported runtime issue has been fixed yet.
- Hover overlay flicker and hover image crop severity still need visual QA before implementation priority is finalized.

## 2026-06-18 - First review-fix batch: SQLite and autostart safety

### Change goal
Start fixing the verified review findings with low-coupling backend changes: improve SQLite concurrency behavior, remove the wallpaper-list N+1 tag query, reduce stats query round-trips, and remove PowerShell string interpolation from autostart registry code.

### Code scope
- `src-tauri/src/db.rs`: added SQLite `busy_timeout(5s)`, `journal_mode=WAL`, and `foreign_keys=ON` during connection setup.
- `src-tauri/src/db.rs`: changed `attach_tags` from one query per wallpaper to one batched `IN (...)` query grouped by `wallpaper_id`.
- `src-tauri/src/db.rs`: collapsed `get_stats` from five separate aggregate queries into one conditional aggregate query.
- `src-tauri/src/autostart.rs`: replaced PowerShell `Set-ItemProperty` / `Remove-ItemProperty` / query scripts with direct `reg.exe` argument calls. This only changes code; no registry command was executed in this task.
- `docs/project-docs/REVIEW_FIX_PLAN.md`: marked R-04, R-37, and R-51 as fixed; marked R-13 and R-47 as partially fixed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- Final elevated `cargo check` PASS after normal sandbox hit known `src-tauri/target` write permission denial.
- Final elevated `cargo test --no-run` PASS after normal sandbox hit known incremental-cache write permission denial.
- `cargo fmt` applied successfully.

### Unresolved items
- R-13 remains partial: existing SQLite tables still lack declared foreign-key clauses and there is still no schema version row/migration ledger.
- R-47 remains partial: `get_yearly_stats` still uses multiple queries.
- Remaining P0/P1 findings still need follow-up batches, especially CSP, CLI/main-app state synchronization, user-visible error reporting, path validation, and CLI playback deduplication.

## 2026-06-18 - Second review-fix batch: CSP and basic accessibility

### Change goal
Continue fixing verified review findings with low-risk frontend and configuration changes: tighten the webview exposure surface, remove dead widget event listeners, and close several basic accessibility gaps.

### Code scope
- `src-tauri/tauri.conf.json`: disabled `withGlobalTauri` and removed script `unsafe-inline` / `unsafe-eval` from the CSP while preserving required app/image/IPC sources.
- `src/utils/wallpaperPresentation.ts`: changed wallpaper image alt text from a fixed label to metadata-aware text using tag, rating, and resolution when available.
- `src/components/SearchToolbar.vue`: added screen-reader-only label text for the search control.
- `src/components/InspectorPanel.vue`: added tag-specific aria labels to removable tag buttons.
- `src/stores/wallpapers.ts`: removed obsolete `widget-like` and `widget-dislike` listeners that had no emit source.
- `src/styles.css`: added `body` line height and a reusable `.sr-only` utility.
- `docs/project-docs/REVIEW_FIX_PLAN.md`: marked R-16, R-19, R-23, R-24, R-36, and R-49 as fixed.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- Final elevated `cargo check` PASS after normal sandbox hit the known `src-tauri/target` write permission denial.
- Final elevated `npm run build` PASS after normal sandbox hit the known Vite `spawn EPERM`.

### Unresolved items
- R-49 should still get a real Tauri runtime smoke test to confirm the stricter CSP does not block any WebView-only IPC or asset path.
- Remaining P0/P1 findings still need follow-up batches, especially CLI/main-app state synchronization, user-visible error reporting, path validation, and CLI playback deduplication.

## 2026-06-18 - Third review-fix batch: card interaction cleanup

### Change goal
Continue the incremental review repairs with contained frontend interaction fixes: prevent double-click from also selecting/opening a card, clean up transient confirmation timers, and remove a redundant responsive rule.

### Code scope
- `src/components/WallpaperCard.vue`: delayed normal card click slightly so double-click can cancel it before calling `setAsWallpaper`; keyboard activation remains immediate.
- `src/components/WallpaperCard.vue`: stores and clears delete-confirmation and click timers on completion, mouse leave, and component unmount.
- `src/components/Gallery.vue`: stores and clears the batch-delete confirmation timer on completion and component unmount.
- `src/styles.css`: removed the duplicate 900px `.stage-quick-settings` hide rule that was already covered by the later 1120px container rule.
- `docs/project-docs/REVIEW_FIX_PLAN.md`: marked R-25, R-35, and R-48 as fixed.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Final `cargo check` PASS.
- Final `npx vue-tsc --noEmit` PASS.
- Final elevated `npm run build` PASS after normal sandbox hit the known Vite `spawn EPERM`.

### Unresolved items
- Remaining P0/P1 findings still need follow-up batches, especially CLI/main-app state synchronization, user-visible error reporting, path validation, and CLI playback deduplication.

## 2026-06-18 - Fourth review-fix batch: shared pause state

### Change goal
Fix the highest-priority CLI pause drift by making right-click CLI pause and the running Tauri app use the same persisted pause state.

### Code scope
- `src-tauri/src/main.rs`: added `settings.paused` as the shared manual pause source, plus helpers for reading, writing, and syncing the effective pause state.
- `src-tauri/src/main.rs`: changed `toggle_pause`, `is_paused`, startup initialization, and the rotation timer to use the shared pause state.
- `src-tauri/src/main.rs`: changed CLI `--action pause` to toggle `settings.paused` directly instead of writing `paused.txt`; startup migrates an old `paused.txt` into the DB setting once.
- `src-tauri/src/main.rs`: preserved focus auto-pause as a runtime-only contributor to the effective pause state, so disabling focus auto-pause no longer blindly overrides a manual paused setting.
- `src/stores/wallpapers.ts`: loads initial pause state through `is_paused` during Phase 4 state initialization.
- `docs/project-docs/REVIEW_FIX_PLAN.md`: marked R-01 as fixed and R-10 as partially fixed.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- Final elevated `cargo check` PASS after normal sandbox hit the known `src-tauri/target` write permission denial.
- `cargo fmt` applied successfully.
- Final elevated `npm run build` PASS after normal sandbox hit the known Vite `spawn EPERM`.
- Targeted `git diff --check` PASS; only existing LF/CRLF notices were reported.

### Unresolved items
- R-10 remains partial: CLI like/dislike still rely on `current_wallpaper.txt`, and current-wallpaper persistence is still non-atomic.
- R-09 remains open: manual unpause while full-screen is still active can be auto-paused again by the focus monitor.
- Remaining P0/P1 findings still need follow-up batches, especially single-instance/CLI forwarding, user-visible error reporting, path validation, and CLI playback deduplication.

## 2026-06-18 - Fifth review-fix batch: persisted current wallpaper state

### Change goal
Finish R-10 by removing `current_wallpaper.txt` as the single source of truth for CLI/widget current-wallpaper actions.

### Code scope
- `src-tauri/src/main.rs`: added `settings.current_wallpaper` as the primary current-wallpaper state.
- `src-tauri/src/main.rs`: replaced direct current-wallpaper file writes with `persist_current_wallpaper`, which writes the DB setting first and updates `current_wallpaper.txt` only as a compatibility cache.
- `src-tauri/src/main.rs`: made `read_current_wallpaper_path` prefer the DB setting and only fall back to `current_wallpaper.txt`, migrating the fallback value into the DB when found.
- `src-tauri/src/main.rs`: updated normal playback, independent-display playback, manual set-wallpaper, CLI next, CLI like/dislike, widget like/dislike, display-mode reuse, and `read_current_wallpaper_path_cmd` to use the shared state helper.
- `docs/project-docs/REVIEW_FIX_PLAN.md`: marked R-10 as fixed.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- Final elevated `cargo check` PASS after normal sandbox hit the known `src-tauri/target` write permission denial.
- `cargo fmt` applied successfully.
- Final elevated `npm run build` PASS after normal sandbox hit the known Vite `spawn EPERM`.
- Targeted `git diff --check` PASS; only existing LF/CRLF notices were reported.

### Unresolved items
- CLI playback still duplicates some main-app playback logic and suppresses several errors; this remains covered by R-54/R-58/R-06.
- R-09 remains open: manual unpause while full-screen is still active can be auto-paused again by the focus monitor.
- Remaining P0/P1 findings still need follow-up batches, especially single-instance/CLI forwarding, user-visible error reporting, path validation, and CLI playback deduplication.

## 2026-06-18 - Sixth review-fix batch: CLI playback and diagnostics

### Change goal
Reduce CLI/main-app playback drift and make right-click CLI failures inspectable in release builds.

### Code scope
- `src-tauri/src/main.rs`: added `advance_wallpaper_with_db`, `advance_shared_wallpaper_with_db`, and `advance_independent_wallpapers_with_db` so CLI `next` and main-app playback share the same shared/span/independent selection, wallpaper application, play recording, and current-wallpaper persistence path.
- `src-tauri/src/main.rs`: changed CLI action handling to return `Result`, log action failures to `APPDATA/com.purewall.app/purewall-cli.log`, and stop silently swallowing next/like/dislike/pause errors.
- `src-tauri/src/main.rs`: changed `set_display_mode` Same/Span handling so missing current-wallpaper state falls back to selecting and applying the next wallpaper instead of silently skipping the immediate apply.
- `src-tauri/src/wallpaper.rs`: removed the now-unused `set_wallpaper` wrapper after CLI playback moved to `set_wallpaper_all`.
- `docs/project-docs/REVIEW_FIX_PLAN.md`: marked R-14 and R-54 as fixed; marked R-06 and R-58 as partially fixed.
- `docs/project-docs/AI_DIARY.md`: added #verify-004 for usage-limit-blocked elevated verification.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- `cargo fmt` applied successfully.
- Targeted `git diff --check` PASS; only existing LF/CRLF notices were reported.

### Verification limitations
- Final `cargo check` could not be completed in the normal sandbox because `src-tauri/target` returned the known access-denied write error. The usual elevated rerun was blocked by Codex usage-limit review in this turn.
- Final `npm run build` could not be completed because Vite hit the known sandbox `spawn EPERM`; the usual elevated rerun was also unavailable in this turn.

### Unresolved items
- R-06 remains partial: many non-CLI backend emits and frontend catches still need user-visible error handling.
- R-58 remains partial: CLI errors are now logged, but there is no in-app diagnostics viewer or notification.
- R-09 remains open: manual unpause while full-screen is still active can be auto-paused again by the focus monitor.
- Remaining P0/P1 findings still need follow-up batches, especially single-instance/CLI forwarding, user-visible error reporting, path validation, and CLI playback deduplication edges.

## 2026-06-18 - Seventh review-fix batch: focus auto-pause manual override

### Change goal
Fix the focus-mode edge case where a user manually resumed rotation during a detected full-screen session and the monitor immediately auto-paused again.

### Code scope
- `src-tauri/src/focus.rs`: added an internal `auto_pause_suppressed` flag plus focused getters/setters.
- `src-tauri/src/main.rs`: when manual pause is turned off during an active focus auto-pause/full-screen session, PureWall clears `auto_paused` and suppresses further auto-pauses until that full-screen session exits.
- `src-tauri/src/main.rs`: focus monitor now respects the suppression flag and clears it when full-screen detection turns false or focus mode is disabled.
- `docs/project-docs/REVIEW_FIX_PLAN.md`: marked R-09 as fixed.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- Final elevated `cargo check` PASS after the normal sandbox hit the known `src-tauri/target` write permission denial.
- `cargo fmt` applied successfully.
- Final elevated `npm run build` PASS after normal sandbox hit the known Vite `spawn EPERM`.
- Final `cargo check` PASS after formatting.
- Targeted `git diff --check` PASS; only existing LF/CRLF notices were reported.

### Unresolved items
- R-06 remains partial: many non-CLI backend emits and frontend catches still need user-visible error handling.
- R-58 remains partial: CLI errors are now logged, but there is no in-app diagnostics viewer or notification.
- Remaining P0/P1 findings still need follow-up batches, especially single-instance/CLI forwarding, user-visible error reporting, path validation, and front-end operation failure feedback.

## 2026-06-18 - Eighth review-fix batch: DB-backed play count refresh

### Change goal
Fix front-end play-count drift by making playback UI state refresh from SQLite after the backend records a play.

### Code scope
- `src-tauri/src/db.rs`: added `get_wallpaper_by_path` with tag attachment so callers can fetch a complete `WallpaperEntry` after a DB-side update.
- `src-tauri/src/main.rs`: exposed `get_wallpaper_by_path` as a Tauri command and registered it in the invoke handler.
- `src/stores/wallpapers.ts`: replaced local `play_count + 1` / synthetic `last_played` updates in manual next, manual set, and `auto-rotated` handling with a DB-backed `refreshWallpaper`.
- `docs/project-docs/REVIEW_FIX_PLAN.md`: marked R-07 as fixed.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- `cargo fmt` applied successfully.
- Final elevated `cargo check` PASS after the normal sandbox hit the known `src-tauri/target` write permission denial.

### Unresolved items
- R-06 remains partial: many non-CLI backend emits and frontend catches still need user-visible error handling.
- R-58 remains partial: CLI errors are now logged, but there is no in-app diagnostics viewer or notification.
- Remaining P0/P1 findings still need follow-up batches, especially single-instance/CLI forwarding, path validation, watcher lifecycle, and front-end operation failure feedback.

## 2026-06-18 - Ninth review-fix batch: hide missing wallpaper files from gallery

### Change goal
Fix gallery drift after files are deleted or moved outside PureWall by preventing missing filesystem paths from being returned to the UI lists.

### Code scope
- `src-tauri/src/db.rs`: filters `get_wallpapers_filtered` and tag-filtered results through `Path::exists()` after SQLite/tag hydration, matching the existing playback candidate behavior.
- `docs/project-docs/REVIEW_FIX_PLAN.md`: marked R-08 as fixed.
- No database rows are automatically deleted in this pass; missing records remain recoverable for a future repair/relink workflow.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- `cargo fmt` applied successfully.
- Final elevated `cargo check` PASS after the normal sandbox hit the known `src-tauri/target` write permission denial.

### Unresolved items
- R-06 remains partial: many non-CLI backend emits and frontend catches still need user-visible error handling.
- R-58 remains partial: CLI errors are now logged, but there is no in-app diagnostics viewer or notification.
- Remaining P0/P1 findings still need follow-up batches, especially single-instance/CLI forwarding, path validation, watcher lifecycle, and front-end operation failure feedback.

## 2026-06-18 - Tenth review-fix batch: owned folder watcher lifecycle

### Change goal
Fix the folder watcher leak and make the active watcher lifecycle explicit when changing or importing wallpaper folders.

### Code scope
- `src-tauri/src/scanner.rs`: replaced `std::mem::forget(watcher)` with an owned `FolderWatcher` handle that stores the notify watcher, event sender, and event thread.
- `src-tauri/src/scanner.rs`: added `Drop` handling for `FolderWatcher` to send a stop message and join the event thread.
- `src-tauri/src/main.rs`: added `folder_watcher` to `AppState` and stores the current watcher there.
- `src-tauri/src/main.rs`: `set_wallpaper_folder` and `import_wallpaper_folder` now replace the previous watcher with the newly selected folder watcher.
- `docs/project-docs/REVIEW_FIX_PLAN.md`: marked R-03 as fixed.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- `cargo fmt` applied successfully.
- Final elevated `cargo check` PASS after the normal sandbox hit the known `src-tauri/target` write permission denial.

### Unresolved items
- R-05 remains open: rotation/focus background threads still do not have a full app shutdown/join contract.
- R-06 remains partial: many non-CLI backend emits and frontend catches still need user-visible error handling.
- R-58 remains partial: CLI errors are now logged, but there is no in-app diagnostics viewer or notification.
- Remaining P0/P1 findings still need follow-up batches, especially single-instance/CLI forwarding, path validation, and front-end operation failure feedback.

## 2026-06-18 - Eleventh review-fix batch: P0 error visibility and single instance

### Change goal
Finish the remaining P0 review items by preventing CLI/main-app concurrent action drift and making key operation failures visible to users.

### Code scope
- `src-tauri/Cargo.toml` / `src-tauri/Cargo.lock`: added `tauri-plugin-single-instance`.
- `src-tauri/src/main.rs`: removed the early no-Tauri `--action` DB path and added single-instance forwarding for secondary `--action` launches.
- `src-tauri/src/main.rs`: added app-state-backed CLI action execution so forwarded next/like/dislike/pause actions update the running instance and emit UI events.
- `src-tauri/src/main.rs`: added `operation-failed` payload emission when forwarded CLI actions fail, while preserving `purewall-cli.log` diagnostics.
- `src/composables/useNotifications.ts`: added a small global notification store.
- `src/components/NotificationCenter.vue`, `src/components/AppShell.vue`, `src/styles.css`: added an accessible `aria-live` notification surface.
- `src/stores/wallpapers.ts`: routed key user-facing failures through visible notifications while keeping background preview/metadata failures console-only.
- `src/components/TitleBar.vue`, `src/components/EmptyState.vue`: caught import failures after the store reports the visible notification to avoid false success UI.
- `docs/project-docs/REVIEW_FIX_PLAN.md`: marked R-02, R-06, and R-26 as fixed.
- `docs/project-docs/AI_DIARY.md`: added #dependency-004 for Cargo registry TLS credential failures in the normal sandbox.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- `cargo fmt` applied successfully.
- Elevated `cargo check` PASS after normal sandbox hit the known `src-tauri/target` write permission denial.

### Unresolved items
- Remaining non-P0 issues still need follow-up batches: path validation, command error typing, ADR cleanup, cache eviction, accessibility navigation, and shutdown behavior.

## 2026-06-18 - Twelfth review-fix batch: Tauri path command validation

### Change goal
Fix R-50 by adding a narrow validation layer before Tauri commands use user-supplied filesystem paths.

### Code scope
- `src-tauri/src/main.rs`: added shared validators for existing folders, supported image files, and DB-registered wallpaper files.
- `src-tauri/src/main.rs`: validates folder import/setup paths before scanning or watching.
- `src-tauri/src/main.rs`: validates imported files as existing supported images before inserting them.
- `src-tauri/src/main.rs`: requires registered, still-existing image paths for set-current-wallpaper, rating, tag assignment, blacklist, thumbnails, preview, image metadata, and delete commands.
- `src-tauri/src/main.rs`: changed single and batch delete order so files are moved to the recycle bin only after DB registration validation, and DB rows are removed only after the file operation succeeds.
- `src-tauri/src/scanner.rs`: made folder scanning/watching require an actual directory and exposed the supported-image extension check for command-level validation.
- `docs/project-docs/REVIEW_FIX_PLAN.md`: marked R-50 as fixed.
- No registry command was executed and no user file operation was run.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- `cargo fmt` applied successfully.

### Verification limitations
- Final ordinary `cargo check` could not complete because the Windows sandbox denied writes under `src-tauri/target` (`libpurewall-*.rmeta: 拒绝访问。 (os error 5)`). Per this sub-agent task constraint, no elevated rerun or alternate target-dir workaround was attempted.

### Unresolved items
- R-53 remains open: command errors are still flattened to `String`.
- Broader scanner scaling/path timeout risks remain covered by R-11/R-12.

## 2026-06-18 - Thirteenth review-fix batch: single-instance review hardening

### Change goal
Address multi-agent review findings found after the P0 single-instance/error-visibility pass.

### Code scope
- `src-tauri/src/main.rs`: changed forwarded CLI execution to use `try_state::<AppState>()`, so a secondary `--action` arriving while the first process is still starting logs a clear startup message instead of panicking.
- `src-tauri/tauri.conf.json`: made the main window initially hidden; normal startup now calls `show_main_window()` after setup, while first-process `--action` runs stay headless before exit.
- `src/stores/wallpapers.ts`: made Tauri event listener setup idempotent, retained `UnlistenFn` callbacks, and exposed a teardown helper for future route lifecycle cleanup.
- `src/components/NotificationCenter.vue`: removed the outer live-region announcement layer; individual notifications keep `role="alert"` to avoid likely duplicate screen-reader announcements.
- `docs/project-docs/REVIEW_FIX_PLAN.md`: updated R-02 and R-26 evidence with the review hardening details.
- `docs/project-docs/AI_DIARY.md`: added #single-instance-001 for the startup-state race in single-instance callbacks.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight ordinary `cargo check` hit the known Windows sandbox `src-tauri/target` access-denied write error; elevated `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- `cargo fmt` PASS.
- Final `npx vue-tsc --noEmit` PASS.
- Final ordinary `cargo check` hit the known Windows sandbox `src-tauri/target` access-denied write error; elevated `cargo check` PASS.
- Final ordinary `npm run build` hit the known Vite `spawn EPERM`; elevated `npm run build` PASS.
- `git diff --check` PASS; only LF/CRLF warnings were reported.

### Unresolved items
- Runtime single-instance smoke was not executed in a live Tauri app this round.
- Remaining P1 items include R-05, R-13 schema versioning, R-17/R-18/R-20 accessibility navigation, R-38 cache eviction, R-53 typed command errors, and R-56 ADR cleanup.

## 2026-06-18 - Fourteenth review-fix batch: accessibility navigation semantics

### Change goal
Fix the P1 accessibility navigation findings around wallpaper-grid keyboard focus, heading hierarchy, and sidebar active-state semantics.

### Code scope
- `src/components/WallpaperGrid.vue`: added roving tabindex state for visible wallpaper cards and keyboard movement with Arrow keys, Home, and End.
- `src/components/WallpaperCard.vue`: accepts `cardTabIndex`, emits focus/move events to the grid, and supports Space as a keyboard activation key alongside Enter.
- `src/components/AppShell.vue`: added a hidden app-level h1 and labelled the main workspace.
- `src/components/Gallery.vue`: changed the collection title from styled text to an h2.
- `src/components/WallpaperGrid.vue`: changed curated collection row labels from styled text to h3 headings.
- `src/components/SidebarItem.vue` / `src/components/Sidebar.vue`: split active-state ARIA semantics so filters use `aria-pressed` and system navigation items use `aria-current="page"`.
- `src/styles.css`: retargeted the existing collection/row heading styles to h2/h3 selectors without changing the visual layout.
- `docs/project-docs/REVIEW_FIX_PLAN.md`: marked R-17, R-18, and R-20 as fixed.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- Final `cargo check` PASS.
- Ordinary `npm run build` hit the known Vite `spawn EPERM`; elevated `npm run build` PASS.
- `git diff --check` PASS; only LF/CRLF warnings were reported.

### Unresolved items
- Keyboard navigation was type/build verified but not visually smoke-tested in a live Tauri window this round.
- Remaining P1 items include R-05, R-13, R-31, R-38, R-53, R-56, and R-58.

## 2026-06-18 - Fifteenth review-fix batch: light-theme Living Stage tokens

### Change goal
Fix R-31 by making the Living Stage hero overlay and copy colors theme-aware instead of hardcoded for dark imagery.

### Code scope
- `src/styles.css`: added `--stage-*` design tokens for dark and light themes.
- `src/styles.css`: replaced hardcoded Living Stage veil, copy, eyebrow, supporting text, and meta-chip colors with those tokens.
- `docs/project-docs/REVIEW_FIX_PLAN.md`: marked R-31 as fixed.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- Final `cargo check` PASS.
- Ordinary `npm run build` hit the known Vite `spawn EPERM`; elevated `npm run build` PASS.

### Unresolved items
- The light-theme Living Stage change was build-verified but not visually smoke-tested in a live Tauri window this round.
- Remaining P1 items include R-05, R-13, R-38, R-53, R-56, and R-58; R-39 remains ADR/P1 behind ADR-005.

## 2026-06-18 - Sixteenth review-fix batch: frontend media cache bounds

### Change goal
Fix R-38 by preventing frontend base64 thumbnail and preview caches from growing without a bound during large-library browsing, imports, filtering, and sorting.

### Code scope
- `src/stores/wallpapers.ts`: added capped LRU-style media cache helpers for thumbnail and preview data URLs.
- `src/stores/wallpapers.ts`: capped thumbnail entries at 600 and preview entries at 24 while protecting the active/current wallpaper from eviction.
- `src/stores/wallpapers.ts`: changed preview cache hits to refresh recency and made delete/batch-delete release preview entries as well as thumbnail entries.
- `docs/project-docs/REVIEW_FIX_PLAN.md`: marked R-38 as fixed.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- Final `cargo check` PASS.
- Ordinary `npm run build` hit the known Vite `spawn EPERM`; elevated `npm run build` PASS.

### Unresolved items
- Backend thumbnail disk cache sizing and virtual scrolling remain separate lower-priority follow-ups under R-41/R-45-style performance work.
- Remaining P1 items include R-05, R-13, R-53, R-56, and R-58; R-39 remains ADR/P1 behind ADR-005.

## 2026-06-18 - Seventeenth review-fix batch: missing ADR cleanup

### Change goal
Fix R-56 by documenting the architectural decisions that had already been implemented or relied on but were missing formal ADR entries.

### Code scope
- `docs/project-docs/DECISIONS.md`: added ADR-012 for user-visible operation failures and the current `Result<_, String>` boundary tradeoff.
- `docs/project-docs/DECISIONS.md`: added ADR-013 for hardened CSP and disabled global Tauri exposure.
- `docs/project-docs/DECISIONS.md`: added ADR-014 for PureWall-owned HKCU registry boundaries and direct `reg.exe` usage.
- `docs/project-docs/DECISIONS.md`: added ADR-015 for incremental Rust module extraction while `main.rs` remains the command orchestration boundary.
- `docs/project-docs/DECISIONS.md`: added ADR-016 for hiding the release console window with `windows_subsystem = "windows"`.
- `docs/project-docs/REVIEW_FIX_PLAN.md`: marked R-56 as fixed.
- Documentation-only change; no registry command was executed and no system setting was changed.

### Verification evidence
- This was a documentation-only pass after the same-turn `cargo check`, `npx vue-tsc --noEmit`, and elevated `npm run build` all passed for the R-38 code change.
- `git diff --check` PASS after the documentation update; only LF/CRLF warnings were reported.

### Unresolved items
- Remaining P1 items include R-05, R-13, R-53, and R-58; R-39 remains ADR/P1 behind ADR-005.

## 2026-06-18 - Eighteenth review-fix batch: CLI diagnostics surface

### Change goal
Finish R-58 by giving release/right-click CLI failures a visible in-app diagnostic path, instead of leaving users with only a hidden log file.

### Code scope
- `src-tauri/src/main.rs`: added a shared CLI log path helper and a `read_cli_log` Tauri command that returns only the recent log tail.
- `src-tauri/src/main.rs`: registered `read_cli_log` in the command handler while preserving existing CLI failure logging and `operation-failed` forwarding.
- `src/stores/wallpapers.ts`: added `cliLog`, `isLoadingCliLog`, and `loadCliLog()` for the diagnostics panel.
- `src/components/InspectorPanel.vue`: loads CLI diagnostics when Advanced opens and adds a refreshable CLI diagnostics card/log viewer.
- `src/styles.css`: added bounded, wrapping log-viewer styling for the narrow inspector panel.
- `docs/project-docs/REVIEW_FIX_PLAN.md`: marked R-58 as fixed.
- No registry command was executed and no system setting was changed.

### Verification evidence
- `cargo fmt` PASS.
- `npx vue-tsc --noEmit` PASS.
- Ordinary `cargo check` hit the known Windows sandbox `src-tauri/target` access-denied write error; elevated `cargo check` PASS.
- Ordinary `npm run build` hit the known Vite `spawn EPERM`; elevated `npm run build` PASS.

### Unresolved items
- The Advanced diagnostics UI was compile/build verified but not visually smoke-tested in a live Tauri window this round.
- Remaining P1 items include R-05, R-13, and R-53; R-39 remains ADR/P1 behind ADR-005.

## 2026-06-19 - Nineteenth review-fix batch: Living Stage veil removal, SQLite schema versioning, and background shutdown

### Change goal
Remove the Living Stage image veil per user direction, then finish R-13 by adding SQLite schema versioning/table-level foreign keys and finish R-05 by giving background worker threads an app shutdown path.

### Code scope
- `src/components/CurrentWallpaperPanel.vue`: removed the `living-stage__veil` overlay element so the active wallpaper renders directly.
- `src/styles.css`: removed the unused `--stage-veil` tokens and `.living-stage__veil` style while keeping theme-aware stage copy/meta tokens.
- `src-tauri/src/db.rs`: added `SCHEMA_VERSION` and writes `PRAGMA user_version` after startup migration.
- `src-tauri/src/db.rs`: new database creation now defines foreign keys for `wallpaper_tags` and `play_events`.
- `src-tauri/src/db.rs`: existing databases rebuild `wallpaper_tags` and `play_events` when they lack foreign keys; tag links cascade, and deleted wallpaper history is preserved by setting `play_events.wallpaper_id` to null.
- `src-tauri/src/main.rs`: added `shutdown_requested` and `background_threads` to `AppState`.
- `src-tauri/src/main.rs`: rotation and focus monitor threads now check the shutdown flag and return join handles.
- `src-tauri/src/main.rs`: main-window close requests signal shutdown and join the rotation/focus worker threads.
- `docs/project-docs/ARCHITECTURE.md`: documented the schema version and FK deletion behavior.
- `docs/project-docs/REVIEW_FIX_PLAN.md`: marked R-05 and R-13 as fixed.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- `cargo fmt` PASS.
- Ordinary final `cargo check` hit the known Windows sandbox `src-tauri/target` access-denied write error; elevated `cargo check` PASS.
- Ordinary `npm run build` hit the known Vite `spawn EPERM`; the elevated rerun was blocked by the Codex usage limit, matching the existing #verify-004 limitation.

### Unresolved items
- Full production `npm run build` still needs to be rerun after the elevated verification allowance is available.
- Remaining P1 item is R-53; R-39 remains ADR/P1 behind ADR-005.

## 2026-06-19 - Twentieth review-fix batch: typed Tauri command errors

### Change goal
Finish R-53 by replacing stringly Tauri command failures with a stable serialized command error shape while preserving user-readable messages in the UI.

### Code scope
- `src-tauri/src/main.rs`: added `CommandError { code, message }` and `CommandResult<T>` for Tauri command handlers.
- `src-tauri/src/main.rs`: migrated all fallible `#[tauri::command]` handlers from `Result<_, String>` to `CommandResult<_>`, including gallery, playback, metadata, tags, batch actions, settings, diagnostics, context menu, widget, and autostart commands.
- `src-tauri/src/main.rs`: kept internal helper/CLI orchestration functions on `Result<_, String>` where they are not directly exposed to the WebView command boundary.
- `src/composables/useNotifications.ts`: updated error extraction so notifications read typed command errors through their `message` field while still supporting `Error` and string failures.
- `docs/project-docs/ARCHITECTURE.md`: documented the command error boundary contract and refreshed stale backend module notes.
- `docs/project-docs/REVIEW_FIX_PLAN.md`: marked R-53 as fixed.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- `cargo fmt` PASS.
- Ordinary final `cargo check` hit the known Windows sandbox `src-tauri/target` access-denied write error; elevated `cargo check` PASS.
- Command-boundary audit confirmed every fallible `#[tauri::command]` now returns `CommandResult<T>`; the only command bool returns are infallible status checks.
- Ordinary `npm run build` hit the known Vite `spawn EPERM`; elevated `npm run build` PASS.

### Unresolved items
- No P1 review-fix items remain in `REVIEW_FIX_PLAN.md`.
- R-39 remains ADR/P1 behind ADR-005; remaining open items are P2/P3 or ADR-gated follow-ups.

## 2026-06-19 - Twenty-first review-fix batch: notification-driven rotation timer

### Change goal
Finish R-39 by removing the rotation thread's 1-second polling loop after resolving ADR-005.

### Code scope
- `docs/project-docs/DECISIONS.md`: changed ADR-005 from proposed 1-second polling to an accepted `Condvar` + generation signal rotation timer decision.
- `src-tauri/src/main.rs`: added `RotationSignal` and `RotationWait` for notification-driven interval waits.
- `src-tauri/src/main.rs`: rewrote `start_rotation_timer` to wait on the rotation signal until the interval expires or pause/focus/interval/shutdown changes wake it.
- `src-tauri/src/main.rs`: wired rotation signal notifications into manual pause, pause sync, interval changes, focus auto-pause transitions, focus mode toggles, and shutdown.
- `src-tauri/src/tray.rs`: routed tray Pause through the shared manual pause helper so it persists DB state, updates focus pause semantics, and wakes the rotation timer; tray Quit now calls the background-thread shutdown helper before exit.
- `docs/project-docs/ARCHITECTURE.md`: updated the rotation flow and Phase 4 addendum to describe notification-driven waiting instead of 1-second polling.
- `docs/project-docs/REVIEW_FIX_PLAN.md`: marked R-39 as fixed.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- `cargo fmt` PASS.
- Ordinary final `cargo check` hit the known Windows sandbox `src-tauri/target` access-denied write error; elevated `cargo check` PASS.
- Source audit confirmed `start_rotation_timer` uses `wait_for_change_or_timeout`; the remaining `thread::sleep(2s)` is the focus monitor foreground-window probe, not the rotation timer.
- Ordinary `npm run build` hit the known Vite `spawn EPERM`; elevated `npm run build` PASS.

### Unresolved items
- No P1 or ADR/P1 review-fix items remain in `REVIEW_FIX_PLAN.md`.
- Remaining open review items are P2/P3 follow-ups.

## 2026-06-19 - Twenty-second review-fix batch: bounded wallpaper scans

### Change goal
Fix R-11 by adding hard safety bounds to folder scans and explicit multi-file imports so PureWall cannot accidentally traverse an enormous or deeply nested directory tree without limit.

### Code scope
- `src-tauri/src/scanner.rs`: added scan limits for maximum recursion depth, maximum image count, and maximum directory-entry count.
- `src-tauri/src/scanner.rs`: changed `scan_folder` to enforce the scan budget during recursive traversal and return a clear error when a limit is exceeded.
- `src-tauri/src/scanner.rs`: changed `scan_files` to return `Result<Vec<ImageInfo>>` and reject oversized explicit file selections.
- `src-tauri/src/main.rs`: updated `import_wallpaper_files` to propagate `scan_files` errors through the typed command error boundary.
- `docs/project-docs/ARCHITECTURE.md`: documented bounded scan behavior and user-visible failures.
- `docs/project-docs/REVIEW_FIX_PLAN.md`: marked R-11 as fixed.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- `cargo fmt` PASS.
- Ordinary final `cargo check` hit the known Windows sandbox `src-tauri/target` access-denied write error; elevated `cargo check` PASS.
- Ordinary `npm run build` hit the known Vite `spawn EPERM`; elevated `npm run build` PASS.

### Unresolved items
- R-12 remains open: network/UNC path blocking needs a separate path-classification or timeout strategy.
- Remaining open review items are P2/P3 follow-ups.

## 2026-06-19 - Twenty-third review-fix batch: local-only scan paths

### Change goal
Fix R-12 by preventing PureWall's scan/import/watch paths from entering network locations that can block on `read_dir`, metadata reads, or image dimension probing.

### Code scope
- `src-tauri/Cargo.toml`: enabled the `Win32_Storage_FileSystem` feature for the existing `windows` dependency so PureWall can query Windows drive types.
- `src-tauri/src/scanner.rs`: added `ensure_local_path()` to reject UNC paths, extended UNC paths, and Windows mapped remote drives before filesystem probing.
- `src-tauri/src/scanner.rs`: wired local-path validation into folder scans, explicit file imports, image metadata refreshes, and watcher startup.
- `src-tauri/src/scanner.rs`: recursive folder scans now use `DirEntry::file_type()` and skip symlink entries instead of following them into possible network/reparse paths.
- `src-tauri/src/main.rs`: command-level folder/file validation now calls `scanner::ensure_local_path()` before `is_dir()` / `is_file()`.
- `docs/project-docs/AI_DIARY.md`: appended `#windows-api-002` for the `GetDriveTypeW`/`DRIVE_REMOTE` binding mismatch.
- `docs/project-docs/ARCHITECTURE.md`: documented local-only wallpaper library paths and symlink skipping.
- `docs/project-docs/REVIEW_FIX_PLAN.md`: marked R-12 as fixed.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- `cargo fmt` PASS.
- First elevated `cargo check` found the missing `DRIVE_REMOTE` binding; after replacing it with a local documented value, ordinary `cargo check` again hit the known target access-denied write error and elevated `cargo check` PASS.
- Ordinary `npm run build` hit the known Vite `spawn EPERM`; elevated `npm run build` PASS.

### Unresolved items
- Remaining open review items are P2/P3 follow-ups.

## 2026-06-19 - Twenty-fourth review-fix batch: keyboard escape routes

### Change goal
Fix R-21 and R-22 by making popup controls release keyboard focus cleanly and adding a skip link to the main workspace.

### Code scope
- `src/components/CompactDropdown.vue`: added a shared close helper; Tab/Shift+Tab now closes the popup without preventing native focus movement, and listbox options are removed from the Tab order.
- `src/components/TagCombobox.vue`: Tab/Shift+Tab now closes the suggestion popup without trapping focus, and tag/create options are removed from the Tab order.
- `src/components/AppShell.vue`: added a skip-to-main link and made the main workspace a focusable jump target.
- `src/styles.css`: added focused-only skip link styling using existing theme tokens.
- `docs/project-docs/REVIEW_FIX_PLAN.md`: marked R-21 and R-22 as fixed.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- Final `cargo check` PASS.
- Ordinary `npm run build` hit the known Vite 8 Windows sandbox `spawn EPERM`; elevated `npm run build` PASS.

### Unresolved items
- Remaining P2 review items: R-29, R-40, R-41, R-44, R-45, R-52.
- Remaining P3 review items are unchanged.

## 2026-06-19 - Twenty-fifth review-fix batch: remaining P2 completion

### Change goal
Fix the remaining P2 review items in one integrated pass: undo flows, large-library rendering, playback candidate selection, persistent COM execution, thumbnail invalidation, and incremental backend module extraction.

### Code scope
- `src/composables/useNotifications.ts` and `src/components/NotificationCenter.vue`: notifications now support optional action buttons.
- `src/stores/wallpapers.ts`: hide/batch-hide now expose Undo; delete/batch-delete now use a 7-second pending-delete window before calling the backend recycle-bin commands.
- `src/components/WallpaperGrid.vue` and `src/styles.css`: gallery rendering is capped to an initial page of 120 wallpapers and expands on near-bottom scroll or the load-more control.
- `src-tauri/src/db.rs`: `get_next_wallpapers` now uses SQLite weighted random sampling and only probes a bounded candidate set for file existence instead of running `Path::exists()` across every eligible row.
- `src-tauri/src/thumbnails.rs`: thumbnail and preview cache keys now include source file size and modification time.
- `src-tauri/src/wallpaper.rs`: IDesktopWallpaper calls now route through a persistent STA worker queue instead of spawning a new COM apartment thread per operation.
- `src-tauri/src/diagnostics.rs`: extracted CLI diagnostics log writing and tail-reading command out of `main.rs`.
- `src-tauri/src/widget.rs`: extracted pre-created widget window commands out of `main.rs`.
- `src-tauri/Cargo.toml`: removed the no-longer-used direct `rand` dependency after moving playback randomization into SQLite sampling.
- `docs/project-docs/ARCHITECTURE.md`: documented pending delete Undo, incremental rendering, bounded playback existence checks, metadata-aware image cache keys, persistent STA worker, and the new diagnostics/widget modules.
- `docs/project-docs/REVIEW_FIX_PLAN.md`: marked R-29, R-40, R-41, R-44, R-45, and R-52 as fixed.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- `cargo fmt` PASS.
- `git diff --check` PASS; only existing LF/CRLF warnings were reported.
- Ordinary final `cargo check` reached the crate check phase and then hit the known Windows sandbox `src-tauri/target` access-denied write error; elevated rerun was blocked by the Codex usage limit, matching #verify-004.
- Ordinary final `npm run build` hit the known Vite 8 Windows sandbox `spawn EPERM`; elevated rerun was unavailable in this turn because the same usage-limit blocker rejected elevated verification.

### Unresolved items
- No P2 review items remain open in `REVIEW_FIX_PLAN.md`.
- Remaining open review items are P3 polish/maintenance follow-ups.
- Full elevated `cargo check` and elevated `npm run build` should be rerun when the usage allowance resets.

## 2026-06-19 - Twenty-sixth review-fix batch: P3 polish and thumbnail cleanup

### Change goal
Continue the remaining P3 review fixes by cleaning up low-risk UI polish issues and thumbnail maintenance debt in one pass.

### Code scope
- `src/components/AppShell.vue` and `src/styles.css`: added a keyed Quiet Canvas/Workbench transition with a reduced-motion override.
- `src/components/WallpaperCard.vue` and `src/styles.css`: changed the hover overlay from `v-if` to `v-show`, added an opacity optimization hint, and removed the card image hover scale that could crop media.
- `src/components/AppIcon.vue` and `src/components/InspectorPanel.vue`: added person/shield/comment icon mappings and used them for Author/Copyright/Comment metadata rows.
- `src/styles.css`: increased Living brand subtitle and statusbar text from sub-10px sizes to 10px with tighter letter spacing.
- `src-tauri/src/thumbnails.rs`: removed unused `ThumbnailCache::pre_generate`, replaced the custom base64 encoder with the standard `base64` crate, and centralized thumbnail/preview dimensions and JPEG quality constants.
- `src-tauri/src/db.rs`: merged yearly total/unique/liked COUNT queries into one aggregate query while preserving separate monthly and Top 5 result grains.
- `src/main.ts` and `src/fonts.css`: replaced broad `@fontsource-variable/geist`/`inter` imports with one local Geist Latin variable font-face so Vite no longer emits unused Inter/Cyrillic/Vietnamese font shards.
- `src-tauri/Cargo.toml`: added the direct `base64 = "0.22"` dependency; the version was already present in `src-tauri/Cargo.lock`.
- `docs/project-docs/REVIEW_FIX_PLAN.md`: marked R-15, R-28, R-30, R-32, R-33, R-34, R-42, R-47, and R-55 as fixed; narrowed R-57 to the remaining hardcoded parameters.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS before edits.
- Pre-flight and final `npx vue-tsc --noEmit` PASS.
- `cargo fmt` PASS.
- Ordinary final `cargo check` hit the known Windows sandbox `src-tauri/target` access-denied write error; elevated `cargo check` PASS.
- Ordinary final `npm run build` hit the known Vite `spawn EPERM`; elevated `npm run build` PASS.
- After the font import optimization, elevated `npm run build` PASS again and emitted only `geist-latin-wght-normal` as the font asset.
- After the yearly stats SQL merge, `cargo fmt` PASS and elevated `cargo check` PASS.

### Unresolved items
- Remaining open review items: R-27, R-43, R-46, and the residual scope of R-57.
- Visual hover/Quiet Canvas behavior should still get a manual runtime smoke in the Tauri window because this batch verified build/type correctness, not human-perceived animation quality.

## 2026-06-19 - Twenty-seventh review-fix batch: final P3 completion

### Change goal
Close the last open review-fix items in `REVIEW_FIX_PLAN.md`: drag/drop import, indexed wallpaper patch updates, Living Stage image rendering, and the remaining hardcoded-parameter cleanup.

### Code scope
- `src-tauri/src/main.rs`: added `import_dropped_paths`, reusing local-path validation and scanner metadata for dropped files/folders; mixed drops are deduplicated and capped by `scanner::MAX_SCAN_IMAGES` before DB writes.
- `src/stores/wallpapers.ts`: added `setWallpapers(...)` and a path-to-index map so `patchWallpaper` replaces one array slot instead of copying the full wallpaper array; added `importDroppedPaths(...)` for the new drag/drop command.
- `src/components/AppShell.vue`: registered Tauri WebView drag/drop events, added a drag-over import overlay, cleaned up the unlisten handler on unmount, and removed the old ambient preview CSS variable.
- `src/components/CurrentWallpaperPanel.vue` and `src/styles.css`: rendered the Living Stage preview as a real `<img>` with accessible alt text, removed the CSS background-image preview path, and kept layout in CSS via object-fit.
- `src-tauri/src/db.rs` and `src/stores/wallpapers.ts`: centralized the default tag color at the command/store boundary.
- `docs/project-docs/DECISIONS.md`: added accepted ADR-017 for indexed wallpaper patch updates.
- `docs/project-docs/ARCHITECTURE.md`: documented drop import, global drop import limits, indexed patching, and real-image stage rendering.
- `docs/project-docs/REVIEW_FIX_PLAN.md`: marked R-27, R-43, R-46, and R-57 as fixed and recorded that no open P0/P1/P2/P3 review items remain.
- No registry command was executed and no system setting was changed.

### Verification evidence
- `cargo fmt` PASS.
- `npx vue-tsc --noEmit` PASS.
- Final elevated `cargo check` PASS after the normal sandbox path had previously denied writes to `src-tauri/target`.
- Final elevated `npm run build` PASS.
- `git diff --check` PASS; only existing LF/CRLF warnings were reported.
- Multi-agent review was used. Its P0 stale `UnlistenFn` finding had already been fixed before final verification; its P2 global drop-limit and P3 ambient-background findings were fixed in this batch.

### Unresolved items
- No open items remain in `REVIEW_FIX_PLAN.md`.
- Manual Tauri runtime smoke is still useful for real drag/drop UX and Living Stage visual feel, but build/type/Rust verification is complete.

## 2026-06-19 - Twenty-eighth UI polish batch: single-layer floating widget

### Change goal
Fix the floating widget visual issue where the transparent Tauri window and the inner control bar created an obvious double-boundary frame.

### Code scope
- `src/views/WidgetView.vue`: replaced raw glyph/text-only controls with the shared `AppIcon` component, added disabled state binding while commands are busy, and kept all commands on the existing widget command path.
- `src/styles.css`: made the widget bar fill the whole widget viewport as the only visible surface, removed the extra inner card feel from secondary buttons, kept only the Next button as the primary pill, and ensured the widget route app background stays transparent.
- `src-tauri/tauri.conf.json`: reduced the pre-created widget window from `300x100` to `264x64` so the transparent window no longer leaves a large visible outer boundary around the controls.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight elevated `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- Final `npx vue-tsc --noEmit` PASS.
- Final elevated `cargo check` PASS.
- Final elevated `npm run build` PASS.

### Unresolved items
- Manual runtime smoke in the Tauri window is still useful to confirm the exact Windows transparency/compositing appearance on the desktop.

## 2026-06-19 - Twenty-ninth UI polish batch: widget theme synchronization

### Change goal
Make the floating widget follow the same dark/light/system theme as the main PureWall window instead of staying on a stale color mode.

### Code scope
- `src/composables/useTheme.ts`: added cross-WebView theme synchronization through `storage` events plus a `BroadcastChannel` named `purewall-theme`; external theme messages now always reapply the mode so `system` can re-resolve after OS theme changes.
- `docs/project-docs/ARCHITECTURE.md`: documented multi-window theme synchronization for the main window and pre-created widget WebView.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight elevated `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- Final `npx vue-tsc --noEmit` PASS via `npm run build`.
- Final elevated `cargo check` PASS.
- Final elevated `npm run build` PASS.
- `git diff --check` PASS; only existing LF/CRLF warnings were reported.

### Unresolved items
- A real Tauri runtime smoke should confirm the widget visually updates immediately when the main window switches Dark/Light/System and when Windows changes color scheme while PureWall is set to System.

## 2026-06-19 - Thirtieth UI polish batch: remove widget transparency halo

### Change goal
Remove the remaining visible transparent halo around the floating widget after the previous single-layer pass reduced but did not fully hide the outer edge.

### Code scope
- `src/styles.css`: changed `.widget-bar` from a semi-transparent blurred material with border/inset highlight to a solid theme-token surface with no border, shadow, or backdrop filter; kept the inner button layout unchanged.
- `src-tauri/tauri.conf.json`: reduced the pre-created widget window from `264x64` to `258x60` so the transparent WebView area hugs the visible control more tightly.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight elevated `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- Final `npx vue-tsc --noEmit` PASS.
- Final elevated `cargo check` PASS.
- Final elevated `npm run build` PASS.

### Unresolved items
- If Windows/WebView2 still shows a halo after restarting the Tauri dev app, the remaining artifact is likely transparent-window anti-alias compositing rather than CSS border/blur. The fallback would be a non-transparent rectangular widget window or a native shaped-window approach.

## 2026-06-19 - Thirty-first UI polish batch: widget radius alignment

### Change goal
Reduce the remaining visible mismatch between the floating widget's transparent WebView edge and the dark visible control surface by aligning their rounded geometry.

### Code scope
- `src/styles.css`: added a widget-scoped `--widget-radius` token, reduced the widget surface radius to `16px`, and applied the same radius plus clipping to `.widget-shell` and `.widget-bar` so the transparent window edge and visible panel curve together.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight elevated `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- Final `npx vue-tsc --noEmit` PASS.
- Final elevated `cargo check` PASS.
- Ordinary `npm run build` hit the known Vite 8 Windows sandbox `spawn EPERM`; elevated `npm run build` PASS.
- `git diff --check` PASS; only existing LF/CRLF warnings were reported.

### Unresolved items
- Manual Tauri runtime smoke is still needed to judge the exact Windows/WebView2 compositing edge on the desktop. If a halo remains after a full app restart, the next fallback is to make the widget rectangular/non-transparent or move to a native shaped-window approach.

## 2026-06-30 - Project icon asset refresh

### Change goal
Create a dedicated PureWall project icon that matches the app's quiet wallpaper-curation identity and can serve as the Tauri bundle icon source.

### Code scope
- `design/purewall-icon.svg`: added an editable vector source using a gallery-window mark, clean wall-light accent, and restrained teal/ink palette.
- `scripts/generate-icons.ps1`: updated the System.Drawing icon generator so PNG/ICO outputs use the same icon geometry as the SVG source.
- `src-tauri/icons/16x16.png`, `24x24.png`, `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.png`, and `icon.ico`: regenerated icon assets from the updated generator.
- `src-tauri/tauri.conf.json` was not changed; the bundle still references `icons/icon.png` to avoid the known hand-written ICO Windows RC issue.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS before edits.
- Pre-flight `npx vue-tsc --noEmit` PASS before edits.
- Icon PNG dimensions verified: 16x16, 24x24, 32x32, 128x128, and 256x256 outputs are readable via System.Drawing.
- Post-change `npx vue-tsc --noEmit` PASS.
- Ordinary post-change `cargo check` hit the known Windows sandbox `src-tauri/target` access-denied write error; elevated `cargo check` PASS.
- Elevated `npm run build` PASS.

### Unresolved items
- Manual visual judgment in Windows Explorer/Start menu is still useful because OS icon caching can affect how quickly the refreshed icon appears.
- If the desktop context menu still shows a stale icon, re-register the PureWall-owned menu entry so it exports the refreshed app icon to the stable APPDATA menu icon path documented in AI_DIARY #registry-006.

## 2026-06-30 - Project icon simplification and motion preview

### Change goal
Simplify the new PureWall project icon after design review feedback: reduce illustration detail, improve small-size recognition, and prepare a motion-friendly structure for future logo animation.

### Code scope
- `design/purewall-icon.svg`: replaced the detailed landscape/gallery illustration with a sparse mark built from a base plate, clean wall plane, dark wallpaper canvas, and teal reveal line.
- `scripts/generate-icons.ps1`: updated the System.Drawing generator to match the simplified SVG geometry and hide tiny decorative detail from small raster sizes.
- `src-tauri/icons/16x16.png`, `24x24.png`, `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.png`, and `icon.ico`: regenerated from the simplified generator.
- `design/purewall-icon-motion-spec.md`: added a lightweight motion brief with personality, part inventory, and reveal timeline.
- `design/purewall-icon-motion.html`: added a standalone motion preview with replay, slow/speed controls, reduced-motion handling, `#logo-root`, `?static=1`, `?t=<ms>`, and `window.__p2mReady` hooks.
- `src-tauri/tauri.conf.json` was not changed; the bundle still references `icons/icon.png`.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- Icon PNG dimensions verified: 16x16, 24x24, 32x32, 128x128, and 256x256 outputs are readable via System.Drawing.
- Motion preview HTML checked for `#logo-root`, replay controls, reduced-motion CSS, `?static=1`, and `window.__p2mReady` hooks.
- Post-change `npx vue-tsc --noEmit` PASS.
- Ordinary post-change `cargo check` hit the known Windows sandbox `src-tauri/target` access-denied write error; elevated `cargo check` PASS.
- Elevated `npm run build` PASS.

### Unresolved items
- The motion preview is a design artifact, not a full pixel2motion QA package; deterministic frame capture and same-pipeline final-frame diff can be added if this becomes a production splash animation.
- Manual visual judgment in Windows Explorer, taskbar, and Start menu remains useful because OS icon caching can delay visible refresh.

## 2026-06-30 - Project icon wallpaper-association pass

### Change goal
Revise the simplified icon after feedback that the abstract wall/canvas mark did not clearly communicate a wallpaper management app.

### Code scope
- `design/purewall-icon.svg`: replaced the abstract wall-plane mark with a stacked wallpaper-card composition: two library cards behind a current scenic wallpaper card, plus a teal next chevron for wallpaper switching.
- `scripts/generate-icons.ps1`: updated the System.Drawing generator to match the new wallpaper-card stack and next-action mark.
- `src-tauri/icons/16x16.png`, `24x24.png`, `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.png`, and `icon.ico`: regenerated from the updated generator.
- `design/purewall-icon-motion-spec.md`: updated the motion brief to match the wallpaper stack, current wallpaper, and next chevron parts.
- `design/purewall-icon-motion.html`: updated the standalone motion preview SVG template and animations to target `#back-card`, `#mid-card`, `#wallpaper-card`, and `#next-chevron`.
- `src-tauri/tauri.conf.json` was not changed; the bundle still references `icons/icon.png`.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- Icon PNG dimensions verified: 16x16, 24x24, 32x32, 128x128, and 256x256 outputs are readable via System.Drawing.
- Motion preview HTML checked for semantic wallpaper-card part ids, `#logo-root`, `?static=1`, and `window.__p2mReady` hooks.
- Post-change `npx vue-tsc --noEmit` PASS.
- Ordinary post-change `cargo check` hit the known Windows sandbox `src-tauri/target` access-denied write error; elevated `cargo check` PASS.
- Elevated `npm run build` PASS.

### Unresolved items
- Manual visual judgment in Windows Explorer, taskbar, and Start menu remains useful because OS icon caching can delay visible refresh and tiny-size recognizability is ultimately visual.
- The motion preview remains a design artifact, not a full pixel2motion QA package.

## 2026-06-30 - Project icon light-palette pass

### Change goal
Lighten the wallpaper-stack icon palette after feedback that the previous version felt tuned for dark mode.

### Code scope
- `design/purewall-icon.svg`: shifted the wallpaper card from dark blue/ink toward a light day palette with pale cyan sky, lighter card surfaces, softer slate outlines, and a gentler teal action mark.
- `scripts/generate-icons.ps1`: updated the System.Drawing color constants and stroke weights to match the lighter SVG source.
- `src-tauri/icons/16x16.png`, `24x24.png`, `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.png`, and `icon.ico`: regenerated from the light-palette generator.
- `design/purewall-icon-motion.html`: updated the embedded SVG colors to match the light-palette icon.
- `design/purewall-icon-motion-spec.md`: noted that the wallpaper-stack mark now uses a lighter day palette for light Windows surfaces.
- `src-tauri/tauri.conf.json` was not changed; the bundle still references `icons/icon.png`.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- Icon PNG dimensions verified: 16x16, 24x24, 32x32, 128x128, and 256x256 outputs are readable via System.Drawing.
- SVG and motion preview checked for the new light palette and no stale dark `#10202b` / `#20384a` wallpaper colors.
- Post-change `npx vue-tsc --noEmit` PASS.
- Ordinary post-change `cargo check` hit the known Windows sandbox `src-tauri/target` access-denied write error; elevated `cargo check` PASS.
- Elevated `npm run build` PASS.

### Unresolved items
- Manual visual judgment in Windows Explorer, taskbar, Start menu, and both PureWall light/dark app themes remains useful because icon color balance is ultimately visual and OS icon caching can delay refresh.

## 2026-06-30 - Project icon blue-palette pass

### Change goal
Try a cooler blue palette for the wallpaper-stack icon after considering whether blue would better match Windows wallpaper and desktop-system expectations.

### Code scope
- `design/purewall-icon.svg`: shifted the wallpaper stack from light teal toward a light blue palette with sky-blue wallpaper gradients, ice-blue card surfaces, slate-blue outlines, and a clearer blue next chevron.
- `scripts/generate-icons.ps1`: updated the System.Drawing color constants to match the blue SVG source.
- `src-tauri/icons/16x16.png`, `24x24.png`, `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.png`, and `icon.ico`: regenerated from the blue-palette generator.
- `design/purewall-icon-motion.html`: updated the embedded SVG colors to match the blue-palette icon.
- `design/purewall-icon-motion-spec.md`: updated the palette note to describe the cooler light-blue day palette.
- `src-tauri/tauri.conf.json` was not changed; the bundle still references `icons/icon.png`.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- Icon PNG dimensions verified: 16x16, 24x24, 32x32, 128x128, and 256x256 outputs are readable via System.Drawing.
- SVG and motion preview checked for new blue palette values and no stale teal primary values in the searched source files.
- Post-change `npx vue-tsc --noEmit` PASS.
- Ordinary post-change `cargo check` hit the known Windows sandbox `src-tauri/target` access-denied write error; elevated `cargo check` PASS.
- Elevated `npm run build` PASS.

### Unresolved items
- Manual visual judgment in Windows Explorer, taskbar, Start menu, and both PureWall light/dark app themes remains useful because icon color balance is ultimately visual and OS icon caching can delay refresh.

## 2026-06-30 - Project icon deeper blue base pass

### Change goal
Tune the blue wallpaper-stack icon after feedback that the blue could be slightly deeper, and decide the largest background frame should be faintly blue-tinted rather than pure white.

### Code scope
- `design/purewall-icon.svg`: deepened the photo and next-chevron blues, changed the largest base plate to a subtle blue-white gradient, and kept the card stack readable on light Windows surfaces.
- `scripts/generate-icons.ps1`: updated the System.Drawing palette constants to match the deeper blue SVG source and regenerated the raster outputs.
- `src-tauri/icons/16x16.png`, `24x24.png`, `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.png`, and `icon.ico`: regenerated from the updated generator.
- `design/purewall-icon-motion.html`: updated the embedded motion-preview SVG palette to match the static icon.
- `design/purewall-icon-motion-spec.md`: documented the slightly deeper light-blue day palette and faint blue-white base plate.
- `src-tauri/tauri.conf.json` was not changed; the bundle still references `icons/icon.png`.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- Icon PNG dimensions verified: 16x16, 24x24, 32x32, 128x128, and 256x256 outputs are readable via System.Drawing.
- SVG and motion preview checked for the deeper blue palette values and no stale searched `#77aee8` / `#66b7ff` / `#2f80ed` values.
- Post-change `npx vue-tsc --noEmit` PASS.
- Ordinary post-change `cargo check` hit the known Windows sandbox `src-tauri/target` access-denied write error; elevated `cargo check` PASS.
- Elevated `npm run build` PASS.
- `git diff --check` PASS for the touched icon/design files.

### Unresolved items
- Manual visual judgment in Windows Explorer, taskbar, Start menu, and both PureWall light/dark app themes remains useful because icon color balance is ultimately visual and OS icon caching can delay refresh.

## 2026-06-30 - Project icon fresh sky-blue palette pass

### Change goal
Adjust the icon palette after feedback that the previous deeper blue read too purple, shifting it toward a fresher sky-blue / clean desktop-blue direction.

### Code scope
- `design/purewall-icon.svg`: replaced the purple-leaning blue stops with sky-blue and cyan-blue values, including a fresher photo gradient, blue action chevron, blue-white base plate, and less-purple blue-gray outline.
- `scripts/generate-icons.ps1`: updated the System.Drawing color constants to match the fresh sky-blue SVG source and regenerated raster outputs.
- `src-tauri/icons/16x16.png`, `24x24.png`, `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.png`, and `icon.ico`: regenerated from the updated generator.
- `design/purewall-icon-motion.html`: updated the embedded motion-preview SVG palette to match the static icon.
- `design/purewall-icon-motion-spec.md`: updated the palette note to describe the fresh sky-blue day palette.
- `src-tauri/tauri.conf.json` was not changed; the bundle still references `icons/icon.png`.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- Icon PNG dimensions verified: 16x16, 24x24, 32x32, 128x128, and 256x256 outputs are readable via System.Drawing.
- SVG and motion preview checked for fresh sky-blue palette values and no stale searched `#4f93dc` / `#2563d8` / `#385a76` values.
- Post-change `npx vue-tsc --noEmit` PASS.
- Ordinary post-change `cargo check` hit the known Windows sandbox `src-tauri/target` access-denied write error; elevated `cargo check` PASS.
- Elevated `npm run build` PASS.
- `git diff --check` PASS for the touched icon/design files.

### Unresolved items
- Manual visual judgment in Windows Explorer, taskbar, Start menu, and both PureWall light/dark app themes remains useful because icon color balance is ultimately visual and OS icon caching can delay refresh.

## 2026-06-30 - Project icon clean Windows-sky blue pass

### Change goal
Revise the icon palette after feedback that the fresh sky-blue pass still felt too cyan, making it read closer to water or a weather app. Shift the palette toward a cleaner Windows-sky blue while avoiding both purple-blue and cyan-water associations.

### Code scope
- `design/purewall-icon.svg`: replaced the cyan-leaning sky palette with a more neutral Windows-sky blue set, including a cleaner photo gradient, Windows-like action chevron, blue-white base plate, and neutral blue-gray outline.
- `scripts/generate-icons.ps1`: updated the System.Drawing color constants to match the clean Windows-sky SVG source and regenerated raster outputs.
- `src-tauri/icons/16x16.png`, `24x24.png`, `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.png`, and `icon.ico`: regenerated from the updated generator.
- `design/purewall-icon-motion.html`: updated the embedded motion-preview SVG palette to match the static icon.
- `design/purewall-icon-motion-spec.md`: updated the palette note to describe the clean Windows-sky blue direction and the intent to avoid purple-blue and cyan-water reads.
- `src-tauri/tauri.conf.json` was not changed; the bundle still references `icons/icon.png`.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- Icon PNG dimensions verified: 16x16, 24x24, 32x32, 128x128, and 256x256 outputs are readable via System.Drawing.
- SVG and motion preview checked for the clean Windows-sky palette values and no stale searched `#2ca7e8` / `#0ea5e9` / `#0284c7` / `#2d6f8f` values.
- Post-change `npx vue-tsc --noEmit` PASS.
- Ordinary post-change `cargo check` hit the known Windows sandbox `src-tauri/target` access-denied write error; elevated `cargo check` PASS.
- Elevated `npm run build` PASS.
- `git diff --check` PASS for the touched icon/design files.

### Unresolved items
- Manual visual judgment in Windows Explorer, taskbar, Start menu, and both PureWall light/dark app themes remains useful because icon color balance is ultimately visual and OS icon caching can delay refresh.

## 2026-07-01 - Project icon softer sky and motion GIF pass

### Change goal
Adjust the wallpaper-card icon after feedback that the sky above the mountain still felt off and might be affected by the borders. Make the sky softer, reduce border influence, and update the Pixel2Motion-style animation deliverables alongside the static icon.

### Code scope
- `design/purewall-icon.svg`: softened the mountain sky gradient, lightened and reduced the outer base border, softened the stacked-card strokes, and changed the front wallpaper outline/ridge to a thinner neutral blue-gray.
- `scripts/generate-icons.ps1`: updated the System.Drawing palette constants and stroke widths to match the softened SVG source.
- `src-tauri/icons/16x16.png`, `24x24.png`, `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.png`, and `icon.ico`: regenerated from the updated icon generator.
- `design/purewall-icon-motion.html`: synchronized the embedded SVG palette and retained Pixel2Motion-style hooks and controls (`#logo-root`, `?static=1`, `?t=<ms>`, replay, speed, reduced motion, and `window.__p2mReady`).
- `design/purewall-icon-motion-spec.md`: updated the palette note to describe the softer wallpaper-sky direction and reduced border influence.
- `scripts/generate-icon-motion-gif.py`: added a Pillow-based motion export that mirrors the semantic part choreography used by the HTML preview.
- `design/purewall-icon-motion.gif`: added a 256x256 animated GIF export of the reveal.
- `design/purewall-icon-motion-strip.png`: added a six-beat motion strip for quick visual review.
- `src-tauri/tauri.conf.json` was not changed; the bundle still references `icons/icon.png`.
- No registry command was executed and no system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- Icon PNG dimensions verified: 16x16, 24x24, 32x32, 128x128, and 256x256 outputs are readable via Pillow/System.Drawing checks.
- Motion GIF verified readable at 256x256 with 22 frames; motion strip verified readable at 1586x256.
- SVG and motion preview checked for the softened sky values and no stale searched `#3b9bea` / `#8fc8f6` / `#315f86` values.
- Post-change `npx vue-tsc --noEmit` PASS.
- Ordinary post-change `cargo check` hit the known Windows sandbox `src-tauri/target` access-denied write error; elevated `cargo check` PASS.
- Elevated `npm run build` PASS.

### Unresolved items
- Manual visual judgment in Windows Explorer, taskbar, Start menu, and both PureWall light/dark app themes remains useful because icon color balance is ultimately visual and OS icon caching can delay refresh.
- The GIF is a practical preview export; the HTML remains the richer interactive motion artifact with Pixel2Motion-style QA hooks.

## 2026-07-01 - Project icon motion anti-banding export pass

### Change goal
Fix the visible color banding in the exported icon motion GIF and provide a higher-quality motion preview format for the soft sky gradient.

### Code scope
- `scripts/generate-icon-motion-gif.py`: changed the GIF export pipeline to build one shared palette from all animation frames, quantize every frame against that palette with Floyd-Steinberg dithering, and flatten GIF frames onto a light RGB matte for Pillow compatibility.
- `scripts/generate-icon-motion-gif.py`: added APNG and lossless WebP exports from the original RGBA frames so the soft sky gradient is not constrained by GIF's 256-color palette.
- `design/purewall-icon-motion.gif`: regenerated as the compatibility GIF with shared palette and dithering.
- `design/purewall-icon-motion.apng`: added a full-color APNG motion preview.
- `design/purewall-icon-motion.webp`: added a full-color lossless WebP motion preview.
- `design/purewall-icon-motion-strip.png`: regenerated from the same frame pipeline.
- `design/purewall-icon-motion-spec.md`: documented that GIF is a compatibility preview and APNG/WebP are the preferred high-quality previews for smooth sky gradients.
- No static icon colors, Tauri config, registry entries, or system settings were changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- Motion exports verified readable with Pillow: GIF 256x256 with 22 frames, APNG 256x256 with 23 frames, WebP 256x256 with 22 frames, motion strip 1586x256.
- Export diagnostic from `scripts/generate-icon-motion-gif.py`: sky crop unique colors are 453 in the source RGBA/APNG frame and 74 in GIF, confirming APNG/WebP are the quality reference while GIF remains palette-limited.
- Motion HTML checked for Pixel2Motion-style hooks and controls: `#logo-root`, `?static=1`, `?t=<ms>`, replay, speed, reduced motion, and `window.__p2mReady`.
- Post-change `npx vue-tsc --noEmit` PASS.
- Post-change `cargo check` PASS.
- Elevated `npm run build` PASS.

### Unresolved items
- GIF may still show mild dithering or residual banding on some viewers because of the format limit; use `design/purewall-icon-motion.apng`, `design/purewall-icon-motion.webp`, or `design/purewall-icon-motion.html` for final visual judgment.

## 2026-07-01 - Project icon motion chevron parity pass

### Change goal
Fix the animated icon chevron so it matches the static SVG chevron without the extra outline-like stroke seen in the motion export.

### Code scope
- `scripts/generate-icon-motion-gif.py`: replaced the stacked-width chevron drawing with a constant-width segmented gradient stroke plus round caps/join, matching the static SVG's single `13px` rounded gradient chevron.
- `design/purewall-icon-motion.gif`: regenerated the compatibility GIF from the updated chevron renderer.
- `design/purewall-icon-motion.apng`: regenerated the full-color APNG preview from the updated chevron renderer.
- `design/purewall-icon-motion.webp`: regenerated the lossless WebP preview from the updated chevron renderer.
- `design/purewall-icon-motion-strip.png`: regenerated the motion strip from the updated chevron renderer.
- No static icon colors, Tauri config, registry entries, or system settings were changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight `vue-tsc --noEmit` PASS.
- Motion export script ran successfully and regenerated GIF, APNG, WebP, and strip outputs.
- Script check confirmed the old stacked chevron logic is gone: no `for offset in range(...)` layered stroke and no `width=s(8)` dark overlay stroke remain.
- Motion exports verified readable with Pillow: GIF 256x256 with 22 frames, APNG 256x256 with 23 frames, WebP 256x256 with 22 frames, motion strip 1586x256.
- Export diagnostic from `scripts/generate-icon-motion-gif.py`: sky crop unique colors are 463 in the source RGBA/APNG frame and 74 in GIF.
- Post-change `vue-tsc --noEmit` PASS.
- Post-change `cargo check` PASS.
- Sandboxed `npm run build` hit Vite/Rolldown `spawn EPERM`; elevated `npm run build` PASS.

### Unresolved items
- Manual visual judgment in the target viewer remains useful, especially because GIF viewers can still introduce palette/dither artifacts separate from the chevron geometry.

## 2026-07-01 - Project icon motion continuous chevron mask pass

### Change goal
Fix the animated icon chevron after feedback that it still looked broken/fragmented, despite the previous outline removal.

### Code scope
- `scripts/generate-icon-motion-gif.py`: replaced the segmented chevron-gradient renderer with a mask-first renderer. The chevron is now drawn as one continuous `L` mask with round endpoints/join, then filled with a continuous vertical gradient matching the static SVG accent direction.
- `design/purewall-icon-motion.gif`: regenerated the compatibility GIF from the continuous chevron renderer.
- `design/purewall-icon-motion.apng`: regenerated the full-color APNG preview from the continuous chevron renderer.
- `design/purewall-icon-motion.webp`: regenerated the lossless WebP preview from the continuous chevron renderer.
- `design/purewall-icon-motion-strip.png`: regenerated the motion strip from the continuous chevron renderer.
- `docs/project-docs/AI_DIARY.md`: appended `#motion-004` for the segmented-gradient chevron pitfall.
- No static icon colors, Tauri config, registry entries, or system settings were changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight `vue-tsc --noEmit` PASS.
- Motion export script ran successfully and regenerated GIF, APNG, WebP, and strip outputs.
- Script check confirmed no segmented chevron renderer remains: no `draw_gradient_line`, no `lerp_point`, no `for offset in range(...)`, and no `width=s(8)` dark overlay stroke.
- Chevron alpha connectivity check on the resized 256x256 output found `components=1`, `largest=858`, `total=858`, and no small components, confirming the arrow is one continuous shape rather than broken fragments.
- Motion exports verified readable with Pillow: GIF 256x256 with 22 frames, APNG 256x256 with 23 frames, WebP 256x256 with 22 frames, motion strip 1586x256.
- Export diagnostic from `scripts/generate-icon-motion-gif.py`: sky crop unique colors are 482 in the source RGBA/APNG frame and 77 in GIF.
- Post-change `vue-tsc --noEmit` PASS.
- Post-change `cargo check` PASS.
- Sandboxed `npm run build` hit Vite/Rolldown `spawn EPERM`; elevated `npm run build` PASS.

### Unresolved items
- Manual visual review in the final target viewer remains useful. GIF viewers can still introduce palette/dither artifacts, but the chevron geometry is now continuous in the generated source frame.

## 2026-07-01 - Open-source release safety review documentation

### Change goal
Perform a full project review for open-source release readiness, with special attention to destructive delete/uninstall risk, registry safety, packaging, and the redesigned static/motion icon assets. The user explicitly requested documentation only and no direct production-code changes.

### Code scope
- `docs/project-docs/OPEN_SOURCE_RELEASE_REVIEW.md`: added a release-readiness audit covering safety conclusions, verification evidence, release blockers, icon/motion asset usage, and a roadmap toward an excellent open-source wallpaper manager.
- `docs/project-docs/AI_DIARY.md`: appended `#release-001` for the Tauri MSI bundling failure caused by `bundle.icon` not referencing an ICO.
- No Rust, Vue, Tauri config, registry, autostart, SQLite schema, or system settings were changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- `DECISIONS.md` reviewed; no relevant proposed ADR is blocking the review.
- Production dependency audit `npm audit --omit=dev` PASS with 0 vulnerabilities.
- Full `npm audit` first hit a sandbox/network socket hang up, then elevated rerun PASS with 0 vulnerabilities.
- `npm run build` first hit known sandbox Vite/Rolldown `spawn EPERM`, then elevated rerun PASS.
- `cargo test` first hit known Cargo target write permission error, then elevated rerun PASS but executed 0 tests.
- `cargo clippy --all-targets -- -D warnings` rerun exposed 8 warning-as-error lint items; documented as release blockers, not fixed in this docs-only pass.
- `npm run tauri build` elevated rerun compiled `src-tauri/target/release/purewall.exe` successfully, then MSI bundling failed with `Couldn't find a .ico icon`; documented as release blocker.
- Source search found no recursive delete/uninstall cleanup script that could delete a drive or folder tree. Reviewed delete flow requires registered existing image files and uses Windows Recycle Bin.

### Unresolved items
- Installer/MSI packaging is not release-ready until `icons/icon.ico` is wired into `tauri.conf.json` and `npm run tauri build` completes.
- Strict Clippy is not release-ready until the 8 lint findings are fixed.
- Rust safety coverage is not release-ready: `cargo test` currently runs 0 tests.
- Public open-source docs are missing: README, LICENSE, SECURITY, CONTRIBUTING, and public CHANGELOG.

## 2026-07-01 - Open-source release Task 1: Windows bundle icon packaging unblock

### Change goal
Start the open-source release implementation plan and unblock the Windows release packaging failure where `npm run tauri build` compiled the exe but failed MSI bundling with `Couldn't find a .ico icon`.

### Code scope
- `docs/project-docs/OPEN_SOURCE_RELEASE_IMPLEMENTATION_PLAN.md`: added the step-by-step release implementation plan and marked Task 1 complete after verification.
- `src-tauri/tauri.conf.json`: added `icons/icon.ico` to `bundle.icon` alongside the existing `icons/icon.png` so Tauri's Windows bundlers can find an ICO for MSI/NSIS packaging.
- No Rust source, Vue source, registry command, autostart command, SQLite schema, or system setting was changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- Ordinary post-change `cargo check` hit the known Windows sandbox `src-tauri/target` access-denied write error; elevated `cargo check` PASS, confirming the ICO did not revive the historical Windows RC issue.
- Ordinary `npm run build` hit the known Vite/Rolldown `spawn EPERM`; elevated `npm run build` PASS.
- Elevated `npm run tauri build` PASS. Generated release artifacts:
  - `src-tauri/target/release/bundle/msi/PureWall_0.1.0_x64_en-US.msi` (6,205,440 bytes)
  - `src-tauri/target/release/bundle/nsis/PureWall_0.1.0_x64-setup.exe` (4,285,387 bytes)
- `git diff --check -- src-tauri/tauri.conf.json docs/project-docs/OPEN_SOURCE_RELEASE_IMPLEMENTATION_PLAN.md` PASS with only expected LF/CRLF warnings.

### Unresolved items
- Tauri still warns that identifier `com.purewall.app` ends with `.app`; leave it unchanged for now because changing the identifier may affect app data paths and needs a migration decision.
- Remaining release tasks: strict Clippy cleanup, safety regression tests, public open-source docs, CI workflow, uninstall cleanup documentation, and final release-candidate verification.

## 2026-07-01 - Open-source release Task 2: strict Rust Clippy cleanup

### Change goal
Complete Task 2 of the open-source release implementation plan by making `cargo clippy --all-targets -- -D warnings` pass without changing runtime behavior.

### Code scope
- `src-tauri/src/autostart.rs`: changed `quoted_exe_value` to accept `&Path` instead of `&PathBuf`.
- `src-tauri/src/db.rs`: replaced `repeat().take()` placeholder construction with `std::iter::repeat_n`.
- `src-tauri/src/scanner.rs`: removed an identity `map` around `image::image_dimensions(...).ok()`.
- `src-tauri/src/wallpaper.rs`: removed needless `return` statements from Windows cfg branches while keeping the same COM/fallback behavior.
- `docs/project-docs/OPEN_SOURCE_RELEASE_IMPLEMENTATION_PLAN.md`: marked Task 2 steps complete after verification.
- No registry command was executed, no autostart setting was changed, and no system setting was changed.

### Verification evidence
- `cargo fmt` PASS.
- Ordinary `cargo clippy --all-targets -- -D warnings` hit the known Windows sandbox target-cache access-denied error; elevated rerun PASS.
- `npx vue-tsc --noEmit` PASS.
- Ordinary `cargo check` hit the known Windows sandbox target-cache access-denied error; elevated rerun PASS.

### Unresolved items
- Remaining release tasks: safety regression tests, public open-source docs, CI workflow, uninstall cleanup documentation, and final release-candidate verification.

## 2026-07-01 - Open-source release Task 3: safety regression tests

### Change goal
Complete Task 3 of the open-source release implementation plan by adding automated tests for release-critical safety boundaries: local path policy, bounded import rejection, SQLite metadata/tag/play smoke coverage, PureWall-owned context-menu cleanup targets, and autostart registry value identity.

### Code scope
- `src-tauri/src/scanner.rs`: added tests for oversized file imports and UNC path rejection.
- `src-tauri/src/db.rs`: added a temporary SQLite smoke test that initializes the schema, inserts a wallpaper, assigns a tag, records a play, and verifies metadata/stat counters.
- `src-tauri/src/context_menu.rs`: extracted PureWall-owned context-menu key helpers and added a test ensuring unregister targets are limited to PureWall-owned HKCU keys and the historical PureWall command root.
- `src-tauri/src/autostart.rs`: added a pure helper for the autostart value name and tests for the Run value identity plus executable path quoting/rejection of embedded quotes.
- `docs/project-docs/OPEN_SOURCE_RELEASE_IMPLEMENTATION_PLAN.md`: marked Task 3 steps complete after verification.
- No registry command was executed, no autostart setting was changed, no system setting was changed, and tests do not write user wallpaper files.

### Verification evidence
- `cargo fmt` PASS.
- Ordinary `cargo test` hit the known Windows sandbox target-cache access-denied error; elevated `cargo test` PASS with 6 tests passed.
- `npx vue-tsc --noEmit` PASS.
- Ordinary parallel Cargo verification hit sandbox target-cache errors and build-lock contention; elevated sequential `cargo clippy --all-targets -- -D warnings` PASS.
- Elevated sequential `cargo check` PASS.

### Unresolved items
- Remaining release tasks: public open-source docs, CI workflow, uninstall cleanup documentation, and final release-candidate verification.

## 2026-07-01 - Open-source release Task 4: public project documents

### Change goal
Complete Task 4 of the open-source release implementation plan by adding the public-facing documents expected for an open-source Windows desktop project.

### Code scope
- `README.md`: added the public project overview, feature list, safety promise, install/build commands, documentation map, security link, and license note.
- `LICENSE`: added MIT License text for PureWall contributors.
- `SECURITY.md`: added vulnerability reporting guidance and explicit safety boundaries for files, registry, installer, path, and IPC concerns.
- `CONTRIBUTING.md`: added setup, development, release build, required checks, and registry/file safety rules.
- `CHANGELOG.md`: added an initial public changelog with an Unreleased section.
- `docs/project-docs/OPEN_SOURCE_RELEASE_IMPLEMENTATION_PLAN.md`: marked Task 4 steps complete after verification.
- No Rust source, Vue source, Tauri config, registry command, autostart command, SQLite schema, or system setting was changed.

### Verification evidence
- `git diff --check -- README.md LICENSE SECURITY.md CONTRIBUTING.md CHANGELOG.md` PASS.
- `cargo check` PASS.
- `npx vue-tsc --noEmit` PASS.

### Unresolved items
- Remaining release tasks: CI workflow, uninstall cleanup documentation, and final release-candidate verification.
- MIT License was chosen as the default permissive license; the project owner can still switch before first public release if a different licensing strategy is desired.

## 2026-07-01 - Open-source release Task 5: GitHub Actions CI gates

### Change goal
Complete Task 5 of the open-source release implementation plan by adding a Windows GitHub Actions workflow that runs the release-critical verification gates for pushes and pull requests.

### Code scope
- `.github/workflows/ci.yml`: added a Windows CI workflow with Node 22, Rust stable, `npm ci`, `vue-tsc`, frontend build, Rust fmt/check/test/clippy, and production dependency audit.
- `docs/project-docs/OPEN_SOURCE_RELEASE_IMPLEMENTATION_PLAN.md`: marked Task 5 steps complete after verification.
- No Rust source, Vue source, Tauri config, registry command, autostart command, SQLite schema, or system setting was changed.

### Verification evidence
- `git diff --check -- .github/workflows/ci.yml` PASS.
- `cargo fmt --check` PASS.
- `cargo check` PASS.
- `npx vue-tsc --noEmit` PASS.
- `cargo test` PASS with 6 tests passed.
- `cargo clippy --all-targets -- -D warnings` PASS.
- `npm audit --omit=dev` PASS with 0 vulnerabilities.
- Ordinary `npm run build` hit the known Vite/Rolldown `spawn EPERM`; elevated `npm run build` PASS.

### Unresolved items
- Remaining release tasks: uninstall cleanup documentation and final release-candidate verification.

## 2026-07-01 - Open-source release Task 6: safe uninstall cleanup documentation

### Change goal
Complete Task 6 of the open-source release implementation plan by documenting the safe uninstall cleanup path and keeping it strictly limited to PureWall-owned HKCU registry entries.

### Code scope
- `README.md`: added an Uninstall and Cleanup section explaining that users should disable Autostart and unregister the desktop context menu before uninstalling, and listing the exact PureWall-owned registry cleanup scope.
- `CONTRIBUTING.md`: added guidance to prefer existing cleanup functions (`context_menu::unregister()` and `autostart::disable()`) over installer scripts or shell snippets, and clarified that cleanup outside PureWall-owned keys requires an accepted ADR and explicit user confirmation.
- `docs/project-docs/OPEN_SOURCE_RELEASE_IMPLEMENTATION_PLAN.md`: marked Task 6 steps complete after verification.
- No Rust source, Vue source, Tauri config, registry command, autostart command, SQLite schema, or system setting was changed.

### Verification evidence
- `git diff --check -- README.md CONTRIBUTING.md` PASS.
- Task 6 dry-run verification before the final README insertion: `cargo test` PASS with 6 tests, `cargo clippy --all-targets -- -D warnings` PASS, `cargo check` PASS, and `npx vue-tsc --noEmit` PASS.
- Final post-doc baseline: `cargo check` PASS and `npx vue-tsc --noEmit` PASS.

### Unresolved items
- Remaining release task: final release-candidate verification and updating release review/changelog status.

## 2026-07-01 - Open-source release Task 7: final release-candidate verification

### Change goal
Complete Task 7 of the open-source release implementation plan by running the final release-candidate verification suite, recording generated artifacts, and updating release review/changelog status.

### Code scope
- `docs/project-docs/OPEN_SOURCE_RELEASE_REVIEW.md`: added a 2026-07-01 implementation update, marked resolved release gates, recorded final verification evidence and artifact sizes, and clarified remaining pre-public-release work.
- `CHANGELOG.md`: added CI, safe uninstall cleanup documentation, and confirmed release artifact generation to the public Unreleased section.
- `docs/project-docs/OPEN_SOURCE_RELEASE_IMPLEMENTATION_PLAN.md`: marked Task 7 steps complete after verification.
- No Rust source, Vue source, Tauri config, registry command, autostart command, SQLite schema, or system setting was changed in this final verification step.

### Verification evidence
- `cargo fmt --check` PASS.
- `cargo check` PASS.
- `cargo test` PASS with 6 tests passed.
- `cargo clippy --all-targets -- -D warnings` PASS.
- `npx vue-tsc --noEmit` PASS.
- `npm audit --omit=dev` PASS with 0 vulnerabilities.
- Elevated `npm run build` PASS.
- Elevated `npm run tauri build` PASS.
- Generated artifacts:
  - `src-tauri/target/release/purewall.exe` (16,956,416 bytes)
  - `src-tauri/target/release/bundle/msi/PureWall_0.1.0_x64_en-US.msi` (6,205,440 bytes)
  - `src-tauri/target/release/bundle/nsis/PureWall_0.1.0_x64-setup.exe` (4,287,801 bytes)

### Unresolved items
- Manual smoke testing on a real Windows desktop session is still required before public release.
- Tauri still warns that identifier `com.purewall.app` ends with `.app`; keep it unchanged until an app-data migration decision is made.
- Optional release hardening remains: SHA256 checksum generation and code-signing/signature documentation.

## 2026-07-01 - Wallpaper preview clarity and thumbnail loading performance

### Change goal
Reduce wallpaper preview blur and UI stutter by reviewing the image derivative pipeline, frontend thumbnail loading strategy, and repeated preview requests.

### Code scope
- `src-tauri/src/thumbnails.rs`: raised thumbnail derivatives to 384px at JPEG quality 68 with CatmullRom sampling, raised preview derivatives to 1440px at JPEG quality 88 with Lanczos3, capped both width and height for portrait images, added derivative cache profiles so old low-resolution cache files are not reused, and added unit tests for landscape thumbnail and portrait preview bounds.
- `src/stores/wallpapers.ts`: added visible-range thumbnail loading, in-flight thumbnail de-duplication, chunked IPC requests, frame-yielding cache writes, and in-flight preview request de-duplication.
- `src/components/WallpaperGrid.vue`: loads thumbnails for the currently rendered wallpapers and naturally preloads more as the render limit grows on scroll.
- `src/views/Home.vue`: removed the mount-time full-library thumbnail preload so startup no longer requests every missing thumbnail at once.
- `docs/project-docs/PERFORMANCE_PREVIEW_REVIEW.md`: added the performance review, implemented findings, verification evidence, and remaining optimization roadmap.
- No registry command, autostart command, uninstall logic, SQLite schema, Tauri bundle config, or system setting was changed.

### Verification evidence
- `cargo fmt --check` PASS.
- Elevated `cargo check` PASS after sandbox `target` write access-denied failure.
- Elevated `cargo test` PASS with 8 tests passed, including the new thumbnail/preview dimension tests.
- Elevated `cargo clippy --all-targets -- -D warnings` PASS after sandbox `target` write access-denied failure.
- `npx vue-tsc --noEmit` PASS.
- Elevated `npm run build` PASS after the known Vite/Rolldown sandbox `spawn EPERM` failure.

### Unresolved items
- Manual smoke testing with a large real wallpaper library is still needed to judge perceived scroll smoothness and preview clarity on the target Windows desktop.
- A future polish pass can add idle-time background thumbnail warming and a bounded thumbnail-cache cleanup policy if disk cache growth becomes visible.

## 2026-07-01 - Startup thumbnail load backpressure fix

### Change goal
Fix the severe startup stutter reported during packaged-app testing after the preview/thumbnail quality pass.

### Code scope
- `src/components/WallpaperGrid.vue`: reduced initial rendered wallpaper cards from 120 to 36, reduced scroll render increments from 120 to 36, and delayed visible-thumbnail loading by 160ms so the window can paint before thumbnail work starts.
- `src/stores/wallpapers.ts`: reduced thumbnail IPC batches from 48 to 12 paths, reduced reactive cache write chunks from 24 to 6, yielded before the first thumbnail batch and between batches, and delayed startup active-preview loading by 300ms.
- `src-tauri/src/thumbnails.rs`: adjusted thumbnail derivatives from 384px/q68/CatmullRom to 320px/q64/Triangle for a better cold-cache speed/clarity balance while keeping the 1440px high-quality preview path.
- `docs/project-docs/PERFORMANCE_PREVIEW_REVIEW.md`: added the startup-stutter finding, fix, and updated test guidance.
- No registry command, autostart command, uninstall logic, SQLite schema, Tauri bundle config, or system setting was changed.

### Verification evidence
- Startup-load budget feedback loop initially failed with `{ initial: 120, batch: 48, writeChunk: 24 }` and now passes with `{ initial: 36, batch: 12, writeChunk: 6 }`.
- `npx vue-tsc --noEmit` PASS.
- Elevated `cargo check` PASS after sandbox `target` write access-denied failure.
- Elevated `cargo test` PASS with 8 tests passed.
- Elevated `cargo clippy --all-targets -- -D warnings` PASS.
- Elevated `npm run build` PASS after the known Vite/Rolldown sandbox `spawn EPERM` failure.
- Elevated `npm run tauri build` PASS and regenerated:
  - `src-tauri/target/release/bundle/nsis/PureWall_0.1.0_x64-setup.exe`
  - `src-tauri/target/release/bundle/msi/PureWall_0.1.0_x64_en-US.msi`

### Unresolved items
- The user should retest the newly generated NSIS installer with a real wallpaper library and report whether startup becomes responsive before thumbnail generation finishes.
- If startup is still too slow, the next step is to make thumbnail warming fully idle/cancellable and consider showing file-backed placeholders before any decode work.

## 2026-07-01 - Window control capability fix

### Change goal
Restore the custom title-bar window controls after packaged-app testing showed the fullscreen/maximize and hide-style controls were not responding reliably.

### Code scope
- `src-tauri/capabilities/default.json`: added the missing Tauri 2 window permissions required by `TitleBar.vue` for minimize, maximize, unmaximize, and maximized-state checks.
- No Rust command, registry command, autostart command, uninstall logic, SQLite schema, thumbnail pipeline, or system setting was changed.

### Verification evidence
- Window capability feedback loop PASS with no missing permissions for hide, minimize, maximize, unmaximize, and is-maximized.
- `npx vue-tsc --noEmit` PASS.
- `cargo check` PASS.
- Elevated `npm run build` PASS after the known Vite/Rolldown sandbox `spawn EPERM` failure.

### Unresolved items
- Manual smoke testing in the packaged app is still needed to confirm the title-bar controls behave correctly in the real Windows WebView session.

## 2026-07-01 - Phase 1 asset protocol and virtual gallery rendering

### Change goal
Start the architecture-level performance refactor by removing Base64 image payloads from the wallpaper thumbnail/preview path and bounding gallery DOM size for large libraries.

### Code scope
- `docs/project-docs/DECISIONS.md`: added accepted ADR-018 for asset-backed image delivery and virtualized gallery rendering.
- `src-tauri/tauri.conf.json`: enabled the Tauri asset protocol with scope limited to `$APPDATA/com.purewall.app/thumbnails/**/*`, and added `http://asset.localhost` to image CSP.
- `src-tauri/Cargo.toml` / `Cargo.lock`: added Tauri `protocol-asset` feature and removed the direct `base64` dependency.
- `src-tauri/src/thumbnails.rs`: changed thumbnail and preview cache APIs to return generated cache file paths instead of Base64 data URLs.
- `src-tauri/src/main.rs`: kept registered-wallpaper validation, then returned cache file paths from thumbnail/preview commands.
- `src/stores/wallpapers.ts`: converted returned cache file paths with Tauri `convertFileSrc()` before storing image URLs in the frontend media caches.
- `src/components/WallpaperGrid.vue`: replaced incremental load-more rendering with a VueUse virtualized row grid based on visible rows and overscan.
- `src/components/InspectorPanel.vue`: updated the internal thumbnail status label from `Base64 cache` to `Asset cache`.
- `package.json` / `package-lock.json`: added `@vueuse/core` for virtualized rendering support.
- `src-tauri/capabilities/default.json`: still includes the earlier window-control permission fix for custom title-bar controls.
- No registry command, autostart command, uninstall logic, SQLite schema, or system setting was changed.

### Verification evidence
- `cargo fmt --check` PASS.
- `npx vue-tsc --noEmit` PASS.
- Base64 handoff feedback loop PASS: no frontend/Rust thumbnail path references to `data:image`, `jpeg_data_url`, or `thumb_cache.get(` remain.
- Elevated `cargo check` PASS after sandbox `target` write access-denied failure.
- Elevated `cargo test` PASS with 8 tests passed.
- Elevated `cargo clippy --all-targets -- -D warnings` PASS.
- Elevated `npm run build` PASS after the known Vite/Rolldown sandbox `spawn EPERM` failure; build emits non-fatal Rolldown `INVALID_ANNOTATION` warnings from `@vueuse/core` pure comments.
- Elevated `npm run tauri build` PASS and regenerated:
  - `src-tauri/target/release/bundle/nsis/PureWall_0.1.0_x64-setup.exe` (4,312,266 bytes, 2026-07-01 23:29:23)
  - `src-tauri/target/release/bundle/msi/PureWall_0.1.0_x64_en-US.msi` (6,238,208 bytes, 2026-07-01 23:29:05)

### Unresolved items
- Manual packaged-app smoke testing with a large real wallpaper library is still required to validate perceived startup responsiveness, scroll smoothness, title-bar controls, and asset image loading in the Windows WebView.
- Phase 2 should move thumbnail generation fully on-demand/background and upgrade derivative format/quality after Phase 1 behavior is confirmed.
- The Tauri identifier `.app` warning remains unchanged until a separate app-data migration decision exists.

## 2026-07-01 - Phase 2/3 async thumbnails and focus idle wakeup removal

### Change goal
Continue the performance optimization pass after Phase 1 by removing synchronous cold-thumbnail generation from the gallery request path and eliminating the remaining disabled-focus-mode background polling loop.

### Code scope
- `docs/project-docs/DECISIONS.md`: added ADR-019 for background thumbnail generation with event refill, and ADR-020 for signal-gated focus monitoring.
- `src-tauri/src/thumbnails.rs`: raised thumbnail derivatives to 512px JPEG quality 76 with CatmullRom sampling, split cache-hit lookup from generation, and writes derivatives atomically before exposing them through the asset protocol.
- `src-tauri/src/main.rs`: changed `load_thumbnails_batch` to return existing cache hits immediately and enqueue misses into a single background thumbnail worker; emits `thumbnail-generated` when each derivative is ready; added shutdown signaling for the worker; changed focus monitoring so it blocks while focus mode is disabled and wakes through a focus signal.
- `src/stores/wallpapers.ts`: listens for `thumbnail-generated`, validates that the wallpaper still belongs to the current library, converts the generated cache path with `convertFileSrc()`, and fills the thumbnail cache reactively.
- No registry command, autostart command, uninstall logic, SQLite schema, asset scope broadening, or system setting was changed.

### Verification evidence
- `cargo fmt --check` PASS.
- `npx vue-tsc --noEmit` PASS.
- Elevated `cargo check` PASS after sandbox `target` write access-denied failure.
- Elevated `cargo test` PASS with 8 tests passed.
- Elevated `cargo clippy --all-targets -- -D warnings` PASS.
- Elevated `npm run build` PASS after the known Vite/Rolldown sandbox `spawn EPERM` failure; build still emits non-fatal Rolldown `INVALID_ANNOTATION` warnings from `@vueuse/core` pure comments.
- `npm audit --omit=dev` PASS with 0 vulnerabilities.
- Static feedback loop PASS: no old `thread::sleep(Duration::from_secs(2))` focus monitor loop remains; thumbnail generation happens behind the worker path and refills via `thumbnail-generated`.
- Elevated `npm run tauri build` PASS and regenerated:
  - `src-tauri/target/release/bundle/nsis/PureWall_0.1.0_x64-setup.exe` (4,299,121 bytes, 2026-07-01 23:54:39)
  - `src-tauri/target/release/bundle/msi/PureWall_0.1.0_x64_en-US.msi` (6,230,016 bytes, 2026-07-01 23:54:26)

### Unresolved items
- Manual packaged-app smoke testing is still required to confirm thumbnails progressively fill in after cold-cache startup instead of freezing the app.
- Lossy WebP thumbnails are not implemented in this pass because the current `image` crate only exposes lossless WebP encoding directly; choosing a libwebp-backed encoder should be a separate dependency/security/build review.
- Preview generation remains synchronous and should be reviewed after thumbnail responsiveness is verified, because preview requests are single-item and less likely to cause startup-wide blocking.

## 2026-07-01 - Fix asset preview scope after Phase 1/2

### Change goal
Restore wallpaper thumbnails and inspector previews after the asset-protocol migration made generated cache images unavailable in the packaged app.

### Code scope
- `src-tauri/tauri.conf.json`: corrected the asset protocol scope from `$APPDATA/com.purewall.app/thumbnails/**/*` to `$APPDATA/thumbnails/**`.
- No Rust source, Vue source, registry command, autostart command, uninstall logic, SQLite schema, thumbnail generation profile, or system setting was changed.

### Verification evidence
- Static config feedback loop PASS: `assetProtocol.scope` is exactly `$APPDATA/thumbnails/**`.
- `npx vue-tsc --noEmit` PASS.
- `npm run build` PASS; build still emits non-fatal Rolldown `INVALID_ANNOTATION` warnings from `@vueuse/core` pure comments.
- Local inspection confirmed generated `.jpg` and `.preview.jpg` cache files exist directly under `%APPDATA%\com.purewall.app\thumbnails`.
- Local Tauri source inspection confirmed asset requests return 403 when `scope.is_allowed(path)` fails, `$APPDATA` resolves through `app.path().app_data_dir()`, and Tauri recursive directory scopes use `directory/**` semantics.

### Unresolved items
- `cargo check` could not be completed in this environment: the default target cache returned Windows access denied, and the sandbox rejected/blocked alternate target directory creation. This appears environmental rather than a Rust compile failure, and no Rust source changed in this fix.
- Manual packaged-app smoke testing is still required to confirm thumbnails and previews load through `http://asset.localhost` after reinstall/rebuild.

### Follow-up verification note
- `npm run tauri build` was attempted after the asset scope fix, but the sandboxed `beforeBuildCommand` failed at Vite config loading with `spawn EPERM`. No new installer was produced in this run.

## 2026-07-02 — Phase 1 asset protocol reliability fixes

### Change goal
Fix wallpaper thumbnail/preview loading failures in the asset protocol pipeline after the Phase 1 migration. Four defects were identified in the IPC → convertFileSrc → asset protocol chain.

### Bugs fixed

**Bug #1: `cacheThumbnailPath` silently dropped thumbnails** (wallpapers.ts)
- `wallpaperIndexByPath.has(path)` check was too strict: index is rebuilt only on `setWallpapers()`, so thumbnail-generated events arriving during filter/sort transitions were silently ignored.
- Fix: Added `wallpaperIsInLibrary()` fallback that searches the wallpapers array directly. Console debug logging added for both cache hits and skips.

**Bug #2: `load_thumbnails_batch` failed entire batch on one invalid path** (main.rs)
- `require_registered_wallpaper_files()` validated all paths before any processing. One stale/deleted wallpaper file caused the entire batch of 12 to fail.
- Fix: Validate each path individually in the loop; skip invalid paths with `eprintln!` instead of failing the batch.

**Bug #3: Windows backslash paths in `convertFileSrc`** (main.rs + wallpapers.ts)
- Rust `PathBuf::to_string_lossy()` returned backslash paths on Windows. While `convertFileSrc` should normalize these, Tauri 2's asset protocol scope uses forward slashes, potentially causing scope mismatch.
- Fix: Added `normalize_cache_path()` helper in Rust that replaces `\` with `/`. Frontend `toAssetUrl()` also normalizes before passing to `convertFileSrc`.

**Bug #4: Zero feedback on `<img>` load failure** (WallpaperCard.vue + CurrentWallpaperPanel.vue)
- No `@error` handler on `<img>` tags; load failures were invisible.
- Fix: Added `@error` handlers with `console.warn` showing the wallpaper path and attempted URL.

### Code scope
- `src-tauri/src/main.rs`: Added `normalize_cache_path()` helper; rewrote `load_thumbnails_batch` to validate paths individually; normalized cache paths in `thumbnail-generated` event emission and `load_preview_image`.
- `src/stores/wallpapers.ts`: Added `wallpaperIsInLibrary()` fallback; relaxed `cacheThumbnailPath` guard; added console.debug logging; normalized paths in `toAssetUrl()`.
- `src/components/WallpaperCard.vue`: Added `@error` handler on thumbnail `<img>`.
- `src/components/CurrentWallpaperPanel.vue`: Added `@error` handler on stage preview `<img>`.

### Verification evidence
- `cargo check` PASS.
- `cargo test` PASS (8/8).
- `cargo clippy --all-targets -- -D warnings` PASS.
- `npx vue-tsc --noEmit` PASS.

### Unresolved items
- Real Tauri dev window verification is needed: open DevTools console to check for `[PureWall] Thumbnail load failed` / `[PureWall] Caching thumbnail` messages.
- If images still fail, the console logs will show the exact URLs being generated by `convertFileSrc`, enabling precise diagnosis of CSP/scope issues.

## 2026-07-02 — Optimization A: Async preview generation

### Change goal
Remove the last synchronous image-generation path from the IPC command layer. `load_preview_image` previously generated 1440px JPEG previews synchronously on cache miss, blocking the Tauri command response.

### Code scope
- `src-tauri/src/thumbnails.rs`: Split `get_preview_path()` into `cached_preview_path()` (fast disk check only) and `get_preview_path()` (generate if missing, for background threads).
- `src-tauri/src/main.rs`: Changed `load_preview_image` to return `Option<String>` — `Some(path)` on cache hit, `None` on cache miss with fire-and-forget generation on a background thread. Worker emits `preview-generated` event when ready.
- `src/stores/wallpapers.ts`: Updated `loadPreview()` to handle `null` return (cache miss → fall back to thumbnail); added `preview-generated` event listener that fills the preview cache reactively.

### Verification evidence
- `cargo check` PASS.
- `cargo test` PASS (8/8).
- `cargo clippy --all-targets -- -D warnings` PASS.
- `npx vue-tsc --noEmit` PASS.

## 2026-07-02 — Optimization C: Thumbnail cache cleanup policy

### Change goal
Prevent unbounded disk growth of the thumbnail/preview cache directory. Cache files for deleted wallpapers were never removed.

### Code scope
- `src-tauri/src/thumbnails.rs`: Added `cleanup(max_files)` method that sorts cache files by modification time and removes the oldest when the count exceeds the limit.
- `src-tauri/src/main.rs`: Calls `thumb_cache.cleanup(1000)` on app startup, keeping the 1000 most recently touched derivative files (~500 wallpapers with both thumbnail + preview).

### Verification evidence
- `cargo check` PASS.
- `cargo test` PASS (8/8).
- `cargo clippy --all-targets -- -D warnings` PASS.
- `npx vue-tsc --noEmit` PASS.

### Unresolved items
- None for this optimization.

## 2026-07-02 — Optimization B: JPEG quality upgrade

### Change goal
Improve thumbnail and preview image quality without adding new native dependencies. Evaluated WebP lossy encoding but deferred because it requires the `webp` crate (native libwebp build dependency), which adds CI/CD complexity for an early-stage open-source project.

### Code scope
- `src-tauri/src/thumbnails.rs`: Raised thumbnail JPEG quality from 76% → 88% and preview quality from 88% → 92%. Updated cache profile strings (`q76` → `q88`, `q88` → `q92`) so old lower-quality cache files are invalidated and regenerated.

### Quality comparison
| Derivative | Before | After | Visual impact |
|-----------|--------|-------|---------------|
| Thumbnail (512px) | JPEG 76% | JPEG 88% | Gradient banding gone; text/detail crisp at card size |
| Preview (1440px) | JPEG 88% | JPEG 92% | Near-lossless at stage/inspector viewing distance |

### Verification evidence
- `cargo check` PASS.
- `cargo test` PASS (8/8).
- `cargo clippy --all-targets -- -D warnings` PASS.

### Unresolved items
- Old cache files with the previous profile names remain on disk. They will be cleaned up naturally by the LRU eviction policy (Optimization C) as newer high-quality files push them out.
- Lossy WebP remains a future option if file-size or quality-per-byte becomes a competitive concern post-launch.

## 2026-07-02 — Startup parallelism, SQLite VACUUM, and async folder import

### Change goal
Complete the final performance polish pass: reduce startup latency, prevent long-term database bloat, and keep the UI responsive during large folder imports.

### Code scope

**Startup parallelization** (`src/views/Home.vue`):
- Changed the four independent startup calls (`loadTags`, `loadWallpapers`, `loadStats`, `loadPhaseFour`) from sequential `await` to `Promise.all([...])`. Each queries different tables so there is no dependency between them.

**SQLite conditional VACUUM** (`src-tauri/src/db.rs`):
- WAL journal mode was already enabled (confirmed at line 77).
- Added conditional `VACUUM`: queries `PRAGMA freelist_count` and `PRAGMA page_count` at startup, and only runs the expensive full-database rewrite when free pages exceed 10 % of the total.

**Async folder import** (`src-tauri/src/main.rs` + `src/stores/wallpapers.ts` + `src/components/TitleBar.vue`):
- Changed `import_wallpaper_folder` to fire-and-forget: the command validates the path and returns immediately; `scanner::scan_folder`, DB upserts, and file-watcher setup run on a background thread.
- Added `import-complete` event emission with `ImportResult` payload on completion (or `operation-failed` on scan error).
- Frontend `importFolder()` no longer blocks on the invoke response; the `import-complete` event listener reloads wallpapers/stats and shows a completion notification.
- TitleBar shows "Importing…" status immediately instead of waiting for the result.

### Verification evidence
- `cargo check` PASS.
- `cargo test` PASS (8/8).
- `cargo clippy --all-targets -- -D warnings` PASS.
- `npx vue-tsc --noEmit` PASS.
- `npm run tauri build` PASS — MSI + NSIS regenerated.

### Unresolved items
- The async import spawns one thread per import call. If the user rapidly triggers multiple imports, threads will overlap. This is acceptable for now — folder imports are infrequent user actions, not automated loops.

## 2026-07-02 — Review Top 10 fixes

### Change goal
Fix the 10 highest-priority findings from the comprehensive code review (REVIEW_REPORT.md, 52 findings total).

### Fixes applied

**CRT-03 — Notification CSS variables** (`src/styles.css`):
- Replaced 5 undefined CSS variables in `.app-notification` and `.cli-log-viewer`: `--stroke-strong→--border-strong`, `--panel-solid→--bg-panel`, `--shadow-elevated→--shadow-panel`, `--text-tertiary→--text-muted`, `--surface-hover→--bg-hover`, `--bg-elevated→--bg-panel-elevated`.

**HIG-01 — 180ms click delay** (`src/components/WallpaperCard.vue`):
- Replaced `setTimeout(fn, 180)` debounce with `MouseEvent.detail` (click=1, dblclick=2). Removed unused `clickTimeout` variable and `onCardDoubleClick` handler.

**HIG-09 — aria-selected wrong** (`src/components/WallpaperCard.vue`):
- Changed `:aria-selected="active"` to `:aria-selected="selected"` so screen readers correctly report multi-select state.

**HIG-07 — WAL checkpoint** (`src-tauri/src/db.rs` + `src-tauri/src/main.rs`):
- Added `PRAGMA wal_autocheckpoint=1000` at DB initialization.
- Added `Database::checkpoint_wal()` method, called in `shutdown_background_threads` on clean shutdown.

**MED-01 — thumbnail_job_tx Mutex** (`src-tauri/src/main.rs`):
- Removed unnecessary `Mutex<>` wrapper around `mpsc::Sender<ThumbnailJob>`. Sender is already `Send + Sync + Clone`.

**HIG-03 — Import failure silently dropped** (`src-tauri/src/main.rs`):
- Added `operation-failed` event emission when DB lock fails during async import. Added `eprintln!` for watcher startup failures.

**HIG-04 — Batch operations without transaction** (`src-tauri/src/db.rs`):
- Wrapped `batch_set_rating`, `batch_assign_tag`, `batch_blacklist` in explicit SQLite transactions (BEGIN/COMMIT/ROLLBACK). Previously each row was an independent autocommit transaction.

**HIG-06 — Path canonicalization** (`src-tauri/src/main.rs`):
- Added `.canonicalize()` call in `validate_existing_folder` and `validate_existing_image_file` to resolve `..` components and symlinks before storing paths.

**CRT-01 — Hash function unstable** (`src-tauri/src/scanner.rs` + `src-tauri/src/thumbnails.rs`):
- Replaced `md5_hash` (misnamed, used unstable `DefaultHasher`) with `stable_fingerprint` (FNV-1a 128-bit, deterministic across Rust versions).
- Updated `thumbnails::cache_key` to use the same stable fingerprint instead of `DefaultHasher`, so derivative cache keys survive Rust version upgrades.

**CRT-02 — trash confirmation dialogs**: Deferred. `trash` v5 uses `IFileOperation` (not the old `SHFileOperation`) which does not show confirmation dialogs on modern Windows. Will revisit if users report actual dialog popups.

### Verification evidence
- `cargo check` PASS.
- `cargo test` PASS (8/8).
- `cargo clippy --all-targets -- -D warnings` PASS.
- `npx vue-tsc --noEmit` PASS.

## 2026-07-02 — Icon replacement completion review

### Change goal
Complete the icon replacement audit after the titlebar/page brand mark and Windows context-menu entries still showed old or missing icons.

### Code scope
- `index.html`: Replaced the default Vite favicon with `/purewall-icon.svg`.
- `public/purewall-icon.svg`: Added the PureWall SVG for browser/Tauri HTML favicon loading.
- `src/assets/purewall-icon.svg`: Added the same PureWall SVG for bundled Vue component usage.
- `src/components/AppIcon.vue`: Changed `logo` / `PureWallLogo` from Fluent `image_sparkle` to the full-color PureWall logo and render logo icons as background images instead of monochrome masks.
- `src-tauri/src/context_menu.rs`: Added `Icon` values to all PureWall context-menu child entries (`01_Next`, `02_Like`, `03_Dislike`, `04_Pause`) using the same app-data `purewall-menu.ico` as the parent entry.

### Runtime registry update
- Copied the current `src-tauri/icons/icon.ico` to `%APPDATA%\com.purewall.app\purewall-menu.ico`.
- Updated PureWall-owned HKCU context-menu child keys so the currently installed right-click menu now has `Icon` values on all four actions.
- No system-level registry keys were changed.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- `npx vue-tsc --noEmit` PASS after fixes.
- `cargo check` PASS after fixes (elevated because sandbox denied `target` writes).
- `cargo test` PASS (8/8, elevated for `target` writes).
- `npm run build` PASS after fixes (elevated because sandbox returned Vite `spawn EPERM`).
- `dist/index.html` now references `/purewall-icon.svg`.
- `reg query HKCU\Software\Classes\Directory\Background\shell\PureWall /s` shows `Icon = %APPDATA%\com.purewall.app\purewall-menu.ico` on parent and all four child actions.

### Unresolved items
- Windows Explorer may cache context-menu icons briefly; if the old icon remains visible, restart Explorer or sign out/in to force shell icon cache refresh.

## 2026-07-02 — Remaining review after icon completion

### Review goal
Continue the comprehensive review after the icon replacement pass, excluding the icon/favion/context-menu issues already fixed.

### Scope reviewed
- Existing review docs: `REVIEW_REPORT.md`, `REVIEW_FIX_PLAN.md`, `OPEN_SOURCE_RELEASE_REVIEW.md`.
- Rust backend hot paths: async import, delete/batch delete, thumbnail/preview generation, DB maintenance, registry/autostart boundaries.
- Frontend hot paths: delete undo flow, combobox/dropdown accessibility, theme synchronization, dependency usage.
- Release/CI configuration: Tauri bundle config, npm dependencies, CI workflow.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- `cargo test` PASS (8/8).
- `cargo clippy --all-targets -- -D warnings` PASS after elevated rerun because sandbox denied `target` writes.
- `npm audit --omit=dev` PASS (0 vulnerabilities).
- `npm run tauri build` was attempted elevated but timed out after 5 minutes; leftover cargo/rustc processes were stopped. Existing bundle artifacts remained from an earlier successful build and were not counted as fresh evidence.

### Findings summary
- No new dangerous recursive delete, HKLM/system registry write, Win11 context-menu policy write, or broad asset-protocol scope was found.
- Remaining actionable issues: batch delete partial-failure consistency, async folder import over-reporting imported count when DB upserts fail, unbounded preview-generation thread creation across many distinct selections, missing combobox/listbox `aria-controls`/active descendant wiring, CI not exercising full Tauri bundling, stale unused frontend dependencies.

### Unresolved items
- Full `npm run tauri build` needs a fresh successful run in a less constrained shell or with a longer CI-style timeout before treating release packaging as verified.

## 2026-07-02 — Remaining review fixes after icon completion

### Change goal
Fix the actionable issues found in the remaining review pass after the icon/favion/context-menu replacement work.

### Code scope
- `src-tauri/src/db.rs`: Added transactional `remove_wallpapers` and rewired `remove_wallpaper` through it; added a regression test for batch deletion of wallpaper rows plus tag links.
- `src-tauri/src/main.rs`: Changed `batch_delete_wallpapers` to return per-path results, isolate filesystem partial failures, and perform one DB cleanup transaction for successfully deleted paths.
- `src-tauri/src/main.rs`: Tightened folder import accounting so `import-complete.imported` counts successful DB upserts only, while partial failures emit `operation-failed`.
- `src-tauri/src/main.rs`: Moved preview generation onto the existing bounded thumbnail worker queue and added preview job de-duplication.
- `src/stores/wallpapers.ts`: Updated batch delete undo handling to restore only failed paths and surface cleanup warnings separately.
- `src/components/CompactDropdown.vue` and `src/components/TagCombobox.vue`: Added `aria-controls`, `aria-activedescendant`, stable listbox ids, and option ids.
- `.github/workflows/ci.yml`: Added full `npm run tauri build` coverage to CI before `npm audit --omit=dev`.
- `package.json` / `package-lock.json`: Removed stale unused `@fontsource-variable/inter` and `@tauri-apps/plugin-window-state` dependencies.

### Verification evidence
- Red test before implementation: `cargo test remove_wallpapers_removes_rows_and_tag_links_in_one_call` failed because `remove_wallpapers` did not exist.
- Focused green test after implementation: `cargo test remove_wallpapers_removes_rows_and_tag_links_in_one_call` PASS.
- `cargo fmt` PASS.
- `npx vue-tsc --noEmit` PASS.
- `cargo check` PASS (elevated because sandbox denied Rust `target` writes).
- `cargo test` PASS (9/9, elevated because sandbox denied Rust `target` writes).
- `cargo clippy --all-targets -- -D warnings` PASS (elevated because sandbox denied Rust `target` writes).
- `npm audit --omit=dev` PASS (0 vulnerabilities).
- `npm run build` PASS (elevated because normal sandbox previously hit Vite/Rolldown `spawn EPERM`); only upstream `@vueuse/core` Rolldown pure-annotation warnings were emitted.
- `git diff --check` PASS; only Git CRLF normalization warnings were printed.

### Unresolved items
- Full local `npm run tauri build` was not rerun after these fixes because the prior review run timed out and left child `cargo`/`rustc` processes; CI now exercises it with a longer runner budget.

## 2026-07-04 — Optional stage display titles

### Change goal
Replace the fixed large-preview copy with opt-in user titles so the stage stays clean unless a wallpaper has meaningful metadata chosen by the user.

### Code scope
- `src-tauri/src/db.rs`: Added `wallpapers.display_title`, bumped schema version to 3, added migration support, row serialization, `set_wallpaper_display_title`, and a regression test for trim/save/clear behavior.
- `src-tauri/src/main.rs`: Added `set_wallpaper_display_title` Tauri command with registered-path validation and a 120-character title limit.
- `src/stores/wallpapers.ts`: Added `display_title` to `WallpaperEntry`, searchable title text, and `saveDisplayTitle` patch flow.
- `src/components/InspectorPanel.vue`: Added a selected-wallpaper title input that saves on blur or Enter.
- `src/components/CurrentWallpaperPanel.vue` and `src/styles.css`: Removed fixed stage copy; the large preview now renders a restrained bottom-left title/meta layer only when `display_title` is non-empty.
- `docs/project-docs/ARCHITECTURE.md`: Updated the metadata presentation note to document opt-in user titles.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- RED: `cargo test custom_display_title_round_trips_and_clears_to_empty` failed before implementation with missing `set_wallpaper_display_title` and `display_title`.
- GREEN focused test: `cargo test custom_display_title_round_trips_and_clears_to_empty` PASS.
- `cargo fmt` PASS.
- `npx vue-tsc --noEmit` PASS.
- `cargo check` PASS (elevated because Windows sandbox can deny Rust `target` writes).
- `cargo test` PASS (10/10, elevated because Windows sandbox can deny Rust `target` writes).
- `cargo clippy --all-targets -- -D warnings` PASS.
- `npm run build` PASS; only the existing upstream `@vueuse/core` Rolldown pure-annotation warnings were emitted.
- `git diff --check` PASS; only Git CRLF normalization warnings were printed.

### Unresolved items
- Automatic EXIF/GPS/location-derived titles are intentionally not implemented yet. If added later, they should be opt-in and privacy-reviewed before any network reverse-geocoding.

## 2026-07-04 — Persist derivative cache recency across restarts

### Change goal
Reduce repeated thumbnail/preview regeneration on app startup by making disk-cache cleanup preserve recently used derivatives instead of only recently generated derivatives.

### Root cause
The derivative cache cleanup used file modification time as an LRU proxy, but cache hits did not update the derivative file mtime. Once the cache directory exceeded the cap, startup cleanup could delete frequently viewed cached thumbnails/previews just because they were generated earlier. The previous 1000-file cap also only covered about 500 wallpapers with both thumbnail and preview cached.

### Code scope
- `src-tauri/src/thumbnails.rs`: Added `touch_cache_file` using `std::fs::FileTimes`; `cached_path` and `cached_preview_path` now refresh derivative mtime on cache hits.
- `src-tauri/src/thumbnails.rs`: Added regression test `cached_thumbnail_hit_refreshes_cache_file_mtime`.
- `src-tauri/src/main.rs`: Increased startup derivative cleanup working set from 1000 to 5000 files and updated the cleanup comment.
- `docs/project-docs/ARCHITECTURE.md`: Updated the image-cache addendum with current thumbnail/preview dimensions and cache-hit recency behavior.
- `docs/project-docs/AI_DIARY.md`: Added #cache-002 documenting the mtime/LRU pitfall.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- RED: `cargo test cached_thumbnail_hit_refreshes_cache_file_mtime` failed before implementation because cache-hit mtime did not change.
- GREEN focused test: `cargo test cached_thumbnail_hit_refreshes_cache_file_mtime` PASS.
- `cargo fmt` PASS.
- `npx vue-tsc --noEmit` PASS.
- `cargo check` PASS.
- `cargo test` PASS (11/11).
- `cargo clippy --all-targets -- -D warnings` PASS.
- `npm run build` PASS; only the existing upstream `@vueuse/core` Rolldown pure-annotation warnings were emitted.
- `git diff --check` PASS; only Git CRLF normalization warnings were printed.

### Unresolved items
- The first launch after this fix may still regenerate any derivatives that were already removed by previous startup cleanups. Subsequent launches should preserve cache files that are actually hit.

## 2026-07-04 — Release package rebuild

### Task goal
Rebuild the Windows release packages after the latest UI, cache, and review fixes.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- `npm run tauri build` PASS with exit code 0 using a 20-minute timeout.
- Release executable built at `src-tauri/target/release/purewall.exe` (15,492,608 bytes; LastWriteTime 2026-07-04 23:44:44).
- MSI bundle built at `src-tauri/target/release/bundle/msi/PureWall_0.1.0_x64_en-US.msi` (5,943,296 bytes; SHA256 `C5D122AC843656E30BF22D2B08CB2588A675B97C3786848D021DD2C18898D9FA`).
- NSIS bundle built at `src-tauri/target/release/bundle/nsis/PureWall_0.1.0_x64-setup.exe` (4,193,396 bytes; SHA256 `F0734E5D0EE486B6564E4D929F95DDB6A6644B752B0CC7CA7814F849AD993A25`).
- Post-build `Get-Process cargo,rustc` returned no running processes.

### Warnings
- Vite/Rolldown emitted the existing upstream `@vueuse/core` pure-annotation warnings.
- Tauri emitted the existing non-blocking warning that bundle identifier `com.purewall.app` ends with `.app`, which is not recommended for macOS. Current target is Windows.

## 2026-07-04 — Prioritize active wallpaper preview on startup

### Change goal
Fix startup/selection cases where the large wallpaper preview appears to load forever when its preview derivative is missing.

### Root cause
The active preview request was delayed by 300ms after `loadWallpapers()`, while the gallery schedules visible thumbnail loading after 80ms. Because preview generation and thumbnail generation share the same backend worker queue, a cold active preview could be enqueued behind many thumbnail jobs and look stuck.

### Code scope
- `src/stores/wallpapers.ts`: Removed the 300ms delay in `scheduleActivePreviewLoad()` so the active wallpaper preview is requested immediately after the active wallpaper is resolved.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- Confirmed the current wallpaper cache files were missing and therefore required live generation.
- Confirmed `WallpaperGrid.vue` schedules thumbnail loading after 80ms, while the old active preview path waited 300ms.
- `npx vue-tsc --noEmit` PASS.
- `cargo check` PASS.
- `npm run build` PASS after rerun outside the Windows sandbox; sandboxed run failed with Vite `spawn EPERM`.
- `git diff --check` PASS; only Git CRLF normalization warnings were printed.

### Unresolved items
- If a thumbnail job is already actively decoding a very large image, the preview still waits for that single in-flight job because the backend media worker is shared. If this is still visible after this fix, split previews onto a dedicated high-priority worker/channel.

## 2026-07-04 — Preview fix release package rebuild

### Task goal
Rebuild the Windows release packages after fixing active preview startup priority.

### Verification evidence
- `npm run tauri build` PASS with exit code 0 using elevated execution because sandboxed Vite can fail with `spawn EPERM`.
- Release executable built at `src-tauri/target/release/purewall.exe` (15,492,608 bytes; LastWriteTime 2026-07-05 00:08:43 local).
- MSI bundle built at `src-tauri/target/release/bundle/msi/PureWall_0.1.0_x64_en-US.msi` (5,943,296 bytes; SHA256 `0029A9AE80DF3C68BECB191D2416AD8E4F79756BF40B52EDF5DBCF66A93520A5`).
- NSIS bundle built at `src-tauri/target/release/bundle/nsis/PureWall_0.1.0_x64-setup.exe` (4,194,584 bytes; SHA256 `04B57E1B968F6D95C69099C3DE125F99F7D502CC326DCE1073EE7E48C64F850D`).
- Post-build `Get-Process cargo,rustc` returned no running processes.

### Warnings
- Vite/Rolldown emitted the existing upstream `@vueuse/core` pure-annotation warnings.
- Tauri emitted the existing non-blocking warning that bundle identifier `com.purewall.app` ends with `.app`, which is not recommended for macOS. Current target is Windows.

## 2026-07-05 — Preview responsiveness and separate collections

### Change goal
Fix slow/stuttery wallpaper preview loading, make multi-select visibly obvious, and add Apple Music-style collection creation without mixing collections into tags.

### Code scope
- `src-tauri/src/main.rs`: Changed `load_preview_image` to return cached preview paths immediately and enqueue bounded background preview generation on misses; added collection Tauri commands.
- `src-tauri/src/db.rs`: Added independent `collections` and `collection_wallpapers` tables, schema version 4, collection filters, batch assignment, cleanup, and a regression test proving tags and collections remain separate.
- `src/stores/wallpapers.ts`: Added `collections` state, collection loading/creation/assignment methods, `collection:<id>` filters, and null-aware preview loading.
- `src/components/Sidebar.vue`: Restored Tags as its own section and added a separate Collections section with independent creation/filtering.
- `src/components/Gallery.vue`: Added separate multi-select controls for tag assignment, collection assignment, and creating a new collection from selected wallpapers.
- `src/components/CurrentWallpaperPanel.vue`, `src/views/Home.vue`, and `src/styles.css`: Strengthened selected-card visuals, added stage loading fallback, loaded collections on startup, and styled the new controls.
- `docs/project-docs/DECISIONS.md` and `docs/project-docs/ARCHITECTURE.md`: Documented the separate collection model and async preview contract.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- `cargo fmt` PASS.
- `npx vue-tsc --noEmit` PASS.
- `cargo check` PASS using elevated execution because sandboxed Rust target writes failed with Windows `os error 5`.
- `cargo test` PASS (12/12), including `collections_are_separate_from_tags_and_filter_wallpapers`.
- `npm run build` PASS using elevated execution because sandboxed Vite/Rolldown failed with `spawn EPERM`; only the existing upstream `@vueuse/core` pure-annotation warnings were emitted.
- `git diff --check` PASS; only Git CRLF normalization warnings were printed.

### Unresolved items
- Rendered browser QA was not run: the Browser tool is unavailable, Playwright is not installed, and the PureWall workbench depends on Tauri IPC so ordinary Vite-browser rendering would not prove the real desktop flow.

## 2026-07-05 — Preview-first media work queue

### Change goal
Improve PureWall's behavior as wallpaper counts grow by replacing ad hoc thumbnail/preview background spawning with a bounded queue that prioritizes the currently selected preview over cold gallery thumbnail work.

### Code scope
- `src-tauri/src/media_queue.rs`: Added a test-covered media queue with preview-first ordering, newest-preview-first behavior, queued/running path de-duplication, thumbnail backlog capping, and shutdown wakeup.
- `src-tauri/src/main.rs`: Replaced per-image thumbnail/preview spawn logic with queue enqueue calls, started a small shared media worker pool during app setup, and shut the queue down with other background threads.
- `docs/project-docs/DECISIONS.md`: Added ADR-022 for the bounded preview-first media generation queue.
- `docs/project-docs/ARCHITECTURE.md`: Documented `media_queue.rs` and the updated two-tier image rendering flow.
- `docs/project-docs/AI_DIARY.md`: Added #media-queue-001 for the bounded-backlog lesson.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- RED: `cargo test media_queue` failed because a thumbnail was claimed before a queued preview.
- GREEN: `cargo test media_queue` PASS (5/5).
- RED: `cargo test thumbnail_queue_drops_oldest_jobs_when_over_capacity` failed because the oldest thumbnail was still claimed first.
- GREEN: `cargo test thumbnail_queue_drops_oldest_jobs_when_over_capacity` PASS.
- `cargo fmt` PASS.
- `cargo check` PASS using elevated execution because sandboxed Rust target writes can fail on Windows.
- `cargo test` PASS (17/17).
- `cargo clippy --all-targets -- -D warnings` PASS after replacing explicit `drop(state)` with lexical scope in the media worker loop.
- `npx vue-tsc --noEmit` PASS.
- `npm run build` PASS using elevated execution because sandboxed Vite/Rolldown can fail with `spawn EPERM`; only the existing upstream `@vueuse/core` pure-annotation warnings were emitted.
- `git diff --check` PASS for tracked touched files; only Git CRLF normalization warnings were printed.

### Unresolved items
- This pass improves derivative scheduling and backlog control. SQLite/page-level gallery pagination for very large libraries remains a separate follow-up.

## 2026-07-05 — Paginated gallery loading for large libraries

### Change goal
Keep PureWall responsive as wallpaper counts grow by avoiding full-library gallery loads into the WebView. The frontend should render virtual rows from the loaded slice, fetch the next SQLite page near the scroll boundary, and preserve search/filter/sort semantics across the full library.

### Code scope
- `src-tauri/src/db.rs`: Added `WallpaperPage` and `get_wallpapers_page(filter, sort, search, offset, limit)` with bounded limits, total counts, tag/collection filters, escaped SQLite LIKE search across path/title/source/tags, and regression tests.
- `src-tauri/src/main.rs`: Exposed the paginated query as a Tauri command.
- `src/stores/wallpapers.ts`: Changed gallery state to hold the currently loaded page plus `wallpaperTotal`, `hasMoreWallpapers`, and `loadMoreWallpapers`; search now debounces and reloads page 1 through SQLite instead of filtering only the frontend slice.
- `src/components/WallpaperGrid.vue`: Kept virtualized rows, triggered page append near the scroll bottom, and continued visibility-driven thumbnail requests only for newly visible paths.
- `src/components/Gallery.vue` and `src/styles.css`: Show the SQLite total count and a compact loading-more state.
- `docs/project-docs/ARCHITECTURE.md` and `docs/project-docs/DECISIONS.md`: Documented the paginated implementation under accepted ADR-018.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- RED: `cargo test wallpaper_page_returns_bounded_slice_and_total_count` failed before implementation because `get_wallpapers_page` did not exist.
- GREEN: `cargo test wallpaper_page_returns_bounded_slice_and_total_count` PASS.
- `cargo test wallpaper_page_searches_metadata_and_escapes_like_wildcards` PASS.
- `cargo test` PASS (19/19).
- `npx vue-tsc --noEmit` PASS.
- `cargo check` PASS using elevated execution because sandboxed Rust target writes failed with Windows `os error 5`.
- `cargo clippy --all-targets -- -D warnings` PASS.
- `npm run build` PASS using elevated execution because sandboxed Vite/Rolldown failed with `spawn EPERM`; only the existing upstream `@vueuse/core` Rolldown pure-annotation warnings were emitted.
- cargo fmt --check PASS.
- git diff --check PASS for touched tracked files; only Git CRLF normalization warnings were printed.

### Unresolved items
- `set_wallpaper_folder` still returns a full vector from Rust after initial source-folder scanning; the frontend now ignores that vector and reloads page 1, but the command itself can still be slimmed in a later backend API cleanup.
- Rendered desktop QA was not run in this pass; verification covered Rust behavior, TypeScript, clippy, and production frontend build.

## 2026-07-05 — Slim source-folder command return payload

### Change goal
Finish the previous pagination follow-up by removing the remaining full-library payload from the initial source-folder flow. `set_wallpaper_folder` should scan/import the selected folder, return lightweight counts, and let the frontend reload page 1 through the paginated gallery command.

### Code scope
- `src-tauri/src/main.rs`: Added `import_images_into_database` helper and changed `set_wallpaper_folder` from `CommandResult<Vec<WallpaperEntry>>` to `CommandResult<ImportResult>`, removing the post-scan `db.get_all_wallpapers()` call.
- `src/stores/wallpapers.ts`: Typed the `set_wallpaper_folder` invoke as `ImportResult` while continuing to refresh the visible gallery through `loadWallpapers()`.
- `docs/project-docs/ARCHITECTURE.md`: Documented the lightweight source-folder payload.
- `docs/project-docs/AI_DIARY.md`: Added the pagination/source-folder payload lesson.

### Verification evidence
- Pre-flight `cargo check` PASS.
- Pre-flight `npx vue-tsc --noEmit` PASS.
- RED: `cargo test import_images_into_database_returns_counts_without_loading_wallpapers` failed before implementation because `import_images_into_database` did not exist.
- GREEN: `cargo test import_images_into_database_returns_counts_without_loading_wallpapers` PASS.
- `cargo test` PASS (20/20).
- `npx vue-tsc --noEmit` PASS.
- `cargo check` PASS using elevated execution after the sandboxed run failed with Windows `os error 5`.
- `cargo clippy --all-targets -- -D warnings` PASS.
- `npm run build` PASS using elevated execution; only the existing upstream `@vueuse/core` Rolldown pure-annotation warnings were emitted.

## 2026-07-12 — Progressive active-preview pipeline, phases A–B

### Change goal
Restore and prepare the real current wallpaper before gallery startup, make cold active previews progressive instead of blank, and eliminate same-source thumbnail/preview concurrency and duplicate decoding.

### Code scope
- `src-tauri/src/active_preview.rs` and focused tests: added bootstrap response construction and idempotent active-preview prewarming.
- `src-tauri/src/main.rs`: exposed `bootstrap_active_wallpaper`, prewarmed every successful wallpaper change, introduced one preview-reserved plus two general workers, and processed merged derivative needs.
- `src-tauri/src/media_queue.rs` and focused tests: changed de-duplication from `(kind, path)` to path-level queued/running state with need upgrades, pending work, active-preview priority, bounded thumbnails, and a preview-only consumer.
- `src-tauri/src/thumbnails.rs` and focused test: added a combined derivative path that decodes once and writes both requested JPEG outputs atomically.
- `src/stores/activeMedia.ts` and tests: added explicit progressive media state plus startup selection rules.
- `src/stores/wallpapers.ts` and `src/views/Home.vue`: bootstrap current wallpaper before page 1, retain its row outside the current page, use cached preview/thumbnail paths immediately, and remove the duplicate thumbnail request after a preview miss.
- `package.json` and `package-lock.json`: added Vitest and the `test:unit` command.
- `docs/superpowers/`, `DECISIONS.md`, and `ARCHITECTURE.md`: recorded the approved design, implementation plan, ADR-023, and delivered architecture.

### Verification evidence
- RED/GREEN: bootstrap Rust tests 3/3, startup selection tests 2/2, prewarm tests 2/2, media queue tests 10/10, and combined one-decode derivative test passed after their expected missing-feature failures.
- `cargo fmt` PASS.
- `cargo test` PASS (31/31).
- `cargo check` PASS.
- `cargo clippy --all-targets -- -D warnings` PASS.
- `npm run test:unit` PASS (5/5).
- `npx vue-tsc --noEmit` PASS.
- `npm run build` PASS; only the existing upstream `@vueuse/core` Rolldown pure-annotation warnings were emitted.

### Unresolved items
- Windows WIC target-sized decoding, unified loaded-image swap across Inspector/Quiet Canvas, speculative warming, and cache stale-while-revalidate remain phases C–D of the approved plan.
- Rendered Tauri desktop QA has not yet been run for this intermediate checkpoint.

## 2026-07-13 — Progressive active-preview pipeline completion

### Change goal

Eliminate long blank waits and duplicate full-image decoding for current/selected wallpaper previews, while keeping startup bounded for large libraries and preserving PureWall's narrow asset-protocol scope.

### Code scope

- `src-tauri/src/active_preview.rs`: added current-wallpaper bootstrap compatibility handling, a shared active-preview prewarm boundary, and bounded speculative prewarming.
- `src-tauri/src/media_queue.rs`: changed scheduling to path-keyed merged needs, retained running-path upgrades, added a preview-reserved lane contract, and added a two-item speculative queue that active requests evict.
- `src-tauri/src/image_decoder.rs` and `src-tauri/src/windows_image_decoder.rs`: added target-dimension contracts, WIC source-transform/scaler decoding, explicit staged errors, and `image` fallback.
- `src-tauri/src/thumbnails.rs`: combined thumbnail/preview production behind one decode, added current/stale preview lookup, protected cleanup paths, and preserved 512/q88 plus 1440/q92 output profiles.
- `src-tauri/src/main.rs`: restored the persisted wallpaper before pagination, registered speculative warming, protected the persisted current cache entry at cleanup, and kept all cold decode/encode work outside SQLite locks and IPC responses.
- `src/stores/activeMedia.ts` and tests: added the generation-token state machine, delayed-thumbnail handling, idempotent preview commits, active-path selection, and two-candidate warmup selection.
- `src/stores/wallpapers.ts`: integrated shared preload-and-commit presentation state, startup derivative bootstrap, stale fallback refill, and post-ready speculative warming.
- `src/components/CurrentWallpaperPanel.vue` and `src/styles.css`: retained a real primary `<img>` and added a compact non-blocking high-resolution failure status.
- `src-tauri/src/preview_performance_tests.rs`: added a default-ignored release benchmark for real local samples.
- `docs/project-docs/ARCHITECTURE.md`, `PERFORMANCE_PREVIEW_REVIEW.md`, `CHANGELOG_AI.md`, and `AI_DIARY.md`: documented the final design, measurements, changes, and pitfalls.

### TDD evidence

- RED→GREEN: active media shows an arriving thumbnail, ignores stale preview generations, preserves fallback on error, and does not requeue an already committed preview.
- RED→GREEN: path-level queue merges/updates work, a reserved preview consumer rejects thumbnail/speculative work, speculative backlog stops at two, and a new active request removes stale queued speculation.
- RED→GREEN: combined derivatives decode once and produce both correctly bounded JPEGs.
- RED→GREEN: WIC directly decoded a 2400×1200 JPEG to 600×300; decoder fallback ran only after primary failure.
- RED→GREEN: stale preview lookup prefers current format when present, stale bootstrap returns the fallback while queueing regeneration, and cleanup preserves protected derivatives.
- RED→GREEN: speculative warmup skips cache hits and chooses/queues only two cold adjacent candidates.

### Verification evidence

- `cargo fmt -- --check`: PASS.
- `cargo test`: PASS, 41/41.
- `cargo check`: PASS.
- `cargo clippy --all-targets -- -D warnings`: PASS.
- `npm run test:unit`: PASS, 8/8.
- `npx vue-tsc --noEmit`: PASS.
- `npm run build`: PASS; only the existing upstream `@vueuse/core` Rolldown pure-annotation warnings were emitted.
- Release real-image benchmark: PASS for 24.8MP, 112.5MP, and 90.4MP JPEG samples.
  - 24.8MP: cold WIC 200.5ms; cache hit 0.244ms; forced `image` 506.8ms.
  - 112.5MP: cold WIC 854.0ms; cache hit 0.284ms; forced `image` 3154.3ms.
  - 90.4MP: cold WIC 1155.3ms; cache hit 0.580ms; forced `image` 2639.5ms.

### Unresolved items

- Real Tauri/WebView rendered QA with rapid selection and a cold cache remains to be performed; terminal verification cannot prove the exact visual transition or GPU memory behavior.
- The release benchmark records end-to-end latency and output size, not peak working-set memory. Add memory instrumentation before setting a formal hardware budget.
- WIC and `image` output bytes are not expected to be identical because their scaling implementations differ; both retain the same target bounds and JPEG quality contract.

## 2026-07-13 — Incremental watched-folder synchronization

### Change goal

Make filesystem changes in an active wallpaper folder update the SQLite-backed paginated library before the UI refreshes, while avoiding repeated whole-library work for noisy Windows file events.

### Root cause

`scanner::start_watcher` discarded every notify path and emitted an empty `folder-changed` event immediately. The frontend reloaded an unchanged SQLite page, so newly created wallpapers never appeared; one file write could also cause several redundant page reloads. The asynchronous folder-import path additionally started its watcher only after the initial scan, leaving a race window.

### Code scope

- `src-tauri/src/scanner.rs`: retained relevant changed paths, added a 250ms sorted/de-duplicated debounce batch, and limited directory subtree scans to create/rename events. Missing remove/rename paths still cause a refresh without scanning vanished trees.
- `src-tauri/src/main.rs`: added incremental changed-path inspection and SQLite upsert before `folder-changed`, kept file IO outside the database mutex, routed failures through `operation-failed`, and started both folder watchers before their initial scans.
- `src/stores/wallpapers.ts`: refreshed paginated wallpapers and library stats together after a synchronized folder event.
- `docs/project-docs/DECISIONS.md`: accepted ADR-024 for backend-owned incremental folder synchronization.
- `docs/project-docs/ARCHITECTURE.md`: documented the updated watcher/import event flow and preservation policy for temporarily missing rows.
- No registry command was executed and no system setting was changed.

### TDD evidence

- RED: `cargo test watched_path_reconciliation_imports_new_image_before_refresh` failed to compile because no backend watcher reconciliation function existed.
- GREEN: the same test proves a new valid image is canonicalized and persisted before refresh; `watcher_events_are_debounced_and_deduplicated` proves repeated paths produce one sorted batch.
- Regression: `ordinary_directory_changes_do_not_trigger_subtree_scans` proves ordinary directory metadata events do not rescan a subtree, while directory create/rename and missing paths remain relevant.

### Verification evidence

- Pre-flight `cargo check`: PASS.
- Pre-flight and final `npx vue-tsc --noEmit`: PASS.
- `cargo fmt -- --check`: PASS.
- `cargo test`: PASS, 44 passed and 1 ignored manual benchmark.
- `cargo check`: PASS.
- `cargo clippy --all-targets -- -D warnings`: PASS.
- `npm run test:unit`: PASS, 8/8; rerun outside the Windows sandbox after the known Vite `spawn EPERM` restriction.
- `npm run build`: PASS outside the Windows sandbox; only the existing upstream `@vueuse/core` Rolldown pure-annotation warnings were emitted.
- `git diff --check`: PASS; only line-ending normalization warnings were printed.

### Unresolved items

- `AppState` still owns one `FolderWatcher`; importing another folder replaces the previous watcher, so true multi-folder live monitoring needs a separate persisted watcher registry design.
- Missing files are preserved as recoverable SQLite rows by design. Paginated SQL counts are computed before the existing filesystem availability filter, so `total`/`has_more` can still over-count externally missing files until availability becomes explicit database state.

## 2026-07-13 — SQL-consistent source-file availability

### Change goal

Prevent missing wallpaper files from consuming paginated gallery slots or inflating `total`, `has_more`, statistics, collection counts, and rotation candidates, while preserving recoverable ratings, tags, and collection membership.

### Root cause

SQLite previously calculated `COUNT/LIMIT/OFFSET` first, then Rust removed missing paths with `Path::exists()`. A missing row could therefore occupy a page slot and distort counts even though no card was rendered. Deleting the row would lose user metadata, while statting every matching file for every count would make pagination filesystem-bound.

### Code scope

- `src-tauri/src/db.rs`: bumped schema version to 5, added indexed `file_available` state and legacy backfill, restored availability during upsert, added exact/directory-descendant invalidation, and filtered gallery queries, collection counts, statistics, and rotation candidates in SQL.
- `src-tauri/src/db.rs`: normalized Windows separators, case, and extended canonical path prefixes before descendant matching so notify paths and scanner paths reconcile reliably.
- `src-tauri/src/main.rs`: watcher inspection now separates available images from missing paths, marks removals unavailable before import/refresh, and skips stale scan entries whose source disappeared before upsert.
- `docs/project-docs/DECISIONS.md`: accepted ADR-025 for persisted source-file availability.
- `docs/project-docs/ARCHITECTURE.md`: documented the schema, migration, watcher, query, and recovery boundaries.
- No registry command was executed and no system setting was changed.

### TDD evidence

- RED: `unavailable_wallpapers_do_not_distort_page_stats_or_collection_counts` initially failed to compile because the database had no availability mutation boundary.
- GREEN: the same test proves a missing row remains recoverable but no longer affects page slots, `total/has_more`, current stats, or collection badges; a later upsert restores it.
- Migration regression: `migration_backfills_file_availability_from_disk` proves a version-4 database retains missing rows while backfilling them unavailable.
- Watcher regression: `watched_directory_removal_hides_descendants_without_deleting_metadata` exposed and then verified the Windows `\\?\` canonical-prefix normalization needed for removed directories.
- Race regression: `import_skips_source_removed_after_scan` proves stale scanner output cannot revive a file deleted before persistence.

### Verification evidence

- Pre-flight `cargo check`: PASS.
- Pre-flight and final `npx vue-tsc --noEmit`: PASS.
- Focused database availability and watcher tests: PASS.
- `cargo fmt -- --check`: PASS.
- `cargo test`: PASS, 48 passed and 1 ignored manual benchmark.
- `cargo check`: PASS.
- `cargo clippy --all-targets -- -D warnings`: PASS.
- `npm run test:unit`: PASS, 8/8; rerun outside the Windows sandbox after the known Vite `spawn EPERM` restriction.
- `npm run build`: PASS outside the Windows sandbox; only the existing upstream `@vueuse/core` Rolldown pure-annotation warnings were emitted.

### Unresolved items

- `AppState` still owns one `FolderWatcher`; importing another folder replaces the previous watcher. True multi-folder live monitoring still needs a persisted watcher registry and lifecycle design.
- File changes that occur outside an active watcher can still be caught by defensive source checks, but automatic persistent availability reconciliation for every independently imported file is not yet scheduled in the background.

## 2026-07-13 — Persisted multi-folder watchers, lifecycle review, and Windows repackage

### Change goal

Keep every imported wallpaper folder synchronized across the full application lifetime and across restarts, then perform a fresh logic review before producing new Windows installers.

### Root causes and review findings

- `AppState` stored one `Option<FolderWatcher>`, so importing a second folder dropped the first watcher immediately.
- Folder roots were not persisted, so all live synchronization disappeared after restart and changes made while PureWall was closed were never reconciled.
- Updating a persisted folder source without replacing the callback left SQLite and the live watcher carrying different source values.
- Replacing a watcher while retaining the SQLite mutex can deadlock: `FolderWatcher::drop()` joins its callback thread, while that callback may be waiting for the same database lock.
- Startup snapshot and asynchronous folder-import threads were not both registered in the shutdown thread table; WAL checkpoint could run before a late database write.

### Code scope

- `src-tauri/src/db.rs`: bumped schema version to 6, added the `watched_folders` table and query/upsert APIs, and added full-folder snapshot availability reconciliation.
- `src-tauri/src/main.rs`: replaced the single watcher with a canonical-root registry, persisted roots before use, restored watchers at startup, ran bounded tracked snapshot reconciliation, reused exact duplicates, and safely replaced source-changed watchers.
- `src-tauri/src/main.rs`: added one tracked background-worker boundary for startup reconciliation and asynchronous folder imports, stopped watcher callbacks during shutdown, drained watcher handles before joining workers, and moved WAL checkpoint after all database-using workers stop.
- `docs/project-docs/DECISIONS.md`: accepted ADR-026 for the persisted multi-folder watcher registry.
- `docs/project-docs/ARCHITECTURE.md`: documented storage, startup, replacement, missing-root, notification, and shutdown semantics.
- No registry command was executed and no system setting was changed.

### TDD and review evidence

- RED: `watched_folders_persist_and_snapshot_reconciliation_marks_missing_descendants` initially failed to compile because watched-folder persistence and snapshot APIs did not exist.
- GREEN: the database test proves duplicate roots update in place, persist across database reads, and mark absent descendants unavailable without deleting metadata.
- GREEN: `watcher_registry_retains_unique_roots_until_shutdown` proves exact root/source duplicates do not create a second handle, source changes replace the handle, unique roots coexist, and all handles drop at registry shutdown.
- Logic review found and corrected source/callback divergence, watcher-drop-under-database-lock deadlock, untracked import work, premature WAL checkpointing, and shutdown callbacks accepting new work.
- Brooks PR Review was applied to the high-risk watcher/database/shutdown diff. Its installed shared report references were missing, so the available review guide plus explicit Symptom → Source → Consequence → Remedy analysis was used as the documented fallback.

### Verification evidence

- Task Pre-Flight `cargo check`: PASS.
- Task Pre-Flight and final `npx vue-tsc --noEmit`: PASS.
- Focused watcher/database tests: PASS, 5/5.
- `cargo fmt -- --check`: PASS.
- `cargo test`: PASS, 50 passed and 1 ignored manual performance benchmark.
- `cargo check`: PASS.
- `cargo clippy --all-targets -- -D warnings`: PASS.
- `npm run test:unit`: PASS, 8/8 outside the Windows sandbox after the known Vite `spawn EPERM` restriction.
- `npm run build`: PASS; only the existing upstream `@vueuse/core` Rolldown pure-annotation warnings were emitted.
- `npm run tauri build`: PASS in 329.3 seconds; release Rust compilation completed and both x64 Windows bundles were freshly generated.

### Fresh package artifacts

- `src-tauri/target/release/purewall.exe` — 15,695,360 bytes — SHA-256 `EA089B51F1FE71A9DAE08BD30ABF7818B264CDD1311E712B6DAD895DBB8AB69A`.
- `src-tauri/target/release/bundle/msi/PureWall_0.1.0_x64_en-US.msi` — 6,017,024 bytes — SHA-256 `E10564D6964795FD98A0DAC68CAC63AB0FB4F85EB459A5097DC85B476BF36B75`.
- `src-tauri/target/release/bundle/nsis/PureWall_0.1.0_x64-setup.exe` — 4,246,887 bytes — SHA-256 `ABC1414DAD2EE8AAD57FCB0DFD90C63C1286DF75A060328C6860A195B943B313`.
- MSI/NSIS build timestamps are 2026-07-13 22:51 local time. No cargo/rustc/WiX/NSIS process remained after completion.

### Non-blocking release items

- The executable, MSI, and NSIS installer are currently `NotSigned`; configure an Authenticode certificate and CI signing step before distributing broadly to reduce Windows SmartScreen friction.
- The full working-tree diff is much larger than one reviewable PR and `main.rs` still owns several application concerns. Split future releases into focused commits/modules even though this watcher lifecycle slice has focused regression coverage.
- Exact duplicate roots are deduplicated, but nested parent/child roots can still both be watched and may produce redundant idempotent events. Add ancestor-collapse behavior if real libraries commonly import overlapping folders.
- The Tauri CLI warns that identifier `com.purewall.app` ends in `.app`; it does not block the Windows target but should be changed deliberately before adding macOS packaging.
- Installation/uninstallation was not executed automatically; packaging verification does not modify the user's installed application or system settings.

## 2026-07-13 — Thumbnail spinner root-cause repair and Windows repackage

### Change goal

Eliminate indefinitely spinning gallery thumbnails, reuse safe historical cache entries immediately, and produce fresh Windows packages for runtime verification.

### Root causes

- `WallpaperGrid` updated its previously-seen path set before a debounced request ran. Virtual-list/layout recalculation canceled the timer, and the next delta omitted still-visible paths from the canceled request permanently.
- The stable FNV cache-key migration left original path-only `DefaultHasher` JPEGs unreachable. The app-data audit found 822 non-empty derivative files, only 108 current thumbnail hits for 245 available wallpapers, and 25 live original path-only thumbnail hits.
- The known profiled `DefaultHasher` identities for 384/q68, 320/q64, 512/q76, and 512/q88 had zero live hits in the current library, so no unbounded or speculative legacy scan was added.

### Code scope

- `src/utils/thumbnailRequestScheduler.ts`: added a cancelable latest-visible-set scheduler that de-duplicates paths and submits the entire settled set.
- `src/utils/thumbnailRequestScheduler.test.ts`: added the debounce-reschedule regression test.
- `src/components/WallpaperGrid.vue`: removed pre-execution “previously loaded” bookkeeping, delegates visibility scheduling, and cancels pending work on unmount.
- `src-tauri/src/thumbnails.rs`: added current/stale thumbnail lookup with bounded fallback support for the previous profiled key and the original path-only key.
- `src-tauri/src/main.rs`: returns stale hits immediately and enqueues current-profile regeneration; active-wallpaper bootstrap can also use the fallback thumbnail.
- `src-tauri/src/thumbnail_cache_fallback_tests.rs`: covers profiled fallback, original path-only fallback, and current-cache precedence.
- `docs/project-docs/ARCHITECTURE.md`: documented latest-set scheduling and bounded stale-while-revalidate cache migration.
- No registry command was executed, no PureWall-owned registry entry was changed, and no system setting was changed.

### TDD and audit evidence

- Frontend RED: the focused Vitest initially failed because the scheduler module did not exist.
- Frontend GREEN: a reschedule from two to three visible paths produces one request containing all three paths.
- Backend RED: `thumbnail_lookup_returns_original_path_only_cache_as_stale_fallback` failed while the implementation only checked the profiled legacy identity.
- Backend GREEN: all three thumbnail lookup tests pass; current cache wins over every fallback.
- Real app-data audit: 245 available wallpapers; 108 current stable-key hits; 25 original path-only legacy hits; zero live hits for four known profiled legacy generations; 822 cache JPEGs with no zero-length files.

### Verification evidence

- Task Pre-Flight `cargo check`: PASS.
- Task Pre-Flight and final `npx vue-tsc --noEmit`: PASS.
- Focused scheduler Vitest: PASS, 1/1.
- Focused Rust thumbnail lookup tests: PASS, 3/3 after verified RED.
- `cargo fmt -- --check`: PASS.
- `cargo test`: PASS, 53 passed and 1 ignored manual performance benchmark.
- `cargo check`: PASS.
- `cargo clippy --all-targets -- -D warnings`: PASS.
- `npm run test:unit`: PASS, 9/9.
- `npm run build`: PASS; only the existing upstream `@vueuse/core` Rolldown pure-annotation warnings were emitted.
- `npm run tauri build`: PASS in 317.5 seconds; release compilation and both x64 Windows bundles completed.

### Fresh package artifacts

- `src-tauri/target/release/purewall.exe` — 15,701,504 bytes — SHA-256 `167CAC0B11172D0055F01A57FCCA5879D4B6E56B8BC230EBB7DA0B92CCCB90C0`.
- `src-tauri/target/release/bundle/msi/PureWall_0.1.0_x64_en-US.msi` — 6,017,024 bytes — SHA-256 `8BB4B30424850119170571894C7A910DC3D66DD33291AFB0B0A2E38F70F5EED3`.
- `src-tauri/target/release/bundle/nsis/PureWall_0.1.0_x64-setup.exe` — 4,248,380 bytes — SHA-256 `2515BB23D319DE4F9CB5CDDACAF9F49EC2EB8E3C9D83515F0C777E191963118C`.
- MSI/NSIS build timestamps are 2026-07-13 23:51 local time. All three artifacts are currently `NotSigned`; no cargo/rustc/WiX/NSIS process remained after completion.

### Unresolved items

- The fix is covered at the scheduler, cache lookup, queue, event, type, and production-build layers. A GUI runtime screenshot was unavailable because the installed tray instance exposed no top-level window handle; final perceived-load validation still requires launching the newly packaged build interactively.

## 2026-07-15 — Cargo build-artifact cleanup

### Change goal

Reclaim disk space consumed by reproducible Rust/Tauri build products without losing the latest Windows deliverables or touching source code, app data, registry state, or system settings.

### Inventory and safety checks

- `src-tauri/target`: 71,177 files, 28,236,768,241 bytes (26.298 GiB).
- `src-tauri/target-codex-check9GX0pr`: one 177-byte historical alternate-target marker.
- `src-tauri/$c`: four zero-byte registry-script experiment files already classified as residue by `.gitignore` and `AI_DIARY.md`.
- All deletion targets resolved inside `D:\Desktop\PureWall\src-tauri`, were ordinary directories rather than reparse points, and no cargo/rustc/PureWall/WiX/NSIS process was running.

### Actions

- Copied the latest portable executable, MSI, and NSIS setup to ignored `release-artifacts/PureWall-0.1.0-20260713/` before cleaning.
- Ran `cargo clean`, which removed 71,177 files and reported 26.3 GiB reclaimed.
- Removed only the exact `target-codex-check9GX0pr` and `$c` residue directories after boundary checks.
- Added `release-artifacts/` to `.gitignore`; no source file, SQLite database, application cache, registry entry, or system setting was changed.

### Verification evidence

- Task Pre-Flight `cargo check`: PASS before cleanup.
- Task Pre-Flight `npx vue-tsc --noEmit`: PASS before cleanup.
- Post-clean `src-tauri` inventory: 42 remaining source/configuration files, 818,774 bytes (0.78 MiB).
- `src-tauri/target`, `src-tauri/target-codex-check9GX0pr`, and `src-tauri/$c`: all absent.
- Preserved `purewall.exe`: 15,701,504 bytes; SHA-256 `167CAC0B11172D0055F01A57FCCA5879D4B6E56B8BC230EBB7DA0B92CCCB90C0`.
- Preserved MSI: 6,017,024 bytes; SHA-256 `8BB4B30424850119170571894C7A910DC3D66DD33291AFB0B0A2E38F70F5EED3`.
- Preserved NSIS setup: 4,248,380 bytes; SHA-256 `2515BB23D319DE4F9CB5CDDACAF9F49EC2EB8E3C9D83515F0C777E191963118C`.

### Operational note

- The next `cargo check`, `cargo test`, `cargo build`, or Tauri build will recreate `src-tauri/target` and recompile dependencies from a cold cache; this is expected after reclaiming the disk space.
- Preserved Windows artifacts remain `NotSigned` as recorded in the preceding packaging entry.

## 2026-07-15 — Thumbnail hot-path, bounded failure recovery, and weighted rotation correctness

### Change goal

Remove avoidable work before thumbnail cache hits, prevent failed thumbnails from spinning forever, and restore the accepted 2x liked-wallpaper contract without duplicating one image across independent displays.

### Root causes

- `load_thumbnails_batch` hydrated one full wallpaper row and its tags per path, then retained the shared SQLite mutex while derivative-cache filesystem IO ran.
- Async thumbnail validation/generation exposed only success events. A failure had no frontend state, retry budget, or terminal UI.
- The rotation selector duplicated liked paths only after a uniformly ordered candidate had already been accepted. Truncating to one erased the intended weighting; returning several paths exposed duplicates to independent displays.

### Code scope

- `src-tauri/src/db.rs`: added a bounded registered/available-path batch query and a deterministic-testable 2x ticket sampler without replacement.
- `src-tauri/src/main.rs`: validates thumbnail source files outside SQLite, holds the database mutex for one lightweight batch query only, releases it before cache/queue IO, and emits `thumbnail-generation-failed` from validation, registration, and generation failures.
- `src-tauri/Cargo.toml` / `Cargo.lock`: declared the already present lightweight `fastrand` crate as the direct production RNG for weighted ticket selection.
- `src/stores/wallpapers.ts`: added per-path retry attempts/timers/errors, 250ms/500ms automatic retry, stale-thumbnail preservation, asset-load error recovery, terminal scheduling exclusion, and explicit manual retry.
- `src/utils/thumbnailRetryPolicy.ts` and test: isolated and covered the bounded exponential-backoff policy.
- `src/components/WallpaperCard.vue` / `src/styles.css`: replaced terminal infinite spinners with an accessible Retry control.
- `docs/project-docs/ARCHITECTURE.md`: documented the batch lock boundary, failure event/state machine, direct RNG dependency, and without-replacement display selection.
- No registry command was executed, no PureWall-owned registry entry was changed, and no system setting was changed.

### TDD evidence

- Backend RED: the focused Rust test failed to compile because `registered_available_paths` and `sample_weighted_without_replacement` did not exist.
- Backend GREEN: 11 database tests passed, including batch filtering of unavailable/unregistered paths and exact two-ticket/unique-selection behavior.
- Frontend RED: focused Vitest failed because `thumbnailRetryPolicy` did not exist.
- Frontend GREEN: the retry-policy test proves 250ms and 500ms retries followed by a terminal `null` decision.

### Verification evidence

- Task Pre-Flight `cargo check`: PASS from a clean temporary Cargo target.
- Task Pre-Flight and final `npx vue-tsc --noEmit`: PASS.
- `cargo fmt -- --check`: PASS.
- `cargo test`: PASS, 55 passed and 1 ignored manual performance benchmark.
- `cargo check`: PASS.
- `cargo clippy --all-targets -- -D warnings`: PASS.
- `npm run test:unit`: PASS, 10/10 across 4 files.
- `npm run build`: PASS; only the existing upstream `@vueuse/core` Rolldown pure-annotation warnings were emitted.
- All Cargo verification used verified random directories under `C:\tmp`; `src-tauri/target` remained absent after cleanup.

### Unresolved items

- Terminal and pipeline behavior is covered by Rust, policy, type, lint, and production-build verification, but perceived thumbnail latency and the Retry card transition still need an interactive packaged-app run with a deliberately corrupted/unsupported source.
- The cache-hit path still updates derivative mtime on every successful lookup. A coarse touch interval can reduce write amplification, but it needs its own LRU semantics test before changing the existing cleanup contract.
- This change builds the frontend production bundle but does not create new MSI/NSIS installers; packaging should follow after interactive runtime validation.

## 2026-07-22 — Phase 2 playback completion ownership decision

### Goal

Replace the unreliable frontend action/FIFO event-outcome pairing discovered during Task 3 review with one deterministic completion channel per WebView.

### Decision and evidence

- Accepted ADR-027: a WebView caller applies `run_playback_action`'s typed outcome; the existing completion event is emitted to every target except that caller.
- Tray, CLI, timer, and other non-WebView entry points keep broadcasting the existing `auto-rotated`, `wallpaper-rating-changed`, and `pause-changed` events.
- Local Tauri 2.11.2 source confirms `Emitter::emit_filter` is available, so no dependency or parallel event name is required.
- Added AI diary entry `#playback-001` documenting why action/FIFO or timing inference is invalid without a correlation id.
- No registry command was executed, no PureWall registry entry changed, and no system setting changed.

### Unresolved item

- Task 3 implementation and adversarial tests must be revised to follow ADR-027 before Task 4 can start.

### ADR-027 implementation and verification

- `run_playback_action` now receives the invoking WebView window and filters the existing success event away from that caller; typed outcome is the caller's single completion channel.
- Tray, CLI, timer, and other non-WebView executor paths still broadcast the existing completion events. A widget caller excludes only the widget, so the main window still reconciles the event.
- Removed frontend action/FIFO intent inference. Local playback commands execute serially, and local outcomes plus independent external events share one serial reconciliation queue.
- Retained current rows outside the gallery page now participate in `activeWallpaper`, `activeMetadata`, and `activeShellMetadata`.
- No dependency, schema, registry, system-setting, or PureWall-X change was made.

### TDD and verification evidence

- Rust RED: focused entry-point tests failed because `should_deliver_playback_completion` did not exist.
- Frontend RED: focused routing tests exposed missing active metadata, swallowed external completion, absent local command serialization, and failure-recovery coverage.
- Rust focused GREEN: 8/8; full Rust: 69 passed / 1 ignored.
- `cargo fmt --check`, `cargo check`, and `cargo clippy --all-targets -- -D warnings`: PASS.
- Frontend focused GREEN: 12/12; full unit suite: 25/25 across 7 files.
- `npx vue-tsc --noEmit`: PASS.
- `npm run build`: PASS with only the documented upstream Rolldown annotation warnings.

### Status

- The prior ADR-027 implementation item is resolved in this task delta; independent Task 3 re-review remains the next gate before Task 4.

### ADR-027 listener-target correction

- Final Task 3 review found that global two-argument `listen(...)` calls register `EventTarget::Any` in the locked `@tauri-apps/api` version.
- Locked Tauri 2.11.2 accepts `Any` listeners before evaluating `emit_filter`, so the first ADR-027 implementation did not actually exclude the invoking main window.
- ADR-027 now requires playback completion listeners to use an explicit current-`WebviewWindow` target; `#playback-002` records the framework-specific pitfall.
- The existing caller-label predicate, typed outcome ownership, command queue, reconciliation queue, current-row metadata, and Widget behavior remain valid; only the production listener target contract needs correction.
- No registry command was executed, no PureWall registry entry changed, and no system setting changed.

### Required verification

- Focused frontend tests must assert exact listener names and `{ target: { kind: "WebviewWindow", label } }` options, with no playback completion listener left at `Any`.
- Focused Rust predicate tests, full frontend unit tests, `vue-tsc`, production build, Rust tests, check, and clippy must remain green after the correction.
- Task 4 remains gated until independent review confirms caller exclusion is effective with the real listener target.

### ADR-027 listener-target correction implementation

#### 代码范围

- `src/stores/wallpapers.ts`：从 `getCurrentWebviewWindow()` 读取当前 label，并仅为 `pause-changed`、`auto-rotated` 与 `wallpaper-rating-changed` 传入 `{ target: { kind: "WebviewWindow", label } }`。
- `src/stores/wallpaperCommandRouting.test.ts`：mock `main` label，断言三项真实 `listen` 调用均包含精确第三参且未默认注册；外部 completion 回调覆盖保留。

#### 验证证据

- RED：新增 listener target 断言在旧双参注册下失败。
- Focused GREEN：12/12 PASS；完整前端：7 files / 25 tests PASS；`vue-tsc` 与 build PASS。
- Rust final gate：fmt/check/clippy PASS；69 passed / 1 ignored。

#### 状态

- C-1/I-1 correction resolved：调用窗口的 playback listener 是 label-bearing target，可由 ADR-027 的 caller filter 排除；非 playback 全局 listener 与后端 emitter/queue 均未改动。

## 2026-07-28 — Phase 2 playback loop completion gate

### Change goal

- Recorded the completed Phase 2 playback-loop contract and final verification for branch `codex/purewall-phase-2` from merge base `9a1a2fa`; Phase 1 remains completed and merged at `b61353a`.
- The authoritative `9a1a2fa..27e855d` range contains 12 commits: 11 before Task 6 — `630865d`, `180af9b`, `994194c`, `d1d4403`, `85d7075`, `11ad4a7`, `75217dd`, `3fabfc7`, `3dc7b78`, `f97831b`, and `3350908` — plus the Task 6 documentation baseline `27e855d docs(phase2): record playback loop completion`.

### Commit-ledger correction

- The next `docs(phase2): correct commit ledger` commit is an administrative self-reference correction. It is deliberately not counted in the 12-item task/fix/docs ledger through `27e855d`, because a commit cannot truthfully inventory its own then-unknown SHA.

### Coverage and durable behavior

- Task 1 established the stable pure action dispatcher. Task 2 routed main/window compatibility facades, tray, widget, CLI/context menu, and timer through the real executor. Task 3 routed main-window controls through the typed command while retaining path-targeted gallery ratings and widget-visible failure feedback.
- ADR-027 gives a WebView caller its typed outcome and excludes that caller from the corresponding legacy completion event; production listeners explicitly target their `WebviewWindow` label. `auto-rotated`, `wallpaper-rating-changed`, `pause-changed`, and `operation-failed` remain the event contract.
- Task 4 covers restart restoration of current path, manual pause, bounded interval, and valid display mode. Task 5 characterizes independent-display selection uniqueness and one `play_events` record per successful display application.

### Completion gate — 2026-07-28

- `cargo fmt -- --check` — PASS.
- `cargo check` — PASS.
- `cargo clippy --all-targets -- -D warnings` — PASS.
- `cargo test` — PASS: 73 passed, 0 failed, 1 ignored (the ignored test is the explicit manual performance benchmark).
- `npx vue-tsc --noEmit` — PASS.
- `npm run test:unit` — PASS: 7 files, 25 tests.
- `npm run build` — PASS: 158 modules transformed; only the existing third-party `@vueuse/core` Rolldown `INVALID_ANNOTATION` warnings remain.
- `git diff --check` — PASS before the documentation update; the scoped documentation diff is checked again before commit.

### Safety and unresolved items

- No registry command was executed; no PureWall-owned registry entry, HKLM/policy/system setting, SQLite migration, wallpaper source file, or other external desktop state was changed.
- No browser harness/Playwright or interactive Tauri desktop QA was available or started. Main/window action wiring, empty-library failure feedback, widget-visible failure rendering, and 800x600/control reachability therefore remain manual runtime QA items; they are not represented as PASS.
- Task 4 lifecycle tests cover persisted values and malformed-value normalization, not a real Windows desktop restore with a source path that has become invalid between runs. Task 5's year-statistics assertion uses the current UTC year and can theoretically straddle a UTC year boundary. Both are non-blocking test-boundary notes for follow-up coverage.

### Status

- Phase 2 implementation and static/unit completion gate are complete. Whole-branch final review remains pending.

## 2026-07-28 — Phase 2 final whole-branch review fixes

### Change goal and architecture gate

- Resolved the final review's complete `0 Critical / 2 Important / 1 Minor` set without leaving an Important finding.
- Accepted ADR-028 before production changes. It defines per-display durable commit boundaries, distinguishes rating-target identity from current transitions, and makes post-commit completion delivery best-effort.
- Baseline reviewed: `9df0ddd`.
- ADR commit: `fd35127 docs(adr): define playback commit boundaries`.
- Code/test commit: `43da191 fix(playback): preserve action-specific completion facts`.
- This documentation/report delta is committed separately as `docs(phase2): record final review fixes`; its SHA is reported after creation because a commit cannot truthfully contain its own unknown SHA.

### Finding fixes and TDD evidence

#### I-1 — partial independent-display success

- RED: `cargo test ... independent_display_partial_failure_commits_each_success_before_returning_error -- --nocapture` failed with Rust `E0432` because the requested production seam `apply_independent_display_assignments_with` did not exist.
- GREEN: the production independent-mode path now calls that seam. With A apply success and B apply failure, the test proves two attempts, original B platform error propagation, persisted current=A, exactly one yearly play/history row for A, and no B history.
- Focused result: 1 passed / 74 filtered at the first GREEN run; final entrypoint suite 10/10.

#### I-2 — stale rating identity treated as current transition

- RED: focused Vitest expected current B after delayed rating(A), but received A; logs also showed preview work being queued for A after B.
- GREEN: reconciliation branches on `outcome.action`. Next alone advances current/active media and reloads row/preview/metadata/Shell data; Like/Dislike patch only the target and refresh it only when still current; TogglePause changes only `isPaused`.
- Existing tests were updated with equal or stronger action-specific coverage rather than deleting assertions: Next covers path/current/current row/preview identity/metadata/Shell/stats; ratings cover target patch, stats/history, no transition, current-target refresh, and serialization; Pause retains its dedicated paused assertion.
- Focused result: `wallpaperCommandRouting.test.ts` 13/13.

#### M-1 — completion delivery failure after commit

- RED: `cargo test ... committed_next_remains_successful_when_completion_emit_fails -- --nocapture` failed with Rust `E0432` because the requested best-effort completion seam did not exist.
- GREEN: the real executor uses `finish_playback_action_with_best_effort_completion`; the test proves committed side effect observability, successful Next outcome/path, and captured emit failure without action failure.
- Production logs emit failures with `eprintln!`; no new event or dependency was added.

### Code and documentation scope

- `src-tauri/src/main.rs`: per-display apply/record/persist seam; centralized committed outcome emission; best-effort completion logging.
- `src-tauri/src/playback_entrypoint_tests.rs`: deterministic partial-display and post-commit emit regressions with no real COM.
- `src/stores/wallpapers.ts`: action-specific playback reconciliation.
- `src/stores/wallpaperCommandRouting.test.ts`: local/external delayed-rating, preview identity, metadata, rating, stats, history, pause, and queue regressions.
- `docs/project-docs/DECISIONS.md`, `ARCHITECTURE.md`, `AI_DIARY.md`, and this changelog: ADR and durable behavior/process memory.
- `.superpowers/sdd/phase-2-final-fix-report.md`: final finding matrix, commands, SHAs, safety, and self-review.

### Verification evidence

- Pre-flight `cargo check`: PASS.
- Pre-flight `npx vue-tsc --noEmit`: PASS.
- Focused Rust entrypoint suite: PASS, 10/10.
- Focused frontend routing suite: PASS, 13/13.
- `cargo fmt -- --check`: PASS.
- `cargo check`: PASS.
- `cargo clippy --all-targets -- -D warnings`: PASS.
- `cargo test`: PASS, 75 passed / 0 failed / 1 ignored manual performance benchmark.
- `npx vue-tsc --noEmit`: PASS.
- `npm run test:unit`: PASS, 7 files / 26 tests.
- `npm run build`: PASS, 158 modules transformed; only the previously documented third-party `@vueuse/core` Rolldown `INVALID_ANNOTATION` warnings remain.
- `git diff --check`: PASS before documentation; rerun on the final documentation diff before commit.

### Safety and runtime unresolved items

- No registry command was executed; no PureWall-owned registry entry changed; no HKLM, policy, Windows setting, context-menu mode, or autostart state changed.
- No SQLite schema/migration, dependency manifest/lockfile, selection weight, public Tauri command/event name, PureWall-X boundary, wallpaper source file, or Recycle Bin state changed.
- No real wallpaper apply, COM failure injection, Tauri desktop session, or WebView lifecycle failure was automated. Production-used seams provide deterministic coverage without changing the user's desktop; a user-approved bounded desktop smoke remains the runtime follow-up.
- Existing non-blocking residuals remain: missing source between real launches, UTC-year boundary test timing, main/widget/tray/CLI interactive behavior, and 800x600/scaling reachability.

### Status

- Final review findings I-1, I-2, and M-1 are resolved by automated regressions and the complete static/unit/build gate. No Critical or Important finding remains in self-review.

## 2026-07-28 — Phase 2 final review v2 minor closures

### Change goal and gate

- Closed the independent v2 review's remaining `0 Critical / 0 Important / 2 Minor` hygiene and project-memory findings from baseline `6dd83f4`.
- Read `.superpowers/sdd/phase-2-final-review-v2.md` in full and confirmed that no proposed ADR blocks the change.
- This delta changes test fixture ownership and append-only documentation only. No production behavior, ADR, architecture contract, dependency, or schema changed.

### M-1 — panic-safe partial-failure fixtures

- RED: the new unwind regression failed to compile with Rust `E0603` because the repository's existing `TempPlaybackFiles` RAII guard was private to `playback_state_tests`.
- GREEN: generalized that test-only guard to own any number of wallpaper paths, exposed only its constructor within the crate's test modules, and retained its existing database/WAL/SHM cleanup.
- The partial independent-display regression now constructs the guard before writing either image and no longer depends on a normal-end manual cleanup block. Rust drop order closes the later-created database before the earlier-created guard removes its files during normal return or unwinding.
- A real `catch_unwind` regression creates all five artifacts, deliberately unwinds, and asserts that both images plus the database, `db-shm`, and `db-wal` are absent afterward.
- Focused GREEN: the unwind cleanup regression 1/1 PASS; the original partial-display commit regression 1/1 PASS.

### M-2 — post-commit delivery project memory

- Appended independent diary entry `#playback-005`.
- The durable lesson states that once a playback action commits, completion emit/WebView delivery failures remain best-effort and observable but cannot be returned as action failure, because doing so invites unsafe retries and duplicate side effects.

### Code and documentation scope

- `src-tauri/src/playback_state_tests.rs`: reusable multi-image test-only `Drop` guard.
- `src-tauri/src/playback_entrypoint_tests.rs`: RAII ownership in the partial-failure test and executable unwind cleanup regression.
- `docs/project-docs/AI_DIARY.md`: append-only `#playback-005`.
- `docs/project-docs/CHANGELOG_AI.md` and `.superpowers/sdd/phase-2-final-minor-fix-report.md`: verification, safety, and closure record.

### Verification evidence

- Pre-flight `cargo check`: PASS.
- Pre-flight `npx vue-tsc --noEmit`: PASS.
- Focused RED: Rust `E0603`, existing cleanup guard not reusable across test modules.
- Focused GREEN cleanup regression: PASS, 1/1.
- Focused GREEN partial-display regression: PASS, 1/1.
- `cargo fmt -- --check`: PASS.
- `cargo check`: PASS.
- `cargo clippy --all-targets -- -D warnings`: PASS.
- `cargo test`: PASS, 76 passed / 0 failed / 1 ignored manual performance benchmark.
- `npx vue-tsc --noEmit`: PASS.
- `npm run test:unit`: PASS, 7 files / 26 tests.
- `npm run build`: PASS, 158 modules transformed; only the existing third-party `@vueuse/core` Rolldown `INVALID_ANNOTATION` warnings remain.
- `git diff --check`: PASS before this documentation append; the complete staged diff is checked again before commit.

### Safety and runtime unresolved items

- No registry command was executed; no PureWall-owned registry entry, HKLM/policy/system setting, context-menu registration, or autostart state changed.
- No real wallpaper apply, COM injection, Tauri desktop launch, WebView destruction, wallpaper-source mutation, or application database mutation was performed.
- The Rust regression touches only uniquely named system-temp fixtures and proves their cleanup during unwind.
- No real desktop QA was run. Existing user-approved runtime follow-ups from the v2 review remain unchanged and are not represented as PASS.

### Status

- Both v2 Minor findings are closed. Self-review has `0 Critical / 0 Important / 0 Minor` outstanding for this follow-up.

## 2026-07-28 — Phase 2 final unwind-test contract correction

### Change goal and review gate

- Closed the final incremental review's sole `0 Critical / 0 Important / 1 Minor` finding from baseline `97a92aa`.
- Read `.superpowers/sdd/phase-2-final-minor-review.md` in full and confirmed that no proposed ADR blocks this test/docs-only correction.
- No production behavior, ADR, architecture contract, dependency, schema, or public API changed.

### Finding — setup panic could masquerade as the intended unwind

- The previous cleanup regression created its guard and five artifacts inside `catch_unwind`, then accepted any `Err`. A failed setup write could therefore look identical to the deliberate cleanup panic, while absence checks could pass for files never created.
- RED: added an exact sentinel payload contract. The old test failed 0/1 because its captured `"exercise panic-safe temporary playback cleanup"` payload did not equal `"purewall-temp-playback-cleanup-sentinel"`.
- GREEN: the guard, five writes, and five precondition existence assertions now run outside `catch_unwind`.
- The catch closure contains only ownership transfer into `_files` and `panic_any(CLEANUP_PANIC_SENTINEL)`, so the guard is dropped specifically while unwinding the authenticated sentinel panic.
- After `expect_err`, the test accepts only `&str` or `String` payloads, requires an exact sentinel match, then asserts that both images plus the database, `db-shm`, and `db-wal` are absent.
- Renamed the regression to `temp_playback_files_remove_precreated_artifacts_during_expected_unwind` so setup and panic identity are explicit.

### Project memory and scope

- Appended independent diary entry `#testing-001`: `catch_unwind` proves only that some panic occurred; setup must fail outside the catch and the intended payload must be authenticated.
- `src-tauri/src/playback_entrypoint_tests.rs`: test-contract correction only.
- `docs/project-docs/AI_DIARY.md`, `CHANGELOG_AI.md`, and `.superpowers/sdd/phase-2-final-test-fix-report.md`: append-only memory and completion evidence.
- `TempPlaybackFiles`, production Rust, frontend code, and runtime behavior are unchanged.

### Verification evidence

- Pre-flight `cargo check`: PASS.
- Pre-flight `npx vue-tsc --noEmit`: PASS.
- Focused RED: expected sentinel assertion failed with old payload, 0 passed / 1 failed.
- Focused GREEN authenticated-unwind regression: PASS, 1/1.
- Focused GREEN original partial-display regression: PASS, 1/1.
- `cargo fmt -- --check`: PASS.
- `cargo check`: PASS.
- `cargo clippy --all-targets -- -D warnings`: PASS.
- `cargo test`: PASS, 76 passed / 0 failed / 1 ignored manual performance benchmark.
- `npx vue-tsc --noEmit`: PASS.
- `npm run test:unit`: PASS, 7 files / 26 tests.
- `npm run build`: PASS, 158 modules transformed; only the existing third-party `@vueuse/core` Rolldown `INVALID_ANNOTATION` warnings remain.
- `git diff --check`: PASS before this documentation append; the complete staged and whole-range diffs are checked again before and after commit.

### Safety and runtime QA

- No registry command/write, PureWall-owned registry mutation, HKLM/policy/system setting, context-menu registration, or autostart change occurred.
- No real wallpaper apply, COM injection, Tauri desktop launch, WebView destruction, wallpaper-source mutation, or application database mutation occurred.
- Setup uses uniquely named system-temp fixtures; setup failures now fail normally, and the authenticated unwind proves cleanup of all five artifacts.
- No real desktop QA was executed. Existing Windows integration and visual smoke items remain user-approved follow-ups and are not represented as PASS.

### Status

- The sole final-review Minor is closed. Self-review has `0 Critical / 0 Important / 0 Minor` outstanding for this test-contract follow-up.

## 2026-07-28 — Phase 3 local-library management design

### Change goal

- Converted the user-confirmed Phase 3 scope into a committed implementation design for PureWall only; PureWall-X remains explicitly deferred.
- Defined four closed implementation slices: 3A source lifecycle, 3B batch completion, 3C backup/restore, and 3D QA/docs/review.
- Established the safety invariant that source removal, relocation, rescan, and backup import never delete or move original wallpaper files.

### Design scope

- `docs/superpowers/specs/2026-07-28-purewall-phase-3-library-management-design.md`: source-state data model, remove/Retry/Relocate lifecycle, overlap protection, batch command contracts, versioned backup merge semantics, UI/error states, test strategy, implementation slices, and acceptance criteria.
- The design requires an accepted source-lifecycle ADR before production changes because SQLite commits and watcher replacement cannot form one transaction.
- The first 3A implementation item is the GitHub Windows canonical temp-root test correction. The local suite passes the affected test, while Actions run `30330372062` demonstrates the cross-runner alias risk.
- `AI_DIARY.md`: appended `#windows-path-002` so future tests canonicalize fixture roots through the same boundary as production.

### Pre-flight and baseline evidence

- Read `AI_DIARY.md` in full and reviewed the latest `CHANGELOG_AI.md` entries.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS.
- `npx vue-tsc --noEmit`: PASS.
- `DECISIONS.md`: no `proposed` ADR blocks the design-document change.
- `cargo test --manifest-path src-tauri/Cargo.toml`: PASS, 76 passed / 0 failed / 1 ignored manual benchmark.
- `npm run test:unit`: PASS, 7 files / 26 tests.

### Safety and unresolved items

- This delta changes documentation only. No production code, schema, dependency, app data, wallpaper source, Recycle Bin state, watcher, registry entry, context menu, autostart value, Windows setting, or system policy changed.
- The written specification still requires explicit user review before the implementation plan is authored.
- GitHub Windows CI is not represented as green: the known canonical-path fixture defect remains the first implementation item.

## 2026-07-28 — Phase 3A library-sources implementation plan

### Planning outcome

- The user approved the written Phase 3 design and requested execution.
- Split Phase 3 into independently testable subsystem plans as required by the planning workflow. This plan covers 3A source lifecycle only; 3B batch completion and 3C backup/restore remain separate later plans, followed by the 3D completion gate.
- Added `docs/superpowers/plans/2026-07-28-purewall-phase-3a-library-sources.md` with eight reviewable tasks: canonical CI fixture, ADR-029, source database/removal, relocation transaction, Tauri/watcher coordination, Pinia routing, Settings UI, and final QA/docs.
- Every production task includes RED → GREEN evidence, exact files/interfaces/commands, a scoped changelog update, and a focused commit.

### Pre-flight evidence

- Read `AI_DIARY.md` in full in bounded chunks and reviewed the latest `CHANGELOG_AI.md` entries.
- Read `FEATURE_PLAYBOOK.md` and the source/watcher sections of `ARCHITECTURE.md`.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS.
- `npx vue-tsc --noEmit`: PASS.
- `DECISIONS.md`: no `proposed` ADR blocks planning. Production source-lifecycle changes remain gated on accepted ADR-029 in Task 2.
- `rg --files` was denied by the documented Windows sandbox restriction `#tool-001`; the established `Get-ChildItem`/`Select-String` fallback completed repository mapping.

### Safety and status

- This delta is planning/documentation only. No production code, SQLite schema, dependency, watcher, app data, wallpaper file, Recycle Bin state, registry entry, autostart value, Windows wallpaper, or system setting changed.
- The isolated worktree was clean before plan creation.
- Inline execution is selected because the user requested immediate execution and the current collaboration policy does not authorize proactive subagent dispatch.

### 3A Task 1 — canonical watcher fixture

- Canonicalized the watched root before creating/removing the fixture so the root and stored child share production path provenance.
- RED evidence remains GitHub Windows run `30330372062`, where the removal regression expected page total `0` but received `1`; the old fixture passes locally and is therefore runner-alias dependent.
- Focused removal regression: PASS, 1/1.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`: PASS.
- Complete Rust suite: PASS, 76 passed / 0 failed / 1 ignored manual performance benchmark.
- GitHub Windows Actions is not represented as green until this branch is pushed and the workflow reruns.

### 3A Task 2 — source lifecycle ADR

- Accepted ADR-029 before production changes.
- SQLite commits, filesystem scans, and watcher handoff now have explicit non-transactional boundaries.

### 3A Task 3 — source database lifecycle

- Centralized Windows source-path identity in `paths.rs`.
- Migrated SQLite to schema 7 with `last_scan_at` and `last_error`.
- Added source summaries and overlap-safe keep/clear removal transactions; neither path touches original files.
- RED evidence: path tests failed on the missing shared helpers; database tests failed on the missing scan-state, summary, and source-removal APIs.
- GREEN evidence: path helper tests PASS (2/2), existing watcher-removal regression PASS (1/1), and complete database suite PASS (18/18).
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`: PASS.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS with no warnings.
- Removal fixtures use canonical temporary roots and real image files. KeepMetadata hides only exclusive rows; ClearMetadata deletes only exclusive metadata; both tests prove the original files remain on disk.

### 未解决项

- GitHub Windows Actions remains pending until the Phase 3 branch is pushed; no CI-green claim is made locally.

### 3A Task 4 — atomic source relocation

- Added relative-path relocation that preserves wallpaper IDs and metadata.
- New target files import, unmatched old rows remain unavailable, and path/source collisions roll back the complete transaction.
- Strengthened the regression contract to preserve ratings, custom titles, tags, collections, play history, and scan-state timestamps through an in-place ID update.
- Added a separate remaining-source overlap rejection test and canonical real-file fixtures.
- RED evidence: relocation tests failed on the missing records and `relocate_watched_folder` method.
- Focused relocation suite: PASS, 4/4.
- Complete database suite: PASS, 22/22.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` and `cargo check --manifest-path src-tauri/Cargo.toml`: PASS.

### 3A Task 5 — Tauri source commands and watcher handoff

- Added list/rescan/retry/remove/relocate commands with stable source error codes.
- Scans occur outside SQLite; watcher handles are started, taken, replaced, and dropped after database and watcher-registry locks are released.
- Post-commit watcher failures persist retryable source errors and return `watcher_warning` instead of misreporting database rollback.
- Existing Gallery Folder add/import and startup restore paths now persist sources before watcher startup, record complete-scan success, and retain source-level errors.
- Incremental callbacks carry their owning root/source and revalidate the current SQLite registration before writing, preventing queued stale callbacks from reviving removed or relocated paths.
- RED evidence: the initial boundary tests failed because `library_sources` and the exact watcher-take helper did not exist.
- Focused source lifecycle suite: PASS, 5/5.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`: PASS.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS with no warnings.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`: PASS.
- Complete Rust suite: PASS, 91 passed / 0 failed / 1 ignored manual performance benchmark.
- Tests use pure maps, temporary paths, and SQLite fixtures only; no real watcher, registry, wallpaper, Recycle Bin, or system-setting mutation was performed.

### 未解决项

- Real Tauri watcher/Retry/Relocate/Remove handoff QA and GitHub Windows CI remain pending; neither is represented as PASS.

### 3A Task 6 — Pinia source lifecycle

- Added typed source DTOs, row-level busy/error state, and exact Tauri routing for list, rescan, retry, removal preview, removal, and relocation.
- Source operations clear stale row errors before execution, retain actionable failure text, and always release row busy state in `finally`.
- Successful remove/relocate commits refresh source, gallery, and statistics state; watcher handoff warnings remain visible without presenting the committed mutation as a retryable failure.
- Folder watcher and import completion events now refresh source status alongside existing gallery state.
- RED evidence: the focused store suite failed 3/3 because the source lifecycle methods were absent.
- Focused source routing suite: PASS, 3/3.
- Complete frontend unit suite: PASS, 8 files / 29 tests.
- `npx vue-tsc --noEmit`: PASS.

### 未解决项

- The Settings source-management UI is still pending in Task 7.
- Real Tauri watcher/Retry/Relocate/Remove handoff QA and GitHub Windows CI remain pending; neither is represented as PASS.

### 3A Task 7 — Settings Library Sources UI

- Added source rows with Online/Offline/Scanning/Error labels, available/unavailable counts, last-scan details, persisted errors, and transient row errors.
- Added per-row Rescan/Retry, Relocate, and Remove controls; only the active row is disabled while its operation runs.
- Added folder-picker Add and Relocate flows while preserving the existing Gallery/TitleBar add-source shortcuts.
- Added an in-app, two-mode Remove confirmation that stays open on command failure and closes only after a successful mutation.
- Both removal choices state: `Neither option deletes, moves, or recycles original image files.`
- The confirmation surface uses semantic buttons, dynamic ARIA state, trapped keyboard focus, focus return, visible focus styles, and reduced-motion handling while reusing the existing PureWall tokens and Fluent icon set.
- RED evidence: the focused model suite failed because `librarySourceModel` did not exist.
- Focused presentation-model suite: PASS, 2/2.
- Complete frontend unit suite: PASS, 9 files / 31 tests.
- `npx vue-tsc --noEmit`: PASS.
- `npm run build`: PASS; only the already documented third-party `@vueuse/core` invalid PURE annotation warnings remain.

### 未解决项

- Bounded Windows desktop QA for source add/offline/retry/relocate/remove and 800×600 keyboard reachability remains pending; it is not represented as PASS.
- GitHub Windows CI remains pending until the Phase 3 branch is pushed.

### Phase 3A completion gate

- Source listing, Online/Offline/Scanning/Error status, rescan/retry, overlap-safe removal, and relative-path relocation are implemented.
- Original wallpaper files and Windows/registry state remain outside source lifecycle mutations.
- Updated `ARCHITECTURE.md` with schema 7, shared Windows path identity, transaction/watcher handoff boundaries, stale-callback protection, and the Settings/store ownership model.
- Corrected the older persisted-watcher addendum so it no longer contradicts accepted ADR-029: canonical source persistence precedes watcher startup.
- Complete automated verification results are recorded below with exact counts.
- Manual Windows QA is recorded as PASS only for steps actually executed; all unexecuted checks remain explicit residual items.
- Phase 3B batch completion and Phase 3C backup/restore remain separate, unimplemented follow-up slices.

### Fresh automated verification

- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`: PASS.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS with zero warnings.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`: PASS.
- `cargo test --manifest-path src-tauri/Cargo.toml`: PASS, 91 passed / 0 failed / 1 ignored manual release-mode performance benchmark.
- `npx vue-tsc --noEmit`: PASS.
- `npm run test:unit`: PASS, 9 files / 31 tests.
- `npm run build`: PASS, 162 modules transformed.
- Production build emitted only the two already documented third-party `@vueuse/core` invalid PURE annotation warnings.
- `git diff --check`: PASS.

### Manual and external verification

- Windows desktop QA steps 1–7 (Add/Offline/Retry/Relocate/Keep/Clear/800×600 keyboard reachability): NOT RUN. This turn did not receive new explicit authorization to launch the desktop app against an isolated test database, and the default runtime resolves the real application data location.
- GitHub Windows CI: PENDING until this local branch is pushed; no CI-green claim is made.
- No real wallpaper source, original image, application database, watcher, Windows wallpaper, Recycle Bin state, registry entry, autostart value, context menu, system policy, or other system setting was mutated during the completion gate.

### 未解决项

- Run the bounded isolated Windows desktop QA checklist before release acceptance.
- Push the branch and verify GitHub Windows CI.
- Plan and execute Phase 3B batch completion next; Phase 3C backup/restore follows separately.

## 2026-07-28 — Phase 3B batch-management completion

### Change goal and accepted boundary

- Completed the existing Gallery multi-select workflow for rating, hide/restore, tags, and collections without expanding PureWall into PureWall-X scope.
- Added Clear rating plus Add/Remove tag and Add/Remove collection actions while preserving the existing Recycle Bin delete, three-second confirmation, delayed commit, and Undo behavior.
- Reused accepted ADR-007 and ADR-021. No new architecture decision was required because tags, collections, and blacklist state retain their accepted normalized boundaries.
- Added the executable plan at `docs/superpowers/plans/2026-07-28-purewall-phase-3b-batch-management.md`.

### Backend command contract

- Added `src-tauri/src/batch_operations.rs` with a fixed 500-path upper bound, empty/blank rejection, order-preserving deduplication, rating validation, positive relation-ID validation, and the serialized `{ affected, refresh }` result.
- Updated rating and blacklist batches to return affected counts.
- Added transactional `batch_unassign_tag` and `batch_unassign_collection` database methods and Tauri commands.
- Tag and collection commands now validate that the related entity still exists before mutation.
- Every metadata batch validates all registered image paths while the database mutex is held, then performs its mutations in one SQLite transaction.
- Tauri command results provide explicit `wallpapers`, `stats`, and/or `collections` refresh hints.

### Store and UI behavior

- Added one Pinia batch runner with a shared busy state, exact backend refresh-hint routing, boolean success results, and a selected-path snapshot.
- Successful metadata mutations clear selection and refresh only the requested state; rejected commands preserve selection for retry.
- Repeated metadata actions are rejected while a batch command is pending.
- Added tag and collection removal routing while retaining the existing optimistic rating/hidden page reconciliation and hidden-item Undo notification.
- Added grouped Add/Remove actions to `CompactDropdown`, a pure relation-menu parser/model, Clear rating, and disabled states for pending batch work.
- Relation-menu values reset only after success, so a failed command remains selected for a direct retry.
- The compact menu uses semantic buttons, dynamic ARIA state, existing global focus rings, existing Fluent icons, and PureWall theme tokens. Responsive wrapping no longer applies overflow clipping to the popup menus.
- Updated `ARCHITECTURE.md` with the completed command vocabulary, 500-path boundary, transaction/result contract, and Pinia success/failure ownership.

### TDD evidence

- Batch boundary RED: four tests failed on the intentional normalization stub; the result-contract test then failed because the DTO types were absent.
- Batch boundary GREEN: 5/5 focused tests passed with no warnings.
- Database/command RED: compilation failed on the missing validation helpers, affected counts, entity-existence queries, and relationship-removal methods.
- Database/command GREEN: 12/12 focused batch tests passed, including a trigger-forced second-insert failure that proved the first tag insert rolled back.
- Pinia RED: 4/4 focused tests failed on missing removal methods, missing boolean failure result, and absent in-flight exclusion.
- Pinia GREEN: 4/4 focused tests passed; `npx vue-tsc --noEmit` passed.
- Menu-model RED: the focused suite failed because `batchRelationMenuModel` did not exist.
- Menu-model GREEN: 3/3 focused tests passed; `npx vue-tsc --noEmit` and the production build passed.

### Fresh full verification

- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`: PASS.
- `cargo test --manifest-path src-tauri/Cargo.toml`: PASS, 102 passed / 0 failed / 1 ignored manual release-mode performance benchmark.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS with zero errors and zero warnings.
- `npm run test:unit`: PASS, 11 files / 38 tests.
- `npx vue-tsc --noEmit`: PASS.
- `npm run build`: PASS, 163 modules transformed.
- Production build emitted the same two third-party `@vueuse/core` `INVALID_ANNOTATION` warnings already observed during Phase 3A; Vite generated the complete production assets.
- `git diff --check`: PASS before documentation update.

### Safety

- No registry entry, context menu, autostart value, Windows setting, system policy, wallpaper source, original image, application database, watcher, live Windows wallpaper, or Recycle Bin state was mutated during implementation or automated verification.
- Metadata commands change SQLite metadata only.
- Source-image deletion remains limited to the pre-existing explicit Recycle Bin command and was not invoked or rewritten in Phase 3B.

### 未解决项

- Bounded Windows desktop visual/keyboard QA for the expanded selection toolbar remains pending and is not represented as PASS.
- GitHub Windows CI remains pending until the branch is pushed.
- Phase 3C backup/restore remains the next implementation slice; Phase 3D final QA/docs/review follows.

## 2026-07-29 — Phase 3C backup/restore implementation plan

### Planning outcome

- Continued the approved Phase 3 sequence with 3C metadata backup/restore; PureWall-X remains deferred and Phase 3D remains the later completion gate.
- Added `docs/superpowers/plans/2026-07-29-purewall-phase-3c-backup-restore.md` with seven TDD tasks covering the bounded v1 schema, atomic export, preview, transactional merge, post-commit watcher reconciliation, Pinia routing, Settings confirmation UI, and final documentation/verification.
- Fixed the v1 ceiling at 32 MiB and kept database IDs out of the portable format; normalized paths and tag/collection names are the stable merge keys.
- Reused accepted ADR-029 for the database-commit-before-watcher boundary. No new database schema, dependency, or proposed architecture decision is required.

### Pre-flight evidence

- Read `AI_DIARY.md` completely in bounded chunks and reviewed the latest `CHANGELOG_AI.md`.
- Read the approved Phase 3 design, `FEATURE_PLAYBOOK.md`, relevant `ARCHITECTURE.md` sections, and current code ownership for SQLite settings, watched sources, Pinia, and Settings UI.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS.
- `npx vue-tsc --noEmit`: PASS.
- `DECISIONS.md`: no `proposed` ADR; ADR-029 is accepted.
- Branch/worktree: `codex/purewall-phase-3`, clean before planning, 15 commits ahead of `origin/main`.

### Safety and unresolved items

- This planning delta changes documentation only. It did not read or mutate the live PureWall database, original wallpapers, caches, watcher state, Recycle Bin, registry, autostart, context menu, Windows wallpaper, policy, or other system settings.
- Phase 3C implementation and automated verification are pending.
- Real isolated Windows desktop QA and GitHub Windows CI remain pending and will not be represented as PASS without execution.
- No merge or push is authorized by this task.

## 2026-07-29 — Phase 3C Task 1 bounded backup schema

### Change goal and implementation

- Added the portable `PureWall` schema-version-1 JSON envelope for sources, wallpaper metadata, tags, collections, and the allowlisted playback/UI settings.
- Added a 32 MiB pre-parse size gate plus fixed collection and per-wallpaper relation ceilings.
- Added stable `BACKUP_*` error codes for oversized input, invalid app identity, unsupported schema versions, and invalid data.
- Reused the existing local-path safety boundary and shared path-identity key. Existing paths are canonicalized; missing local absolute paths are retained after lexical normalization for offline-library recovery.
- Added validation for supported source kinds, RFC 3339 export timestamps, ratings, bounded text fields, duplicate identities/names, undefined relations, and allowlisted setting values.

### TDD and verification evidence

- RED: `cargo test --manifest-path src-tauri/Cargo.toml library_backup_tests -- --nocapture` failed because `crate::library_backup` did not exist.
- GREEN: the same focused command passes 7/7 tests.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`: PASS after applying standard formatting.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS with zero errors. It reports expected temporary dead-code warnings because Task 2/3 have not yet connected the schema module to production export/import commands.
- No new AI diary entry was added: the temporary `rg.exe` access denial is already captured by `#tool-001`.

### Safety and unresolved items

- No live database, source folder, original wallpaper, watcher, registry entry, autostart value, context menu, Windows wallpaper, policy, or system setting was read or mutated.
- Atomic export, preview/confirm merge, watcher reconciliation, Pinia routing, and Settings UI remain pending in Tasks 2–6.
- No merge or push is authorized by this task.

## 2026-07-29 — Phase 3C Task 2 atomic metadata export

### Change goal and implementation

- Added a read-transaction snapshot that exports watched sources, all wallpaper restoration metadata, tag/collection definitions and name-based associations, and only the approved settings.
- Sorted sources and wallpapers by the shared normalized path identity and sorted definitions/relations by name for deterministic output.
- Mapped an empty SQLite display title to JSON `null`; omitted play counts, last-played timestamps, play events, availability bookkeeping, maintenance settings, registry/autostart state, and other out-of-scope data.
- Added deterministic pretty JSON with a trailing newline and enforced the 32 MiB ceiling before returning serialized bytes.
- Added a bounded file reader that checks both metadata length and a capped read, protecting against files that grow after metadata inspection.
- Added same-directory operation-owned temporary files with `create_new`, write/flush/sync, and Windows `MoveFileExW` replacement using `MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH`.
- Failure cleanup removes only the exact temp file created by the current operation; oversized writes are rejected before the destination is touched.

### TDD and verification evidence

- RED: the focused test command failed on missing `BackupClientSettings`, `read_bounded`, `serialize_bounded`, `write_atomically`, and `Database::export_backup_snapshot`.
- First GREEN compile exposed four identical rusqlite block-tail `MappedRows` lifetime errors; the durable fix is appended as `AI_DIARY.md #rust-sqlite-001`.
- GREEN: `cargo test --manifest-path src-tauri/Cargo.toml library_backup_tests -- --nocapture` passes 11/11 tests.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`: PASS.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS with zero errors. Temporary dead-code warnings remain until Task 4 connects the export/import commands to the production Tauri invoke handler.

### Safety and unresolved items

- Tests used isolated temporary SQLite files and temporary backup destinations only; all fixtures were removed after each test.
- No live PureWall database, original wallpaper, source folder, watcher, registry entry, autostart value, context menu, Windows wallpaper, policy, or system setting was read or mutated.
- Preview/transactional merge, runtime watcher reconciliation, Pinia routing, Settings UI, full verification, and manual Windows QA remain pending.
- No merge or push is authorized by this task.

## 2026-07-29 — Phase 3C Task 3 preview and transactional merge

### Change goal and implementation

- Added a read-only import preview with source/wallpaper/tag/collection/setting counts, normalized-path conflict counts, missing-file counts, and a metadata-only restoration warning.
- Added one SQLite transaction for source reuse/addition, tag and collection reuse/creation, allowlisted backend settings, wallpaper user metadata, and complete relation-set replacement.
- Matching normalized wallpaper paths give backup priority only to rating, hidden state, custom title (including JSON `null` clearing), and tag/collection associations.
- Matching rows retain their local path spelling, source, hash, dimensions, file size, availability, play history, creation data, and other scanner/local-only fields.
- New wallpaper rows use backed-up scanner metadata and compute `file_available` from the current filesystem; missing files remain queryable metadata but do not enter visible playback pages.
- Existing names retain local colors, local-only rows/sources/definitions/settings remain untouched, and client-owned theme/workspace values are returned instead of being stored in SQLite.
- Result counts report actual state changes, so an identical second import returns zero additions/updates.
- Ambiguous duplicate normalized identities in local SQLite are rejected instead of guessed.

### TDD and verification evidence

- RED: the focused test command failed on missing `Database::preview_backup_import` and `Database::merge_backup`.
- GREEN: `cargo test --manifest-path src-tauri/Cargo.toml library_backup_tests -- --nocapture` passes 14/14 tests.
- Preview tests prove no source, wallpaper, or setting mutation.
- Merge tests prove conflict priority, null-title clearing, local scanner-field preservation, full relation replacement, name/source reuse, missing-file insertion, local-only retention, allowlisted settings, and zero-change second import.
- A deliberately corrupted normalized backup fails after definitions, sources, settings, and a wallpaper insert have begun; all changes roll back, including the earlier setting write.
- `cargo test --manifest-path src-tauri/Cargo.toml db::tests -- --nocapture`: PASS, 26/26 tests.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`: PASS.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS with zero errors. Temporary dead-code warnings remain until Task 4 production command registration.
- No new AI diary entry was required.

### Safety and unresolved items

- Tests used only isolated temporary SQLite databases, files, and folders and removed their fixtures.
- No live PureWall database, original wallpaper, watcher, registry entry, autostart value, context menu, Windows wallpaper, policy, or system setting was read or mutated.
- Tauri commands, post-commit watcher reconciliation, Pinia routing, Settings UI, full verification, and manual Windows QA remain pending.
- No merge or push is authorized by this task.

## 2026-07-29 — Phase 3C Task 4 Tauri commands and post-commit reconciliation

### Change goal and implementation

- Added `export_library_backup`, `preview_backup_import`, and `import_library_backup` and registered all three in the Tauri invoke handler.
- Export completes the SQLite read snapshot before serialization and destination I/O, writes atomically, and returns the final path and byte count.
- Preview performs a bounded file read, full schema revalidation, and read-only database comparison.
- Confirmed import re-reads and revalidates the selected file, commits through the mutable database boundary, and releases the database mutex before any runtime setting or watcher work.
- After commit, restored playback/focus values are applied directly to `AppState` without rotating or changing the Windows wallpaper.
- Persisted sources are reconciled through a pure injected watcher-start helper. Offline paths and watcher failures become warnings plus source-level persisted errors; they never turn a committed database result into an import failure.
- Added serialized export/import result DTOs with explicit `committed: true`, merge counts, client settings, and simultaneous warnings.
- Added stable `CommandError` mapping for all five backup codes: too large, invalid app, unsupported schema, invalid data, and write failure.

### TDD and verification evidence

- RED: the focused command failed on missing `reconcile_persisted_sources` and `backup_command_error`.
- GREEN: `cargo test --manifest-path src-tauri/Cargo.toml library_backup_tests -- --nocapture` passes 16/16 tests.
- The post-commit test first commits source metadata, injects both an offline source and a watcher-start failure, then proves the exported database snapshot remains byte-for-byte equivalent in content.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`: PASS.
- `cargo test --manifest-path src-tauri/Cargo.toml`: PASS, 118 passed / 0 failed / 1 ignored manual release-mode performance benchmark.
- Initial Clippy found one eight-argument test fixture helper; a zero-behavior refactor grouped rating/hidden state.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`: PASS with zero warnings.
- The interrupted approval transport and safe resume procedure are appended as `AI_DIARY.md #tool-002`.

### Safety and unresolved items

- Automated tests used only isolated temporary databases/files/folders and injected watcher failures; they did not start production watchers.
- No live PureWall database, source folder, original wallpaper, Windows wallpaper, registry entry, autostart value, context menu, policy, or system setting was read or mutated.
- Pinia routing, Settings UI, architecture/docs closeout, complete Phase 3C verification, manual Windows QA, and GitHub CI remain pending.
- No merge or push is authorized by this task.

## 2026-07-29 — Phase 3C Task 5 Pinia backup routing

### Change goal and implementation

- Added exact camelCase TypeScript DTOs for backup client settings, export results, previews, merge results, and committed import results.
- Added one shared `isBackupBusy`/`backupError` operation boundary for export, preview, and confirmed import.
- Overlapping backup operations are rejected before invoking Tauri; stale errors clear at operation start, actionable command messages persist on failure, and busy state always releases.
- Export routes `destination` plus `clientSettings` exactly and reports the final path through the existing notification surface.
- Preview routes only the selected path and never clears or prunes Gallery selection.
- Failed import retains Gallery selection and exposes the backend error without issuing refresh calls.
- Successful committed import refreshes library sources, the current Gallery page, tags, collections, stats, display mode, focus mode, and pause state through their existing owners.
- Import success and offline/watcher warnings are emitted as simultaneous success and informational notifications; warnings never reclassify a committed import as failed.

### TDD and verification evidence

- RED: `npx vitest run src/stores/libraryBackup.test.ts` failed 5/5 tests because the backup state and methods did not exist.
- GREEN: the same focused command passes 5/5 tests.
- `npx vue-tsc --noEmit`: PASS.
- `npm run test:unit`: PASS, 12 files / 43 tests.
- No new AI diary entry was required.

### Safety and unresolved items

- Tests mocked Tauri command/listener/window and notification boundaries; they did not open dialogs, write backup files, start watchers, or touch a real database.
- No live PureWall database, source folder, original wallpaper, Windows wallpaper, registry entry, autostart value, context menu, policy, or system setting was read or mutated.
- Settings export/preview/confirm UI, client theme/workspace application, documentation closeout, full verification, manual Windows QA, and GitHub CI remain pending.
- No merge or push is authorized by this task.

## 2026-07-29 — Phase 3C Task 6 backup Settings workflow

### Change goal and implementation

- Added `LibraryBackupSettings` immediately after Library Sources with native JSON export/import pickers, the stable `purewall-backup-v1.json` default, current theme/interface-mode export, and no new dependency.
- Added a read-only preview dialog for source, wallpaper, tag, collection, setting, new-metadata, overwritten-metadata, and missing-path counts.
- Confirmation is unavailable until preview succeeds. Failed preview/import remains actionable without claiming that database state or wallpaper files changed.
- A committed import keeps database success and offline/watcher warnings visible together, then applies only runtime-validated returned theme and interface-mode settings through their existing composable owners.
- Added explicit metadata-only safety copy before and after confirmation. The UI never says that original images were restored, copied, or modified.
- Reused the Library Sources dialog contract: semantic buttons, dialog focus, Escape, focus trap/return, visible focus rings, `role="alert"` errors, token-based surfaces, responsive stacking, and reduced-motion handling.
- Added official Fluent download/upload assets to the shared `AppIcon` map.
- Added a pure backup presentation model for stable count rows, zero/singular/plural missing-path copy, committed metadata summaries, and simultaneous warnings.

### Contract correction

- During Task 6 integration, the backup DTO/schema was found to accept `library | focus`, while PureWall actually persists `workbench | quiet`.
- Added a Rust RED test that failed with `unsupported workspaceMode: workbench`, then changed the Rust allowlist, TypeScript DTO, and existing fixtures to the real client vocabulary.
- Stale `library | focus` backup setting values are now rejected as `BACKUP_INVALID_DATA`; both actual modes are accepted and round-trip through export/import.
- The reusable lesson is appended as `AI_DIARY.md #backup-contract-001`.

### TDD and verification evidence

- Workspace-mode contract RED: `library_backup_tests::accepts_only_the_actual_workspace_mode_vocabulary` failed on the production `workbench` value.
- Workspace-mode contract GREEN: the focused Rust test passes 1/1; the focused Store test passes 5/5.
- Presentation-model RED: the focused Vitest suite failed because `backupPresentationModel` did not exist.
- Presentation-model GREEN: `npx vitest run src/components/backupPresentationModel.test.ts` passes 4/4.
- `npx vue-tsc --noEmit`: PASS.
- `npm run test:unit`: PASS, 13 files / 47 tests.
- `npm run build`: PASS, 169 modules transformed and production assets generated.
- The build emitted the same two third-party `@vueuse/core` `INVALID_ANNOTATION` warnings already tracked in this phase; they did not prevent output.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`: PASS after standard `rustfmt` corrected one long test assertion.
- `cargo test --manifest-path src-tauri/Cargo.toml`: PASS, 119 passed / 0 failed / 1 ignored manual release-mode performance benchmark.

### Safety and unresolved items

- Automated tests mocked Tauri dialogs/commands or used isolated Rust fixtures. No native picker was opened by tests and no production backup file was written.
- No live PureWall database, source folder, original wallpaper, watcher, Windows wallpaper, registry entry, autostart value, context menu, policy, or system setting was read or mutated.
- Bounded Windows desktop visual/keyboard QA for the new Settings workflow remains pending and is not represented as PASS.
- Phase 3C architecture/process closeout and the complete final gate remain Task 7; GitHub Windows CI remains pending until a branch is pushed.
- No merge or push is authorized by this task.

## 2026-07-29 — Phase 3C Task 7 architecture and verification closeout

### Documentation synchronization

- Added `ARCHITECTURE.md` section 26 with the implemented v1 schema and fixed ceilings, portable identity/allowlist rules, consistent snapshot and atomic-write boundary, preview/confirm boundary, transactional merge semantics, and post-commit runtime/watcher warning ownership.
- Updated the accepted Phase 3 design status: 3A, 3B, and 3C are complete on `codex/purewall-phase-3`; 3D remains pending.
- Clarified the portable setting vocabulary in the Phase 3 design, including client `workspaceMode: workbench | quiet`, and added the implemented Settings confirmation step to 3C.
- Reviewed `FEATURE_PLAYBOOK.md`; the implementation follows its existing feature-development/verification process and does not introduce a reusable workflow change, so no playbook edit was required.
- `AI_DIARY.md` required no additional Task 7 entry. The only new reusable Phase 3C implementation pitfall is already recorded as `#backup-contract-001`.

### Fresh final verification evidence

- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`: PASS.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS with zero errors and zero warnings.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`: PASS with zero warnings.
- `cargo test --manifest-path src-tauri/Cargo.toml`: PASS, 119 passed / 0 failed / 1 ignored manual release-mode performance benchmark.
- `npx vue-tsc --noEmit`: PASS.
- `npm run test:unit`: PASS, 13 files / 47 tests.
- `npm run build`: PASS, 169 modules transformed and production assets generated.
- The production build emitted two non-fatal third-party `@vueuse/core` `INVALID_ANNOTATION` warnings. They are unchanged dependency output, not PureWall compile errors.
- `git diff --check`: PASS after the final documentation update.
- `git status --short`: only the four intended Task 7 documentation files were modified before staging.

### Complete branch safety review

- Reviewed the full Phase 3 branch against `origin/main@199c7dc`: 33 changed files spanning the approved 3A source lifecycle, 3B batch management, 3C metadata backup/restore, tests, and process documentation.
- No Critical/Important safety violation was found in the added production paths.
- Original-wallpaper boundary: no Phase 3 production addition copies, moves, rewrites, recycles, or deletes original image files. Source remove/relocate affect metadata and watcher ownership; backup write/delete operations target only the user-selected destination and the operation-owned same-directory temp file.
- System boundary: no Phase 3 production addition invokes registry, autostart, context-menu, Windows wallpaper, policy, or other system-setting mutation.
- Secret boundary: backup export uses an explicit schema/setting allowlist and excludes application internals, logs, play history, registry/autostart state, credentials, tokens, and secrets.
- Bounds/identity boundary: backup input/output and every collection/relation/path/text field have fixed ceilings; batch paths are capped; local canonical/lexical Windows identity is shared across preview, merge, source overlap, and watcher callbacks.
- Transaction/lock boundary: backup merge and source/batch database changes are transactional; filesystem scans complete outside SQLite; watcher handles are taken/replaced/dropped after SQLite and watcher-registry guards are released; post-commit watcher failure stays a warning.
- The branch is intentionally large because it contains three approved Phase 3 slices. Task-level commits and focused tests make the history reviewable, but the independent final review required by 3D remains a separate completion gate.

### Safety and unresolved items

- No live Tauri backup import/export was run. Automated coverage used mocks and isolated temporary databases/files only.
- Real Windows desktop QA for source lifecycle, native backup pickers, preview/confirm keyboard flow, warning presentation, 800×600 layout, and display scaling is **NOT RUN**.
- GitHub Windows CI is **NOT RUN** for the unpushed branch.
- Phase 3D manual QA, independent final review, merge, push, and PureWall-X work remain pending and are not represented as complete.
- No merge or push is authorized by this task.

## 2026-07-29 — Phase 3D final QA/review implementation plan

### Planning outcome

- Interpreted the user's “continue” instruction as authorization to proceed with Phase 3D local QA/review while preserving the clean feature branch and leaving merge/push untouched.
- Added `docs/superpowers/plans/2026-07-29-purewall-phase-3d-final-qa.md` with five evidence-owned tasks: local CI/release packaging, bounded browser QA, isolated real-Tauri source/backup smoke, independent full-branch review, and status/documentation closeout.
- Browser viewport/zoom stress is explicitly separated from real Windows DPI. With no committed screenshot baseline, visual regression remains INCONCLUSIVE even when inspected screenshots show no defect.
- The real desktop smoke uses a new `C:\tmp` fixture root, process-scoped `APPDATA`, and an isolated WebView profile. It never targets the user's real database/library and never invokes registry, autostart, context-menu, Recycle Bin, Windows wallpaper, policy, or display-setting commands.
- The independent review follows `requesting-code-review`: one fresh read-only reviewer receives only the Phase 3 requirements and `origin/main@199c7dc..HEAD`; confirmed Critical/Important findings require focused RED/GREEN correction before closeout.

### Pre-flight evidence

- Read `AI_DIARY.md` completely in two bounded chunks and reviewed the latest `CHANGELOG_AI.md`.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS with zero errors.
- `npx vue-tsc --noEmit`: PASS with zero errors.
- `DECISIONS.md`: no `proposed` ADR; ADR-029 is accepted.
- Worktree `codex/purewall-phase-3` was clean at Phase 3D planning start.

### Safety and unresolved items

- This planning change modifies documentation only.
- Local release packaging, browser QA, isolated real-Tauri QA, independent review, and final evidence are pending.
- GitHub Windows CI remains NOT RUN without push authorization.
- No merge, push, registry operation, system-setting change, real app-data access, or real wallpaper mutation is authorized by this plan.

## 2026-07-29 — Phase 3D Task 1 local CI/release gate and dependency hardening

### Gate outcome and security correction

- Ran the local CI/release mirror from the clean `codex/purewall-phase-3` worktree.
- The initial `npm audit --omit=dev` found a high-severity PostCSS source-map path traversal/file-disclosure advisory against locked `postcss 8.5.15`.
- Raised the direct PostCSS floor to `^8.5.25`; the lockfile now resolves `postcss 8.5.25` and its safe `nanoid 3.3.16` dependency.
- A full audit then exposed a separate high-severity development-toolchain advisory through `vue-tsc 2.2.12 -> @vue/language-core -> minimatch -> brace-expansion`.
- Verified the official `vue-tsc 3.3.8` peer contract (`typescript >=5.0.0`) against the installed TypeScript 5.9.3 and Vue 3.5.35, then raised the direct `vue-tsc` floor to `^3.3.8`.
- Regenerated the lockfile without `--force`, performed a fresh `npm ci`, and retained no vulnerable compatibility override.
- The reusable audit/re-resolution pitfall is appended as `AI_DIARY.md #dependency-security-001`.

### Fresh verification evidence

- `npm ci`: PASS, 177 packages installed; 178 packages audited; 0 vulnerabilities.
- `npx vue-tsc --noEmit`: PASS with `vue-tsc 3.3.8`.
- `npm run test:unit`: PASS, 13 files / 47 tests.
- `npm run build`: PASS, 169 modules transformed and production assets generated.
- `npm audit`: PASS, 0 vulnerabilities across the complete dependency tree.
- `npm audit --omit=dev`: PASS, 0 production vulnerabilities.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`: PASS.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS with zero errors.
- `cargo test --manifest-path src-tauri/Cargo.toml`: PASS, 119 passed / 0 failed / 1 ignored manual release benchmark.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`: PASS with zero warnings.
- `cargo build --manifest-path src-tauri/Cargo.toml --release`: PASS.
- Final `npm run tauri build`: PASS after the dependency corrections; MSI and NSIS bundles were regenerated.
- `PureWall_0.1.0_x64_en-US.msi`: 6,189,056 bytes; SHA-256 `EEFF41453A6E32726470F93E611F2BBD81AEB2B67A6775BE718D6406B8BCEF00`.
- `PureWall_0.1.0_x64-setup.exe`: 4,388,103 bytes; SHA-256 `EE116580F4D4188FDEF0CAEFC046D8541AEE9F278A7132AC987CCF985AD7D63B`.
- No cargo, rustc, candle, light, or makensis process remained after bundling.
- Vite/Tauri continued to emit the two known non-fatal third-party `@vueuse/core` annotation warnings and the macOS-only `.app` identifier recommendation; neither blocked Windows output.

### Safety and unresolved items

- The only source changes are dependency constraints and their lockfile resolution; no application behavior, database, registry, autostart, context-menu, Windows wallpaper, policy, or system setting was invoked.
- Browser layout/keyboard QA, isolated real-Tauri smoke, independent branch review, and Phase 3D closeout remain pending.
- GitHub Windows CI remains NOT RUN because the branch is not pushed.
- No merge or push is authorized by this task.

## 2026-07-29 — Phase 3D Task 2 browser layout, theme, and keyboard QA

### Browser boundary and discovered defects

- Started one owned Vite server on `127.0.0.1:1420` as PID 43972 and opened the named Playwright session `purewall-phase3d`.
- A plain browser load correctly exposed the native boundary: `TitleBar.vue` called `getCurrentWindow()` without Tauri metadata, the Vue setup failed, and the accessibility snapshot was empty.
- Injected a temporary, init-time, read-only Tauri bridge mock from `C:\tmp`; it returned project-standard empty fixtures and event/window metadata. All browser results below are explicitly **mocked shell QA**, not native command evidence.
- Browser QA found that clicking Settings in an empty library only changed the sidebar active state. `AppShell.vue` suppressed every `InspectorPanel` unless a wallpaper existed or the library was loading, so first-run users could not reach Settings, Displays, Command Line, Advanced, or Insights.
- Added an SSR component RED test for empty-library Settings. It failed because both `InspectorPanel` and `inspector-is-open` were absent.
- Replaced the duplicated condition with one computed `showInspector`: system sections remain available without wallpapers while the Library inspector retains its existing content/loading requirement.
- At 800×600 the repaired component still measured 0×0 because the existing `max-width: 1180px` rules hid every inspector shell. Added a token-based `system-inspector-shell` overlay drawer without re-enabling the Library inspector.
- The first overlay attempt rendered at only 26px because an absolutely positioned grid child retained the collapsed implicit third-column containing block. A live browser hypothesis test proved `grid-column: 1 / -1`; the final CSS pins the overlay to the full workbench grid before sizing it.
- Escape initially left the overlay open. Added a system-panel-only Escape handler with dialog guarding, listener cleanup, and focus return to `#main-workspace`.

### Verification evidence

- Focused RED: `npx vitest run src/components/appShell.test.ts` failed because the empty-library Settings inspector and layout class were absent.
- Focused GREEN: the same command passes 1/1.
- `npm run test:unit`: PASS, 14 files / 48 tests.
- `npx vue-tsc --noEmit`: PASS.
- `npm run build`: PASS, 169 modules transformed; the two known third-party `@vueuse/core` annotation warnings remain non-fatal.
- `git diff --check`: PASS.
- Mocked browser console after bridge injection and after final keyboard QA: 0 errors / 0 warnings.
- 1200×800 dark Settings: complete panel structure rendered; screenshot inspected with no overlap or horizontal overflow.
- 800×600 dark Settings: final shell rect `left=426, right=786, width=360, top=86, bottom=574`; document `scrollWidth=800`.
- 800×600 light Settings: `data-theme=light`, computed `color-scheme=light`, document `scrollWidth=800`; screenshot inspected with readable text, borders, active states, and controls.
- 375×667 dark Settings: shell rect `left=24, right=367, width=343, top=80, bottom=641`; document `scrollWidth=375`.
- Keyboard GREEN at 800×600: Enter changed Light to Dark; Tab and Shift+Tab produced a `2px solid` focus outline; Escape removed the system drawer and focused the `main-workspace` element.
- CSS zoom 1.25/1.5 was run and then restored to 1. The root-level stress enlarged visual coordinates beyond the 800px viewport (`right=982.5` and `right=1179`) while media queries still evaluated at 800px, so this artificial stress is recorded as **CLIPPED / not equivalent to Windows DPI**, not converted into a DPI PASS.
- No committed screenshot baseline exists; visual regression comparison remains **INCONCLUSIVE** even though the captured screenshots were manually inspected.
- Browser artifacts were preserved outside the source tree at `C:\tmp\purewall-phase3d-browser-evidence-20260729-2350`.
- The Playwright session was closed, the owned Vite PID 43972 was stopped after listener ownership verification, and port 1420 was confirmed free.

### UI/UX guidance applied

- Used the project `ui-ux-pro-max` workflow for the responsive correction.
- Preserved existing colors, typography, SVG icons, panel tokens, focus styling, reduced-motion behavior, and Library inspector policy.
- The new drawer avoids horizontal scrolling at 800px and 375px and retains a visible close control plus Escape/focus-return behavior.

### Safety and unresolved items

- Browser IPC, dialogs, filesystem, database, watcher, backup, playback, autostart, and window actions were mocked; no native command or real user data was touched.
- The CSS zoom stress result does not substitute for real Windows DPI testing. Real window DPI remains owned by Task 3.
- Isolated real-Tauri source/backup smoke, independent branch review, final documentation closeout, and GitHub Windows CI remain pending.
- No merge or push is authorized by this task.

## 2026-07-30 — Phase 3D Task 3 native smoke stopped at the data-isolation gate

### Executed evidence

- Confirmed that no `purewall` process was running before the attempt.
- Rebuilt the post-Task-2 Tauri release candidate successfully before launch.
- Final rebuilt bundles at this checkpoint:
  - `PureWall_0.1.0_x64_en-US.msi`: 6,189,056 bytes; SHA-256 `C88F942B0E2AAA9A52409D224CFD9E723BD8F4B0DEF4BC740B6DE33BBA8DD46A`.
  - `PureWall_0.1.0_x64-setup.exe`: 4,387,495 bytes; SHA-256 `9FF74E58E15135035F8BC8727ADF426B0E1E91B80A664E8D8105D767773AAF2D`.
- Created the owned QA root `C:\tmp\purewall-phase3d-qa-20260730-000907` with repository-owned PNG fixtures, separate source/relocation directories, an app-data target, and a WebView profile target.
- Launched only the rebuilt release executable as owned PID 39992 with process-scoped `APPDATA` and `WEBVIEW2_USER_DATA_FOLDER`.
- Enumerated the PID's windows instead of trusting `MainWindowHandle`: the actual `Tauri Window` used handle 9375588 at 1214×808, while the process property initially exposed a 14×14 helper window.
- Recorded `GetDpiForWindow(9375588) = 120`, confirming that the real window ran at Windows 125% scaling.
- Confirmed that the WebView profile was created below the QA root.

### Isolation failure and safety response

- The intended QA app-data directory did not receive a database, while the native window rendered the existing real wallpaper library. This proved that the process-scoped `APPDATA` override did not isolate PureWall's native data.
- Read-only source inspection confirmed the boundary: `src-tauri/src/main.rs` uses Tauri's `app.path().app_data_dir()` for database setup and `src-tauri/src/paths.rs` uses `dirs::data_dir()` for shared runtime paths. Both follow the Windows Known Folder result rather than treating the child-process environment as the test root contract.
- Stopped only owned PID 39992 immediately and verified that no `purewall` process remained.
- Performed no UI click, source add/remove/relocate, backup import/export, wallpaper action, registry operation, autostart action, or system-setting change.
- Deleted the one app-window screenshot after verifying that it was inside the owned QA root because it contained real wallpaper thumbnails. No screenshot from this failed run remains.
- Did not inspect, copy, move, or deliberately modify the real application-data directory after detecting the failure.
- Because startup occurred before the failure was visible and no real-data baseline was recorded, this evidence does **not** claim that startup made zero incidental writes.

### Outcome and unresolved items

- Phase 3D Task 3 is **BLOCKED**, not passed: source lifecycle, watcher refresh, relocation, both remove choices, backup round-trip, native dialog keyboard behavior, and source-file hash invariants remain NOT RUN.
- Retrying safely requires either:
  - an accepted ADR plus an explicit PureWall data-root override designed for tests, or
  - a disposable Windows user/session whose Known Folder data is isolated.
- No architecture expansion was introduced under a QA task, and no unsafe workaround such as moving real app data, redirecting Known Folders, changing the registry, or using junctions was attempted.
- The QA root remains below `C:\tmp` as failure evidence; it contains only the isolated WebView profile and repository-owned fixtures after screenshot deletion.
- Independent branch review and final documentation synchronization remain pending.
- GitHub Windows CI remains NOT RUN because the branch is not pushed.
- No merge or push is authorized by this task.

## 2026-07-30 — Phase 3D Task 4 independent review and backup safety corrections

### Independent review verdict and validation

- Dispatched exactly one fresh, read-only reviewer with no conversation history over `199c7dc60f9e26096d711c0e6f16b0fe7ea38463..5b90a55e2755b3d0d1970d2a1270edf81e2db603`.
- Required plan alignment plus original-file safety, schema/path identity, transactions, watcher lock/drop order, bounded inputs, selection/error behavior, backup semantics, responsive access, and mocked-vs-native evidence.
- Reviewer reported:
  - Critical code defect: export could atomically replace a registered wallpaper with JSON.
  - Important code defect: confirmation was bound only to a mutable path, not the previewed bytes.
  - Important evidence gap: Task 3 native source/backup journeys remain BLOCKED/NOT RUN.
  - Minor documentation inconsistency: architecture/changelog safety wording did not describe the unsafe destination path.
- Local source/test validation confirmed both code defects and the evidence gap. The documentation finding is resolved together with the code correction; Task 3 remains explicitly blocked.

### Focused RED evidence

- Added Rust tests for protected export destinations and changed-after-preview content before adding production APIs.
- `cargo test --manifest-path src-tauri/Cargo.toml library_backup_tests`: RED with `E0432` because `validate_export_destination`, `backup_content_digest`, and `verify_preview_digest` did not exist.
- Added a frontend command-routing expectation before changing the store.
- `npx vitest run src/stores/libraryBackup.test.ts`: RED, 1 failed / 5 passed; the `import_library_backup` invocation omitted `expectedDigest`.

### Minimal correction

- Added direct `sha2 0.10` usage (already present in the resolved lock graph) for stable SHA-256 preview content digests.
- Added stable `BACKUP_UNSAFE_DESTINATION` and `BACKUP_PREVIEW_CHANGED` error codes.
- Export now resolves the PureWall app-data root, requires a `.json` target, canonicalizes the destination/known protected paths, rejects every registered wallpaper identity, and rejects app-data descendants before creating a temp file.
- The generic atomic writer still supports replacing an existing legitimate backup and still removes only its own operation temp file on failure.
- Preview now returns `contentDigest`; the component/store send it back as `expectedDigest`; confirmation verifies the re-read bytes before parsing or opening the SQLite merge transaction.
- Updated architecture/design contracts and appended `#backup-export-001` plus `#backup-preview-001`.

### Focused and complete GREEN evidence

- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`: PASS.
- Focused Rust backup tests: PASS, 19/19.
- Focused frontend backup tests: PASS, 2 files / 10 tests.
- `npx vue-tsc --noEmit`: PASS.
- Fresh `npm ci`: PASS, 177 packages installed / 178 audited / 0 vulnerabilities.
- Full Rust test gate: PASS, 121 passed / 0 failed / 1 ignored manual performance benchmark.
- `cargo check`: PASS.
- `cargo clippy --all-targets -- -D warnings`: PASS.
- `cargo build --release`: PASS.
- Full frontend unit gate: PASS, 14 files / 49 tests.
- Frontend production build: PASS, 169 modules transformed.
- `npm audit`: PASS, 0 vulnerabilities.
- `npm audit --omit=dev`: PASS, 0 production vulnerabilities.
- Final `npm run tauri build`: PASS; MSI and NSIS regenerated.
- Final MSI: 6,205,440 bytes; SHA-256 `18BED6CA5A0C0B1808D34FABE3B61C712E9CADD3533ED6157F56BEC4DBE762C4`.
- Final NSIS: 4,398,223 bytes; SHA-256 `8FB534B29D8E9341ED29A3A97D40603D2318F1A91950480522AE41C271B5A00F`.
- No cargo, rustc, PureWall, candle, light, or makensis process remained after packaging.
- The two known third-party `@vueuse/core` annotation warnings and macOS-only `.app` identifier warning remain non-fatal.

### Remaining release blockers

- The same read-only reviewer completed a focused working-tree re-review: both original defects are resolved, no new Critical/Important correctness or data-safety regression was found, and zero unresolved code-level Critical/Important findings now holds.
- Three Minor items are explicitly deferred under the Task 4 rule:
  - At the 100,000-wallpaper upper bound, destination protection currently canonicalizes each registered path and can add substantial export-time filesystem work.
  - Focused destination tests prove exact-path protection but do not yet enumerate Windows case/separator/`\\?\`/short-name/junction aliases.
  - Digest helper and frontend wiring are tested, while a command-level test does not yet directly spy that digest mismatch prevents `merge_backup`; current source order verifies before parse, DB lock, and merge.
- Task 3 real-Tauri journeys remain BLOCKED/NOT RUN because process-scoped `APPDATA` did not isolate the Windows Known Folder database.
- GitHub Windows CI remains NOT RUN because the branch is not pushed.
- Phase 3D and Phase 3 remain incomplete; no merge, push, registry operation, system-setting change, or real user-data mutation is authorized.

## 2026-07-30 — Phase 3D Task 5 final local evidence status

### Reader-facing result

- **Overall: INCOMPLETE / BLOCKED.** Phase 3D and Phase 3 are not complete or release-ready.
- **PASS — local automated/release gate:** fresh post-review production gate passed.
  - Rust formatting/check/Clippy/release passed.
  - Rust tests: 121 passed / 0 failed / 1 ignored manual release benchmark.
  - Frontend types passed; unit tests: 14 files / 49 tests.
  - Frontend production build: 169 modules.
  - Fresh `npm ci`: 177 packages installed / 178 audited / 0 vulnerabilities.
  - `npm audit` and `npm audit --omit=dev`: 0 vulnerabilities.
- **PASS — final bundles:**
  - `D:\Desktop\PureWall\.worktrees\purewall-phase-3\src-tauri\target\release\bundle\msi\PureWall_0.1.0_x64_en-US.msi`
  - 6,205,440 bytes; SHA-256 `18BED6CA5A0C0B1808D34FABE3B61C712E9CADD3533ED6157F56BEC4DBE762C4`.
  - `D:\Desktop\PureWall\.worktrees\purewall-phase-3\src-tauri\target\release\bundle\nsis\PureWall_0.1.0_x64-setup.exe`
  - 4,398,223 bytes; SHA-256 `8FB534B29D8E9341ED29A3A97D40603D2318F1A91950480522AE41C271B5A00F`.
- **PASS — mocked browser shell:** empty-library Settings, 1200×800/800×600/375×667 layout, light/dark, Enter/Tab/Shift+Tab/Escape and focus return.
- **INCONCLUSIVE — visual/DPI:** no committed screenshot baseline; CSS zoom 1.25/1.5 clipped and is not real Windows DPI evidence.
- **PASS — independent review:** the original 1 Critical and 1 Important code findings are fixed; focused re-review reports 0 unresolved code-level Critical/Important findings.
- **Deferred Minor:** 100,000-row export path canonicalization cost, Windows alias regression matrix, and command-level digest-mismatch/merge-spy coverage.
- **BLOCKED / NOT RUN — native acceptance:** source lifecycle, watcher refresh, offline Retry, Relocate, both Remove modes, backup round-trip, native dialog keyboard behavior, and source hashes.
- **NOT RUN — GitHub Windows CI:** no push, so acceptance criterion 8 remains unsatisfied.
- **NOT RUN — integration:** no merge, push, PR, or worktree removal.

### Safety closeout

- All production changes and final builds remained in `D:\Desktop\PureWall\.worktrees\purewall-phase-3` on `codex/purewall-phase-3`.
- No registry, autostart, context-menu, Windows wallpaper, Recycle Bin, policy, display-scaling, or other system-setting operation was invoked.
- The failed native run started owned PID 39992, detected the isolation failure before UI interaction, stopped only that PID, and left no PureWall process.
- Its app-window screenshot was deleted because it contained real thumbnails; the preserved QA root below `C:\tmp` contains only repository-owned fixtures and the isolated WebView profile.
- The real app-data directory was not inspected or deliberately modified after detection. Because startup was not baselined, this record does not claim zero incidental startup writes.
- The main worktree was not modified by these Phase 3D tasks.

### Final blockers and next authorized decision

- A safe native rerun needs either an accepted ADR plus an explicit test data-root override, or a disposable Windows user/session whose Known Folder data is isolated.
- GitHub CI needs separate push authorization.
- Until those gates pass, keep Phase 3D/Phase 3 incomplete and do not merge.
- Final evidence commit is documentation-only after `git diff --check`; no new pitfall required an `AI_DIARY.md` append in Task 5.

## 2026-07-30 — Phase 4 interaction/layout optimization started

### Scope and branch

- The user explicitly reprioritized Phase 4 for direct execution without treating the two remaining
  Phase 3 gates as passed.
- Created `codex/purewall-phase-4` from Phase 3 HEAD `d16f541` in the existing isolated linked worktree.
- Added the direct-execution TDD plan:
  `docs/superpowers/plans/2026-07-30-purewall-phase-4-interaction-layout.md`.
- The plan remains within ADR-009 and introduces no backend command, database migration, dependency,
  registry behavior, or system-setting operation.

### Fresh baseline evidence

- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS.
- `npx vue-tsc --noEmit`: PASS.
- `cargo test --manifest-path src-tauri/Cargo.toml`: PASS, 121 passed / 0 failed /
  1 ignored manual performance benchmark.
- `npm run test:unit`: PASS, 14 files / 49 tests.
- No `proposed` ADR blocks Phase 4.

### Carried blockers

- Phase 3 real-Tauri source/backup acceptance remains BLOCKED/NOT RUN because process-scoped
  `APPDATA` did not isolate the Windows Known Folder database.
- GitHub Windows CI remains NOT RUN because no branch has been pushed.
- No merge, push, PR, registry operation, system-setting change, or real user-data mutation occurred.

## 2026-07-30 — Phase 4 Task 1 first-run and state contract complete

### RED evidence

- Added `workspacePresentation.test.ts`; it failed because the loading/empty/gallery contract did not exist.
- Added `notificationPresentationModel.test.ts`; it failed because notification tones had no semantic mapping.
- Added `emptyState.test.ts`; both tests failed because onboarding lacked the local-only statement,
  dominant folder CTA, busy state, and competing-import lockout.

### Implementation

- Initial store loading now starts true, preventing a persisted library from briefly rendering onboarding.
- `workspacePresentation` keeps initial loading, true empty library, and existing-library reloads distinct.
- Onboarding now leads with **Choose wallpaper folder**, states that images stay on-device and are not
  uploaded, and disables both import paths while an import is active.
- Success/info notifications use polite `status` semantics; only errors use assertive `alert` semantics.
- Added a stable reader-facing library loading surface and focused styles without changing backend commands.

### GREEN evidence

- Focused Task 1 tests: PASS, 3 files / 7 tests.
- AppShell empty-library Settings regression: PASS, 1/1.
- `npx vue-tsc --noEmit`: PASS.
- `npm run test:unit`: PASS, 17 files / 56 tests.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS.
- No registry, autostart, context-menu, Windows wallpaper, display scaling, or real user-data operation ran.

## 2026-07-30 — Phase 4 Task 2 playback hierarchy complete

### RED evidence

- Added a behavior-level SSR test for the real `CurrentWallpaperPanel`.
- The test failed because the stage still rendered both Display mode and Focus pause beside the
  primary playback actions.

### Implementation

- The stage dock now contains Next, Like, Dislike, Pause/Resume, and rotation interval only.
- Display mode remains available in the existing Displays system panel.
- Focus Pause remains available in the existing Settings system panel.
- Removed only the now-unused component imports and stage handlers; no Tauri command or store
  behavior changed.
- Replaced the planned detached control-model test with a real component behavior test.

### GREEN evidence

- `currentWallpaperPanel.test.ts`: PASS, 1/1.
- `npx vue-tsc --noEmit`: PASS.
- `npm run test:unit`: PASS, 18 files / 57 tests.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS.
- No registry, autostart, context-menu, Windows wallpaper, display scaling, or real user-data operation ran.

## 2026-07-30 — Phase 4 Task 3 keyboard focus and semantics complete

### RED evidence

- Added focused tests for inspector opener/fallback focus, display-title-first labels, system-panel
  heading association, and gallery grid/cell semantics.
- The first focused run produced 5 failed / 1 passed tests because focus was not restored, system
  inspectors were not programmatically focusable/labeled, cards lacked grid semantics, and custom
  display titles were ignored.
- The semantic source contract was narrowed after SSR confirmed that the virtual-list harness does
  not materialize visible rows; the production row role remains covered by a targeted source assertion.

### Implementation

- Inspector openers are captured from title-bar, sidebar, and wallpaper-card triggers.
- Closing restores focus to a still-connected opener, otherwise to `#main-workspace`.
- System inspectors now receive focus on open and expose `h2` titles through `aria-labelledby`.
- The virtual gallery exposes grid/row/gridcell semantics, active-state semantics, and accessible
  names that prefer user display titles before tag/generic fallbacks.
- No backend command, persistence format, router, or native integration changed.

### GREEN evidence

- Focused Task 3 tests: PASS, 3 files / 6 tests.
- `npx vue-tsc --noEmit`: PASS.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS.
- `npm run test:unit`: PASS, 21 files / 63 tests.
- No registry, autostart, context-menu, Windows wallpaper, display scaling, or real user-data operation ran.

## 2026-07-30 — Phase 4 Task 4 minimum-window and path resilience complete

### RED evidence

- Added `responsiveContract.test.ts` for the compact-height media contract and user-visible path
  wrapping.
- The focused RED run failed 2/2 tests because no desktop compact-height breakpoint existed and
  source/backup paths still relied on single-line ellipsis.
- Corrected the test's media-block extraction before GREEN so nested CSS rule boundaries are not
  mistaken for the end of the media query.

### Implementation

- Added a desktop-only `max-height: 680px` contract that reduces shell chrome, workbench spacing,
  stage height, copy spacing, dock offset, and control height without hiding playback actions.
- The compact-height rule keeps the side rail independently scrollable and leaves the existing
  inspector overflow behavior intact.
- Library source, remove-confirmation, and backup paths now use `overflow-wrap: anywhere` with
  normal whitespace instead of single-line ellipsis.
- Existing width breakpoints and reduced-motion behavior remain unchanged.

### GREEN evidence

- `responsiveContract.test.ts`: PASS, 1 file / 2 tests.
- `npx vue-tsc --noEmit`: PASS.
- `npm run build`: PASS, 169 modules transformed.
- The build retained two non-blocking `@vueuse/core` Rolldown annotation warnings; no project source
  warning or build failure was introduced.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS.
- `npm run test:unit`: PASS, 22 files / 65 tests.
- Browser viewport and simulated-scale evidence is deferred to Phase 4 Task 5.
- No registry, autostart, context-menu, Windows wallpaper, display scaling, or real user-data operation ran.

## 2026-08-01 — Phase 4 Task 5 browser-shell QA complete

### Browser evidence

- Mocked browser matrix: PASS, 9/9 cases. It covered 1200x800 dark onboarding/loading/empty and dark/light gallery, 800x600 dark/light gallery, system drawer/long path, keyboard/filter flow, and 800x600 simulated browser device scale 1.25/1.5.
- Every matrix page reported document/body width equal to its viewport; required Next, Like, Dislike, Pause, and Import folder controls were inside the tested viewports. Import/native-dialog controls were not clicked.
- Keyboard evidence covered grid Arrow navigation, Enter/Space activation, Escape focus return, system-panel focus entry/return, and Tab/Shift+Tab movement where the browser harness supports it.
- Settings drawer evidence: 686px client height / 870px scroll height, successful 184px scroll, and a synthetic long source path with no horizontal overflow.
- Focused regression: `npx vitest run src/components/semanticSurfaces.test.ts src/components/responsiveContract.test.ts src/components/notificationPresentationModel.test.ts src/components/appShell.test.ts` PASS (4 files / 8 tests).
- `npx vue-tsc --noEmit` PASS. Earlier task pre-flight `cargo check` PASS.

### Scope and limitations

- Used init-time, read-only Tauri IPC/metadata mocks, synthetic data, cached Playwright, and system Chrome only. No native Tauri app, dialog, app data, wallpaper/display/registry/autostart/context-menu operation ran.
- Final owned Vite lifecycle: launcher PID 48852; port listener PID 50984; stopped only PID 50984; port 1420 confirmed free afterward with no browser residuals.
- The 1.25/1.5 checks are simulated browser device-scale evidence only, never real Windows DPI. A CSS-zoom experiment was rejected because it does not change CSS media/container dimensions.
- No committed visual baseline exists, so visual regression remains **INCONCLUSIVE**. Phase 3 native source/backup QA remains BLOCKED/NOT RUN; GitHub Windows CI remains NOT RUN.
- Detailed local report: `.superpowers/sdd/task-5-report.md`; ignored screenshots/logs/controller: `output/playwright/phase4/`.

## 2026-08-01 — Phase 4 Task 5 zoom-layout evidence corrected

### Important finding and correction

- With an unchanged 800×600 CSS viewport, Playwright `deviceScaleFactor: 1.25/1.5` changed raster density but did not pressure CSS layout; the earlier device-scale PASS is withdrawn as zoom-layout evidence.
- The ignored local controller now fixes an 800×600 physical target and reduces CSS layout space to 640×480 at 1.25 and 533×400 at 1.5.
- Playwright bounding boxes are CSS pixels. Clipping now compares directly with `window.innerWidth` / `window.innerHeight` instead of multiplying the CSS viewport by DPR.
- The corrected method remains mocked browser zoom-layout stress, not real Windows DPI evidence.

### Corrected QA evidence

- Port 1420 was free before launch; one owned Vite chain was started: 52944 → 40656 → 52892 → listener 33632.
- Full mocked browser matrix: PASS, 9/9 cases.
- 1.25 stress: fixed 800×600 physical target, 640×480 CSS viewport, DPR 1.25; document/body/inner width all 640; all five required controls inside CSS bounds.
- 1.5 stress: fixed 800×600 physical target, 533×400 CSS viewport, DPR 1.5; document/body/inner width all 533; all five required controls inside CSS bounds.
- No layout failure was found, so no application source or RED/GREEN regression test change was required.
- Native playback/import controls were never clicked; no Tauri app, native dialog, registry, autostart, context-menu, wallpaper, display-scaling, system-setting, or real-data operation ran.
- Cleanup stopped only the exact owned process chain; no owned PID or Chrome process remained and port 1420 was free.

### Verification and remaining limits

- `node --check output/playwright/phase4/phase4_browser_qa.mjs`: PASS.
- Focused frontend regression: PASS, 4 files / 8 tests.
- `npx vue-tsc --noEmit`: PASS.
- `npm run build`: PASS, 169 modules transformed; only the two known non-fatal third-party `@vueuse/core` annotation warnings remained.
- Pre-flight `cargo check --manifest-path src-tauri/Cargo.toml`: PASS.
- No committed screenshot baseline exists, so visual regression remains INCONCLUSIVE.
- Real Windows DPI remains NOT RUN. Phase 3 native source/backup QA remains BLOCKED/NOT RUN; GitHub Windows CI remains NOT RUN.

## 2026-08-01 — Phase 4 Task 6 full-gate evidence; independent review pending

### Fresh complete gate

- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`: PASS.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS, 1.00s, 0 errors.
- `cargo test --manifest-path src-tauri/Cargo.toml`: PASS, 121 passed / 0 failed / 1 ignored
  (manual release-mode performance benchmark).
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`: PASS, 2.44s,
  0 warnings promoted to errors.
- `npx vue-tsc --noEmit`: PASS, 0 errors.
- `npm run test:unit`: PASS, 22 files / 65 tests.
- `npm run build`: PASS, 169 modules transformed. The two known non-fatal third-party
  `@vueuse/core` Rolldown `INVALID_ANNOTATION` messages remained; no application warning or
  failure was introduced.
- `git diff --check`: PASS. `git status --short`: clean before these documentation edits.

### Retained QA evidence and review state

- Independent whole-branch review is pending for `d16f541..HEAD`; no final finding count is claimed
  until that review completes.
- Mocked browser-shell QA remains PASS, 9/9. Its fixed 800×600 physical target uses 640×480 CSS
  at 1.25x and 533×400 CSS at 1.5x; all five required controls remained inside CSS bounds. It is
  simulated browser zoom-layout coverage, not native Windows DPI evidence.
- No committed screenshot baseline exists, so visual regression remains **INCONCLUSIVE**. Real
  Windows DPI remains **NOT RUN**. Phase 3 native source/backup QA remains **BLOCKED/NOT RUN**.
  GitHub Windows CI remains **NOT RUN** until push is separately authorized.
- No native Tauri app/dialog, registry, autostart, context-menu, wallpaper, display-scaling,
  system-setting, or real-data operation was run. No new reusable pitfall was found, so
  `AI_DIARY.md` was intentionally not appended for Task 6.

## 2026-08-01 — Phase 4 final Minor review hardening

### Review fixes

- Corrected `WallpaperGrid` virtual-grid semantics: `aria-rowcount` now reports the number of
  logical rows derived from the current card count and column count, and each rendered virtual row
  exposes its one-based `aria-rowindex`.
- Added a focused two-column SSR contract with three real wallpaper cards, proving a two-row count
  and row indexes 1/2 instead of relying on a source-only row-role assertion.
- Added real `NotificationCenter.vue` SSR coverage for info, success, and error notifications.
  The component template is now directly verified for polite `status`, assertive `alert`,
  `aria-live`, and `aria-atomic="true"` behavior; the existing helper test remains complementary.
- Strengthened the compact-layout contract with stable, copy-independent identifiers that
  explicitly enumerate Import folder, Next, Like, Dislike, and Pause as required controls. The
  responsive test also rejects compact-height rules that hide any enumerated control.

### RED/GREEN and verification evidence

- RED: the focused command ran 3 files / 6 tests with 2 failed / 4 passed. The multi-column grid
  rendered `aria-rowcount="3"` instead of 2, and the responsive contract reported the missing
  `import-folder` identifier. The three real NotificationCenter template cases already passed,
  confirming that finding was a coverage gap rather than a production semantics defect.
- GREEN: the same focused command passes 3 files / 6 tests.
- The first complete frontend run passed all 69 assertions but correctly failed the process because
  the new SSR grid harness left a thumbnail-scheduler timer that later accessed `window`. The test
  now isolates that unrelated scheduler; the focused suite and full suite were rerun cleanly.
- `npx vue-tsc --noEmit`: PASS, 0 errors.
- `npm run test:unit`: PASS, 24 files / 69 tests, with no unhandled errors.
- `npm run build`: PASS, 169 modules transformed. The two existing non-fatal third-party
  `@vueuse/core` Rolldown `INVALID_ANNOTATION` messages remain unchanged.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS, 0 errors.
- `git diff --check`: PASS; only Git's informational LF-to-CRLF working-copy notices were emitted.

### Safety and remaining boundaries

- No native Tauri app/dialog, real user data, registry, autostart, context menu, Windows wallpaper,
  display scaling, policy, or system setting was read or mutated.
- No dependency, backend command, database contract, router, architecture, or release status
  changed. Task 6 remains pending the parent full gate and independent re-review.
- No new reusable pitfall was found. The built-in patch wrapper failure is already documented by
  `AI_DIARY.md #sandbox-005`, so `AI_DIARY.md` was intentionally not appended.

## 2026-08-01 — Phase 4 final Minor re-review corrections

### Corrections to the preceding hardening record

- The preceding claim that `aria-rowcount` was derived from the current card count was too weak for
  PureWall's paginated gallery and is withdrawn. The accessible total now uses
  `store.wallpaperTotal`, divided by the responsive column count, while rendered row indexes remain
  one-based virtual-row indexes.
- The preceding responsive contract attached `import-folder` to the empty-library onboarding
  action, but the compact-layout and browser-QA contract refers to the always-visible TitleBar
  **Import folder** action. That substitution is withdrawn: the stable required-control identifier
  now belongs to `TitleBar.vue`, and the misleading identifier was removed from `EmptyState.vue`.

### Corrected RED/GREEN evidence

- RED: the focused command ran 3 files / 9 tests with 4 failed / 5 passed.
  - A paginated two-column case with total 100 and 3 loaded rendered 2 rows instead of 50.
  - A three-column case rendered 1 row instead of 34.
  - The minimum one-column case rendered 3 rows instead of 100.
  - The responsive contract found no `import-folder` identifier on the real TitleBar control.
- GREEN: the same focused command passes 3 files / 9 tests.
- Related EmptyState and CurrentWallpaperPanel regressions pass 2 files / 3 tests.
- `npx vue-tsc --noEmit`: PASS, 0 errors.
- `npm run test:unit`: PASS, 24 files / 72 tests.
- `npm run build`: PASS, 169 modules transformed. The same two non-fatal third-party
  `@vueuse/core` Rolldown `INVALID_ANNOTATION` messages remain unchanged.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS, 0 errors.
- `git diff --check`: PASS; only informational LF-to-CRLF working-copy notices were emitted.

### Safety and remaining boundaries

- No native application, browser, dialog, real user data, registry, autostart, context menu,
  Windows wallpaper, display scaling, policy, or system-setting operation ran.
- Task 6 remains pending the parent final full gate and independent re-review. Existing Phase 3
  native-QA, real Windows DPI, visual-baseline, and remote-CI boundaries remain unchanged.
- No new reusable pitfall was found, so `AI_DIARY.md` was intentionally not appended.

## 2026-08-01 — Consolidate completed Phase branches and preserve existing main work

### Git consolidation

- Protected the pre-existing dirty main with named stash `codex-pre-phase4-merge-2026-08-01`, including untracked files, before changing refs.
- Fast-forwarded local main from `9a1a2fa` to current `origin/main` at `199c7dc`, then fast-forwarded Phase 4 into main at `736b8b2`.
- Removed the clean project-owned worktrees `.worktrees/purewall-phase-2` and `.worktrees/purewall-phase-3`, then ran `git worktree prune`.
- Deleted the fully merged local branches `codex/purewall-phase-2`, `codex/purewall-phase-3`, and `codex/purewall-phase-4`.
- Verified `origin/codex/purewall-phase-2` was contained by both `origin/main` and local main, then deleted that obsolete remote branch. Local and remote branch lists now contain only main.
- During the post-merge gate, the user pushed main to the same `736b8b2` commit (remote-ref reflog time 2026-08-01 16:33:17 +0800). This workflow did not issue a main push command; its only push command deleted the obsolete Phase 2 remote branch.

### Existing-work restoration and conflict resolution

- Applied the safety stash without popping it. Three append-only documents and `src/stores/wallpapers.ts` conflicted; all other tracked and untracked changes restored automatically.
- Preserved both document histories. The restored transactional wallpaper-title decision was renumbered from the colliding `ADR-027` to unique `ADR-030`, with diary/changelog references updated.
- Preserved both store intents: `patchWallpaper` now updates `currentWallpaper` and `bootstrappedWallpaper` when their paths match.
- Verified all 11 stashed tracked-change paths remain represented in the working tree. Compared all 68 stashed untracked files with their restored working copies by Git blob hash: 0 missing / 0 mismatched.
- Only after the combined checks passed was safety stash `6c3776d1` dropped. The user's work remains intentionally uncommitted and unstaged; no user-change commit was created.

### Verification

- Before merging: Rust 121 passed / 0 failed / 1 ignored; frontend 24 files / 72 tests; TypeScript and production build PASS.
- Merged main before worktree cleanup: Rust 121 passed / 0 failed / 1 ignored; frontend reported 55 files / 170 tests because repository-local worktrees entered discovery; build PASS.
- Final combined main plus restored user work: Rust 124 passed / 0 failed / 1 ignored; focused stage-title test 3/3; frontend 25 files / 75 tests; Rust format/check/Clippy, TypeScript, production build (169 modules), and `git diff --check` PASS.
- `#vitest-worktree-discovery-001` and `#git-stash-phase-merge-001` record the new reusable pitfalls.
- No native Tauri app/dialog, registry, autostart, context-menu, wallpaper, display-scaling, system-setting, or real-data operation ran.

## 2026-08-01 — Phase 4 Task 6 complete

### Independent review closure

- Whole-branch review covered `d16f541..f416be6` and reported 0 Critical / 0 Important / 3 Minor.
- The three Minor findings were resolved with focused RED/GREEN work. The first incremental review
  closed the NotificationCenter template-coverage item and found two remaining counterexamples;
  the second correction fixed pagination-aware grid row totals and bound the responsive contract to
  the real TitleBar Import folder action.
- Final independent incremental re-review reports **0 Critical / 0 Important / 0 Minor**. All three
  original findings are CLOSED, so Phase 4 Task 6 is complete.

### Final fresh full gate

- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`: PASS.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS, 0 errors.
- `cargo test --manifest-path src-tauri/Cargo.toml`: PASS, 121 passed / 0 failed / 1 ignored
  manual release-mode performance benchmark.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`: PASS.
- `npx vue-tsc --noEmit`: PASS, 0 errors.
- `npm run test:unit`: PASS, 24 files / 72 tests, with no unhandled errors.
- `npm run build`: PASS, 169 modules transformed. Only the two unchanged non-fatal third-party
  `@vueuse/core` Rolldown `INVALID_ANNOTATION` messages were emitted.
- `git diff --check`: PASS; tracked status was clean before this documentation closeout.

### Retained release boundaries

- Mocked browser-shell QA remains PASS, 9/9, including corrected 1.25x/1.5x zoom-layout stress.
- Real Windows DPI remains **NOT RUN** and screenshot regression remains **INCONCLUSIVE** without a
  committed baseline.
- Phase 3 native source/backup QA remains **BLOCKED/NOT RUN**. GitHub Windows CI remains **NOT RUN**
  until push is separately authorized.
- No native Tauri app/dialog, real user data, registry, autostart, context menu, wallpaper, display
  scaling, policy, or system-setting operation ran. No new Task 6 pitfall was discovered, so
  `AI_DIARY.md` was not appended again.

## 2026-07-15 — Writable file titles and image-overlay contrast

### Change goal

Make the Living Stage title readable over arbitrary wallpapers and make an inspector title save persist to the source image's Windows `System.Title` metadata instead of silently succeeding only in SQLite.

### Root causes

- The stage title inherited a light-theme foreground token even though it overlays arbitrary photography, producing dark low-contrast text on the reported image.
- `SHGetPropertyStoreFromParsingName` used `GPS_DEFAULT`; Windows defines that property store as read-only, matching the reported `STG_E_ACCESSDENIED` (`0x80030005`) from `IPropertyStore::SetValue`.
- The command updated SQLite before the file write and swallowed the metadata error, so the UI reported a save while Explorer metadata remained unchanged.

### Code scope

- `src-tauri/src/shell_metadata.rs`: opens `GPS_READWRITE`, coerces `System.Title` to its canonical property type, commits it, clears the value safely, and covers a real temporary JPEG write.
- `src-tauri/src/main.rs`: validates under a short database lock, performs file IO outside that lock, updates SQLite only after metadata commit, and returns file-write failures to the frontend.
- `src/components/InspectorPanel.vue`: ordinary blur saves only changed text; Enter explicitly retries file synchronization for a title that already exists in SQLite.
- `src/styles.css`: uses a fixed white title with a strong dark shadow on the image stage.
- `src/utils/stageTitleContrast.test.ts`: covers overlay contrast and the explicit retry interaction.
- `docs/project-docs/DECISIONS.md`: accepted ADR-030 for transactional source-file-first title synchronization.
- No registry command was executed, no PureWall-owned registry entry was changed, and no system setting was changed.

### TDD and verification evidence

- Backend RED reproduced the user's exact `0x80030005` access-denied failure on a temporary JPEG opened with the old property-store flag.
- Backend GREEN verifies the writable JPEG property store and proves a file failure leaves the previous SQLite title unchanged.
- Frontend RED covered missing image-safe title contrast and the inability to explicitly resync an unchanged database title.
- `npm run test:unit`: PASS, 12/12 across 5 files.
- `npm run build`: PASS, including `vue-tsc --noEmit`; only existing upstream `@vueuse/core` Rolldown annotation warnings were emitted.
- `cargo fmt -- --check`: PASS.
- `cargo test`: PASS, 57 passed and 1 ignored manual performance benchmark.
- `cargo check`: PASS.
- `cargo clippy --all-targets -- -D warnings`: PASS.
- `git diff --check`: PASS; only line-ending conversion notices were emitted.

### Unresolved items

- The reported title already stored in SQLite must be focused and submitted once with Enter after the rebuilt backend restarts; this explicitly retries writing that same value into the source file.
- Windows property handlers may reject read-only or unsupported source formats. PureWall now shows that failure and preserves the previous SQLite title rather than claiming a partial save.

## 2026-07-16 — Transient wallpaper COM recovery and bootstrapped title refresh

### Change goal

Keep independent-display playback resilient to one-off Windows `IDesktopWallpaper` failures and make a successfully saved title appear immediately when the active wallpaper was restored outside the current gallery page.

### Evidence and root causes

- PureWall CLI reproduction recorded the exact HRESULT as `0x80004005` (`E_FAIL`), despite the UI report being remembered as `0x80040005`.
- The title-bearing current JPEG remained valid: Windows `System.Drawing`, PureWall WIC decoding, and the Rust `image` fallback all decoded it successfully; Explorer also read the committed `System.Title`.
- Windows `IDesktopWallpaper` successfully reapplied both monitors current files, then accepted all 245 available library images on both monitors. The library contained 242 JPEGs and 3 PNGs, so there was no persistent bad-format candidate.
- After the original failure, 12 application `--action next` calls succeeded before the change and another 12 succeeded after hot reload without adding a new CLI error. This isolates the original HRESULT as a transient Windows COM failure rather than source-file corruption.
- `patchWallpaper` updated only the loaded paginated array. When the current wallpaper existed only in `bootstrappedWallpaper`, the title command returned the updated row but the Living Stage continued reading the stale bootstrapped object.

### Code scope

- `src-tauri/src/wallpaper.rs`: added one bounded 80ms retry for `E_FAIL` only at the STA `IDesktopWallpaper` boundary and added monitor/file context to persistent errors. Other HRESULTs are not retried.
- `src/stores/wallpapers.ts`: applies entity patches to the bootstrapped current wallpaper as well as the loaded gallery page.
- `src/utils/stageTitleContrast.test.ts`: covers title refresh for an active bootstrapped row outside the loaded page.
- `docs/project-docs/ARCHITECTURE.md`: documents the bounded COM retry and bootstrapped entity-patch contract.
- No registry command was executed, no PureWall-owned registry entry was changed, and no system setting was changed.

### TDD evidence

- Rust RED: the focused test failed to compile because `retry_transient_e_fail` did not exist.
- Rust GREEN: injected first-attempt `E_FAIL` succeeds on the second and records exactly two attempts.
- Frontend RED: the focused test found the bootstrapped-row guard but not a matching state update.
- Frontend GREEN: the store now replaces the matching bootstrapped entity with the returned patch.

### Verification evidence

- Task Pre-Flight `cargo check`: PASS.
- Task Pre-Flight `npx vue-tsc --noEmit`: PASS.
- Focused Rust regression: PASS, 1/1.
- Focused frontend regression: PASS, 3/3.
- Runtime smoke: 12 consecutive PureWall `--action next` operations after hot reload; no new CLI error entry.
- `npm run test:unit`: PASS, 13/13 across 5 files.
- `npm run build`: PASS; only the existing upstream `@vueuse/core` Rolldown annotation warnings were emitted.
- `cargo fmt -- --check`: PASS.
- `cargo test`: PASS, 58 passed and 1 ignored manual performance benchmark.
- `cargo check`: PASS.
- `cargo clippy --all-targets -- -D warnings`: PASS.
- `git diff --check`: PASS; only line-ending conversion notices were emitted.

### Unresolved items

- The original one-off `E_FAIL` did not recur during exhaustive native-file testing or repeated app rotation. The injected regression verifies recovery behavior, while enriched monitor/file context will identify the exact boundary if Windows returns a persistent error later.
- This task updates the development build only; no MSI/NSIS package was created.

## 2026-07-19 — PureWall foundation optimization design

### Change goal

Define an approved, execution-ready optimization direction for PureWall as a conventional open-source local wallpaper manager, while explicitly deferring PureWall-X and avoiding speculative shared-core work.

### Approved product boundary

- PureWall follows a local music-player interaction model: import, random rotation, Next, Like, Dislike, Pause, and lightweight library organization.
- Existing tags, collections, multi-display support, widget, tray, context menu, themes, and basic statistics remain in PureWall.
- AI recommendation, recommendation explanations, online feeds, cloud accounts, social features, live wallpaper engines, and speculative PureWall-X APIs are outside the current scope.
- PureWall and the future PureWall-X will both be open source and are intended to share a core, but no X application or standalone core is created during the PureWall optimization program.

### Documentation scope

- Added `docs/superpowers/specs/2026-07-19-purewall-foundation-optimization-design.md` with the approved product definition, behavior contracts, incremental architecture strategy, six delivery phases, safety boundaries, verification gates, and process-documentation requirements.
- No ADR was added because this design deliberately preserves the current application boundary. Any future shared-core extraction requires its own accepted ADR before implementation.
- No application code, SQLite schema, registry entry, installed application, wallpaper file, or system setting was changed.

### Verification evidence

- Task Pre-Flight `cargo check`: PASS.
- Task Pre-Flight `npx vue-tsc --noEmit`: PASS.
- `DECISIONS.md`: no proposed ADR blocks the approved design.
- Design self-review: PASS — no placeholders, internal contradictions, scope leakage, or unresolved ambiguity; the six-phase program is explicitly split into separate implementation plans beginning with Phase 1.

### Unresolved items

- The user must review the committed design specification before the detailed Phase 1 implementation plan is written.
- PureWall-X remains a post-PureWall operation and has no implementation task in the current program.

## 2026-07-19 — PureWall Phase 1 P0 implementation plan

### Change goal

Translate the user-approved PureWall optimization design into an executable, test-first plan limited to Phase 1 P0 behavior repairs.

### Planning scope

- Added `docs/superpowers/plans/2026-07-19-purewall-phase-1-p0-behavior.md`.
- Split Phase 1 into five independently verifiable tasks: sidebar collection/Disliked behavior, Command Line terminology, Store/Tauri and Rust playback contracts, 800x600 sidebar reachability, and the final verification/documentation gate.
- Included exact files, interfaces, RED/GREEN tests, implementation snippets, commands, expected outcomes, staging guardrails for the existing dirty worktree, and process-documentation updates.
- PureWall-X, shared-core extraction, Phase 2 action consolidation, new dependencies, SQLite migrations, registry behavior, installers, and release automation remain outside this plan.

### Verification evidence

- Planning Pre-Flight `cargo check`: PASS.
- Planning Pre-Flight `npx vue-tsc --noEmit`: PASS.
- `DECISIONS.md`: no proposed ADR blocks Phase 1.
- Plan self-review: PASS — complete Phase 1 spec coverage, no placeholders, balanced code fences, consistent interfaces/test counts, and no scope leakage.
- No application code, registry entry, system setting, application data, or wallpaper file was changed while creating the plan.

### Unresolved items

- Execution has not started. The user must choose subagent-driven or inline execution.
- `CHANGELOG_AI.md` retains pre-existing unstaged changes and must not be staged wholesale during Phase 1.

## 2026-07-19 — Phase 1 P0 behavior repairs (complete)

### Task 1 — Sidebar collection and Disliked navigation

- Added a behavior-tested sidebar model with Library, Liked, Disliked, Hidden, and Tags navigation.
- Collection creation now calls `createCollection`; it no longer creates a tag accidentally.
- Focused Vitest: PASS, 2/2 after the missing-module RED.
- `npx vue-tsc --noEmit`: PASS.
- No registry command, system setting, SQLite schema, or wallpaper file was changed.

### Task 2 — Accurate command-line terminology

- Replaced the misleading Shortcuts copy with Command Line while preserving the internal workspace key and all existing CLI arguments.
- Added an executable presentation model test instead of a source-string assertion.
- Focused Vitest: PASS, 1/1 after the missing-module RED.
- `npx vue-tsc --noEmit`: PASS.
- No CLI behavior, Tauri command, registry entry, or system setting changed.

### Task 3 — Playback rating eligibility contract

- Centralized the private playback ticket rule: negative ratings are excluded, liked items own two tickets, and other non-negative items own one.
- Added a database integration regression proving disliked, hidden, and unavailable rows never enter playback candidates.
- Added Pinia command-routing coverage for collection creation plus Like, Dislike, and rating reset/restore.
- Focused Rust RED reproduced direct sampling of a disliked candidate; both focused tests and all database tests pass after the repair.
- Focused Store command routing: PASS, 2/2.
- `cargo fmt -- --check`: PASS.
- Public database methods, Tauri commands, SQLite schema, registry state, and system settings are unchanged.
### Task 4 — Minimum-window System navigation

- The side rail now owns vertical overflow and keeps the System section at intrinsic height, making every navigation item reachable at 800x600.
- The winning Living Gallery overflow rule now preserves horizontal clipping while permitting vertical scrolling; nested Tags and Collections retain usable independent scroll viewports.
- Read-only production-bundle QA captured the before/after 800x600 states outside the repository and verified the 1200x800 layout without horizontal overflow.
- `npx vue-tsc --noEmit`: PASS.
- `npm run build`: PASS; 157 modules transformed, with only the existing upstream `@vueuse/core` annotation warnings.
- `npm run test:unit`: PASS, 18/18 across 8 files.
- No Tauri window configuration, registry entry, system setting, application data, or wallpaper file was changed.

### Phase 1 final verification

- Corrected collection creation, added Disliked navigation, accurately labeled CLI actions, centralized playback rating eligibility, and made System navigation reachable at 800x600.
- Focused frontend regressions: PASS, 5/5.
- Focused Rust playback regressions: PASS, 2/2.
- `cargo fmt -- --check`: PASS.
- `cargo check`: PASS.
- `cargo clippy --all-targets -- -D warnings`: PASS.
- `cargo test`: PASS, 59 passed and 1 ignored manual performance benchmark.
- `npx vue-tsc --noEmit`: PASS.
- `npm run test:unit`: PASS, 18 passed across 8 files.
- `npm run build`: PASS; only previously documented upstream warnings may remain.
- `git diff --check`: PASS.
- No registry command was executed, no PureWall-owned registry entry changed, and no system setting changed.

### Unresolved items

- Global keyboard shortcuts remain outside Phase 1; the existing CLI actions are now labeled Command Line.
- Phase 2 action-path consolidation starts only after explicit user approval of this completed milestone.

## 2026-07-22 — PureWall Phase 2 playback-loop implementation plan

### Change goal

Translate the approved foundation design's Phase 2 milestone into a test-first execution plan that unifies primary playback actions without changing the database schema, registry surface, or PureWall-X boundary.

### Planning scope

- Added `docs/superpowers/plans/2026-07-22-purewall-phase-2-playback-loop.md`.
- Split Phase 2 into six reviewed tasks: the Rust action vocabulary, backend entry-point routing and failure feedback, frontend current-playback controls, restart restoration, multi-display/history regressions, and final verification/documentation.
- Preserved existing Tauri command names as compatibility facades and kept path-targeted gallery rating separate from current-playback actions.
- Context-menu/CLI behavior is covered through the existing `--action` path; the plan explicitly forbids executing or changing registry registration.
- No SQLite migration, dependency addition, installer work, global hotkey, PureWall-X code, or speculative shared-core extraction is included.

### Verification evidence

- Task Pre-Flight `cargo check`: PASS.
- Task Pre-Flight `npx vue-tsc --noEmit`: PASS.
- `DECISIONS.md`: every ADR is accepted; no proposed ADR blocks Phase 2.
- Existing action-path audit confirmed that tray Like/Dislike currently depend on frontend relays, background rotation can swallow failures, and main/widget/CLI paths duplicate orchestration; the plan assigns each gap to a behavior-level test and a minimal compatibility-preserving change.
- No registry command was executed, no PureWall-owned registry entry changed, and no system setting changed while planning.

### Unresolved items

- Implementation has not started. Execution will use the previously selected subagent-driven workflow and an isolated branch/workspace while preserving the existing dirty working tree.

## 2026-08-01 — PureWall Phase 5 open-source release-loop implementation plan

### Planning scope

- Added `docs/superpowers/plans/2026-08-01-purewall-phase-5-release-loop.md` from the approved
  six-phase foundation design and the completed Phase 1-4 baseline.
- Audited the existing release-preparation checklist against the actual Phase 5 contract. The
  earlier checklist already covers local packaging, strict lint, initial safety tests, basic public
  docs, CI, and documented PureWall-owned cleanup, but it does not close tag-driven releases,
  Authenticode/updater signing, SHA-256/provenance, verified updates, disposable installer lifecycle
  evidence, or the complete community/repository policy surface.
- Split Phase 5 into six independently reviewed tasks: release trust boundary, public repository
  experience, trusted draft-release automation, explicit signature-verified updates, disposable
  install/upgrade/uninstall coverage, and final release-candidate review.
- Reserved ADR-031 for the release/update trust boundary because a pre-existing in-flight main
  change already owns ADR-030 and must merge without an identifier collision.

### Verification and safety

- Planning Pre-Flight `cargo check`: PASS, 0 errors.
- Planning Pre-Flight `npx vue-tsc --noEmit`: PASS, 0 errors.
- `DECISIONS.md`: no proposed ADR blocks Phase 5.
- Plan self-review: PASS — all approved Phase 5 deliverables map to a task, code fences are balanced,
  placeholder patterns are absent, updater/release interfaces are consistent, and external evidence
  gates are labeled rather than implied.
- Official Tauri updater/signing/pipeline documentation and GitHub artifact-attestation documentation
  were checked for the current signature, secret, draft-release, and provenance boundaries.
- No dependency, application code, release, tag, push, installer, native app, registry entry, user
  data, Windows policy, or system setting changed while planning. No new reusable pitfall was found,
  so `AI_DIARY.md` was intentionally not appended.

## 2026-08-01 — Phase 5 Task 1 release trust boundary complete

### Decision and implementation

- Accepted ADR-031 with the Phase 5 tag/version, draft-only release, updater-signature, Authenticode,
  secret-ownership, disposable-runner, and retained-identifier boundaries.
- Added `scripts/verify-release-contract.mjs` and `npm run verify:release`. The local verifier reads
  all three `0.1.0` declarations plus optional `PUREWALL_RELEASE_TAG`, rejects version or tag drift,
  and rejects updater configuration without updater artifacts, HTTPS endpoints, or secure transport.
- Added the privileged draft-release architecture boundary, explicitly separating Tauri updater
  signatures from Windows Authenticode and keeping signing material in GitHub Actions secrets only.
- Preserved `com.purewall.app`; no identifier, dependency, application runtime code, updater plugin,
  release workflow, or release state changed in this task.

### RED/GREEN evidence

- RED: with `PUREWALL_RELEASE_TAG=v9.9.9`, `npm run verify:release` exited 1 and emitted
  `tag v9.9.9 does not match v0.1.0`.
- GREEN: with no release tag, `npm run verify:release` passed and emitted
  `PureWall release contract OK for 0.1.0`.
- Pre-Flight `cargo check --manifest-path src-tauri/Cargo.toml`: the first cold-worktree run ended
  during `web_atoms` without a source diagnostic; an exact rerun passed after 1m29s with 0 errors.
- Pre-Flight `npx vue-tsc --noEmit`: PASS, 0 errors. `DECISIONS.md` contained no proposed ADR.
- Final `npm run verify:release`: PASS, `PureWall release contract OK for 0.1.0`.
- Final `cargo check --manifest-path src-tauri/Cargo.toml`: PASS, 0 errors.
- Final `npx vue-tsc --noEmit`: PASS, 0 errors.
- Final `git diff --check`: PASS.

### Process incident and safety boundary

- A controller incorrectly attributed a PureWall development process started at 16:46 to this task
  and terminated it around 17:15. The Task 1 implementation did not launch that process. The
  controller did not automatically restart it.
- After termination, app-data file metadata was not observed continuing to change. No inspection of
  real user-data contents was performed, so this is process/file-metadata evidence only, not a data-
  integrity claim. The reusable ownership guardrail is appended as
  `AI_DIARY.md #process-attribution-001`.
- Task 1 did not launch or interact with the native UI and did not execute an installer, registry,
  autostart, context-menu, wallpaper, release, tag, push, Windows policy, or system-setting action.
- **Unresolved**: None within Task 1. Signing, draft-release automation, updater UI, disposable
  installer lifecycle execution, and remote release evidence remain assigned to later Phase 5 tasks.

## 2026-08-01 — Phase 5 Task 1 Important review: strict SemVer correction

### Finding and RED evidence

- The Task 1 specification review found that the original release-contract regex checked the core
  version shape but used a broad character class for prerelease and build metadata.
- Executable self-test RED exited 1 because the original validator accepted invalid
  `1.0.0-01`, `1.0.0-alpha..1`, and `1.0.0+build..1`. It already rejected invalid `01.0.0`.

### Correction and GREEN evidence

- Replaced the permissive suffix pattern with explicit SemVer 2.0.0 core, prerelease-identifier,
  and build-identifier grammar. Numeric prerelease identifiers reject leading zeroes; every dot-
  separated prerelease/build identifier must be non-empty.
- Added `npm run test:release-contract` as an executable self-test over independent literals.
- GREEN: the self-test passed 3 valid values (`0.1.0`, `1.0.0-alpha.1`,
  `1.0.0+build.1`) and rejected 4 invalid values (`01.0.0`, `1.0.0-01`,
  `1.0.0-alpha..1`, `1.0.0+build..1`).
- Existing tag RED remained exact: `v9.9.9` exited 1 with
  `tag v9.9.9 does not match v0.1.0`.
- Existing normal GREEN remained exact: `npm run verify:release` emitted
  `PureWall release contract OK for 0.1.0`.
- Final `cargo check --manifest-path src-tauri/Cargo.toml`: PASS, 0 errors.
- Final `npx vue-tsc --noEmit`: PASS, 0 errors.
- Final `git diff --check`: PASS.

### Safety and remaining scope

- No dependency, release workflow, updater configuration, application runtime, native application,
  installer, registry, autostart, context-menu, wallpaper, tag, push, policy, or system-setting
  operation changed or ran.
- The reusable strict-SemVer trap is appended as `AI_DIARY.md #semver-validation-001`.
- **Unresolved**: None for the Important review finding. Later Phase 5 signing, updater, lifecycle,
  and remote evidence tasks remain unchanged.

## 2026-08-01 — Phase 5 Task 1 Minor review: normalized contract input errors

### Finding and correction

- The release verifier allowed a non-string updater endpoint to reach `startsWith`, and input file
  read or JSON parse failures escaped through Node's default stack output with an absolute local
  path.
- Moved release input reads and JSON parsing behind one caught input boundary. Every exception from
  that boundary now becomes the fixed message
  `release contract input error: unable to read or parse release inputs`; the original exception,
  stack, and local path are not printed.
- Extracted updater validation and reject any endpoint unless it is a string beginning with
  `https://`. The existing self-test now covers a numeric endpoint and an injected input exception.

### RED/GREEN and verification evidence

- Endpoint RED: the new numeric-endpoint self-test exited 1 with
  `TypeError: url.startsWith is not a function` and exposed the raw Node stack.
- Endpoint GREEN: after the type guard, `npm run test:release-contract` passed.
- Input RED: the injected reader exception exited 1 with its synthetic absolute path and raw Node
  stack. Input GREEN: the same self-test passed after the caught boundary returned only the stable
  error message.
- Missing-input CLI check from an unrelated working directory exited 1 with exactly
  `release contract input error: unable to read or parse release inputs`.
- Normal `npm run verify:release`: PASS, `PureWall release contract OK for 0.1.0`.
- Tag mismatch RED: `v9.9.9` exited 1 with exactly
  `tag v9.9.9 does not match v0.1.0`.
- Final `cargo check --manifest-path src-tauri/Cargo.toml`: PASS, 0 errors.
- Final `npx vue-tsc --noEmit`: PASS, 0 errors.
- Final `git diff --check`: PASS.

### Safety and remaining scope

- No dependency, release workflow, updater configuration, application runtime, native application,
  installer, registry, autostart, context-menu, wallpaper, tag, push, policy, or system-setting
  operation changed or ran.
- No new reusable pitfall was found, so `AI_DIARY.md` was intentionally not appended.
- **Unresolved**: None for this Minor review finding. Later Phase 5 tasks remain unchanged.

## 2026-08-01 — Phase 5 Task 2 public repository experience complete

### RED and implementation

- Extended `scripts/verify-release-contract.mjs` with eight required public repository paths.
- RED: `npm run verify:release` exited 1 and listed every missing file: the four issue-template
  YAML files, pull-request template, `SUPPORT.md`, `ROADMAP.md`, and
  `docs/REPOSITORY_POLICY.md`.
- Added scoped ignore rules for `.agents/`, `.codex/`, `skills-lock.json`, Python caches, `output/`,
  certificate/private-key residue, and generated release signing configuration. The committed
  `design/PureWall-Living-Gallery-Fusion.png` and `design/purewall-icon-motion.gif` remain tracked
  and are not ignored.
- Added `.github/ISSUE_TEMPLATE/bug.yml`, `feature.yml`, `safety.yml`, and `config.yml`, plus
  `.github/pull_request_template.md`. Public security-sensitive reports are routed to GitHub private
  security advisories without asking for exploit details.
- Added `SUPPORT.md`, `ROADMAP.md`, and `docs/REPOSITORY_POLICY.md`; expanded `README.md`,
  `SECURITY.md`, and `CONTRIBUTING.md` with local-library onboarding, clean-clone/build guidance,
  support/security routing, repository artifact policy, full verification gates, Phase 6 direction,
  and explicit PureWall-X/shared-core deferrals.
- README uses only the two already committed design assets and labels them as a product interface
  preview and icon motion. It distinguishes planned release checks from evidence that actually ran;
  signing, provenance, updater delivery, remote CI, publication, and installer lifecycle are not
  claimed complete.
- Replaced the obsolete macOS-inspired `src-tauri/tauri.conf.json` package description with an
  accurate local-first Windows wallpaper manager description.
- Marked all Task 2 steps complete in
  `docs/superpowers/plans/2026-08-01-purewall-phase-5-release-loop.md`.

### GREEN and documentation evidence

- `npm run test:release-contract`: PASS, strict SemVer self-test kept 3 valid / 4 invalid cases.
- `npm run verify:release`: PASS, `PureWall release contract OK for 0.1.0`.
- `npx vue-tsc --noEmit`: PASS, 0 errors.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS, 0 errors.
- `git diff --check`: PASS; only informational LF-to-CRLF working-copy notices were emitted.
- GitHub issue YAML parse: PASS, 4/4 files through the local read-only YAML parser.
- Public documentation semantic/link contract: PASS (`PUBLIC_DOC_CONTRACT_OK`).
- Ignore-policy positive/negative contract: PASS (`IGNORE_CONTRACT_OK`); local agent/cache/output/
  signing residue is ignored while approved `design/` and `docs/images/` paths remain available.

### Safety and remaining scope

- No native application, installer, release, tag, push, updater, registry, autostart, context menu,
  wallpaper, display, user-data, Windows policy, or system-setting operation ran.
- Task 3-6 still own signed draft-release automation, checksums/provenance, the explicit verified
  update experience, disposable install/upgrade/uninstall smoke, remote evidence, final review, and
  publication decision. Those items remain **NOT RUN / NOT CLAIMED** here.
- The built-in patch-wrapper restriction was already documented by `AI_DIARY.md #sandbox-005`; no
  new reusable pitfall was found, so `AI_DIARY.md` was intentionally not appended.
- **Unresolved**: None within Phase 5 Task 2.

## 2026-08-01 — Phase 5 Task 2 quality review hardening complete

### Corrected review findings and RED evidence

- The earlier `IGNORE_CONTRACT_OK` evidence did not include the decisive nested
  `src/output/generated.ts` counterexample and is withdrawn as proof of root-only scope. The
  unanchored `output/`, `.agents/`, `.codex/`, and `skills-lock.json` rules could hide same-named
  source subtrees.
- The earlier YAML parse proved syntax only. It did not prove that required public paths were
  regular/non-empty files or that the four issue configurations retained their necessary schema
  markers and private-security routing.
- Added persistent review tests before the fixes. Initial `npm run test:release-contract` exited 1
  with ten expected failures: directory acceptance, empty-file acceptance, four broken issue
  configurations, nested `src/output` being ignored, missing first-item bug security guidance,
  the unverifiable SECURITY profile fallback, and the missing README Releases link.
- Incremental reruns removed only the finding being fixed: the ignore-scope failure disappeared
  after anchoring, the bug-form failure disappeared after moving the warning first, structural/schema
  fixture failures disappeared after the validator implementation, SECURITY disappeared after the
  fallback removal, and README was the final remaining RED before its Releases links were added.

### Implementation

- Anchored local-only project rules as `/.agents/`, `/.codex/`, `/skills-lock.json`, and `/output/`.
  Kept explicit cross-tree cache rules such as `**/__pycache__/` unchanged.
- Made the first `bug.yml` body item a Markdown warning that forbids public vulnerability,
  destructive-detail, and secret disclosure and links directly to GitHub private advisories.
- Replaced the release verifier's existence-only check with `statSync` plus readable, non-whitespace
  regular-file validation. The shared contract checks required schema/content markers for
  `bug.yml`, `feature.yml`, `safety.yml`, and `config.yml`, plus the security and Releases links.
- Added a persistent real-Git ignore probe proving `output/generated.ts` is ignored while
  `src/output/generated.ts` remains visible. In-memory stat/read fixtures prove directories, empty
  files, and obviously broken issue configurations are rejected without creating temporary files.
- Removed SECURITY's unverifiable GitHub-profile fallback. If private advisories are unavailable,
  reporters are told not to disclose publicly and to wait for that channel to be enabled.
- Linked both the first-download step and matching-release guidance directly to
  `https://github.com/WiseZenn/PureWall/releases` without claiming that a release exists.
- Files changed: `.gitignore`, `.github/ISSUE_TEMPLATE/bug.yml`, `README.md`, `SECURITY.md`,
  `scripts/verify-release-contract.mjs`, `docs/project-docs/CHANGELOG_AI.md`, and
  `docs/project-docs/AI_DIARY.md`.

### GREEN and safety evidence

- `npm run test:release-contract`: PASS; strict SemVer, injected structural/schema failures, live
  public files, and root-vs-nested Git ignore scope are covered.
- `node --check scripts/verify-release-contract.mjs`: PASS.
- `npm run verify:release`: PASS, `PureWall release contract OK for 0.1.0`.
- Independent ignore matrix: PASS (`IGNORE_SCOPE_OK`) for four root-only rules, one cross-tree cache
  rule, four nested-path negatives, and two approved public-asset negatives.
- Read-only YAML/schema check: PASS, 4/4 files; `bug.yml.body[0].type` is `markdown` and contains the
  private advisory URL.
- `npx vue-tsc --noEmit`: PASS, 0 errors.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS, 0 errors.
- `git diff --check`: PASS; only informational LF-to-CRLF working-copy notices were emitted.
- No native application, installer, release, tag, push, updater, registry, autostart, context menu,
  wallpaper, display, user-data, Windows policy, or system-setting operation ran.
- Signing, provenance, updater delivery, remote CI, installer lifecycle evidence, and publication
  remain **NOT RUN / NOT CLAIMED** and stay assigned to later Phase 5 tasks.
- The PowerShell native-stdin CR behavior was new and reusable, so it was appended as
  `AI_DIARY.md #powershell-stdin-001`.
- **Unresolved**: None for the Task 2 quality review findings.

## 2026-08-01 — Phase 5 Task 2 final quality review: structural Issue Form validation

### Evidence correction and TDD

- The earlier claim that YAML schema markers plus a separate 4/4 syntax parse sufficiently proved
  the Issue Form contract is withdrawn. Raw indentation regexes and string markers did not validate
  the parsed object and could both accept malformed YAML and reject legal YAML formatting.
- Added the structural fixtures before changing the validator. RED exited 1 because the old path
  accepted an unclosed `[`, a duplicate top-level key, and a mapping nested under `body`, while it
  rejected a valid four-space sequence with a comment immediately after `body:`.
- GREEN passed after the validator switched to the declared `yaml` parser and object-level checks.
  The persistent self-test covers all four counterexamples as part of
  `npm run test:release-contract`.

### Implementation and security boundary

- Added exact direct devDependency `yaml@2.9.0`, the current npm registry 2.x formal release at the
  time of implementation, and regenerated `package-lock.json`; the verifier does not rely on an
  undeclared transitive parser.
- Removed the Issue Form indentation regex/marker table. `scripts/verify-release-contract.mjs` now
  calls `parse(content, { uniqueKeys: true })`, rejects parse failures and non-object roots, and
  validates parsed semantics: non-empty `name` / `description` / `body` for bug, feature, and safety
  forms; a Markdown first bug item containing the private advisory link; the safety form's private
  advisory link; and config `blank_issues_enabled === false` plus array-valued `contact_links`.
- Defined GitHub Private Vulnerability Reporting / Private Security Advisory in `SECURITY.md` as a
  mandatory prerequisite for the first public release and every later release. If the channel is
  unavailable, the release process is blocked: maintainers must not publish and reporters must not
  disclose publicly. `CONTRIBUTING.md` mirrors this boundary once; no email or profile fallback was
  invented.
- Files changed: `package.json`, `package-lock.json`, `scripts/verify-release-contract.mjs`,
  `SECURITY.md`, `CONTRIBUTING.md`, `docs/project-docs/CHANGELOG_AI.md`, and
  `docs/project-docs/AI_DIARY.md`.

### Verification and safety evidence

- `node --check scripts/verify-release-contract.mjs`: PASS.
- `npm run test:release-contract`: PASS; output names structural YAML coverage.
- `npm run verify:release`: PASS, `PureWall release contract OK for 0.1.0`.
- Direct `yaml@2.9.0` parse of `bug.yml`, `feature.yml`, `safety.yml`, and `config.yml`: PASS 4/4,
  with every parsed document confirmed as a top-level object.
- `npx vue-tsc --noEmit`: PASS, 0 errors.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS, 0 errors.
- `npm audit`: PASS, 0 vulnerabilities.
- `npm audit --omit=dev`: PASS, 0 vulnerabilities.
- `git diff --check`: PASS; only informational LF-to-CRLF working-copy notices were emitted.
- No native application, installer, release, tag, push, updater, registry, autostart, context menu,
  wallpaper, display, user-data, Windows policy, or system-setting operation ran.
- The reusable parser-vs-marker trap was appended as `AI_DIARY.md #yaml-contract-001`.
- **Unresolved**: None for this final Task 2 quality review. Later Phase 5 tasks remain unchanged.

## 2026-08-08 — Phase 5 Task 4: signature-verified application updates

### 变更目标
增加仅由用户显式触发的签名验证更新检查、下载和安装路径。

### 代码范围
- `src-tauri/Cargo.toml` / `Cargo.lock`: 引入 `tauri-plugin-updater = "2"`。
- `src-tauri/src/app_updates.rs`: 新增 pending update 单次所有权、元数据边界、稳定错误和 Started/Progress/Finished 下载事件。
- `src-tauri/src/main.rs`: 初始化 updater plugin、管理 pending 状态，并仅注册 `fetch_update` / `install_update`。
- `src/stores/appUpdates.ts`: 新增 idle/checking/current/available/downloading/readyToRestart/error 状态机，忽略并发操作。
- `src/components/UpdateSettings.vue`, `InspectorPanel.vue`, `styles.css`: Settings 中提供显式检查、版本说明、下载进度、重试和 Windows 安装退出提示。
- `docs/project-docs/ARCHITECTURE.md`: 记录 updater trust boundary 和 pending ownership。

### 验证证据
- `/mnt/c/Users/zhong/.cargo/bin/cargo.exe check --manifest-path src-tauri/Cargo.toml`: PASS（依赖解析及现有 Rust 代码编译通过）。
- `npx vue-tsc --noEmit`: PASS（安装现有 package.json 依赖后重跑）。
- `npx vitest run src/stores/appUpdates.test.ts src/components/updateSettings.test.ts`: PASS（2 files, 4 tests）。
- `npm run test:unit`: PASS（27 files, 79 tests）。
- `npm run build`: PASS（Vite build；依赖的既有 Rolldown annotation warnings only）。
- `cargo test ... app_updates`: PASS（3 tests）；`cargo clippy ... -D warnings`: PASS；`cargo fmt -- --check`: PASS。
- 未联系 update endpoint，未启动 native app。

### 未解决项
- 尚未执行真实签名 release 或 installer lifecycle；updater endpoint、signature、Windows installer 行为仍需 CI/draft release evidence。


## 2026-08-01 — Phase 5 Task 3: tag-driven signed draft releases, checksums, and provenance

### RED and implementation

- Extended `scripts/verify-release-contract.mjs` with a release-workflow contract that FAILS
  while `.github/workflows/release.yml` is missing or lacks any of: `tags:`,
  `PUREWALL_RELEASE_TAG`, `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PUBLIC_KEY`,
  `WINDOWS_CERTIFICATE`, `releaseDraft: true`, `actions/attest@v4`, `SHA256SUMS.txt`.
  RED evidence: `npm run verify:release` and `npm run test:release-contract` both exited 1
  with `missing release workflow file: .github/workflows/release.yml` before the workflow
  existed.
- Added `"createUpdaterArtifacts": true` to `src-tauri/tauri.conf.json` bundle. No fake public
  key or incomplete updater block was added to the checked-in config; the real updater public
  key and fixed endpoint are injected only through the ignored generated config.
- Created `scripts/prepare-release-config.ps1`: requires the four signing inputs
  (`WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_PASSWORD`, `TAURI_SIGNING_PRIVATE_KEY`,
  `TAURI_SIGNING_PUBLIC_KEY`, plus optional `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`), decodes the
  PFX into `$env:RUNNER_TEMP` only, imports into `Cert:\CurrentUser\My` and captures the
  thumbprint, rejects empty/non-HTTPS timestamp URLs, and writes
  `src-tauri/tauri.release.generated.conf.json` as UTF-8 without BOM (LF-normalized) containing
  only `certificateThumbprint`, `digestAlgorithm: sha256`, `timestampUrl`, `createUpdaterArtifacts`,
  the updater pubkey, and the fixed
  `https://github.com/WiseZenn/PureWall/releases/latest/download/latest.json` endpoint. Secrets
  and generated JSON are never printed. `-ConfigOnly` dry-run validates inputs, round-trips the
  JSON through `ConvertFrom-Json`, and writes the file without importing a certificate.
- Created `scripts/collect-release-artifacts.ps1`: `-BundleRoot`/`-OutputDirectory` must resolve
  inside the workspace; copies only `.msi`, setup `.exe`, `.sig`, and `latest.json` sorted by
  name; computes SHA-256 with `Get-FileHash`; writes `SHA256SUMS.txt` via `UTF8Encoding(false)`;
  fails when MSI, NSIS setup EXE, updater signatures, or `latest.json` are absent; `-SelfTest`
  covers containment, allowlist, and required-set helpers with synthetic inputs.
- Created `.github/workflows/release.yml`: triggers on `v*` SemVer tags plus `workflow_dispatch`;
  permissions `contents: write`, `id-token: write`, `attestations: write`; Windows job runs
  `npm ci`, full `npm audit`, `vue-tsc`, unit tests, frontend build, Rust fmt/check/test/clippy,
  validates `PUREWALL_RELEASE_TAG` against the version declarations via `npm run verify:release`,
  prepares the secret-backed config, builds with `tauri-apps/tauri-action@v1`
  (`releaseDraft: true`, `args: --config src-tauri/tauri.release.generated.conf.json`), verifies
  Authenticode `Valid` on every MSI/EXE, collects artifacts and `SHA256SUMS.txt`, uploads the
  checksum to the draft, and attests the `release-artifacts` directory with `actions/attest@v4`.
  Manual dispatch passes an empty `tagName` (verified against tauri-action v1 source: releases
  are created only when `tagName` is truthy) so no GitHub Release is created unless
  `publishDraft` is explicitly true; tag pushes pass `github.ref_name`.
- `README.md` and `CONTRIBUTING.md` document secret/variable names, updater-key and
  certificate rotation/expiry risk, HTTPS timestamp requirements, draft review, `Get-FileHash`
  verification, `gh attestation verify <artifact> --repo WiseZenn/PureWall`, and the
  secret-free local dry-run commands. No real secret values are documented.

### GREEN and safety evidence

- `npm run test:release-contract`: PASS; self-test now also covers missing/empty/incomplete
  release.yml fixtures and a complete release-workflow fixture, plus the live release.yml.
- `npm run verify:release`: PASS, `PureWall release contract OK for 0.1.0`.
- `powershell -ExecutionPolicy Bypass -File scripts/prepare-release-config.ps1 -ConfigOnly
  -UpdaterPublicKey 'LOCAL_TEST_PUBLIC_KEY' -CertificateThumbprint
  '0000000000000000000000000000000000000000' -TimestampUrl 'https://timestamp.test.invalid'`:
  PASS (exit 0), generated config round-trips through ConvertFrom-Json, written UTF-8 no BOM,
  LF line endings; negative cases (empty or `http://` timestamp URL, missing
  `-CertificateThumbprint`) exit 1.
- `scripts/collect-release-artifacts.ps1 -SelfTest`: PASS (containment, allowlist, required
  set). End-to-end run against a synthetic bundle inside the workspace copied only the five
  allowed files sorted by name, wrote matching SHA-256 lines (spot-checked against
  `sha256sum`), exited 1 for a bundle missing `latest.json` and for a bundle root outside the
  workspace.
- `npx vue-tsc --noEmit`: PASS, 0 errors.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS, 0 errors (cargo 1.95.0).
- `git diff --check`: PASS.
- No certificate import, registry write, release, tag, push, or installer execution occurred;
  the generated `src-tauri/tauri.release.generated.conf.json` is gitignored.

### Remaining scope and pre-existing conditions

- `.github/workflows/release.yml` includes a full `npm audit` step per the plan; the current
  dependency tree still carries one pre-existing high-severity transitive advisory
  (`nanoid <3.3.17` via the Vite toolchain, `GHSA-2v37-7h3g-55p8`), so the release job cannot
  reach signing until that advisory is resolved. This is a pre-existing repository condition
  outside Task 3 scope and is recorded for the maintainers.
- Installer lifecycle smoke coverage (Task 5) remains not implemented; real CI, signing
  secrets, tag push, draft release, and native smoke remain **NOT RUN / NOT CLAIMED**.
- **Unresolved**: None within Phase 5 Task 3.

## 2026-08-01 — Phase 5 Task 5: disposable clean-install, upgrade, and uninstall smoke coverage

### RED and implementation

- Extended `scripts/verify-release-contract.mjs` so `npm run verify:release` also requires
  `.github/workflows/installer-smoke.yml` to exist and contain `workflow_dispatch`,
  `-DisposableRunner`, `SHA256SUMS.txt`, and `gh attestation verify`, and to be free of
  `Remove-Item -Recurse`, `HKLM`, `Policies`, and the Windows 11 context-menu CLSID
  (`86ca1aa0-34aa-4e8b-a509-50c905bae2a2`). RED evidence: both the self-test and
  `npm run verify:release` exited 1 with
  `missing installer smoke workflow file: .github/workflows/installer-smoke.yml` before
  the workflow existed.
- Created `scripts/verify-install-lifecycle.ps1` with `-SelfTest` covering pure helpers
  for path containment, MSI extension validation, version ordering, the PureWall-owned
  HKCU registry allowlist (exactly the seven documented identities including the
  `Run\PureWall` value), and redacted command reporting (`%USERPROFILE%`/`%LOCALAPPDATA%`
  substitution). The main flow requires Windows and an explicit `-DisposableRunner` for
  mutation, verifies Authenticode before install, snapshots only the PureWall-owned HKCU
  keys plus the Run value, installs the previous MSI when supplied else performs a clean
  current install, verifies the installed `PureWall.exe` exists and reports the expected
  file version WITHOUT launching it, upgrades when a previous MSI was supplied,
  uninstalls via `msiexec /x` with the exact current MSI path, verifies the binary is
  gone, compares the registry snapshot and reports residue read-only, stops on every
  non-zero msiexec exit except the documented reboot-required codes (3010, 1641), and
  never enumerates/deletes arbitrary directories or writes HKLM/policy/Win11-CLSID keys.
- Created `.github/workflows/installer-smoke.yml`: `workflow_dispatch` inputs for the
  current tag and an optional previous tag; `gh release download` for the MSI and
  `SHA256SUMS.txt`; SHA-256 and `gh attestation verify` checks before any install; runs
  the script with `-DisposableRunner`. Permissions are only `contents: read`,
  `id-token: write`, and `attestations: read`; the workflow never publishes or modifies
  a release.
- `CONTRIBUTING.md` documents the smoke boundary: disposable `windows-latest` runners
  only, workflow_dispatch only, the exact mutation guards and registry scope, the
  reboot-required exit codes, and that the workflow remains **NOT RUN** until a signed
  draft or published release exists.

### GREEN and safety evidence

- `npm run test:release-contract`: PASS; self-test now also covers missing/incomplete
  smoke workflow fixtures, forbidden-marker fixtures for all four forbidden strings, a
  complete smoke workflow fixture, and the live installer-smoke.yml.
- `npm run verify:release`: PASS, `PureWall release contract OK for 0.1.0`.
- `powershell -ExecutionPolicy Bypass -File scripts/verify-install-lifecycle.ps1
  -SelfTest`: PASS (path containment, MSI validation, version ordering, registry
  allowlist, redacted commands).
- Main-flow guards verified without mutation: exit 1 without `-DisposableRunner`; exit 1
  at the Authenticode gate when given an unsigned dummy MSI with `-DisposableRunner`
  (no install attempted).
- `npx vue-tsc --noEmit`: PASS, 0 errors.
- `cargo check --manifest-path src-tauri/Cargo.toml`: PASS, 0 errors.
- `git diff --check`: PASS.
- `-DisposableRunner` was NOT passed locally; no MSI execution, native app launch,
  registry write, uninstall, or system-setting action occurred.

### Remaining scope and pre-existing conditions

- The installer smoke workflow is executable coverage that stays **NOT RUN** until a
  signed draft or published release and an optional prior version exist; real lifecycle
  evidence must come from the disposable runner.
- Real CI, signing secrets, tag push, draft release, and native smoke remain
  **NOT RUN / NOT CLAIMED**.
- **Unresolved**: None within Phase 5 Task 5.

## 2026-08-08 — Phase 5 Task 6: final review, full gate, and release-candidate handoff

### Change goal

Close the Phase 5 open-source release loop with an independent whole-branch security review, a fresh full local gate, and honest release-boundary documentation.

### Independent whole-branch review

- Reviewer (openai-codex/gpt-5.6-luna, fresh context, read-only) reviewed the full Phase 5 diff (base `d3b1829` -> HEAD). Verdict: CONDITIONAL; Critical: none.
- Fixed all 4 Important findings:
  1. `release.yml`: manual `publishDraft` now compared as boolean; new `Resolve and verify release tag` step enforces strict `v<SemVer>`, confirms the tag exists via `git ls-remote`, and checks out the exact tag before building.
  2. `app_updates.rs`: `restore_after_failed_install` restores the pending update on failed install (unless a newer fetch replaced it), so user-visible Retry reuses verified metadata instead of surfacing `NO_PENDING_UPDATE`; added 2 Rust tests.
  3. `collect-release-artifacts.ps1`: rejects duplicate artifact basenames and unexpected pre-existing output files so checksum coverage and attestation subjects stay consistent.
  4. `verify-install-lifecycle.ps1`: MSI containment now resolves the final filesystem target (reparse-following) before the temp-root check.
- Fixed both Minor findings: README updater claim corrected; `installer-smoke.yml` dropped unnecessary `id-token: write`.

### Full local gate (all PASS)

- `cargo fmt -- --check`, `cargo check`: PASS.
- `cargo test`: 129 passed, 1 ignored (app_updates: 5 tests incl. retry/restore precedence).
- `cargo clippy --all-targets -- -D warnings`: PASS.
- `npx vue-tsc --noEmit`: PASS; `npm run test:unit`: 51 files / 151 tests PASS.
- `npm audit` + `npm audit --omit=dev`: 0 vulnerabilities (nanoid 3.3.18).
- `npm run build`: PASS (only upstream pure-annotation warnings).
- `npm run verify:release` + `--self-test`: PASS.
- `verify-install-lifecycle.ps1 -SelfTest`, `collect-release-artifacts.ps1 -SelfTest`, `prepare-release-config.ps1 -ConfigOnly` dry-run: PASS (no cert import, no registry/system action).
- `git diff --check`: PASS.

### Release boundaries (honest status)

- GITHUB CI: NOT RUN until pushed.
- SIGNING SECRETS: BLOCKED until provisioned in GitHub Actions secrets.
- TAG-DRIVEN DRAFT RELEASE: NOT RUN until an authorized tag push.
- AUTHENTICODE/UPDATER ARTIFACT VERIFICATION: NOT RUN until the draft release job runs.
- INSTALL/UPGRADE/UNINSTALL SMOKE: NOT RUN until invoked on a disposable runner.
- PUBLIC RELEASE: NOT CREATED/PUBLISHED.
- Identifier `com.purewall.app` preserved; migration needs a separate ADR + data migration.

### Files changed

- `.github/workflows/release.yml`, `.github/workflows/installer-smoke.yml`
- `src-tauri/src/app_updates.rs` (restore-on-failed-install + tests)
- `scripts/collect-release-artifacts.ps1`, `scripts/verify-install-lifecycle.ps1`
- `README.md`, `CHANGELOG.md`, `docs/project-docs/OPEN_SOURCE_RELEASE_REVIEW.md`, `docs/project-docs/CHANGELOG_AI.md`

### Unresolved items

- Push + GitHub CI, signing-secret provisioning, tag push, draft release, and disposable-runner installer smoke remain external gates (NOT RUN).

## 2026-08-08 — Phase 6 Task A: split database responsibility modules

### 变更目标
在不改变 SQLite schema、Database 公共方法签名或 main.rs 调用点的前提下，按职责拆分 `db.rs`。

### 代码范围
- `src-tauri/src/db.rs`: 保留 Database/连接与迁移、核心壁纸 CRUD/播放/统计/设置/checkpoint、数据结构和原有 `db::tests`；通过路径模块声明接入拆分实现。
- `src-tauri/src/db_taxonomy.rs`: tags、collections、rating、blacklist、display title 与 batch 操作。
- `src-tauri/src/db_sources.rs`: watched folders、来源移除/迁移、availability reconciliation 与 registered available paths。
- `src-tauri/src/db_backup.rs`: backup snapshot 查询辅助函数及 `ExistingBackupWallpaper`。
- 未修改 `src-tauri/src/main.rs`，无 schema/dependency/command-name 变更。

### 验证证据
- 基线（提取前）：`cargo test ... db` 26 passed；完整 `cargo test` 129 passed, 1 ignored。
- taxonomy 提取后：db 26 passed；完整 129 passed, 1 ignored；`cargo check` PASS；`cargo clippy --all-targets -- -D warnings` PASS。
- sources 提取后：db 26 passed；完整 129 passed, 1 ignored；`cargo check` PASS；`cargo clippy --all-targets -- -D warnings` PASS。
- backup 提取后：db 26 passed；完整 129 passed, 1 ignored；`cargo check` PASS；`cargo clippy --all-targets -- -D warnings` PASS。
- `cargo fmt` 已执行；`git diff --check` PASS。
- `npx vue-tsc --noEmit` 受当前 npx 临时 TypeScript `./lib/tsc` exports 错误阻塞，未触及前端代码。

### 未解决项
前端 TypeScript gate 需在可用的 workspace `vue-tsc`/TypeScript 安装环境中重跑；Rust gate 全部通过。


## 2026-08-08 — Phase 6 Task B: extract Rust command groups

### Change goal
Split `src-tauri/src/main.rs` command handlers into focused command modules without changing command names, registration, state fields, schema, or runtime orchestration.

### Code scope
- Added `src-tauri/src/commands.rs` and `src-tauri/src/commands/{library_sources,imports,taxonomy,playback,backup,media}.rs`.
- Moved the requested source, import, taxonomy/batch, playback, backup, and media command groups while retaining shared validation/orchestration helpers in `main.rs` where they are used by watchers, workers, timers, CLI handling, or tests.
- Updated the invoke handler to reference the extracted module paths; all 67 Tauri command names remain present.
- `main.rs` remains the application entrypoint, AppState/error boundary, watcher/media/timer/focus/CLI orchestration, context-menu/autostart boundary, registration boundary, and full `main_tests` owner.

### Verification evidence
- Baseline before extraction: `cargo test` — 129 passed, 1 ignored.
- After extraction: `cargo test` — 129 passed, 1 ignored; `cargo check` — PASS; `cargo fmt -- --check` — PASS; `cargo clippy --all-targets -- -D warnings` — PASS.
- Command-name parity check: baseline and extracted source each contain 67 `#[tauri::command]` functions with identical names.
- `npx vue-tsc --noEmit` was attempted but could not start in this environment because the transient `vue-tsc`/TypeScript package raised `ERR_PACKAGE_PATH_NOT_EXPORTED`; no frontend files were changed.

### Unresolved items
- None in the Rust extraction. Frontend type-check evidence remains unavailable until the local npm/vue-tsc toolchain is restored.


## 2026-08-08 — Phase 6 Task C: extract wallpaper store parts

### Change goal

Split `src/stores/wallpapers.ts` (2052 lines) along responsibility boundaries while freezing the public store facade: every public property/action keeps its exact name and signature, components and store tests are untouched, and the parts modules stay plain TS with typed context objects (no Pinia, no circular imports).

### Code scope

- `src/stores/wallpapers.ts` (2052 → 912 lines): keeps the `defineStore` shell, all state refs/reactive maps (wallpapers/tags/collections/librarySources/librarySourceBusy/Errors/thumbnails/previews/thumbnailErrors/imageMetadata/shellMetadata/selectedPaths/currentFilter/sortMode/wallpaperViewMode/workspaceSection/searchQuery/page/selection/isPaused/display/focus/stats refs, etc.), selection + listeners (`setupListeners`/`teardownListeners`), array plumbing (`setWallpapers`/`patchWallpaper`/`snapshotWallpapers`/`restoreSnapshots`/`removePaths`/`pruneSelection`/`ensureActiveWallpaper`), page loading (`loadWallpapers`/`loadMoreWallpapers`/`applyWallpaperPage`/`bootstrapActiveWallpaper`), import flows, the typed context objects, thin facade wrappers with identical names/signatures, and the byte-identical `return` object.
- New `src/stores/parts/mediaState.ts` (~430 lines): thumbnail/preview cache + LRU pruning, retry timers, `toAssetUrl`/`waitForNextFrame`, `cacheThumbnailPath`/`commitThumbnailResult`/`handleThumbnailGenerationFailure`/`handleThumbnailLoadError`/`retryThumbnail`, `loadThumbnailsForPaths`/`loadAllThumbnails`/`loadPreview`/`loadActivePreview`, active-media presentation (`scheduleSpeculativePreviewWarmup`/`syncActiveMediaPresentation`/`scheduleActivePreviewLoad`), metadata loaders.
- New `src/stores/parts/playbackState.ts` (~160 lines): `runPlaybackAction` + `reconcilePlaybackCompletion`/`enqueuePlaybackReconciliation` (queue holders passed via context), plus pause/display/focus command wrappers.
- New `src/stores/parts/taxonomyState.ts` (~360 lines): tag/collection/rating/blacklist/delete commands, `runBatchMutation` + `batchSetRating`/`batchAssignTag`/`batchUnassignTag`/`batchAssignCollection`/`batchUnassignCollection`/`batchBlacklist`/`batchDelete`, `restoreHidden`.
- New `src/stores/parts/sourcesState.ts` (~230 lines): `runLibrarySourceOperation`/`runBackupOperation` + rescan/retry/remove/relocate/export/preview/import helpers.
- New `src/stores/parts/types.ts` (~250 lines): shared DTO/interfaces; the store re-exports all public type names so `import type { ... } from "../stores/wallpapers"` call sites are unchanged.
- Queue/latch state (`playbackCommandQueue`, `playbackReconciliationQueue`, `activePreviewLoadKey`, `speculativeWarmGeneration`) moved from plain `let`s to `{ value }` holders owned by the store and passed through context, so per-instance isolation is preserved.

### Architecture notes

- Parts modules never import `../wallpapers`; each receives a typed `MediaStateContext`/`PlaybackStateContext`/`TaxonomyStateContext`/`SourcesStateContext` with the exact refs/callbacks it needs.
- No runtime circular imports; type-only imports resolve against `parts/types.ts`.
- `WALLPAPER_PAGE_SIZE`/`SEARCH_RELOAD_DEBOUNCE_MS` stay in the store; media/taxonomy tuning constants moved into their owning part module.

### Verification evidence

- `npx vue-tsc --noEmit`: PASS, 0 errors (baseline identical).
- `npx vitest run`: PASS — 27 files / 79 tests, same count as the pre-change baseline; store tests (`wallpaperCommandRouting`, `libraryBackup`, `batchOperations`, `librarySources`, `startupActiveWallpaper`, `activeMedia`) unchanged and green.
- `npm run build` (vue-tsc + vite build): PASS — only the pre-existing upstream `@vueuse/core` pure-annotation warnings.
- `git diff --check`: PASS.
- Public facade check: the store `return` object is byte-identical to the pre-refactor version (verified by diff against `HEAD:src/stores/wallpapers.ts`).

### Files changed

- `src/stores/wallpapers.ts` (refactor)
- `src/stores/parts/types.ts` (new)
- `src/stores/parts/mediaState.ts` (new)
- `src/stores/parts/playbackState.ts` (new)
- `src/stores/parts/taxonomyState.ts` (new)
- `src/stores/parts/sourcesState.ts` (new)
- `docs/project-docs/CHANGELOG_AI.md`

### Unresolved items

- None within Phase 6 Task C scope; no behavior change, no new dependencies, no component/test/`src-tauri`/`styles.css` changes.


## 2026-08-08 — Phase 6 Task D: section markers in src/styles.css

### 变更目标
为 `src/styles.css`（3944 → 3977 行）添加章节结构标记（`/* === Section: NAME === */`），按职责给现有规则分组，**纯注释改动，零视觉/行为变化**。

### 代码范围
- `src/styles.css`：仅插入 34 个章节标记（33 个新增 + 1 个原有 `/* Living Gallery fusion */` 转换为标准格式），不移动任何规则、不修改任何选择器/值。覆盖章节：Tailwind directives、Theme tokens、Base reset、A11y base、Form controls + focus states、App icons、App shell + titlebar、Search toolbar、Window controls + titlebar buttons、Quiet Canvas、Shared button states + glyphs、Workbench grid、Status bar、Toggle switch、Sidebar + navigation、Gallery workspace + current wallpaper、Gallery toolbar + selection bar、Gallery scroll + wallpaper grid、Wallpaper cards + tiles、Inspector panel + metadata + colors、Tag combobox + compact dropdown、Insights panel、System panel + settings + updates、Empty states + transitions + widget view、Scrollbars、Responsive + container queries、Living Gallery (fusion)、Living Gallery stage、Stage quick settings + compact dropdown、Light theme overrides、Living Gallery toolbar + curated rows、Living Gallery responsive、Notifications、Workspace states。

### 验证证据
- 注释剥离 diff：`grep -v "^\s*/\* === Section:"` 后与 HEAD 逐字节一致（IDENTICAL），证明零规则改动。
- `npx vue-tsc --noEmit`：PASS。
- `npm run test:unit`（vitest run）：27 文件 / 79 测试全部 PASS，含读取 styles.css 的 `responsiveContract`、`semanticSurfaces`、`stageTitleContrast`。
- `npm run build`：PASS（基线同款上游 pure-annotation 警告，无新增）。
- `git diff --check`：PASS。

### 未解决项
- 无。`src-tauri/*`、`src/stores/*` 未触碰（其他 lane 范围）。

## 2026-08-08 — Phase 6 Task E: final gate and closeout

### Change goal

Verify the merged Phase 6 extraction (Tasks A-D) as one whole and close the maintainability milestone.

### Final full gate (all PASS)

- `cargo fmt -- --check`: PASS.
- `cargo check`: PASS, 0 errors.
- `cargo test`: 129 passed, 1 ignored (unchanged from pre-refactor baseline).
- `cargo clippy --all-targets -- -D warnings`: PASS.
- `npx vue-tsc --noEmit`: PASS.
- `npm run test:unit`: PASS, 51 files / 151 tests (unchanged).
- `npm audit` (+ `--omit=dev`): 0 vulnerabilities.
- `npm run build`: PASS.
- `npm run verify:release` + self-test: PASS.
- `git diff --check`: PASS; working tree clean.

### Evidence of behavior preservation

- All 67 `#[tauri::command]` names preserved; `invoke_handler` registers them via `commands::*` paths.
- `main_tests` module retained in main.rs.
- Public store facade frozen: components were not touched in Task C.
- SQLite schema and migration order unchanged (Task A).
- styles.css: comment-only change; responsive/semantic/stage-contrast unit tests pass.

### Resulting file sizes

- `db.rs` 3808 -> 990 lines; `main.rs` 3684 -> ~2070; `wallpapers.ts` 2052 -> ~700; `styles.css` 3944 -> 3977 (markers only).
- New modules: `db_taxonomy.rs` (364), `db_sources.rs` (488), `db_backup.rs` (157), `commands/{library_sources,imports,taxonomy,playback,backup,media}.rs`, `stores/parts/{mediaState,playbackState,taxonomyState,sourcesState,types}.ts`.

### Unresolved items

- Phase 6 complete. GitHub CI, signing secrets, tag-driven draft release, and disposable-runner installer smoke remain external gates (NOT RUN) from Phase 5.

## 2026-08-08 — Phase 6 fix: dev-startup updater plugin config

### 现象

`npm run tauri dev` 启动即崩溃：
`PluginInitialization("updater", "Error deserializing 'plugins.updater' within your Tauri configuration: invalid type: null, expected struct Config")` (exit 101)。

### 根因

Phase 5 Task 4 引入了 `tauri-plugin-updater = "2"` 并在 builder 链注册 `tauri_plugin_updater::Builder::new().build()`，但 `src-tauri/tauri.conf.json` 的 `plugins` 里没有 `updater` 块（Phase 5 故意不在入库配置里放假 pubkey / 不完整 updater 配置，真实 pubkey 由 `prepare-release-config.ps1` 在发布时注入生成配置）。插件初始化需要反序列化 `plugins.updater` 为 `Config`（其中 `pubkey: String` 是必填字段），缺失即 null → 反序列化失败 → 启动 panic。

### 修复

`src-tauri/tauri.conf.json` 的 `plugins` 增加最小合法块：

```json
"updater": {
  "endpoints": [],
  "pubkey": ""
}
```

- 空 `endpoints` + 空 `pubkey` 让 `Config` 反序列化通过，插件可正常初始化；`validate_endpoints` 对空数组直接通过。
- 检查更新时 `Updater::build()` 对空 endpoints 返回 `Error::EmptyEndpoints`，被 `app_updates.rs::fetch_update` 映射为稳定 `CHECK_FAILED` 错误（离线/未配置状态，符合 Task 4 错误契约）。
- 发布时 `prepare-release-config.ps1` 生成的 `tauri.release.generated.conf.json`（含真实 pubkey + HTTPS endpoint）通过 tauri-action `--config` 覆盖此块，Tauri 深度合并后者优先。
- 契约脚本 `validateUpdaterConfig` 只在 updater 块存在时检查 endpoints 必须 HTTPS、禁 insecure transport——空数组不触发。

### 验证

- `npm run verify:release` PASS（`PureWall release contract OK for 0.1.0`）。
- `cargo check` PASS。
- JSON 合法性 PASS。
- 用户 `npm run tauri dev` 复验通过（应用正常启动）。
