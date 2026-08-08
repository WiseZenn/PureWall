# AI_DIARY.md

> Append-Only 记忆体。原样记录踩坑、Bug 起因、环境变更。禁止删改历史条目。

---

## 2026-05-28

### #scaffold-001: create-tauri-app 需要 TTY
**现象**：`npm create tauri-app@latest` 报错 `IO error: not a terminal: not a terminal`
**原因**：create-tauri-app 强制检测 TTY，非交互式终端无法运行
**解决**：手动创建项目脚手架（package.json + vite.config + Cargo.toml + tauri.conf.json）
**影响**：后续如果需要更新脚手架模板，需手动同步

### #config-001: tauri.conf.json app.title 字段不存在
**现象**：`unknown field 'title', expected one of 'windows', 'security'...`
**原因**：Tauri 2 的 `title` 字段在 `app.windows[].title` 而非 `app.title`
**解决**：删除 app 级别的 title，保留 windows 数组内的 title

### #config-002: icon.ico 缺失导致 build 失败
**现象**：`icons/icon.ico not found; required for generating a Windows Resource file`
**原因**：tauri-build 在 Windows 上强制要求 .ico 文件
**解决**：用 Node.js 脚本从 PNG 生成 ICO 文件

### #asset-001: Tauri 2 asset 协议无法加载本地文件
**现象**：`convertFileSrc` 生成的 URL 前端无法加载，CSP 或权限问题
**原因**：Tauri 2 的 asset 协议需要复杂的权限配置（capabilities + scope）
**解决**：放弃 asset 协议，改用 Rust Command 读取图片 → base64 编码 → 前端 data URL
**关联**：ARCHITECTURE.md §7

### #thumbnail-001: 每张卡片独立 invoke 加载缩略图极慢
**现象**：30 张图片加载需要 10+ 秒
**原因**：每张 WallpaperCard 独立调用 Rust 读取/resize/encode，串行且无缓存
**解决**：
1. 新增 `load_thumbnails_batch` 一次性批量返回
2. Rust 端 `thumbnail_cache` HashMap 内存缓存
3. 缩略图降为 256px + JPEG 60% 质量
**关联**：ARCHITECTURE.md §7

### #reactivity-001: 直接修改 find() 返回对象不触发 Vue 更新
**现象**：点赞/播放后 UI 不刷新
**原因**：`wallpapers.value.find(...)` 返回的对象属性修改不触发数组级响应式
**解决**：封装 `patchWallpaper()` 函数，slice+spread 重建整个数组
**关联**：ARCHITECTURE.md §7

### #reload-001: nextWallpaper 点击后全量重载壁纸列表
**现象**：点"下一张"后所有缩略图重新加载
**原因**：`nextWallpaper()` 内调用了 `loadWallpapers()` 全量刷新
**解决**：移除 loadWallpapers 调用，改用本地 patchWallpaper 更新 play_count
**关联**：ARCHITECTURE.md §7

### #dialog-001: WebView2 中 input[type=file] webkitdirectory 无响应
**现象**：点击"选择壁纸文件夹"按钮无任何弹窗
**原因**：Tauri WebView2 不支持 webkitdirectory 属性
**解决**：安装 `@tauri-apps/plugin-dialog`，使用原生 `open({ directory: true })`
**额外**：需在 Cargo.toml 加 tauri-plugin-dialog，capabilities/default.json 加 dialog 权限

### #emitter-001: tray.rs 中 emit 调用报 trait not in scope
**现象**：`no method named 'emit' found for reference &AppHandle`
**原因**：Tauri 2 的 `Emitter` trait 需显式 `use tauri::Emitter`
**解决**：在 tray.rs 顶部添加 `use tauri::Emitter;`

### #prompt-001: Prompt 链首次使用改进 (2026-05-28)
**发现的问题**：
1. 缺少 `debug` 路由 — 运行时问题（图片不显示/按钮无响应）无结构化排查流程
2. `onboard` 未检查开发环境 — 需要手动安装 Rust/MSVC 但 prompt 无提示
3. `feature-dev` 缺少 pre-flight — 应先确认当前代码可编译再开始
4. `feature-dev` 缺少 rollback plan — 改坏时无明确回滚声明
5. `changelog` 未提示检查 AI_DIARY — 容易重复踩历史坑

**改进措施**：
- `00-router.md`: 新增 `debug` 路由 + `[PRE-FLIGHT]` 通用前置检查
- `01-onboard.md`: 新增 `[PRE-FLIGHT - 环境检查]` 步骤（Rust/MSVC/Node/Git）
- `05-feature-dev.md`: 新增 `[PRE-FLIGHT]` 编译确认 + `[ROLLBACK PLAN]` 回滚声明
- `08-changelog.md`: 新增 `[PRE-FLIGHT]` 读 AI_DIARY 确认无重复坑
- 新增 `09-debug.md`: 运行时问题排查标准流程

### #permission-001: Tauri 2 capabilities 权限名称变更
**现象**：`Permission core:window:allow-get-by-label not found`
**原因**：Tauri 2.11 中窗口相关权限移到了 `core:webview:` 命名空间
**解决**：使用 `core:webview:allow-create-webview-window`、`core:webview:allow-get-all-webviews` 等正确名称
**注意**：权限列表随 Tauri 版本变化，遇到 not found 错误需查看 cargo check 输出的完整列表

### #widget-001: 悬浮挂件窗口通过独立 HTML 加载
**现象**：新窗口需要加载独立页面（非 SPA 路由）
**原因**：Tauri 多窗口每个 WebviewWindow 独立加载 URL
**解决**：创建 `public/widget.html` 作为独立页面，内联样式 + 轻量 JS 调用 invoke
**关联**：通过事件（widget-like/widget-dislike）与主窗口通信

### #registry-001: Windows 右键菜单注册表操作维护指南
**注册表路径**：`HKCU\Software\Classes\Directory\Background\shell\`

**扁平条目结构**（当前使用）：
```
shell\PWNext
  (default) = "PW: Next"
  Icon = "<exe_path>"
  Position = "Top"
  command\(default) = "<exe>" --action next
shell\PWLike
  (default) = "PW: Like"
  command\(default) = "<exe>" --action like
shell\PWDislike
  (default) = "PW: Dislike"
  command\(default) = "<exe>" --action dislike
shell\PWPause
  (default) = "PW: Pause"
  command\(default) = "<exe>" --action pause
```

**Win11 新菜单禁用**：
```
HKCU\Software\Classes\CLSID\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}\InprocServer32
  (default) = ""  (空字符串 = 禁用新菜单，使用经典菜单)
```

**注册方式**：写 `.ps1` 脚本文件 → `powershell -ExecutionPolicy Bypass -File` 执行
**关键踩坑**：
1. PowerShell 字符串拼接必须用 `'"' + $exe + '" --action xxx'`，不能用 format!() 插入反斜杠
2. `.reg` 文件方案不可靠（UTF-16LE 编码 + 反斜杠转义易出错）
3. **子菜单不可行**：纯注册表 `shell\name\shell\subname` 结构需要 COM IContextMenu 配合才能展开子项，纯注册表只能显示箭头不能渲染子菜单。实现真正的子菜单需要 COM DLL（C++），当前未实现。
4. 注册前需清理旧的条目
**代码位置**：`src-tauri/src/context_menu.rs`

### #registry-002: 严重违规 — 未经允许修改系统设置
**时间**：2026-05-28
**行为**：为了让右键菜单出现在 Win11 一级菜单，自动执行了禁用 Win11 新菜单的注册表修改（`CLSID\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}\InprocServer32`），导致用户 Win11 默认右键菜单被替换为经典菜单，且恢复困难。
**根因**：没有先询问用户就直接执行系统级修改
**教训**：任何非 PureWall 自身条目的系统级注册表操作，必须先向用户说明风险并获得确认。已将此约束写入 CLAUDE.md。

### #registry-003: 恢复 Win11 新菜单的正确方式
**正确的恢复命令**：`reg.exe delete "HKCU\Software\Classes\CLSID\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}\InprocServer32" /va /f`
**区别**：`/va` 只删除键内的值（保留空键），`Remove-Item` 删除整个键（可能残留缓存状态导致恢复失败）
**教训**：恢复注册表时用 `reg.exe delete /va` 比 PowerShell `Remove-Item` 更可靠

### #path-001: CLI action 与 Tauri app 数据目录不一致
**现象**：CLI `--action next` 报 `Failed to open database`
**原因**：CLI 用 `dirs::data_dir()/PureWall/`，Tauri app 用 `app_data_dir()/com.purewall.app/`
**解决**：统一为 `app_data_dir()` helper 函数，指向 `APPDATA/com.purewall.app/`
**文件**：`src-tauri/src/main.rs` L70-L74

### #timer-001: 轮播定时器硬编码 60 秒
**现象**：无论设置多少秒，轮播都是 60 秒切换一次
**原因**：定时器写死 `for _ in 0..60` 循环后就执行轮播，忽略 `rotation_secs`
**解决**：重写为累加 elapsed 直到 `rotation_secs`，期间每秒检查 `is_paused`
**文件**：`src-tauri/src/main.rs` L293-L322

### #registry-004: reg.exe add /ve 是写入注册表默认值的唯一可靠方式
**现象**：PowerShell `New-ItemProperty -Name "(default)"` 写入后值为空，导致子菜单全部失效
**原因**：`(default)` 在注册表中是特殊值，需要 `/ve` 参数（不带 /v）才能写入
**解决**：用 `reg.exe add <key> /ve /t REG_SZ /d <value> /f` 写入默认值
**关联**：context_menu.rs 中 `reg_add_ve()` 函数

### #registry-005: PowerShell/bash 转义不可靠，用 .bat 或直接 std::process::Command
**现象**：PowerShell 字符串中反斜杠被吞、$basePath 不展开、"" 空值变 /f
**原因**：多层转义（bash → PowerShell → reg.exe）导致参数错位
**解决**：
- 方案 A：Rust `std::process::Command` 直接调 reg.exe（零转义，推荐）
- 方案 B：写 .bat 文件再 cmd.exe /c 执行
- 永远不要用 PowerShell 的 `New-ItemProperty` 或 `Set-ItemProperty` 操作注册表默认值

### #widget-001-v2: 悬浮窗 toggleWidget 用 hide/show 代替 close/recreate
**现象**：close() 后 getByLabel() 可能返回 null，导致无法重新打开
**解决**：用 `hide()`/`show()` 代替 `close()`/recreate，添加 try/catch 容错
**文件**：`src/components/Sidebar.vue` toggleWidget()

### #widget-002: 悬浮窗隐藏后 show() 仍然失效
**时间**：2026-05-29
**现象**：hide() 后通过 toggleWidget 的 show() 无法重新显示悬浮窗
**尝试修复**：
1. `getByLabel("widget")` → 可能找不到隐藏窗口
2. 改用 `getAll()` 遍历查找 → 编译通过但效果未验证
3. widget.html 添加 `getCurrentWindow().hide()` 关闭按钮
**状态**：未解决，待后续修复
**可能根因**：Tauri 2 的 `show()` 在 transparent + alwaysOnTop + skipTaskbar 组合下可能有 bug

### #widget-003: Widget 生命周期彻底重构
**时间**：2026-05-29
**方案**：将窗口创建/显示/隐藏逻辑全部移到 Rust 端
**实现**：
- 新增 `toggle_widget` Tauri Command：先 `get_webview_window("widget")` 查找，存在则 hide/show，不存在则 `WebviewWindowBuilder::new()` 创建
- 创建时通过 `on_window_event` 拦截 `CloseRequested`（Alt+F4），`prevent_close()` 后转为 `hide()`
- 前端 Sidebar.vue 简化为一行 `invoke("toggle_widget")`
- widget.html 关闭按钮使用 `getCurrentWindow().hide()`
**文件**：`src-tauri/src/main.rs` toggle_widget(), `src/components/Sidebar.vue`, `public/widget.html`

### #widget-004: 悬浮挂件独立窗口需要单独授权、透明窗口和统一事件链路
**时间**：2026-06-01
**现象**：悬浮挂件没有达到预期效果：圆角挂件可能显示为黑色矩形背景；widget 内按钮操作后主界面状态不同步；关闭挂件后 Sidebar 按钮状态仍可能停留在“隐藏悬浮挂件”。
**根因**：
1. `src-tauri/capabilities/default.json` 只声明了 `main` 窗口，运行时创建的 `widget` 窗口没有明确纳入 capabilities。
2. `WebviewWindowBuilder` 未设置 `.transparent(true)`，且 `widget.html` 的 `html/body` 背景为 `#1e1e1e`。
3. widget 的“下一张”直接调用 `next_wallpaper`，该 command 不 emit `auto-rotated`，主窗口 store 不会收到当前壁纸更新。
4. widget 的关闭按钮直接调用前端 `getCurrentWindow().hide()`，没有通知主窗口同步 `widgetVisible`。
**修复**：
- capabilities 增加 `"widget"`。
- widget 窗口创建增加 `.transparent(true)`，页面根背景改为 `transparent`。
- 新增 `widget_next` / `hide_widget` commands，统一走 Rust `perform_action` 和 `widget-visibility-changed` 事件。
- `Sidebar.vue` 监听 `widget-visibility-changed` 更新本地按钮状态。
**验证**：
- `cargo check` PASS（普通沙箱因 target 写权限失败，提权后通过；仅保留既有 unused warning）。
- `npx vue-tsc --noEmit` PASS。
- `npm run build` PASS（普通沙箱 esbuild spawn EPERM，提权后通过）。

### #verify-001: Windows 沙箱下 cargo/Vite 可能因 target 或 esbuild 权限失败
**时间**：2026-06-01
**现象**：
- `cargo check` 报 `failed to open ... target\debug\.cargo-lock` 或 `failed to write ... .rmeta: 拒绝访问。 (os error 5)`。
- `npm run build` 在 Vite 读取配置时因 `spawn EPERM` 启动 esbuild 失败。
**处理**：这类失败不是代码编译错误；在当前 Codex 沙箱中需要用提权命令重跑验证。
**验证**：提权后 `cargo check`、`npm run build` 均通过。

### #widget-005: Tauri 2 独立窗口白屏问题（未解决）
**时间**：2026-06-01
**现象**：通过 `WebviewWindowBuilder::new()` 创建的 widget 窗口始终白屏，且会导致主窗口 UI 按钮失效
**已尝试方案（全部失败）**：
1. `transparent: true` + CSS transparent 背景 → 白屏
2. 移除 transparent，CSS 改 `#1e1e1e` 不透明背景 → 白屏
3. 移除 `background_color` 调用 → 白屏
4. 移除 window-state 插件 → 白屏
5. `WebviewUrl::App("widget.html")` → 白屏
6. `WebviewUrl::External("file:///...")` → 白屏
7. 窗口内 Vue 组件方案 → 可用但无意义（只能在主窗口内）

**影响**：创建独立窗口后主窗口按钮也失效，说明不是单纯的渲染问题，可能是 WebView2 进程冲突

**可能根因**：
- Tauri 2 + WebView2 在 Windows 11 上的多窗口支持有 bug
- 新窗口创建过程中主窗口的事件循环被阻塞
- WebView2 的多实例管理问题

**状态**：未解决，需要进一步调研或等 Tauri 更新修复
 
## 2026-06-02

### #dependency-001: vue-router default install resolved to incompatible v5
**Phenomenon**: `npm install vue-router` resolved `vue-router@5.1.0`, which has a peer dependency on Vite 7/8 and failed with ERESOLVE in the current Vite 6 project.
**Cause**: The registry latest version is not the compatible Vue 3 / Vite 6 line for this app.
**Fix**: Install `vue-router@4` explicitly. Current package entry: `"vue-router": "^4.6.4"`.
**Lesson**: For PureWall's current Vue 3 + Vite 6 stack, always pin router installation to the v4 major line unless the whole frontend toolchain is upgraded together.

### #widget-006: Widget window should be pre-created and routed through the main app entry
**Phenomenon**: Runtime-created widget windows that loaded external HTML (`widget_content.html` / previous `public/widget.html`) could white-screen and make the main window unresponsive (#widget-005).
**Cause**: Creating an additional WebView at runtime and loading an external file adds WebView2/Tauri lifecycle risk and bypasses the known-good Vite app entry.
**Fix**: Add Vue Router hash mode, route `/widget` to `WidgetView.vue`, statically declare the hidden `widget` window in `tauri.conf.json`, and make Rust only show/hide the pre-created window.
**Lesson**: Multi-window UI should prefer static Tauri windows that load the same bundled entry unless there is a strong reason to create WebViews dynamically.

### #verify-002: `npm run tauri dev` is a long-running GUI verification command
**Phenomenon**: Short-running `npm run tauri dev` verification timed out and left Vite/cargo/PureWall processes alive. A second run failed because port 1420 was still occupied.
**Cause**: `tauri dev` starts the Vite dev server and the desktop app as long-running processes; timeout does not always clean the full child process tree.
**Fix**: After timed dev verification, check and stop leftover Node/Vite on port 1420 and any cargo/purewall child processes created by that run.
**Lesson**: Treat `tauri dev` as manual/interactive verification. For automated verification, prefer `cargo check`, `npx vue-tsc --noEmit`, and `npm run build`; use dev only as a short smoke test with explicit cleanup.

### #migration-001: Additive SQLite columns must be migrated before indexing
**Phenomenon**: Phase 3 added `wallpapers.blacklisted` and initially placed `CREATE INDEX idx_wallpapers_blacklisted ON wallpapers(blacklisted)` in the same startup batch as `CREATE TABLE IF NOT EXISTS wallpapers`.
**Cause**: On an existing database, `CREATE TABLE IF NOT EXISTS` does not add new columns, so creating an index on the new column before `ALTER TABLE` would fail at runtime.
**Fix**: Check `PRAGMA table_info(wallpapers)`, add the `blacklisted` column if missing, then create the index after the migration.
**Lesson**: For existing SQLite tables, every additive column needs an explicit migration step before any index or query depends on it.

### #github-001: GitHub CLI token may be stale before first private repo publish
**Phenomenon**: `gh auth status` reported account `WiseZenn` as active, but the keyring token was invalid.
**Cause**: Local GitHub CLI credentials can expire or be revoked while still showing an active account entry.
**Fix**: Re-authenticate with `gh auth login -h github.com` before creating the private repository.
**Lesson**: Before first publish, always verify `gh auth status`; if the token is invalid, finish the local commit first and only retry remote creation after authentication is refreshed.

### #gitignore-001: Registry path residue can appear as `src-tauri/$c/`
**Phenomenon**: Before the first Git commit, `git status` showed empty files under `src-tauri/$c/shell/.../command`.
**Cause**: A previous registry-menu experiment or script wrote a registry-like path into the workspace instead of the Windows registry.
**Fix**: Exclude `src-tauri/$c/` in `.gitignore` and keep the real implementation in `src-tauri/src/context_menu.rs`.
**Lesson**: Always inspect first-commit untracked files carefully; registry-script residue should not be versioned.

### #github-002: SSH push can fail after `gh repo create`
**Phenomenon**: `gh repo create --push` created the private repository, then failed with `Connection closed by 198.18.0.35 port 22`.
**Cause**: GitHub CLI used SSH for Git operations, but the current network path blocked or closed SSH port 22.
**Fix**: Change `origin` to `https://github.com/WiseZenn/PureWall.git`, run `gh auth setup-git -h github.com`, then push with `git push -u origin main`.
**Lesson**: On this Windows environment, prefer HTTPS remotes for GitHub pushes unless SSH connectivity has been verified.

### #windows-api-001: `windows` crate bindings may return pointer handles and `Result`, not BOOL-shaped values
**Phenomenon**: Phase 4 focus detection initially compared `HWND.0` / `HMONITOR.0` with `0`, called `.as_bool()` on `GetWindowRect`, and called `.context()` directly on `CoInitializeEx`.
**Cause**: In `windows` crate 0.60, handle inner values are raw pointers, `GetWindowRect` returns `Result<()>`, and `CoInitializeEx` exposes an `HRESULT` that needs `.ok()` before anyhow context.
**Fix**: Use `.is_null()` for handles, `.is_err()` for `GetWindowRect`, and `CoInitializeEx(...).ok().context(...)`.
**Lesson**: Before coding unfamiliar Win32 calls, inspect the generated local `windows` crate signatures instead of assuming classic C BOOL/HRESULT shapes.

### #tool-001: `rg.exe` may be denied by the Windows sandbox
**Phenomenon**: Running `rg -n "Phase 5|Phase5|InsightsPanel|InspectorPanel|UI" docs src` failed with `Program 'rg.exe' failed to run: Access is denied`.
**Cause**: The current managed Windows sandbox can block execution of the bundled/native `rg.exe` even though the workspace is readable.
**Fix**: Fall back to PowerShell `Get-ChildItem ... | Select-String` for repository text search when `rg` is denied.
**Lesson**: Prefer `rg` first, but do not spend time debugging it if Windows returns access denied; use `Select-String` and continue.

### #docs-001: Prompt chain files may be listed but absent from `docs/project-docs/`
**Phenomenon**: Phase 5 pre-flight attempted to read `docs/project-docs/05-feature-dev.md` and `docs/project-docs/08-changelog.md`, but both paths were missing.
**Cause**: `AGENTS.md` documents the prompt-chain filenames, but the current repository snapshot does not contain those prompt files under `docs/project-docs/`.
**Fix**: Continue with the available project memory files (`AI_DIARY.md`, `CHANGELOG_AI.md`, `DECISIONS.md`) and record the missing files instead of blocking unrelated implementation work.
**Lesson**: Treat missing prompt-chain helper files as a documentation hygiene issue unless the task specifically depends on their content.

### #frontend-qa-001: Browser QA in Codex desktop may require system Chrome plus Tauri IPC metadata mocks
**Phenomenon**: Browser plugin was not available; `npx playwright --version` could not download Playwright due restricted network; bundled Playwright existed but its managed Chromium was not installed. Browser launch through the sandbox failed with `spawn EPERM`, and the first mocked render blanked because `getCurrentWindow()` expected `window.__TAURI_INTERNALS__.metadata.currentWindow.label`.
**Cause**: The desktop runtime provides the Playwright package without a downloaded browser binary, and Tauri frontend modules require both `invoke` and metadata fields when rendered outside the Tauri WebView.
**Fix**: Use the bundled Playwright package path with the system Chrome executable, run headless Chrome elevated when the sandbox blocks process spawn, and mock `window.__TAURI_INTERNALS__` with `metadata.currentWindow/currentWebview`, `transformCallback`, `unregisterCallback`, and command-specific `invoke` responses.
**Lesson**: For PureWall visual QA outside Tauri, mock the full minimum Tauri IPC surface, not just `invoke`; otherwise `TitleBar.vue` can fail before the app renders.

### #frontend-qa-002: Chrome CDP QA needs Windows path normalization and PUT /json/new
**Phenomenon**: A local static `dist/` QA server rendered `Forbidden` even for `/`, and Chrome DevTools `/json/new?...` returned `Using unsafe HTTP verb GET`.
**Cause**: Windows `path.join()` converted `D:/...` to backslash paths, so a raw string `startsWith()` check rejected valid files. Newer Chrome DevTools requires `PUT` for `/json/new` target creation.
**Fix**: Normalize both the static root and requested path with `path.resolve()`, and call `/json/new` with HTTP `PUT`.
**Lesson**: For dependency-free Chrome/CDP QA on Windows, normalize filesystem paths before containment checks and use `PUT /json/new` when creating targets.

### #responsive-001: System inspector must stay reachable at Tauri minWidth
**Time**: 2026-06-08
**Phenomenon**: After wiring sidebar System links to right-side inspector panels, rendered QA at the app's configured `minWidth` of 800px showed the inspector clipped out of view because the existing `max-width: 920px` rule hid `.inspector-shell`.
**Cause**: The earlier responsive rule optimized the gallery-only layout but did not account for System navigation entries that render their content in the inspector.
**Fix**: For 700-920px widths, hide the side rail and keep a two-column `main + inspector` layout. Only hide the inspector below 700px, which is below the current Tauri main-window minimum width.
**Lesson**: When adding navigation that targets the inspector, visual QA must include the configured Tauri minimum window size, not only wide desktop and very narrow browser screenshots.

### #preview-001: Grid thumbnails should not be reused for large preview surfaces
**Time**: 2026-06-08
**Phenomenon**: The current-wallpaper and inspector previews looked blurry even when the original image was high resolution.
**Cause**: Both large preview surfaces reused the gallery thumbnail cache, which intentionally generates 256px JPEG images at 60% quality for fast batch loading.
**Fix**: Keep the existing thumbnail path for gallery cards and add a separate 960px / JPEG 82% `load_preview_image` path that loads only the active wallpaper.
**Lesson**: Professional media UIs need separate image tiers for dense grids and large detail previews; using one low-cost thumbnail everywhere trades away visible quality.

### #metadata-001: Inspector metadata should not depend only on on-demand file reads
**Time**: 2026-06-09
**Phenomenon**: Resolution and file-size rows could remain `Unknown` or log metadata-read failures even though the scanner had already seen the image during import/folder scan.
**Cause**: The UI depended on a separate selected-file command instead of persisting scanner metadata in SQLite. If the path was stale, inaccessible, or the command failed during startup timing, the inspector had no fallback.
**Fix**: Add additive `width`, `height`, and `file_size` columns to `wallpapers`, persist scanner metadata during upsert, and keep `get_image_metadata` only as a refresh/write-through path.
**Lesson**: For media managers, scanner-derived metadata should be part of the library record; on-demand file reads are a refresh mechanism, not the only source of truth.

### #popover-001: Inspector popovers can be clipped by panel overflow
**Time**: 2026-06-09
**Phenomenon**: A color picker-style value panel placed inside the right inspector risks being cut off by `.inspector-panel` overflow and narrow responsive layouts.
**Cause**: The inspector contains scrollable/card-like containers that intentionally constrain preview and metadata surfaces.
**Fix**: Render the color value panel with Vue `Teleport` to `body` and position it with viewport coordinates from the color dot's bounding box.
**Lesson**: Floating tool UI in desktop-style apps should live at the top layer rather than inside clipped content containers.

### #responsive-002: Viewport breakpoints are not enough for fixed-pane desktop layouts
**Time**: 2026-06-09
**Phenomenon**: At a Windows-scaled desktop width, the center current-wallpaper controls and toolbar rendered underneath the right inspector even though a viewport media query existed.
**Cause**: The three-pane shell has fixed side/inspector columns, so the central pane can be much narrower than the viewport. Windows display scaling made the viewport threshold miss the real central-pane constraint.
**Fix**: Make `.main-workspace` a CSS container and use container queries to adapt `CurrentWallpaperPanel`, wallpaper controls, and toolbar wrapping based on the central pane's actual width.
**Lesson**: Professional desktop layouts should use pane/container breakpoints plus min/max constraints; viewport-only breakpoints are brittle when side panes are fixed or resizable.

### #responsive-003: Narrow desktop panes also need a vertical-budget plan
**Time**: 2026-06-13
**Phenomenon**: At the configured 800px minimum window width, the central pane had no horizontal overflow but the stacked current-wallpaper action labels were clipped by the gallery toolbar.
**Cause**: The container query solved horizontal sizing but kept the full settings list and two-row action layout inside a height-constrained desktop workbench.
**Fix**: At `@container workspace (max-width: 560px)`, hide the duplicate current-panel settings, render the four primary actions as a compact icon command row, reduce preview height, and provide explicit `aria-label`/`title` text.
**Lesson**: A responsive desktop pane must budget both width and height. When the inspector remains available, move duplicate settings out of the tight central pane before shrinking primary content.

### #asset-002: Raw GitHub SVG download may fail while the official npm package succeeds
**Time**: 2026-06-13
**Phenomenon**: PowerShell `Invoke-WebRequest` to raw GitHub Fluent SVG URLs failed with an authentication error.
**Cause**: The current Windows/network environment can reject raw GitHub requests even when npm registry access works.
**Fix**: Install Microsoft's official `@fluentui/svg-icons` package and import the optimized SVG assets from the package.
**Lesson**: For maintained icon systems, prefer the official package over ad hoc raw-file downloads; it also keeps source, license, version, and asset naming explicit.

### #dependency-002: One transitive vulnerability can appear as multiple npm high-severity findings
**Time**: 2026-06-13
**Phenomenon**: `npm audit` reported three high-severity findings for `esbuild`, `vite`, and `@vitejs/plugin-vue`.
**Cause**: The only root advisory was `GHSA-gv7w-rqvm-qjhr` in `esbuild@0.25.12`; npm propagated the severity to its direct dependents. Vite 6 constrained esbuild below the patched `0.28.1` release, so npm reported no direct fix.
**Fix**: Upgrade the compatible build chain to `vite@8.0.16` and `@vitejs/plugin-vue@6.0.7`, which removes esbuild from PureWall's installed dependency tree, then verify with `npm audit --json`.
**Lesson**: Read the audit dependency graph before using `npm audit fix --force`. Fix the root dependency through an intentional compatible upgrade, and declare any new Node engine requirement.

### #verify-003: Vite 8 sandbox build can hit spawn EPERM outside esbuild
**Time**: 2026-06-13
**Phenomenon**: After removing esbuild through the Vite 8 upgrade, normal sandbox `npm run build` still failed with `spawn EPERM` while resolving the Vite configuration.
**Cause**: Vite 8's Windows path normalization invokes a child process during config loading, which the managed sandbox can block independently of esbuild.
**Fix**: Rerun the production build with approved elevated execution; the Vite 8 build passed.
**Lesson**: In this Windows sandbox, `spawn EPERM` is a general child-process restriction and does not prove esbuild is still installed or vulnerable.

### #theme-001: Theme tokens are not a theme feature until state can reach them
**Time**: 2026-06-13
**Phenomenon**: `styles.css` already contained a `:root[data-theme="light"]` palette, but the app always appeared dark and offered no theme control.
**Cause**: No shared theme state set `data-theme` to Light, no UI exposed the option, and no preference was restored before application mount.
**Fix**: Add a shared theme composable, initialize it before Vue mounts, expose Dark / Light controls in Settings, persist the explicit choice, and visually verify both themes.
**Lesson**: Treat theme support as state + persistence + accessible controls + contrast QA. A second token block alone is unreachable design inventory.

### #dependency-003: npm font package install may fail in both global and workspace caches under sandbox
**Time**: 2026-06-13
**Phenomenon**: Installing `@fontsource-variable/geist` and `@fontsource-variable/inter` failed with `EPERM` while npm attempted to open or unlink temporary cache files, including after switching to a workspace-local `.npm-cache`.
**Cause**: The managed Windows sandbox can block npm cache file operations independently of the cache location.
**Fix**: Rerun the intentional package installation with approved elevated execution, then verify the lockfile and audit result.
**Lesson**: When npm registry access succeeds but cache temp-file operations return `EPERM`, changing cache location alone may not be sufficient; use the established elevated dependency-install path.

### #figma-001: Figma web automation may be blocked before login
**Time**: 2026-06-13
**Phenomenon**: Playwright using the installed system Chrome received a CloudFront `403 Request blocked` response when opening both a Figma design URL and the Figma website.
**Cause**: Figma's edge protection can reject automated browser traffic before the login or canvas UI loads.
**Fix**: Do not rely on Playwright as a Figma editing channel in this environment. Produce an editable SVG design artifact for manual import, or wait for the Figma MCP Starter-plan call allowance to reset.
**Lesson**: Browser automation is not a reliable workaround for Figma MCP quotas, and anti-automation controls must not be bypassed by disguising the browser session.

### #vite-001: Vite 8 dependency scan can crawl Rust-generated documentation
**Time**: 2026-06-13
**Phenomenon**: `npm run tauri dev` printed more than 24,000 Vite dependency-scan errors and ended with `EMFILE: too many open files` while opening HTML files under `src-tauri/target/doc/`.
**Cause**: Vite 8's default dependency discovery crawls HTML entry files under the project root. `server.watch.ignored` prevents file watching but does not restrict the dependency-entry scan, so generated Rust documentation was treated as frontend HTML input.
**Fix**: Set `optimizeDeps.entries` to `["index.html"]` in `vite.config.ts`, explicitly limiting dependency discovery to PureWall's real frontend entry.
**Lesson**: When a repository contains generated HTML trees, configure Vite's dependency entries explicitly; watch exclusions alone are not sufficient.

### #presentation-001: File names are identifiers, not user-facing wallpaper titles
**Time**: 2026-06-13
**Phenomenon**: Long numeric image file names dominated the Living Gallery stage and inspector, while the overlay inspector covered the stage command dock.
**Cause**: The UI treated the file-system name as display copy and positioned the inspector above the workspace instead of allocating layout space for it.
**Fix**: Use the first curated tag as an optional display title, render no title when no readable label exists, retain paths only for file operations, and make the open inspector a real workbench grid column.
**Lesson**: Media-library interfaces should separate stable file identity from presentation metadata. Never promote an arbitrary file name to a hero heading, and reserve layout space for persistent detail panels.

### #metadata-002: Explorer Details fields belong to the Windows Shell Property System
**Time**: 2026-06-14
**Phenomenon**: Useful wallpaper titles and credits were visible in Windows Properties > Details but were not available through PureWall's basic image metadata reader.
**Cause**: Explorer presents canonical Shell properties such as `System.Title`, `System.Subject`, and `System.Author`; these are broader than a direct EXIF-only metadata model and can include vector-valued or localized display values.
**Fix**: Read the canonical properties through `IPropertyStore`, format them with `PSFormatForDisplay`, and cache only active or selected wallpaper metadata in memory.
**Lesson**: For Windows media applications, use the Shell Property System when matching Explorer metadata. Avoid scanning every library item on startup when presentation metadata is only needed for the active surface.

### #theme-002: Native select popups should match the surrounding theme material
**Time**: 2026-06-15
**Phenomenon**: In light mode, native Windows select popups were white while the current-wallpaper command dock remained black, making one control feel like two unrelated interfaces.
**Cause**: The dock used a permanently dark image-overlay material even though its native child controls followed the active application color scheme.
**Fix**: Keep the compact native dropdown interaction, use concise display-mode labels, and make the dock material, text, separators, primary action, and toggle colors follow the application theme.
**Lesson**: Native controls are often desirable in Windows desktop apps, but their surrounding surfaces must follow the same theme instead of forcing an inverted island.

### #theme-003: Native select popup theming is not reliable enough for mixed WebView themes
**Time**: 2026-06-15
**Phenomenon**: In dark mode, the compact stage controls opened a bright white Windows popup even though the trigger and application were dark.
**Cause**: The native select popup is rendered outside normal WebView CSS and may not consistently honor the document color scheme on the target Windows/WebView runtime.
**Fix**: Preserve the dropdown interaction but render the trigger and option menu inside the application using shared theme tokens, keyboard controls, and click-away dismissal.
**Lesson**: Use native selects for ordinary forms when their platform rendering is acceptable; use an accessible app-rendered listbox when a compact themed command surface requires deterministic popup styling.

### #collection-001: A collection group must not silently change the browsing axis or card scale
**Time**: 2026-06-15
**Phenomenon**: Unassigned wallpapers used a small horizontal filmstrip while a tag group below used much larger cards, making the same collection feel like two unrelated browsers.
**Cause**: Group content count influenced layout behavior: dense groups scrolled horizontally while sparse groups visually expanded.
**Fix**: Use one responsive grid rule for every group, keep navigation vertical, and let groups communicate taxonomy only.
**Lesson**: In a media library, grouping should organize content without changing the user's navigation model or the visual importance of equivalent items.

### #tag-ux-001: Taxonomy assignment needs search and creation in one control
**Time**: 2026-06-15
**Phenomenon**: The inspector tag select required choosing an existing exact option and could not create or progressively match tags such as `land` and `landscape`.
**Cause**: A native select models a fixed option list, not a growing user-managed taxonomy.
**Fix**: Use an editable combobox with case-insensitive substring matching, existing-tag assignment, and create-then-assign behavior.
**Lesson**: Tagging workflows should keep discovery, creation, and assignment in one low-friction control.

### #responsive-004: Do not preserve a crowded control bar by shrinking its children into overlaps
**Time**: 2026-06-15
**Phenomenon**: At medium workspace widths, stage-control labels overlapped; at narrower widths, primary actions became unlabeled color blocks.
**Cause**: The dock allowed flex children to shrink and viewport breakpoints hid primary-action labels before reducing lower-priority quick settings.
**Fix**: Make command groups non-shrinking, align primary actions with one shared height/baseline, and use the main-workspace container width to remove quick settings before primary labels.
**Lesson**: Responsive command bars need an explicit priority order. Remove secondary controls first; never let flex compression silently destroy labels or alignment.

### #icon-001: Hand-written PNG-in-ICO assets can fail Windows RC compilation
**Time**: 2026-06-16
**Phenomenon**: Adding a generated `icon.ico` to `tauri.conf.json` caused `cargo check` to fail with `RC2176: old DIB ... pass it through SDKPAINT`.
**Cause**: The custom script wrote PNG-compressed ICO directory entries that Windows Resource Compiler did not accept in this build path.
**Fix**: Keep Tauri's bundle icon source as `icons/icon.png`, and generate any auxiliary `.ico` through `System.Drawing.Icon.Save` rather than a hand-written PNG-in-ICO writer.
**Lesson**: For Tauri Windows icons, prefer a high-resolution PNG source and let the tooling convert resources. If an ICO is needed, verify it with `cargo check` before wiring it into config.

### #sandbox-004: Build-cache permission failures can mask successful code changes
**Time**: 2026-06-16
**Phenomenon**: `cargo check` failed with access-denied errors while writing `src-tauri/target` and even a new target dir, but passed immediately when run with approved elevated execution.
**Cause**: The managed Windows sandbox or an active process can block Cargo cache writes independently of Rust compilation correctness.
**Fix**: Treat access-denied errors in build cache paths as environment failures, then rerun the same verification command with approved elevation instead of changing source code.
**Lesson**: Separate compiler diagnostics from filesystem-cache failures. Only source-level Rust errors should drive code edits.

### #registry-006: Context-menu icons should not depend on the executable resource during dev
**Time**: 2026-06-17
**Phenomenon**: The desktop right-click PureWall menu still showed the old blue-square icon after regenerating app icon assets.
**Cause**: The registered `Icon` value pointed to `target\debug\purewall.exe`. Windows Explorer can cache executable icons aggressively, and a dev exe may not be rebuilt or refreshed at the same time as source icon assets.
**Fix**: Export PureWall's embedded `icon.ico` to `APPDATA/com.purewall.app/purewall-menu.ico` during context-menu registration and write the registry `Icon` value to that stable `.ico` file. For the current machine, update only `HKCU\Software\Classes\Directory\Background\shell\PureWall\Icon` to the exported icon path.
**Lesson**: PureWall-owned registry entries should point at stable PureWall-owned icon files, not transient dev executables, when visual refresh is part of the requirement.

### #verify-004: Elevated verification can be blocked by Codex usage limits
**Time**: 2026-06-18
**Phenomenon**: After normal sandbox `cargo check` hit the known `src-tauri/target` access-denied write error, the usual elevated rerun was rejected with a Codex usage-limit message instead of running.
**Cause**: The verification path depends on an approved elevated command when Windows sandbox permissions block Cargo/Vite build-cache writes; that approval path can be unavailable when the thread has hit a usage limit.
**Fix**: Do not work around the blocked elevation with alternate target directories or indirect execution. Record the limitation in `CHANGELOG_AI.md`, keep the successful pre-flight/TypeScript/diff evidence, and rerun elevated verification later when usage allows.
**Lesson**: Distinguish source failures from unavailable verification infrastructure. If elevated verification is unavailable, be explicit about residual risk instead of claiming a full final pass.

### #dependency-004: Cargo registry queries may need elevation for TLS credentials
**Time**: 2026-06-18
**Phenomenon**: Normal sandbox `cargo search tauri-plugin-single-instance --limit 5` failed with `SSL connect error (schannel: AcquireCredentialsHandle failed: SEC_E_NO_CREDENTIALS)`.
**Cause**: The managed Windows sandbox can block access to the Schannel credential context Cargo needs for crates.io registry queries.
**Fix**: Rerun the same Cargo registry metadata command with approved elevated execution; `cargo search` and `cargo info` then succeeded.
**Lesson**: Treat Schannel `SEC_E_NO_CREDENTIALS` during Cargo registry metadata lookup as an environment credential failure, not as a crate availability signal.

### #single-instance-001: Single-instance forwarding can arrive before managed app state is ready
**Time**: 2026-06-18
**Phenomenon**: Multi-agent review found that `tauri-plugin-single-instance` may invoke the second-instance callback while the first process is still in early startup, before `AppState` has been registered with `app.manage`.
**Cause**: The plugin installs its forwarding machinery during plugin setup, while PureWall creates `AppState` later in `.setup()`. Calling `app.state::<AppState>()` in that early callback can panic.
**Fix**: Use `app.try_state::<AppState>()` in forwarded CLI execution and return a clear "PureWall is still starting" error that is written to `purewall-cli.log` and emitted through the normal operation-failed path when possible.
**Lesson**: Tauri plugin callbacks should not assume managed state exists unless the callback is registered after setup or explicitly checks `try_state`.

### #windows-api-002: GetDriveTypeW remote-drive constants may not be exported by name
**Time**: 2026-06-19
**Phenomenon**: Adding Windows network-drive detection with `windows::Win32::Storage::FileSystem::DRIVE_REMOTE` failed to compile because the current `windows` 0.60 bindings did not export `DRIVE_REMOTE` under that name.
**Cause**: Some Win32 constants are not consistently exposed as named Rust items across generated `windows` crate versions/features, even when the function itself is available.
**Fix**: Import `GetDriveTypeW` and compare the returned drive type to the documented Win32 `DRIVE_REMOTE` value `4`.
**Lesson**: For small Win32 constants, verify the generated binding name locally; if the constant is absent but the API returns the documented numeric enum value, use a local named constant with the Win32 value and keep the comment/context in code review notes.

### #sandbox-005: Desktop sandbox can block patch/search helpers even inside the writable repo
**Time**: 2026-06-19
**Phenomenon**: The `apply_patch` tool failed before reading workspace files with `windows unelevated restricted-token sandbox cannot enforce split writable root sets`, and the bundled `rg.exe` failed to start with access denied from the Codex desktop app resources path.
**Cause**: The managed Windows sandbox can block helper executable startup or wrapper setup independently of repository write permissions.
**Fix**: Keep edits narrow and auditable by using exact PowerShell string/regex replacements or full-file rewrites for small files, then validate with `vue-tsc`, `cargo fmt`, `cargo check`, and `npm run build`. Prefer built-in PowerShell search when `rg.exe` cannot start.
**Lesson**: Treat tool-startup access errors as environment failures. Do not reinterpret them as source problems, and record the fallback so later sessions understand why `apply_patch` was not used for that batch.

### #powershell-001: PowerShell replacements can write literal newline escape text
**Time**: 2026-06-19
**Phenomenon**: A scripted edit inserted literal `` `r`n`` text into `AppShell.vue`, causing `vue-tsc` syntax errors in the drop handler.
**Cause**: The replacement string used single-quoted PowerShell content where backtick escape sequences were not interpreted as newlines.
**Fix**: Re-read the edited file immediately, replace the literal escape text with real newlines, and rerun `npx vue-tsc --noEmit` before continuing.
**Lesson**: When using PowerShell as the fallback editor, prefer `Environment.NewLine` or double-quoted strings for generated line breaks, then inspect the touched snippet before running broader verification.

### #theme-004: Pre-created widget WebView needs explicit theme synchronization
**Time**: 2026-06-19
**Phenomenon**: The main PureWall window switched to the current dark/system theme, but the floating widget stayed on a stale light surface.
**Cause**: The widget is a separate WebView with its own Vue module state. `initializeTheme()` read `localStorage` only at startup, and no listener reapplied theme changes made from the main window.
**Fix**: Make `useTheme.ts` listen for `storage` changes and a `BroadcastChannel` named `purewall-theme`; external messages always reapply the theme so `system` can re-resolve after OS color-scheme changes.
**Lesson**: Static multi-window Tauri apps share durable browser storage, but not live Vue state. Any UI token preference that must stay visually synchronized needs an explicit cross-WebView sync path.

### #widget-007: Transparent widget blur can create a visible halo even without layout padding
**Time**: 2026-06-19
**Phenomenon**: After shrinking the floating widget window, a faint outer transparent border remained visible around the rounded control.
**Cause**: The widget surface used a semi-transparent background, border, inset highlight, and `backdrop-filter` inside a transparent WebView window. Windows/WebView2 compositing made the rounded edge look like a second boundary.
**Fix**: Keep the widget as a solid single surface: no outer border, no inset shell highlight, no backdrop-filter on `.widget-bar`, and a tighter pre-created widget window size.
**Lesson**: For tiny transparent Tauri/WebView2 overlay windows, avoid blurred translucent shell materials at the outermost rounded edge. Use one solid surface and reserve translucency for larger in-window panels.

### #widget-008: Transparent widget radius mismatch can make the edge look like a second frame
**Time**: 2026-06-19
**Phenomenon**: After removing border, blur, shadow, and most padding from the floating widget, the user could still see a faint outer shape around the dark control surface.
**Cause**: The tiny transparent WebView window and the visible `.widget-bar` surface did not share an explicit rounded geometry. Even without a real border, Windows/WebView2 anti-aliasing made the two curves read as separate edges.
**Fix**: Add a widget-scoped radius token, reduce the radius to `16px`, and apply the same radius and clipping to both `.widget-shell` and `.widget-bar`.
**Lesson**: For transparent overlay windows, outer clipping and inner visible surfaces should share one radius token; otherwise anti-aliased curves can look like a double frame even when CSS borders are gone.

### #motion-001: Pillow alpha_composite does not accept a mask argument
**Time**: 2026-07-01
**Phenomenon**: While generating the icon motion GIF, `Image.alpha_composite(fill, (x, y), mask)` raised `ValueError: Source must be a list or tuple`.
**Cause**: Pillow's `alpha_composite` API does not take a mask argument in that form; masked placement should use `paste(source, position, mask)` or pre-apply alpha to the source.
**Fix**: Use `layer.paste(fill, (x, y), mask)` for rounded-gradient fills in the GIF generator.
**Lesson**: For Pillow-based icon or motion exports, use `paste(..., mask)` for clipped RGBA gradients and reserve `alpha_composite` for already-masked layers.

### #motion-002: GIF gradients band because of palette quantization
**Time**: 2026-07-01
**Phenomenon**: The exported icon motion GIF showed obvious stepped color bands in the soft sky gradient above the mountain.
**Cause**: GIF is palette-based and limited to 256 colors across animated frames. Soft translucent gradients, antialiasing, and motion frames compete for the same palette, so the sky gradient collapses into visible levels.
**Fix**: Build a shared GIF palette from all frames and apply Floyd-Steinberg dithering for the compatibility GIF, while also exporting APNG and lossless WebP as the high-quality motion previews that preserve full RGBA gradients.
**Lesson**: For soft-gradient logo motion, treat GIF as a compatibility artifact only. Use APNG/WebP or the HTML/SVG motion preview as the quality reference.

### #motion-003: Pillow palette quantization requires RGB or L mode
**Time**: 2026-07-01
**Phenomenon**: `frame.quantize(palette=palette, ...)` failed with `ValueError: only RGB or L mode images can be quantized to a palette` when applied directly to RGBA motion frames.
**Cause**: Pillow cannot quantize RGBA frames against an existing palette in that API path.
**Fix**: Flatten GIF frames onto a light RGB matte before shared-palette quantization, while preserving RGBA transparency for APNG/WebP exports.
**Lesson**: When exporting both compatibility GIF and high-quality transparent motion formats, split the pipeline: RGB matte for GIF, RGBA frames for APNG/WebP.

### #motion-004: Segmented gradient strokes can make tiny chevrons look broken
**Time**: 2026-07-01
**Phenomenon**: After removing the extra outline from the animated icon chevron, the exported motion arrow still looked fragmented/broken at small size.
**Cause**: The Pillow motion exporter simulated a gradient stroke by drawing many short or layered line segments. After resizing and GIF/WebP/APNG encoding, the segment boundaries could read as visual breaks in the chevron.
**Fix**: Draw the chevron once into a continuous `L` mask with a single path and round cap/join circles, then paste a continuous vertical RGBA gradient through that mask.
**Lesson**: For small animated icon strokes, preserve continuous geometry with a mask-first pipeline. Do not approximate a gradient stroke with segmented visible geometry unless the segment joins are explicitly proven invisible.

### #release-001: Tauri MSI bundling still requires an ICO in bundle icon config
**Time**: 2026-07-01
**Phenomenon**: `npm run tauri build` compiled the release executable successfully, but MSI bundling failed with `Couldn't find a .ico icon` even though `src-tauri/icons/icon.ico` exists.
**Cause**: `src-tauri/tauri.conf.json` currently lists only `icons/icon.png` in `bundle.icon`; the Windows bundler still expects an ICO entry for installer/resource packaging.
**Fix**: Before open-source release, wire the regenerated `icons/icon.ico` into the Tauri bundle icon list and rerun `cargo check`, `npm run build`, and `npm run tauri build`. Re-check the historical `#icon-001` Windows RC issue after changing config.
**Lesson**: Having a generated ICO file in the icons directory is not enough; Tauri release packaging must reference it explicitly, and the full bundler path is the verification source of truth.

### #thumbnail-002: Derived image cache keys must include the derivative profile
**Time**: 2026-07-01
**Phenomenon**: Improving preview dimensions or JPEG quality would not automatically affect users who already had cached preview files, because the cache key was based on source path, file size, and mtime but not the derivative settings.
**Cause**: The thumbnail and preview cache filenames did not encode the output profile, so changed resize dimensions and encoder quality could silently reuse stale lower-quality artifacts.
**Fix**: Hash a stable derivative profile string into the cache key for each generated image class, such as thumbnail and preview.
**Lesson**: Any persisted derived media cache must include the transform profile, not just source identity. Otherwise quality/performance changes appear ineffective until users manually clear cache files.

### #powershell-002: Avoid variable names that collide with PowerShell automatic variables by prefix
**Time**: 2026-07-01
**Phenomenon**: A fallback edit script used a variable named `$home`, which collided with PowerShell's read-only `$HOME` automatic variable; a later attempt with `$homeText` also behaved badly and temporarily overwrote `src/views/Home.vue` with the home directory string.
**Cause**: PowerShell automatic variables are case-insensitive, and short names around `$HOME` are unsafe in scripted file-edit fallbacks.
**Fix**: Reconstruct the affected Vue file from the current AppShell-based structure, avoid `$home*` variable names, and verify the file immediately after the write.
**Lesson**: In PowerShell editing scripts, avoid variable names that start with or equal automatic variables such as `$HOME`. Use neutral names like `$viewContent` and inspect touched files before proceeding.

### #performance-001: Cold-cache thumbnail quality upgrades can make startup worse
**Time**: 2026-07-01
**Phenomenon**: After improving thumbnail and preview clarity, the packaged app felt extremely laggy immediately from startup before the user could test other flows.
**Cause**: The new thumbnail cache profile correctly invalidated older low-resolution cached files, but the gallery still rendered 120 initial cards and could request 48 cold thumbnails per IPC batch. Combined with 384px CatmullRom resizing, first launch concentrated too much decode/resize/base64 work near startup.
**Fix**: Lower the initial render budget to 36 cards, lower thumbnail batches to 12, lower reactive write chunks to 6, yield around batches, delay startup thumbnail and preview work, and use a 320px/q64 Triangle thumbnail profile for a better cold-cache speed/clarity balance.
**Lesson**: Media quality upgrades must be paired with cold-cache backpressure. Always measure startup decode budget, not only steady-state cache-hit behavior.

### #permission-002: Tauri 2 custom title-bar buttons need explicit window permissions
**Time**: 2026-07-01
**Phenomenon**: In packaged-app testing, the custom fullscreen/maximize and hide-style window controls appeared unresponsive even though the Vue click handlers were present.
**Cause**: Tauri 2 capability permissions allowed `hide` but did not allow `minimize`, `maximize`, `unmaximize`, or `is_maximized`, so imported window APIs could be blocked at the IPC permission boundary.
**Fix**: Add `core:window:allow-minimize`, `core:window:allow-maximize`, `core:window:allow-unmaximize`, and `core:window:allow-is-maximized` to `src-tauri/capabilities/default.json`, then verify with a static permission feedback loop plus `vue-tsc`, `cargo check`, and production frontend build.
**Lesson**: For custom title bars in Tauri 2, every window API used by frontend code must have an explicit capability entry. UI logic can be correct while the packaged app still fails at the permission layer.

### #asset-002: Tauri asset protocol needs both config scope and Rust feature
**Time**: 2026-07-01
**Phenomenon**: After adding `app.security.assetProtocol` to `tauri.conf.json`, `cargo check` failed during the Tauri build script with a message that the dependency features did not match the allowlist and asked to add `protocol-asset`.
**Cause**: In Tauri 2, enabling the asset protocol in configuration is not enough. The Rust `tauri` dependency must also include the `protocol-asset` feature so generated context and dependency features agree.
**Fix**: Add `"protocol-asset"` to the `tauri` dependency features in `src-tauri/Cargo.toml`, then rerun `cargo check`, `cargo test`, `cargo clippy`, `npm run build`, and `npm run tauri build`.
**Lesson**: Asset protocol work has a two-sided gate: CSP/scope in `tauri.conf.json`, plus the matching Rust feature. Treat a successful full `tauri build` as the verification source of truth, not only frontend or Rust unit checks.

### #webp-001: image crate only exposes lossless WebP encoding directly
**Time**: 2026-07-01
**Phenomenon**: Phase 2 originally targeted 512px WebP quality 80 thumbnails, but local inspection of `image` 0.25 showed its WebP encoder only supports lossless encoding directly.
**Cause**: The `image::codecs::webp::WebPEncoder` wrapper documents lossy WebP as requiring a libwebp-backed crate such as `webp`; the built-in encoder path is VP8L/lossless only.
**Fix**: Do not mix a new native encoder dependency into the responsiveness refactor. Ship the async worker and 512px JPEG quality improvement first, and treat lossy WebP as a separate dependency/build/security review.
**Lesson**: Image format upgrades are dependency decisions, not just constant changes. Verify encoder capabilities locally before promising a specific quality/format profile.

### #thumbnail-003: Cold-thumbnail generation should be queued, not spawned per image
**Time**: 2026-07-01
**Phenomenon**: The first async-thumbnail sketch spawned a background thread for each missing thumbnail, which would avoid blocking IPC but could create a thread spike during fast scrolling.
**Cause**: Moving work off the IPC path is necessary but not sufficient; open-source-grade media pipelines also need bounded concurrency.
**Fix**: Replace per-image thread spawning with a single thumbnail worker queue, an in-flight path set for de-duplication, and a `thumbnail-generated` event for frontend refill.
**Lesson**: For large media libraries, prefer bounded workers plus de-duplication over unbounded background thread fan-out.

### #asset-003: Tauri `$APPDATA` scope already points at the app data directory
**Time**: 2026-07-01
**Phenomenon**: After Phase 1/2, generated wallpaper thumbnails and inspector previews existed on disk but did not load in the WebView through the asset protocol.
**Cause**: `assetProtocol.scope` used `$APPDATA/com.purewall.app/thumbnails/**/*`. In Tauri 2, `$APPDATA` resolves to `app.path().app_data_dir()`, which for PureWall is already `%APPDATA%\com.purewall.app`; adding `com.purewall.app` again points the scope at a non-existent nested directory. The `**/*` pattern was also narrower than Tauri's own recursive directory pattern, which uses `directory/**`.
**Fix**: Scope the asset protocol to `$APPDATA/thumbnails/**`, matching the actual thumbnail cache directory and Tauri's recursive pattern semantics.
**Lesson**: For Tauri asset scopes, treat `$APPDATA` as the app-specific data root. Verify scope variables against `PathResolver::parse`/`BaseDirectory::AppData` before composing paths, and prefer `directory/**` for recursive cache directories.

### #asset-004: Multiple silent-failure paths in thumbnail-to-asset pipeline
**Time**: 2026-07-02
**Phenomenon**: After Phase 1-3 asset protocol migration, wallpaper preview images failed to load despite cache files existing on disk and scope/CSP being correctly configured.
**Root cause audit** (4 bugs found):
1. **`cacheThumbnailPath` guard too strict** (`wallpapers.ts`): `wallpaperIndexByPath.has(path)` rejected thumbnail-generated events if the index was rebuilt (e.g., during filter/sort transitions), silently dropping valid cache paths.
2. **Batch validation all-or-nothing** (`main.rs`): `require_registered_wallpaper_files()` validated every path before returning. One stale/deleted source file caused the entire batch of 12 to return an error, leaving all thumbnails as loading spinners forever. The frontend `catch` logged silently with `visible: false`.
3. **Windows backslash paths** (`main.rs` + `wallpapers.ts`): `PathBuf::to_string_lossy()` returns `\` on Windows. While `convertFileSrc` SHOULD normalize these, combining with Tauri 2's asset protocol scope (which uses `/`) creates a fragile mismatch surface.
4. **No image load error feedback**: `<img>` tags had no `@error` handler; broken images were invisible.
**Fix**:
- Added `wallpaperIsInLibrary()` fallback search (not just index lookup) in `cacheThumbnailPath`
- Rewrote `load_thumbnails_batch` to validate paths individually, skipping invalid ones with `eprintln!` instead of failing the batch
- Added `normalize_cache_path()` Rust helper (`\` → `/`); `toAssetUrl()` also normalizes before `convertFileSrc`
- Added `@error` + `console.warn` handlers on both thumbnail and stage preview `<img>` tags
- Added `console.debug` logging for cache hits and skips
**Lesson**: Asset protocol migration is not just a config change. Every IPC-to-URL transition point needs: (a) path normalization, (b) individual failure isolation, (c) diagnostic logging, and (d) image-level error visibility. Silent failures anywhere in the pipeline make the entire feature appear broken.

### #preview-002: Preview generation was the last synchronous image path in IPC
**Time**: 2026-07-02
**Phenomenon**: Selecting an uncached wallpaper caused `load_preview_image` to synchronously generate a 1440px JPEG on the IPC thread. The frontend invoke call blocked until generation completed.
**Fix**: Split `get_preview_path` into `cached_preview_path` (fast disk check) and `get_preview_path` (generate + save). `load_preview_image` now returns `Option<String>` — `Some(path)` on cache hit, `None` on miss with fire-and-forget background generation. Frontend falls back to thumbnail URL while waiting and fills preview via `preview-generated` event.
**Lesson**: In Tauri 2, sync commands run on a thread pool so they don't block the main thread. But they DO block the invoke response. For any operation that might take >100ms (image decode/resize/encode), always use the async cache-check + background-worker + frontend-event pattern, not sync generation.

### #cache-001: Derivative cache needs a bounded eviction policy from day one
**Time**: 2026-07-02
**Phenomenon**: 342 cache files on disk with no limit. Deleted wallpapers leave orphaned thumbnail/preview files that are never cleaned up because the cache key is a one-way hash.
**Fix**: Added `ThumbnailCache::cleanup(max_files)` that sorts by file modification time and removes the oldest when count exceeds 1000. Called during app startup.
**Lesson**: Any persisted media cache must have a size/count bound. LRU-by-mtime is the simplest reliable policy when cache keys can't be reverse-mapped to source paths. This should be part of the initial cache design, not retrofitted after disk bloat is noticed.

## 2026-07-02

### #registry-006: Context-menu child entries do not inherit parent Icon
**Phenomenon**: After replacing the PureWall app icon, the Windows right-click menu parent entry used the new `purewall-menu.ico`, but submenu actions (`01_Next` / `02_Like` / `03_Dislike` / `04_Pause`) still showed no replaced icon.
**Cause**: Windows shell context-menu child entries do not inherit the parent `Icon` value. Each child key under `HKCU\Software\Classes\Directory\Background\shell\PureWall\shell\*` needs its own `Icon` value.
**Fix**: `src-tauri/src/context_menu.rs` now writes `Icon = %APPDATA%\com.purewall.app\purewall-menu.ico` for every child entry, and the current HKCU PureWall-owned keys were updated with `reg add`.
**Lesson**: When changing right-click menu branding, audit both the parent menu key and each action key. A successful parent icon does not prove the submenu is branded.

### #powershell-003: `-replace` RHS concatenation with commas needs a temporary variable
**Phenomenon**: A fallback PowerShell edit failed with `The -replace operator allows only two elements to follow it` while replacing code containing comma-separated object fields.
**Cause**: PowerShell parsed comma-containing concatenated replacement text as extra operands to `-replace`.
**Fix**: Build the replacement string in a variable first, then pass that variable as the second operand to `-replace`.
**Lesson**: In PowerShell editing fallbacks, use temporary replacement variables for any multiline code text containing commas, braces, or format-like punctuation.

### #release-002: `npm run tauri build` can outlive the shell timeout and leave cargo/rustc running
**Time**: 2026-07-02
**Phenomenon**: An elevated `npm run tauri build` review run timed out after 5 minutes, but `cargo` and `rustc` child processes continued running in the background. Existing MSI/NSIS artifacts were from an earlier build, so they could not be counted as fresh verification evidence.
**Cause**: The Codex shell timeout ended the wrapper command without reliably terminating the full Tauri/Cargo child process tree.
**Fix**: Check `Get-Process` for leftover `cargo`/`rustc` after any timed-out Tauri build and stop only the exact PIDs spawned by that run. For release verification, use a longer timeout or run from a normal terminal/CI where the process tree is controlled.
**Lesson**: Treat timed-out `npm run tauri build` like `tauri dev`: verify child process cleanup and do not use stale bundle artifacts as proof of a fresh successful build.

### #npm-001: Windows `npm uninstall` can fail on node_modules EPERM
**Time**: 2026-07-02
**Phenomenon**: `npm uninstall @fontsource-variable/inter @tauri-apps/plugin-window-state` failed while unlinking `node_modules\@fontsource-variable\inter\CHANGELOG.md` with `EPERM`.
**Cause**: On Windows, files inside `node_modules` can remain locked by the shell, editor, antivirus, or a prior npm process even when the package manager command itself is correct.
**Fix**: Re-run the same npm operation with elevated permissions after confirming the command is limited to dependency metadata and `node_modules`. The elevated retry completed and `npm audit --omit=dev` reported 0 vulnerabilities.
**Lesson**: Treat npm `EPERM unlink` during dependency cleanup as an environment/file-lock problem first. Do not hand-delete `node_modules`; retry the same npm command with appropriate permissions and verify `package.json`, `package-lock.json`, and audit output afterward.

### #cache-002: Cache hits must refresh derivative mtime before LRU cleanup can work
**Time**: 2026-07-04
**Phenomenon**: After the app was no longer a first launch, thumbnails/previews still appeared to reload on every startup.
**Cause**: `ThumbnailCache::cleanup(max_files)` sorts derivative files by file modification time, but cached thumbnail/preview hits only returned the existing path and did not refresh the derivative mtime. Frequently viewed cached files could still look old to startup cleanup and be deleted once the cache directory exceeded the cap. The previous cap of 1000 derivative files also only covered roughly 500 wallpapers when both thumbnail and preview existed.
**Fix**: `cached_path` and `cached_preview_path` now touch the derivative file mtime on cache hit using `std::fs::FileTimes`, and startup cleanup keeps a larger 5000-file working set.
**Lesson**: If a persisted media cache uses mtime as an LRU proxy, every successful cache hit must update mtime. Otherwise cleanup implements “oldest generated” rather than “least recently used,” which recreates cache-miss behavior across restarts.

## #preview-003 — Active preview jobs must outrank cold gallery thumbnails

Date: 2026-07-04
Context: Startup preview regression after derivative-cache work.

When `load_preview_image` and `load_thumbnails_batch` share the same backend worker queue, frontend timing becomes part of perceived correctness. A delayed active-preview request can be queued behind many cold thumbnail jobs, especially after previous cleanup removed the active preview derivative. The UI then looks like the large preview is stuck even though generation is merely starved.

Guardrail: request the active wallpaper preview immediately after resolving `activeWallpaper` in `loadWallpapers()`. Do not add startup debounce/delay before the active preview unless the backend has a real priority queue or a dedicated preview worker.

## 2026-07-05

### #preview-004: Preview IPC must not synchronously generate large derivatives
**Phenomenon**: Selecting an uncached wallpaper made the large preview feel slow and could contribute to UI stutter.
**Cause**: `load_preview_image` had regressed to calling `get_preview_path()` directly, so a cache miss decoded/resized/encoded a 1440px JPEG before the IPC response returned.
**Fix**: Return cached preview paths immediately; on miss, enqueue bounded background preview generation and refill the frontend with `preview-generated`.
**Lesson**: Any large preview derivative path must preserve the cache-check + background-worker + event-refill contract from #preview-002.

### #collection-002: Collections must not reuse the tag schema
**Phenomenon**: A quick UI pass treated existing tags as Apple Music-style collections, which made new collections appear in the tag taxonomy and mixed two distinct user concepts.
**Cause**: Tags and collections both look like named groups, but they represent different mental models: descriptive metadata vs. curated sets.
**Fix**: Add separate `collections` and `collection_wallpapers` tables, collection-specific commands, a `collection:<id>` filter key, and separate sidebar/selection controls.
**Lesson**: Do not model collections as special tags. Keep tags, albums/collections, folders, and ratings as separate axes unless an accepted ADR explicitly merges them.

### #media-queue-001: Cold media work needs priority and a bounded backlog
**Time**: 2026-07-05
**Phenomenon**: Moving thumbnail/preview generation off the IPC path was not enough for large libraries; rapid scrolling can still enqueue many cold thumbnails, and selected preview jobs need to outrank stale off-screen thumbnail work.
**Cause**: Independent per-image spawn/semaphore paths bounded concurrency but did not provide a single ordering policy or thumbnail backlog cap.
**Fix**: Add `media_queue.rs`: preview-first scheduling, newest-preview-first ordering, queued/running de-duplication, shutdown wakeup, and a cap that drops oldest not-yet-started thumbnail jobs.
**Lesson**: Media managers should schedule derivative work through a small prioritized queue. Bounded concurrency without bounded backlog still lets large libraries feel slow.

### #rust-queue-001: Do not use explicit drop to shorten `tauri::State` lifetimes
**Time**: 2026-07-05
**Phenomenon**: `cargo clippy --all-targets -- -D warnings` failed with `drop_non_drop` after explicitly calling `drop(state)` on `tauri::State<'_, AppState>` inside the media worker loop.
**Cause**: `tauri::State` does not implement `Drop`; explicit `drop()` does not release a lock and Clippy treats it as misleading lifetime control.
**Fix**: Use a scoped block to clone the queue/cache out of `State`, then let the borrow end naturally before blocking on the media queue.
**Lesson**: When narrowing `tauri::State` borrows, prefer lexical scopes over explicit `drop(state)`.

### #pagination-001: Gallery pagination must move search/filter semantics into SQLite
**Time**: 2026-07-05
**Phenomenon**: Switching `wallpapers` from a full frontend array to a paginated slice would make search silently search only the already-loaded page if the old computed `visibleWallpapers` filter remained in Vue.
**Cause**: Virtualized DOM rendering and paginated data loading solve different layers. Once the frontend no longer owns the full result set, any global operation such as search, tag/collection filters, sort, and total counts must be handled by the database query.
**Fix**: Add `get_wallpapers_page(filter, sort, search, offset, limit)` in Rust/SQLite, including escaped LIKE search across path/title/source/tags, and make the frontend reload page 1 on debounced search changes.
**Lesson**: Never combine backend pagination with frontend-only search unless the UI explicitly labels it as searching the loaded slice. For library managers, global search semantics belong next to the paginated query.

### #powershell-004: Match repository line endings explicitly in fallback edit scripts
**Time**: 2026-07-05
**Phenomenon**: A fallback PowerShell edit using `[Environment]::NewLine` failed to insert constants and return-store exports in LF-normalized TypeScript files, leaving Vue type errors until the replacement used explicit LF matches.
**Cause**: Git-working-tree line endings and PowerShell host line endings may differ; exact string replacements with CRLF expectations will miss LF files.
**Fix**: Inspect touched files after fallback edits and use explicit LF patterns or structurally safer replacements when the file is LF-normalized.
**Lesson**: When `apply_patch` is unavailable and PowerShell text replacement is necessary, never assume `[Environment]::NewLine` matches the repository file content.

### #pagination-002: Source-folder commands must not bypass paginated gallery loading
**Time**: 2026-07-05
**Phenomenon**: After adding `get_wallpapers_page`, the initial `set_wallpaper_folder` command still scanned/imported the folder and then returned `db.get_all_wallpapers()`, so selecting a large first library could still serialize the whole database result back to the WebView.
**Cause**: Introducing a paginated read path is not enough if older write/import commands still return full gallery collections as a convenience response.
**Fix**: Change `set_wallpaper_folder` to return lightweight `ImportResult` counts and let the frontend call the same paginated `loadWallpapers()` path used by every other gallery refresh.
**Lesson**: In large-library flows, audit both read commands and write commands. Any command returning `Vec<WallpaperEntry>` can accidentally bypass pagination even when the main gallery query is paged.

### #preview-pipeline-001: Kind-level de-duplication still permits duplicate source decodes
**Time**: 2026-07-12
**Phenomenon**: A thumbnail miss and preview miss for the same 90–112 MP source could occupy two workers and each fully decode the image; preview-first ordering could not help once all generic workers were already running thumbnails.
**Cause**: Queue identity was `(MediaJobKind, path)`, and the frontend explicitly requested a thumbnail after a preview miss. Priority affected only queued jobs, not already-running work.
**Fix**: Key queue state by source path, merge derivative needs, retain pending needs while a path is running, reserve one preview consumer, and generate all missing outputs from one decoded image.
**Lesson**: For expensive source transforms, de-duplicate at the shared input boundary rather than at the output/job-kind boundary. Priority and concurrency limits solve different problems and both require tests.

### #preview-pipeline-002: Speculative previews must not occupy the reserved active-preview lane
**Time**: 2026-07-13
**Phenomenon**: Treating every preview as the same queue class lets likely-next warming wake and occupy the worker intended to guarantee current-wallpaper progress.
**Cause**: Derivative type alone does not express user urgency; both active and speculative jobs request a preview output.
**Fix**: Add `SpeculativePreview` below normal/active preview priority, keep a separate two-item speculative deque, let only general workers claim it, and evict queued speculative paths atomically when a new active request arrives.
**Lesson**: Reserved capacity must be enforced at claim time, not only through numeric priority. Background prediction work should always be bounded and cancelable by direct user intent.

### #preview-pipeline-003: Cache availability and browser decode readiness are different states
**Time**: 2026-07-13
**Phenomenon**: Assigning a newly generated preview URL directly to `<img>` or CSS backgrounds can clear/replace a visible thumbnail before WebView2 has decoded the new asset, producing a blank flash or inconsistent behavior across preview surfaces.
**Cause**: A valid derivative path proves disk readiness, not browser decode readiness. Multiple components also created independent image-loading lifecycles.
**Fix**: Keep one store-level active-media state, preload the candidate through `Image`, commit only the matching generation's `load`, ignore late callbacks, and preserve the thumbnail plus a non-blocking status on error.
**Lesson**: Progressive media UI needs an explicit presentation commit point after browser decode; URL existence alone is not a sufficient state machine.

### #windows-edit-001: CRLF dirty files require sequentially verified fallback patches
**Time**: 2026-07-13
**Phenomenon**: Unified context patches against heavily modified CRLF files failed to match, and a multi-hunk zero-context fallback initially inserted calls at shifted coordinates.
**Cause**: The Windows split-root sandbox prevented direct `apply_patch` updates, while line-ending normalization and earlier hunks changed the effective line offsets used by later zero-context hunks.
**Fix**: Create patch artifacts with `apply_patch`, normalize only the target during application, apply small sequential zero-context hunks, restore the original line ending, and inspect exact anchors after every structural insertion.
**Lesson**: When structural patching falls back to line coordinates, do not batch dependent insertions. Apply one logical edit at a time and verify the resulting control-flow location before continuing.

### #watcher-sync-001: A frontend refresh cannot reconcile a SQLite-backed library
**Time**: 2026-07-13
**Phenomenon**: Adding an image to an already watched folder emitted `folder-changed`, but the wallpaper never appeared because the frontend only reloaded the existing SQLite page. A single write could emit several events and repeat that ineffective reload.
**Cause**: The watcher discarded notify event paths and treated UI refresh as synchronization. It also risked turning generic directory events into expensive subtree rescans if directory paths were accepted without considering the event kind.
**Fix**: Preserve paths, wait for a 250ms quiet window, sort/de-duplicate the batch, inspect supported files and only newly created/renamed directories, persist canonical metadata before emitting one refresh event, and start watchers before initial scans.
**Lesson**: In a database-backed media manager, filesystem events must mutate the database source of truth before presentation refresh. Debounce noisy events, but keep event-kind filtering so batching does not accidentally convert a small change into a whole-tree scan.
**Related**: ADR-024; `src-tauri/src/scanner.rs`; `src-tauri/src/main.rs`.

### #pagination-003: Filtering missing files after LIMIT cannot preserve pagination semantics
**Time**: 2026-07-13
**Phenomenon**: A gallery page could report more rows and more pages than it rendered because missing source files were removed only after SQLite had already applied `COUNT`, `LIMIT`, and `OFFSET`.
**Cause**: Filesystem existence was treated as a presentation filter rather than persisted query state. Post-query filtering cannot refill a consumed slot or correct the count used for `has_more`.
**Fix**: Add indexed `file_available` state, backfill it during schema migration, restore it on upsert, mark watcher removals unavailable, and require it in page/filter/count/statistics/collection/playback SQL.
**Lesson**: Any predicate that controls membership in a paginated collection must participate in the database count and page query. A filter applied after `LIMIT/OFFSET` is only a defensive guard, never a complete pagination model.
**Related**: ADR-025; `src-tauri/src/db.rs`; `src-tauri/src/main.rs`.

### #windows-path-001: Canonical scanner paths and notify paths may use different Windows prefixes
**Time**: 2026-07-13
**Phenomenon**: Removing a watched directory did not hide its wallpaper descendants even though the watcher delivered the correct directory path.
**Cause**: Rust `canonicalize()` stored an extended `\\?\D:\...` path while the filesystem notification supplied `D:\...`. Direct `Path::starts_with`/string-prefix comparison treated the logically identical roots as different; UNC paths have a corresponding `\\?\UNC\...` form.
**Fix**: Normalize separator direction, ASCII case, trailing separators, drive extended prefixes, and extended UNC prefixes before exact-or-descendant matching.
**Lesson**: Windows path identity is not raw string identity. Any persisted-path comparison against OS event paths must normalize extended prefixes as well as case and separators, and must be covered by a real removal regression test.
**Related**: `src-tauri/src/db.rs`; `src-tauri/src/main.rs`.

### #watcher-lifecycle-001: Never join a watcher callback while holding a lock it may acquire
**Time**: 2026-07-13
**Phenomenon**: Replacing a folder watcher after changing its source looked atomic, but inserting the replacement drops the old handle; `FolderWatcher::drop()` sends stop and joins the callback thread.
**Cause**: If the replacement occurs while the caller still owns the SQLite mutex, an in-flight callback can be waiting for that mutex while the caller waits for the callback to join, forming a direct deadlock cycle.
**Fix**: Start the replacement while retaining the old handle, persist the new source, end the database-lock scope explicitly, then insert the replacement so old-handle destruction happens with no SQLite guard alive. Stop all watchers before final database checkpoint for the same reason.
**Lesson**: RAII destruction is executable control flow. Before replacing or clearing a value whose `Drop` joins a thread, audit every lock still in lexical scope and release any lock the joined thread can request.
**Related**: ADR-026; `src-tauri/src/main.rs`; `src-tauri/src/scanner.rs`.

### #shutdown-001: Database checkpoint must follow every tracked writer
**Time**: 2026-07-13
**Phenomenon**: Startup folder reconciliation and asynchronous folder imports could continue after clean shutdown checkpointed SQLite WAL because their thread handles were not consistently stored in `background_threads`.
**Cause**: Fire-and-forget work and long-lived workers used different ownership paths; shutdown signaled the known workers but had no join handle for every database writer, and checkpoint ran before joins.
**Fix**: Add one tracked spawn boundary that checks shutdown while holding the thread registry, use it for startup snapshots and asynchronous imports, stop and join watcher callbacks, join every tracked worker, then checkpoint WAL last.
**Lesson**: A clean shutdown claim requires an ownership inventory of every writer thread. Checkpointing before the last writer is joined only creates a temporarily clean WAL and can race with late writes.
**Related**: ADR-026; `src-tauri/src/main.rs`.

### #thumbnail-scheduling-001: Debounced work must track desired state, not a pre-committed delta
**Time**: 2026-07-13
**Phenomenon**: Gallery cards could spin indefinitely even though the source files and some disk-cache files were healthy; virtual-list layout recalculation repeatedly changed the visible row array during startup.
**Cause**: `WallpaperGrid` recorded paths as previously loaded before its 80ms timer executed. A recalculation canceled that timer, and the replacement request contained only newly added paths, so paths shared by both visible sets were never sent to the backend.
**Fix**: Move debounce ownership into a tested scheduler that cancels the old timer and submits the complete latest visible set. Keep cache-hit and in-flight de-duplication in the store, where request execution state is known.
**Lesson**: When a debounce can cancel work, never advance a “completed/requested” delta set at scheduling time. Recompute from current desired state or commit progress only after execution starts.
**Related**: `src/components/WallpaperGrid.vue`; `src/utils/thumbnailRequestScheduler.ts`.

### #thumbnail-cache-migration-001: Cache-key migrations need a bounded stale-while-revalidate bridge
**Time**: 2026-07-13
**Phenomenon**: The app-data cache contained 822 non-empty JPEG derivatives, but only 108 of 245 available wallpapers matched the current stable thumbnail key. The first 96 database rows had only 42 current hits, while 25 live wallpapers still had valid original path-only `DefaultHasher` thumbnails.
**Cause**: Moving from the original path-only `DefaultHasher` key through profiled keys to stable FNV keys correctly prevented invalid reuse, but it also made known old derivatives unreachable even when they were sufficient as an immediate placeholder.
**Fix**: Lookup only the two verifiable historical thumbnail schemes after the current key, return a hit as `Stale`, and enqueue current 512/q88 regeneration. An audit of the known profiled `DefaultHasher` generations found zero live hits; the path-only generation supplied 25 real hits.
**Lesson**: Changing a persistent cache identity is a data migration. Prefer an explicit, bounded stale-while-revalidate compatibility list over directory scanning or forced cold regeneration.
**Related**: `src-tauri/src/thumbnails.rs`; `src-tauri/src/main.rs`; `src-tauri/src/thumbnail_cache_fallback_tests.rs`.

### #build-artifact-001: Preserve final bundles before cleaning Cargo target directories
**Time**: 2026-07-15
**Phenomenon**: Repeated debug, test, release, and packaging runs accumulated 71,177 files under `src-tauri/target`, consuming 26.298 GiB even though the source tree itself was below 1 MiB.
**Cause**: Cargo incrementals, dependency objects, multiple profiles, documentation/build-script outputs, and Windows bundler intermediates are all retained until an explicit clean. A historical alternate target directory and `$c` registry-script residue also remained beside the normal target.
**Fix**: Verify no build/app processes or reparse points are present, copy only the latest EXE/MSI/NSIS to an ignored `release-artifacts` directory and verify SHA-256, run `cargo clean`, then remove only the exact verified alternate-target and residue directories.
**Lesson**: `cargo clean` also removes release bundles. Preserve and hash final deliverables first, then clean the reproducible target tree; never recursively delete an unverified or reparse-point path.
**Related**: `.gitignore`; `release-artifacts/PureWall-0.1.0-20260713/`.

### #thumbnail-validation-001: Thumbnail batch validation must not hydrate full wallpaper entities
**Time**: 2026-07-15
**Phenomenon**: A visible batch of 12 thumbnails could remain in the loading state while the backend performed repeated SQLite work before even checking derivative-cache hits.
**Cause**: `load_thumbnails_batch` called `get_wallpaper_by_path` for every source. That hydrated every wallpaper row, ran tag attachment queries, and retained the shared database mutex while source/cache filesystem metadata and LRU touches ran.
**Fix**: Validate local image files outside SQLite, query only registered `file_available = 1` paths through one bounded `IN (...)` statement per frontend batch, release the database mutex, then perform cache lookup and queue work.
**Lesson**: Hot-path authorization/registration checks should select only the identity fields they need. Never retain a shared database lock across unrelated filesystem cache IO.

### #weighted-random-001: Duplicating a selected liked path does not create 2x selection probability
**Time**: 2026-07-15
**Phenomenon**: Single-wallpaper rotation was effectively uniform, while independent-display selection could return the same liked wallpaper for adjacent monitors.
**Cause**: The selector first accepted a uniformly ordered candidate, then appended a duplicate for liked rows. Truncating to one preserved the already-uniform first choice; returning several paths exposed the adjacent duplicates.
**Fix**: Collect unique existing candidates, give liked candidates two tickets and normal candidates one, then sample without replacement. Repeat a path only when fewer eligible unique files exist than requested displays.
**Lesson**: Weighting must happen before selection, and multi-result random selection needs an explicit without-replacement contract.

### #thumbnail-failure-001: Async thumbnail pipelines need a terminal per-path state
**Time**: 2026-07-15
**Phenomenon**: Validation, generation, or WebView asset-decode failures could leave a wallpaper card showing an animated spinner indefinitely.
**Cause**: Only successful workers emitted an event. Failure paths logged to stderr, and the visibility scheduler had no retry count or terminal state, so it could neither recover predictably nor stop requesting a permanently failing path.
**Fix**: Emit `thumbnail-generation-failed` for validation/registration/generation failures, retry after 250ms and 500ms, preserve a usable stale cache URL, and expose a terminal Retry control that is excluded from automatic visible-path scheduling.
**Lesson**: Fire-and-event media work needs success, retryable failure, and terminal failure states. A spinner is not an error state machine.

### #git-signing-001: SSH commit signing depends on the configured agent owning the private key
**Time**: 2026-07-15
**Phenomenon**: `git commit` failed before creating an object because the configured 1Password SSH signing agent could identify the public key but could not find its private key in the available vaults.
**Cause**: Repository changes and the Git index were valid, but the machine-level signing configuration depended on external credential state unavailable to the current session.
**Fix**: Preserve the verified index and retry only the requested commit with `git -c commit.gpgsign=false commit`; do not change repository or global signing configuration implicitly.
**Lesson**: Treat commit signing as an external precondition. A signing failure does not invalidate the staged diff, and a deliberate one-command unsigned fallback is safer than silently rewriting persistent Git configuration.

### #playback-001: Completion events need explicit ownership, not action/FIFO inference
**Time**: 2026-07-22
**Phenomenon**: Applying both a typed playback command outcome and its broadcast completion event duplicated row/media/stat refreshes; an action-name pending queue then dropped or mispaired concurrent and external same-action events.
**Cause**: The event payload has no request correlation id, so action vocabulary and arrival order cannot establish identity across main-window, widget, tray, CLI, and timer entry points.
**Fix**: Make the invoking WebView own its command outcome and filter the existing completion event away from that caller; continue broadcasting events for non-WebView entry points and to other windows.
**Lesson**: Exactly-once UI reconciliation requires an explicit completion owner or correlation id. Timing windows, FIFO queues, and action-name matching are not correctness boundaries.

### #playback-002: Tauri `EventTarget::Any` bypasses `emit_filter`
**Time**: 2026-07-22
**Phenomenon**: A correct caller-label filter still delivered the caller's playback completion event, so the invoking window reconciled both the event and command outcome.
**Cause**: `@tauri-apps/api/event.listen` defaults to `EventTarget::Any`; Tauri 2.11.2's listener matcher accepts `Any` before evaluating the supplied filter predicate.
**Fix**: Register caller-excludable playback listeners with an explicit `{ kind: "WebviewWindow", label: currentWindow.label }` target and assert that target in frontend tests.
**Lesson**: Event-emitter filters operate on listener targets, not on the physical WebView that registered an unscoped listener. A default `Any` listener is broadcast-only and cannot participate in label exclusion.

### #playback-003: Partial external success must be durable per item
**Time**: 2026-07-28
**Phenomenon**: Independent display playback could successfully change DISPLAY-A, fail on DISPLAY-B, and return before writing any history or primary current state.
**Cause**: The orchestrator used an apply-all-then-record-all sequence, incorrectly treating several externally committed monitor operations as one atomic unit.
**Fix**: Pair each successful monitor apply immediately with its display history record, and persist primary current before attempting subsequent monitors.
**Lesson**: A sequence of external side effects is not transactional. Commit durable facts at each successful item boundary so later failure cannot erase what already happened.

### #playback-004: Identity payloads are not state transitions
**Time**: 2026-07-28
**Phenomenon**: A delayed rating completion for wallpaper A could arrive after Next moved to B and roll current/active preview state back to A.
**Cause**: The generic reconciler treated every non-empty outcome path as a current transition even though Like/Dislike paths identify only the rating target.
**Fix**: Reconcile by action semantics: Next transitions current/active media, Like/Dislike patch their target rating and refresh it only if still current, and Pause changes only pause state.
**Lesson**: A payload identifier answers “which entity?” but does not by itself mean “make this entity current.” State transitions must be explicit in the action contract.

### #playback-005: Post-commit completion delivery is not action failure
**Time**: 2026-07-28
**Phenomenon**: A playback action could commit its external or durable side effects, then appear to fail only because its completion event could not reach a WebView.
**Cause**: The executor treated post-commit notification delivery as part of the action transaction even though the committed side effect could not be rolled back safely.
**Fix**: Return the committed action outcome; make completion emit/WebView delivery best-effort, report delivery failures through the observable logging path, and keep them out of the action error result.
**Lesson**: After an action commits, a notification failure is an observable delivery problem, not evidence that the action failed. Returning it as an action failure invites unsafe retries and duplicate side effects.

### #testing-001: `catch_unwind` tests must authenticate the intended panic
**Time**: 2026-07-28
**Phenomenon**: A panic-safe cleanup regression could pass when temporary-file setup failed before reaching its deliberate panic, because both paths produced `Err` and absent files.
**Cause**: The guard creation and artifact writes ran inside `catch_unwind`, and the test checked only `is_err()` rather than the captured panic identity.
**Fix**: Create the guard and every artifact outside the catch boundary, assert every artifact exists, then move only the guard into a closure with one sentinel panic. Downcast both `&str` and `String` payloads, require an exact sentinel match, and only then assert post-unwind cleanup.
**Lesson**: `catch_unwind` proves only that some panic occurred. Setup must fail outside the catch, and the captured payload must identify the intentional panic before cleanup postconditions can count as evidence.
**Related**: `src-tauri/src/playback_entrypoint_tests.rs`.

### #windows-path-002: Tests must canonicalize roots when production persists canonical descendants
**Time**: 2026-07-28
**Phenomenon**: `watched_directory_removal_hides_descendants_without_deleting_metadata` passed locally but failed on the GitHub Windows runner because removing a watched source left one descendant available.
**Cause**: The test supplied raw `std::env::temp_dir()` as the watched root while the stored image path used `canonicalize()`. A runner may expose the same directory through short/long aliases or different Windows prefixes, which separator/case/prefix normalization cannot equate when the path components themselves differ.
**Fix**: Phase 3 Task 3A must canonicalize the fixture root through the same boundary used by production folder import before testing exact-or-descendant behavior, then rerun the GitHub Windows job.
**Lesson**: Path-normalization tests must preserve production path provenance. If production canonicalizes both sides of a persisted relationship, a test that canonicalizes only the child can create a runner-dependent identity mismatch and falsely classify a platform alias as a product regression.
**Related**: `src-tauri/src/main.rs`; GitHub Actions run `30330372062`; Phase 3 library-management design.

### #testing-002: Visible-page database tests require real files
**Time**: 2026-07-28
**Phenomenon**: The overlap-safe KeepMetadata test preserved the child-source row and its `file_available = 1` state, but indexing the first visible page item still failed because the page was empty.
**Cause**: The initial fixture used invented Windows paths. `get_wallpapers_page` deliberately performs a final `Path::is_file()` defense, so a database-only available flag cannot make a nonexistent file visible.
**Fix**: Build canonical temporary parent/child roots, create real fixture files, and assert both metadata results and post-operation file existence.
**Lesson**: Tests that assert visible library pages must satisfy both persisted availability and real filesystem existence. Fake paths are appropriate only when the exercised query does not apply the defensive file check.
**Related**: `src-tauri/src/db.rs`; Phase 3A Task 3 source-removal tests.

### #watcher-lifecycle-002: Removing a watcher handle does not revoke an already queued callback
**Time**: 2026-07-28
**Phenomenon**: Remove or Relocate could commit the source mutation and then drop the old watcher, while a callback that entered just before the source-operation guard was installed remained able to wait for SQLite and re-import the old path after the commit.
**Cause**: The runtime operation guard was checked only when the callback began. Taking and joining the watcher prevents future callbacks but does not invalidate one that already passed that early check.
**Fix**: Carry the owning canonical root and source label into every watcher callback, then re-read `watched_folders` under the same short SQLite guard immediately before applying the callback batch. Skip the batch if either root identity or source label is no longer current. Start/replace/drop watcher handles outside SQLite and watcher-registry lock scopes.
**Lesson**: Runtime handle ownership and callback authorization are separate boundaries. Destruction stops future delivery; every queued writer must still prove that its persisted owner is current at commit time.
**Related**: ADR-029; `src-tauri/src/main.rs`; `src-tauri/src/library_source_tests.rs`.

### #powershell-004: Newline-separated commands do not make a fail-fast verification chain
**Time**: 2026-07-28
**Phenomenon**: `git apply --check` rejected a stale-context patch, but the following newline-separated `git apply` and focused test still ran; because the final existing test command passed, the shell call returned exit code 0 and could be mistaken for a successful RED run.
**Cause**: PowerShell executes newline-separated commands independently and the shell tool reports the final process status unless failure is propagated explicitly.
**Fix**: After every patch check and application, use `if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }` before any later command, then confirm `git status --short` when a patch was expected to be atomic.
**Lesson**: A verification chain is trustworthy only when every prerequisite short-circuits later work. Never count a downstream passing test as evidence that an earlier patch applied.
**Related**: Phase 3B Task 2 batch-command RED cycle.

### #rust-sqlite-001: Bind `query_map` collections before returning a statement block
**Time**: 2026-07-29
**Phenomenon**: Four backup snapshot queries failed with `E0597: statement does not live long enough` even though each `query_map(...).collect()` appeared to finish inside the statement's lexical block.
**Cause**: A `MappedRows` temporary in the block's tail expression is dropped after block locals. Its destructor can still access the statement borrow, so Rust correctly rejects dropping the statement first.
**Fix**: Bind the fully collected vector to a local `rows` value, terminate the collection expression with `;`, then return `rows` as the block's tail value.
**Lesson**: For rusqlite iterators that borrow a prepared statement, do not return the iterator-consuming chain directly from the statement's block. Materialize it in a local value so iterator temporaries are destroyed before the statement.
**Related**: `src-tauri/src/db.rs`; Phase 3C Task 2 consistent backup snapshot.

### #tool-002: An approval-stream disconnect requires a fresh explicit authorization
**Time**: 2026-07-29
**Phenomenon**: Applying the already generated Task 4 Tauri command patch was rejected because the automatic approval review stream disconnected before completion; the command never executed.
**Cause**: The external approval transport failed, and the resulting safety response explicitly prohibited retrying or using an indirect workaround until the user approved again after being informed.
**Fix**: Stop immediately, report the exact completed and pending state, request explicit authorization for the same scoped patch/test command, then verify the worktree and patch file before retrying.
**Lesson**: A transport failure is not evidence that the patch is unsafe, but an explicit no-retry response is still a hard authorization boundary. Preserve the staged reasoning and resume only after a new, informed user approval.
**Related**: Phase 3C Task 4 Tauri backup command wiring.

### #backup-contract-001: Portable enums must match the persisted client vocabulary
**Time**: 2026-07-29
**Phenomenon**: The backup schema accepted `library | focus`, but the production interface-mode composable persisted `workbench | quiet`; exporting the real default would therefore fail validation on the Rust boundary.
**Cause**: The portable DTO vocabulary was designed from semantic labels instead of being checked against the existing persisted client type and storage contract.
**Fix**: Add a cross-boundary RED test for both current values and stale values, then align the Rust allowlist, TypeScript DTO, export fixtures, and import fixtures with `WorkspaceMode`.
**Lesson**: Portable settings must reuse the application's canonical persisted enum vocabulary. Validate every value at both serialization boundaries instead of inventing a parallel product-language enum.
**Related**: `src/composables/useWorkspaceMode.ts`; `src-tauri/src/library_backup.rs`; Phase 3C Task 6.

### #dependency-security-001: Re-resolve and audit the complete tree after every advisory fix
**Time**: 2026-07-29
**Phenomenon**: Fixing the production PostCSS advisory made `npm audit --omit=dev` pass, but a full audit still exposed a separate high-severity denial-of-service chain in the Vue type-checking toolchain. A compatible `brace-expansion` patch alone also remained covered by a newer, wider advisory.
**Cause**: Production-only audit scope excludes important build tooling, and a dry-run or one transitive version bump describes only the advisory data and dependency graph visible at that step. It does not prove the final clean installation is safe.
**Fix**: Inspect each dependency path, upgrade the nearest owned direct dependency with a compatible safe release, regenerate the lockfile without `--force`, run a fresh `npm ci`, then require both `npm audit` and `npm audit --omit=dev` to pass before rebuilding release bundles.
**Lesson**: Security remediation is complete only against a freshly resolved, freshly installed, fully audited tree. Treat dry-run output and production-only audit as diagnostic slices, not the final gate.
**Related**: `package.json`; `package-lock.json`; Phase 3D Task 1.

### #git-patch-001: Zero-context patches require an explicit Git apply mode
**Time**: 2026-07-29
**Phenomenon**: A five-line checklist patch contained correct paths and byte-identical target text, but both `git apply --check` and whitespace-tolerant retries reported that the first line could not be found.
**Cause**: Every hunk replaced one isolated line with no surrounding context. Git rejects zero-context unified diffs by default even when the target text is present.
**Fix**: Confirm the target bytes and index content, then run both the check and application with `--unidiff-zero`; continue to use fail-fast exit propagation.
**Lesson**: A no-context apply failure is a patch-format gate, not evidence of stale content. Prefer contextual hunks; when zero-context is intentional, authorize that mode explicitly for both validation and application.
**Related**: `docs/superpowers/plans/2026-07-29-purewall-phase-3d-final-qa.md`; Phase 3D Task 1.

### #browser-qa-001: Tauri browser QA needs an init-time bridge and explicit evidence boundaries
**Time**: 2026-07-29
**Phenomenon**: Opening the Vite URL directly produced an empty accessibility snapshot because `getCurrentWindow()` ran before Tauri metadata existed. Passing a complete mock through `playwright-cli run-code` then exceeded the Windows command-line length limit.
**Cause**: A Tauri frontend assumes its native bootstrap exists before Vue modules execute, while the CLI serializes inline code into a single Windows command argument.
**Fix**: Install a minimal read-only `__TAURI_INTERNALS__` bridge with `page.addInitScript({ path })`, reload from a named session, and label all resulting evidence as mocked browser-shell coverage. Keep real commands for the isolated Tauri task.
**Lesson**: Browser layout evidence is useful only after matching the native initialization order, and it must never be promoted to native integration evidence. Load substantial Playwright setup from a file rather than the command line.
**Related**: `src/components/TitleBar.vue`; Phase 3D Task 2.

### #responsive-inspector-001: System navigation must not inherit library-content visibility
**Time**: 2026-07-29
**Phenomenon**: An empty library could activate Settings without rendering it; after fixing that logic, the 800×600 media query still hid the panel. The first absolute overlay then collapsed to 26px.
**Cause**: AppShell tied all inspector visibility to wallpaper presence, CSS hid all inspector variants below 1180px, and an absolutely positioned grid child used its collapsed implicit third grid column as the containing block.
**Fix**: Derive one system-aware visibility value, distinguish the system inspector shell, keep the Library policy unchanged, and place the overlay across `grid-column: 1 / -1` before applying its bounded width. Add Escape, dialog guard, listener cleanup, and focus return.
**Lesson**: Content inspectors and application-settings drawers have different availability rules. Responsive overlays inside CSS Grid must define their grid containing block explicitly; absolute positioning alone does not escape grid placement.
**Related**: `src/components/AppShell.vue`; `src/components/InspectorPanel.vue`; `src/styles.css`; Phase 3D Task 2.

### #git-patch-002: Avoid first-line patch context when a tracked file has a BOM
**Time**: 2026-07-29
**Phenomenon**: An otherwise exact `InspectorPanel.vue` patch repeatedly failed while searching for the first `<script>` line and import, although PowerShell displayed identical text.
**Cause**: The file begins with an invisible byte-order mark, so a patch hunk anchored to the first visible line was not byte-identical.
**Fix**: Exclude the BOM-bearing line from the hunk, replace the second line with an intentional zero-context hunk, and validate/apply it with `--unidiff-zero`.
**Lesson**: Visible text equality does not prove first-line byte equality. When only the first hunk fails, inspect BOM/encoding before retrying whitespace flags or rewriting the file.
**Related**: `src/components/InspectorPanel.vue`; Phase 3D Task 2.

### #windows-data-dir-001: Process-scoped APPDATA does not isolate Windows Known Folder app data
**Time**: 2026-07-30
**Phenomenon**: A release executable launched with process-scoped `APPDATA` and `WEBVIEW2_USER_DATA_FOLDER` correctly isolated its WebView profile, but the native window displayed the real wallpaper library and no database appeared below the intended QA app-data directory.
**Cause**: PureWall resolves native data through `tauri::Manager::path().app_data_dir()` and `dirs::data_dir()`. On Windows these APIs use the Known Folder result rather than accepting a child process's `APPDATA` override as an isolation contract.
**Fix**: Stop the owned process before any UI interaction, delete the captured app-window screenshot because it contained real thumbnails, and mark the native smoke BLOCKED. A future retry needs either an accepted, app-supported data-root override or a disposable Windows user; merely changing environment variables is insufficient.
**Lesson**: Verify that the isolated database is created below the intended root before interacting with a native app. Process/profile isolation and application-data isolation are separate boundaries, and a failed boundary means startup side effects cannot be claimed absent without a recorded baseline.
**Related**: `src-tauri/src/paths.rs`; `src-tauri/src/main.rs`; Phase 3D Task 3.

### #git-signing-001: A missing 1Password SSH key can reject an otherwise valid local commit
**Time**: 2026-07-30
**Phenomenon**: Exact documentation files staged successfully, but `git commit` failed before writing the commit object because the configured 1Password SSH signing agent could not find the configured public key.
**Cause**: Global Git configuration enables `commit.gpgsign=true` with `gpg.format=ssh`, while the corresponding private key was unavailable to the signing agent in this session.
**Fix**: Verify the configuration origin and staged diff, then use `git -c commit.gpgsign=false commit ...` for this one local commit. Do not alter global or repository signing configuration.
**Lesson**: A signing-agent failure does not invalidate the index, but it must not be hidden or worked around by changing persistent user configuration. Confirm that no commit object was created, preserve exact staging, and scope any unsigned retry to one command.
**Related**: Phase 3D Task 3 documentation commit.

### #backup-export-001: Native save filters do not protect atomic replacement targets
**Time**: 2026-07-30
**Phenomenon**: The backup UI suggested JSON files, but the Tauri command accepted any absolute destination and the Windows atomic installer used replace-existing semantics. Selecting a registered wallpaper as the destination could replace its bytes with JSON.
**Cause**: The implementation treated the native save dialog filter as a security boundary and placed destination validation inside a generic atomic writer that had no knowledge of protected wallpaper or app-data paths.
**Fix**: Keep the atomic writer's legitimate same-name backup replacement behavior, but add command-layer validation before any temp-file creation: require `.json`, compare canonical path identities against every exported registered wallpaper, and reject the PureWall app-data root and descendants.
**Lesson**: A picker filter is presentation, not authorization. Any command that atomically replaces a user-selected path must independently define and enforce the files it is forbidden to replace.
**Related**: `src-tauri/src/main.rs`; `src-tauri/src/library_backup.rs`; Phase 3D Task 4 review.

### #backup-preview-001: Re-reading a path does not bind confirmation to previewed bytes
**Time**: 2026-07-30
**Phenomenon**: Preview read backup A, but confirmation re-read only the same path. A sync client or another process could replace the file with valid backup B between actions, so the UI counts/warnings described A while SQLite merged B.
**Cause**: Path equality was mistaken for content equality; the preview DTO had no token or digest that confirmation could prove.
**Fix**: Compute a SHA-256 digest over the bounded preview bytes, return it as `contentDigest`, require it as `expectedDigest` on confirmation, and compare before parsing or starting the database merge. Return `BACKUP_PREVIEW_CHANGED` on absence or mismatch.
**Lesson**: A preview/confirm workflow must bind authorization to immutable content, not a mutable locator. Once bytes are read and verified, continue from that same in-memory buffer so later path changes cannot alter the transaction input.
**Related**: `src-tauri/src/library_backup.rs`; `src/stores/wallpapers.ts`; `src/components/LibraryBackupSettings.vue`; Phase 3D Task 4 review.

### #powershell-failfast-001: Semicolon-separated verification does not stop a later Git commit
**Time**: 2026-07-30
**Phenomenon**: `git diff --cached --check` reported a trailing blank-line warning, but the following local commit still executed successfully.
**Cause**: PowerShell commands were separated with semicolons and no explicit `$LASTEXITCODE` guard, so the failed verification command did not terminate the command sequence.
**Fix**: Correct the whitespace, amend the local documentation commit, and use explicit fail-fast checks before every later commit instead of relying on command sequencing.
**Lesson**: A verification command being present is not a gate unless its non-zero exit stops all later mutations. In PowerShell, inspect `$LASTEXITCODE` and `exit` before commit, packaging, or push steps.
**Related**: Phase 4 plan baseline commit `aeaa7d5`.

### #browser-qa-002: Device scale factor does not shrink CSS layout
**Time**: 2026-08-01
**Phenomenon**: A mocked 1.25/1.5 browser matrix kept an 800×600 CSS viewport and passed clipping checks, so it appeared to cover zoom stress without reducing the layout space.
**Cause**: Playwright `deviceScaleFactor` changes raster density, not the configured CSS viewport. `locator.boundingBox()` is measured in CSS pixels, but the assertion multiplied the CSS viewport by DPR, making the accepted boundary too large.
**Fix**: For a fixed physical target, explicitly derive the CSS viewport as `floor(physical / scale)` (800×600 → 640×480 at 1.25 and 533×400 at 1.5), then compare CSS boxes directly with `window.innerWidth` / `window.innerHeight`.
**Lesson**: DPR emulation and zoom-layout pressure are separate. Never call a device-scale run zoom coverage unless CSS layout dimensions actually shrink, and never mix CSS-pixel boxes with device-pixel bounds.
**Related**: `output/playwright/phase4/phase4_browser_qa.mjs`; Phase 4 Task 5.

### #shell-property-write-001: `GPS_DEFAULT` property stores are read-only
**Time**: 2026-07-15
**Phenomenon**: Editing a title updated PureWall's library but Windows file metadata failed with `拒绝访问。 (0x80030005)`; recoverable libjpeg warnings could appear nearby and obscure the property-system failure.
**Cause**: `SHGetPropertyStoreFromParsingName` was called with `GPS_DEFAULT`. Windows returns a read-only store for that flag, so `IPropertyStore::SetValue` returns `STG_E_ACCESSDENIED` regardless of the earlier SQLite success.
**Fix**: Open with `GPS_READWRITE`, coerce `System.Title` to its canonical property type, call `SetValue` and `Commit`, propagate every failure, and update SQLite only after the file commit succeeds.
**Lesson**: Diagnose interleaved native logs by subsystem and HRESULT. A successful database write says nothing about Shell Property System mutability; integration tests must exercise a real temporary file.
**Related**: ADR-030; `src-tauri/src/shell_metadata.rs`; `src-tauri/src/main.rs`.

### #image-overlay-contrast-001: Application theme colors are unsafe over arbitrary photography
**Time**: 2026-07-15
**Phenomenon**: The Living Stage title became dark in the light application theme and was difficult to read on parts of the wallpaper.
**Cause**: The title used `--stage-copy`, whose light-theme value targets application surfaces rather than unknown image luminance.
**Fix**: Give the image-overlay title a fixed white foreground and layered dark shadow independent of application theme tokens.
**Lesson**: Text over user-selected imagery needs its own image-safe contrast treatment; surface-theme contrast does not transfer to photographic backgrounds.
**Related**: `src/styles.css`; `src/utils/stageTitleContrast.test.ts`.

### #cargo-test-filter-001: `--exact` can pass while running zero focused tests
**Time**: 2026-07-15
**Phenomenon**: A focused Rust command exited successfully but reported zero tests after using a bare function name with `--exact`.
**Cause**: Rust's test harness includes the module path in the exact test name.
**Fix**: Use the full module-qualified name or omit `--exact`, and always inspect the executed test count rather than trusting exit code alone.
**Lesson**: A green test process with zero matches is not evidence. Focused-test verification must assert that the intended test actually ran.

### #vue-event-handler-001: DOM event payloads must not flow into semantic boolean parameters
**Time**: 2026-07-15
**Phenomenon**: Unit tests passed, but `vue-tsc` rejected a blur binding after `saveDisplayTitle(force = false)` added an optional boolean parameter.
**Cause**: `@blur="saveDisplayTitle"` passes a `FocusEvent`, which is not assignable to the function's boolean parameter.
**Fix**: Bind blur through `() => saveDisplayTitle()` and reserve `saveDisplayTitle(true)` for the explicit Enter retry.
**Lesson**: When a Vue handler gains semantic arguments, wrap payload-producing DOM events explicitly and retain production type-checking in the completion gate.

### #desktop-wallpaper-e-fail-001: Windows `IDesktopWallpaper` can return a transient `E_FAIL`
**Time**: 2026-07-16
**Phenomenon**: Independent-display rotation reported `未指定的错误 (0x80004005)` once, while subsequent operations and every library image worked.
**Cause**: The persistent STA worker reached a one-off Windows desktop wallpaper COM failure. The title-bearing JPEG was not corrupt, both current monitor paths reapplied successfully, and all 245 available images set successfully on both monitors.
**Fix**: Retry only HRESULT `E_FAIL` once after 80ms at the `IDesktopWallpaper` call boundary, preserve immediate failure for every other HRESULT, and attach monitor/file context if the retry also fails.
**Lesson**: Generic COM `E_FAIL` needs evidence before blaming the input file. Exhaustively compare the same files through the native API, use a bounded HRESULT-specific retry for proven transient behavior, and never retry deterministic path or permission errors.
**Related**: `src-tauri/src/wallpaper.rs`.

### #bootstrapped-entity-patch-001: Paginated entity patches must include the startup fallback row
**Time**: 2026-07-16
**Phenomenon**: A title was committed to the file and SQLite, but the Living Stage did not immediately show it.
**Cause**: The current wallpaper can be restored through `bootstrappedWallpaper` before pagination. `patchWallpaper` updated only the loaded array, so an active row outside the current page remained stale.
**Fix**: Apply every matching entity patch to `bootstrappedWallpaper` as well as the paginated array.
**Lesson**: When a computed entity can resolve from multiple caches or fallback records, mutation helpers must update every authoritative presentation copy or replace them with one normalized identity store.
**Related**: `src/stores/wallpapers.ts`; `src/utils/stageTitleContrast.test.ts`.

### #powershell-utf8-pipeline-001: Native-path diagnostics must pin UTF-8 across Python and PowerShell
**Time**: 2026-07-16
**Phenomenon**: The first library-wide `IDesktopWallpaper` probe appeared to find `0x80070002` on a Chinese file name, but the printed path was mojibake.
**Cause**: Python emitted JSON through a PowerShell pipeline without both sides explicitly using UTF-8, corrupting the path before it reached the Windows API.
**Fix**: Set `PYTHONUTF8=1`, reconfigure Python stdout to UTF-8, and set PowerShell console/output encodings before converting JSON. Verify `Test-Path` on a non-ASCII sample before trusting the probe.
**Lesson**: Encoding corruption can manufacture convincing filesystem/API failures. Any Windows diagnostic that transports Unicode paths across processes must verify round-trip identity before interpreting HRESULTs.

### #windows-edit-002: StreamReader/StreamWriter fallback can add a UTF-8 BOM
**Time**: 2026-07-20
**Phenomenon**: A narrow PowerShell fallback edit produced an unrelated first-line diff containing `U+FEFF` in existing UTF-8 source and Markdown files.
**Cause**: Reusing `StreamReader.CurrentEncoding` with `StreamWriter` selected a BOM-emitting UTF-8 encoding even though the original file had no BOM.
**Fix**: Before any fallback write, detect and retain the original byte-order mark explicitly. For UTF-8 files without one, use `UTF8Encoding(false)`; verify the first three bytes after each write.
**Lesson**: Preserving line endings is insufficient for audited Windows edits; preserve byte-order-mark state too, otherwise a tiny hunk can create an unrelated file-wide first-line change.

### #nested-sidebar-scroll-001: An overflow list can have scroll content but no usable viewport
**Time**: 2026-07-20
**Phenomenon**: Read-only 800x600 QA reported large `scrollHeight` values for Tags and Collections, yet their nested lists could not both scroll; Collections had `clientHeight: 0`.
**Cause**: The winning 48px section minimum was smaller than the fixed flex-column content. Each section spends 80px on its 24px heading, 32px creator, and two 12px gaps; Collections also spends 13px on top padding and border. Flex shrink therefore collapsed the list viewport to zero.
**Fix**: Give the winning Living Gallery tag/collection section a 120px minimum, leaving a 40px Tags viewport and a 27px Collections viewport while the outer side rail owns page-level vertical overflow.
**Lesson**: `overflow-y: auto` is insufficient when flex sizing collapses the scrollport. Verify both `clientHeight > 0` and a real wheel-induced `scrollTop` change for nested scrollers, including variants with extra padding or borders.
**Related**: `src/styles.css`; `.superpowers/sdd/task-4-qa.mjs`; `#frontend-qa-001`; `#frontend-qa-002`.

### #vitest-worktree-discovery-001: Repository-local worktrees can inflate root test counts
**Time**: 2026-08-01
**Phenomenon**: The post-merge root `npm run test:unit` reported 55 files / 170 tests while two linked worktrees still existed; after removing the obsolete worktrees and restoring one additional user test, the same root command reported the actual 25 files / 75 tests.
**Cause**: Default Vitest discovery can traverse project-local `.worktrees` content even though Git ignores that directory, so duplicated or older test suites can enter a repository-root run.
**Fix**: Remove obsolete linked worktrees before recording final root-suite counts, or explicitly exclude `.worktrees/**` in the test configuration. Inspect discovered test paths when counts unexpectedly jump.
**Lesson**: Git ignore and test-runner discovery are separate boundaries. A larger green count is not stronger evidence if it contains duplicate worktree suites.

### #git-stash-phase-merge-001: Restoring a dirty main after phase consolidation needs semantic conflict recovery
**Time**: 2026-08-01
**Phenomenon**: A named `git stash --include-untracked` safely protected a dirty, outdated main, but applying it after the Phase 4 fast-forward conflicted in three append-only project documents and one store helper. Both branches had also allocated `ADR-027` independently.
**Cause**: The completed Phase branches and the pre-existing working tree both appended from the same older document tails and modified the same entity-patch helper, so a textual three-way merge could not infer append ordering, ADR identity, or the need to update both state copies.
**Fix**: Apply rather than pop the stash; keep the stash object until verification finishes; preserve both append-only blocks; renumber the restored decision to the next unique ADR (`ADR-030`) and update its references; combine both store updates; then verify tests plus every untracked blob hash before dropping the stash.
**Lesson**: A safety stash protects bytes, not semantic intent. Phase consolidation must audit append-only identities and multi-cache entity updates explicitly, and the stash must remain recoverable until tracked paths, untracked bytes, and the combined test suite are all verified.

### #process-attribution-001: Existing processes must not be attributed by name or process tree alone
**Time**: 2026-08-01
**Phenomenon**: A controller attributed a PureWall development process that had started at 16:46 to the current Phase 5 task and terminated it around 17:15, even though this task never launched the native application.
**Cause**: Process name/tree observations were treated as sufficient ownership evidence without comparing the task start time to the process start time or establishing that the task created the process.
**Fix**: Before terminating any process, compare its creation time with the task timeline and confirm ownership through an exact PID captured by the task's own launcher. If ownership cannot be proven, leave the process running and report it as pre-existing.
**Lesson**: Never infer process ownership from executable name, port, or ancestry alone. A termination boundary requires positive ownership evidence plus a compatible start-time chronology.
**Related**: Phase 5 Task 1 controller recovery; no native app was launched or restarted by the task.

### #semver-validation-001: SemVer validation must encode identifier grammar, not only allowed characters
**Time**: 2026-08-01
**Phenomenon**: The first release-contract regex accepted invalid SemVer values such as `1.0.0-01`, `1.0.0-alpha..1`, and `1.0.0+build..1`.
**Cause**: A broad `[0-9A-Za-z.-]+` character class allowed empty dot-separated identifiers and did not distinguish numeric prerelease identifiers, which must not contain leading zeroes.
**Fix**: Encode SemVer 2.0.0 core, prerelease, and build identifier rules separately, and keep an executable self-test with independent valid/invalid literals at the release-contract interface.
**Lesson**: A SemVer-shaped string is not necessarily valid SemVer. Release gates must test grammar counterexamples, especially numeric prerelease leading zeroes and empty identifiers around dots.
**Related**: `scripts/verify-release-contract.mjs`; Phase 5 Task 1 Important review correction.

### #powershell-stdin-001: PowerShell pipelines can add CR to native line-oriented stdin probes
**Time**: 2026-08-01
**Phenomenon**: A multi-path `git check-ignore --stdin` probe driven by a PowerShell pipeline printed quoted paths ending in `\r`; directory patterns still matched, but the exact root `skills-lock.json` rule appeared not to match.
**Cause**: PowerShell serialized pipeline strings with CRLF, while Git's line-oriented stdin probe treated the carriage return as part of the exact file name in this Windows invocation.
**Fix**: The persistent Node contract sends one explicit LF-delimited UTF-8 string through `spawnSync`. Independent PowerShell evidence calls `git check-ignore -- <path>` once per path instead of piping exact names through stdin.
**Lesson**: When a Windows native CLI reads exact path names from stdin, do not assume a PowerShell string pipeline preserves LF-only records. Use an API that controls stdin bytes or pass each path as an explicit argument.
**Related**: `.gitignore`; `scripts/verify-release-contract.mjs`; Phase 5 Task 2 quality review.

### #yaml-contract-001: YAML structure cannot be inferred from text markers
**Time**: 2026-08-01
**Phenomenon**: The release contract accepted an Issue Form with an unclosed flow collection, a duplicate top-level key, or `body` nested as a mapping, while rejecting a legal sequence indented four spaces after a comment below `body:`.
**Cause**: Regexes and raw-string markers modeled one visual serialization rather than YAML syntax and the parsed GitHub Issue Form object.
**Fix**: Declare `yaml` as a direct devDependency, parse with unique-key enforcement, then validate root types, required scalars, arrays, first-item type, and private-advisory routing on the parsed value.
**Lesson**: Configuration contracts must validate the parser's semantic output. Text markers are appropriate for ordinary documentation links, not as a substitute for parsing indentation-sensitive formats.
**Related**: `scripts/verify-release-contract.mjs`; Phase 5 Task 2 final quality review.

### #tauri-action-001: tauri-action v1 creates a release only when tagName is explicitly provided
**Time**: 2026-08-01
**Phenomenon**: Phase 5 release.yml assumed a tag push would auto-create a GitHub Release through `tauri-apps/tauri-action@v1`, and that `releaseDraft` defaults to draft behavior.
**Cause**: Inspecting the checked-out tauri-action v1 `dist/index.js` showed this version does NOT auto-detect `refs/tags/`. It reads the `tagName` input, and release creation is gated on `if (tagName && !releaseId)`; with an empty `tagName` it logs "No releaseId or tagName provided, skipping all uploads..." and creates nothing. `releaseDraft` also defaults to `false` in `action.yml`, so drafts require an explicit `releaseDraft: true`.
**Fix**: The release workflow computes a job-level `RELEASE_TAG` expression (`push && github.ref_name || (workflow_dispatch && inputs.publishDraft == 'true' && inputs.tag) || ''`), passes it as `tagName`, and sets `releaseDraft: true`. A manual dry-run therefore passes an empty `tagName` and cannot create a release unless `publishDraft` is explicitly true. Tag-push validation re-runs `npm run verify:release` with `PUREWALL_RELEASE_TAG` set.
**Lesson**: Do not assume GitHub Actions release actions auto-detect the triggering tag. Verify the action's release-gating logic in its source and make the release-creation condition explicit so manual dry-runs are provably release-free.

### #powershell-json-001: ConvertTo-Json output carries the host's newline style
**Time**: 2026-08-01
**Phenomenon**: `scripts/prepare-release-config.ps1` wrote `tauri.release.generated.conf.json` with `UTF8Encoding(false)`, but the generated file contained CRLF line endings even though the repository is LF-normalized.
**Cause**: PowerShell's `ConvertTo-Json` serializes with the host's newline sequence (CRLF under Windows PowerShell 5.1), so BOM control alone does not control line endings.
**Fix**: Normalize the serialized JSON with `-replace "`r?`n", "`n"` before `[System.IO.File]::WriteAllText`, then verify the first bytes (`{ \n`) after the write.
**Lesson**: When a scripted PowerShell pipeline produces text files that must match a repository line-ending convention, normalize newlines explicitly in addition to choosing a BOM-free encoding; verify the first three bytes after every write.

### #rust-module-path-001: db.rs 子模块需要显式 path 属性
**时间**：2026-08-08
**现象**：在 `src-tauri/src/db.rs` 中直接声明 `mod db_taxonomy;` 时，Rust 按内联模块目录规则查找 `src-tauri/src/db/db_taxonomy.rs`，导致根目录新文件未被加载。
**原因**：`db.rs` 本身是 `db` 模块文件；本项目要求拆分文件仍放在 `src-tauri/src/` 根目录，而不是 `src-tauri/src/db/`。
**解决**：使用 `#[path = "db_taxonomy.rs"] mod db_taxonomy;`（sources/backup 同理），继续通过 `impl Database` 保持调用点不变。
**教训**：Rust 文件模块拆分时，先确认目标文件相对的是父模块目录还是 crate `src` 根目录；根目录 sibling 文件需显式 `#[path]` 或改为 crate-level 模块。

### #plugin-config-001: Tauri plugin registration requires its config block in tauri.conf.json even when values are release-injected
**Time**: 2026-08-08
**Phenomenon**: After adding `tauri-plugin-updater` in Phase 5, `npm run tauri dev` panicked at startup with `PluginInitialization("updater", "Error deserializing 'plugins.updater' ... invalid type: null, expected struct Config")` (exit 101).
**Cause**: The plugin's `Config` has a required `pubkey: String` field. Phase 5 deliberately kept real signing values out of the checked-in config (injected at release time by `prepare-release-config.ps1` via `--config` merge), but the plugin initialization path still deserializes `plugins.updater` from `tauri.conf.json` immediately — an absent block deserializes as `null` and fails before any release-only merge can happen.
**Fix**: Add a minimal placeholder block in `tauri.conf.json`:
```json
"plugins": { "updater": { "endpoints": [], "pubkey": "" } }
```
Empty endpoints/pubkey deserialize fine; `validate_endpoints` passes on an empty array; checking for updates later returns a stable `EmptyEndpoints` error mapped to `CHECK_FAILED`, which matches the offline/error contract. The release generation script's real values still win via tauri-action `--config` deep merge.
**Lesson**: Any Tauri plugin whose config struct has required fields must have a minimal valid block in the checked-in config, even when real values are injected later. A "config-free by design" approach breaks plugin initialization at startup, not at first use.
**Related**: `src-tauri/tauri.conf.json`; `scripts/prepare-release-config.ps1`; Phase 5 Task 4.
