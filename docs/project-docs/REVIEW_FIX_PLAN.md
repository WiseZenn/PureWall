# REVIEW_FIX_PLAN.md

> 2026-06-18 对 `REVIEW_REPORT.md` 的源码核查结论与修复计划。
> 本文只记录核查和计划，不直接修改业务代码。

---

## 0. 核查基线

- `cargo check`：PASS（`src-tauri`）
- `npx vue-tsc --noEmit`：PASS
- `DECISIONS.md`：ADR-005 已于 2026-06-19 accepted 为通知式轮播定时器；当前无 P1/ADR 门禁阻塞项。

## 1. 总体结论

`REVIEW_REPORT.md` 中大多数问题真实存在。少数表述需要降级或合并：

- “无标题层级”不是字面上的“零标题”：`EmptyState.vue` 有一个 `h2`。但主工作台确实缺少 `h1`、区域标题和有序标题层级，问题主体成立。
- “无焦点陷阱”对普通下拉菜单表述偏重：`CompactDropdown`/`TagCombobox` 是弹出式 listbox/combobox，不一定需要 modal 级焦点陷阱；但当前确实未处理 Tab/Shift+Tab，键盘体验问题成立。
- “同步 COM 初始化在主线程”表述不准：壁纸操作实际通过 `run_sta()` 新建 STA 线程执行，问题应改为“每次 COM 操作创建新 OS 线程，显示器枚举无缓存”。
- CSP 在第 3.11 和第 4.1 重复出现，按同一安全问题合并处理。

---

## 2. 已核实问题清单

| ID | 报告问题 | 结论 | 证据 | 修复优先级 |
|---|---|---|---|---|
| R-01 | CLI Pause 与主应用暂停状态脱节 | 已修复 | 2026-06-18：CLI/UI pause 统一写 `settings.paused`；主计时器和 `is_paused` 会同步 DB 状态 | Done |
| R-02 | 缺少单实例强制，CLI 可与主应用并发 | 已修复 | 2026-06-18：接入 `tauri-plugin-single-instance`；运行中主实例会接收第二实例 `--action` 并通过 `AppState` 执行/emit，首个 CLI action 进程初始化 state 后执行并退出；review 后补 `try_state` 启动期防护和 hidden main window，避免早期转发 panic/CLI 闪窗 | Done |
| R-03 | watcher 泄露且只保活最近一次调用 | 已修复 | 2026-06-18：`FolderWatcher` 由 `AppState` 持有；换目录/导入目录会替换旧 watcher，Drop 时停止事件线程并释放 notify watcher | Done |
| R-04 | SQLite 未启用 WAL/busy_timeout | 已修复 | 2026-06-18：`Database::new` 已设置 `busy_timeout(5s)`、`journal_mode=WAL`、`foreign_keys=ON` | Done |
| R-05 | 后台线程无关闭机制 | 已修复 | 2026-06-19：`AppState` 增加 shutdown flag 和后台线程句柄；主窗口关闭时通知 rotation/focus 线程退出并 join | Done |
| R-06 | Rust/Vue 系统性静默错误 | 已修复 | 2026-06-18：P0 静默失败面已收口：CLI action 走 `Result`/日志，转发失败 emit `operation-failed`，前端关键业务失败统一进入 `role="alert"` 通知；剩余 best-effort `let _` 归入后续清理 | Done |
| R-07 | 前端 play_count 与 DB 漂移 | 已修复 | 2026-06-18：播放/自动轮播后前端通过 `get_wallpaper_by_path` 读取 DB 更新后的壁纸记录，不再本地 `+1` 推测 | Done |
| R-08 | 删除/移动文件在画廊中保留 | 已修复 | 2026-06-18：`get_wallpapers_filtered`/标签过滤返回前过滤不存在的文件；DB 记录暂不自动删除，避免误伤可恢复路径 | Done |
| R-09 | 全屏自动暂停覆盖用户手动恢复 | 已修复 | 2026-06-18：用户在全屏自动暂停期间手动恢复后，会抑制当前全屏会话的再次自动暂停，直到全屏退出 | Done |
| R-10 | `current_wallpaper.txt` 是 CLI 单点故障 | 已修复 | 2026-06-18：当前壁纸主状态已迁移到 `settings.current_wallpaper`；CLI/widget/display mode 优先读 DB，txt 仅作兼容缓存 | Done |
| R-11 | 扫描无数量/深度限制 | 已修复 | 2026-06-19：`scanner::scan_folder` 增加最大深度、最大图片数、最大目录项预算；`scan_files` 增加单次显式文件导入数量上限并返回可见错误 | Done |
| R-12 | 网络/UNC 路径可能阻塞 | 已修复 | 2026-06-19：command/scanner/watcher 入口拒绝 UNC、扩展 UNC 和 Windows remote drive；递归扫描跳过 symlink/reparse-like entries，避免跟进网络/循环路径 | Done |
| R-13 | SQLite 无外键和 schema 版本 | 已修复 | 2026-06-19：新增 `SCHEMA_VERSION`/`PRAGMA user_version`；新库表定义带 FK，旧库会重建 `wallpaper_tags`/`play_events` 子表以补外键 | Done |
| R-14 | `set_display_mode` 读取当前壁纸失败时静默跳过 | 已修复 | 2026-06-18：Same/Span 读不到当前壁纸时会选取下一张并应用，同时 emit 更新 | Done |
| R-15 | `ThumbnailCache::pre_generate` 未调用 | 已修复 | 2026-06-19：删除未使用的 `pre_generate` 死代码，缩略图入口仅保留实际调用路径 | Done |
| R-16 | 废弃 `widget-like/widget-dislike` 监听 | 已修复 | 2026-06-18：已移除 `wallpapers.ts` 中无 emit 来源的 `widget-like`/`widget-dislike` 监听 | Done |
| R-17 | 壁纸网格无 roving tabindex | 已修复 | 2026-06-18：`WallpaperGrid` 统一管理当前可 Tab 卡片；`WallpaperCard` 接收 `cardTabIndex`，方向键/Home/End 在可见卡片间移动焦点 | Done |
| R-18 | 主工作台标题层级不足 | 已修复 | 2026-06-18：`AppShell` 增加隐藏 h1，`Gallery` 集合标题改为 h2，curated collection 行标题改为 h3 并保留原视觉样式 | Done |
| R-19 | 图片 alt 文本全部相同 | 已修复 | 2026-06-18：`wallpaperAccessibleLabel` 已基于标签、评分、分辨率生成差异化文本 | Done |
| R-20 | `SidebarItem` 用 `aria-current="page"` 表示过滤器 | 已修复 | 2026-06-18：`SidebarItem` 增加 `activeKind`；Library/Liked/Hidden 等过滤器使用 `aria-pressed`，System 区当前项才使用 `aria-current="page"` | Done |
| R-21 | 下拉/组合框未处理 Tab 导航 | 已修复 | 2026-06-19：`CompactDropdown`/`TagCombobox` 在 Tab/Shift+Tab 时收起弹层并让浏览器继续自然移动焦点；listbox options 退出 Tab 顺序 | Done |
| R-22 | 无 skip link | 已修复 | 2026-06-19：`AppShell` 增加 skip-to-main 链接，主工作台补 `id`/`tabindex="-1"` 作为跳转目标 | Done |
| R-23 | Inspector 标签移除按钮缺少具体 aria-label | 已修复 | 2026-06-18：标签移除按钮已添加 `Remove tag ${tag.name}` aria-label | Done |
| R-24 | 搜索框 label 无显式文本 | 已修复 | 2026-06-18：搜索 label 已添加 `.sr-only` 文本 | Done |
| R-25 | 双击设置壁纸同时触发单击选中 | 已修复 | 2026-06-18：卡片单击改为短延迟确认，双击会取消单击动作并设置壁纸 | Done |
| R-26 | 操作失败无用户反馈 | 已修复 | 2026-06-18：新增全局 `NotificationCenter` + `useNotifications`，store 中关键用户操作失败会弹出 `role="alert"` error notification；review 后移除外层 assertive live-region 避免重复播报 | Done |
| R-27 | 无拖放组织/导入 | 已修复 | 2026-06-19：`AppShell` 接入 Tauri WebView drag/drop；drop 文件或目录会调用 `import_dropped_paths`，后端复用本地路径/图片扫描校验，按一次拖放总量执行全局导入上限，去重后写入 DB，前端 reload 后显示导入结果 | Done |
| R-28 | Quiet Canvas 切换无过渡 | 已修复 | 2026-06-19：`AppShell` 用 keyed `Transition` 包裹 Quiet Canvas/Workbench 切换，并支持 reduced motion | Done |
| R-29 | 删除/隐藏无撤销 | 已修复 | 2026-06-19：隐藏/批量隐藏新增 Undo notification；删除/批量删除改为 7 秒 pending delete，Undo 可恢复 UI，超时后才调用回收站删除 | Done |
| R-30 | 卡片 hover overlay 可能闪烁 | 已修复 | 2026-06-19：hover overlay 改为 `v-show`，避免每次 hover mount/unmount；生产构建通过，视觉回归仍建议人工 smoke | Done |
| R-31 | Living Stage 亮色主题遮罩/文字硬编码暗色 | 已修复 | 2026-06-18：Stage veil/copy/meta chip 色值抽成 `--stage-*` 主题变量；暗色保持深遮罩浅文字，亮色改为浅遮罩深文字 | Done |
| R-32 | 品牌副标题/状态栏字号过小 | 已修复 | 2026-06-19：`brand-subtitle` 和 Living 状态栏字号统一提升到 10px，并收紧字距 | Done |
| R-33 | Inspector 元数据图标语义不匹配 | 已修复 | 2026-06-19：Author/Copyright/Comment 分别改用 person/shield/comment 语义图标 | Done |
| R-34 | 卡片 hover 缩放可能裁切 | 已修复 | 2026-06-19：移除卡片 hover 图片放大，避免媒体边界裁切 | Done |
| R-35 | 容器查询重复隐藏 quick settings | 已修复 | 2026-06-18：移除被 1120px 规则覆盖的 900px 重复隐藏声明 | Done |
| R-36 | body 无 line-height | 已修复 | 2026-06-18：`body` 已补充 `line-height: 1.45`，并新增通用 `.sr-only` 样式 | Done |
| R-37 | 壁纸列表 N+1 标签查询 | 已修复 | 2026-06-18：`attach_tags` 改为 `IN (...)` 批量查询并按 wallpaper_id 分组 | Done |
| R-38 | base64 缩略图/预览 Map 无驱逐 | 已修复 | 2026-06-18：前端 `thumbnails`/`previews` 增加 LRU 上限，活动/当前壁纸受保护，删除壁纸时同步释放预览缓存 | Done |
| R-39 | 1 秒轮询定时器持续唤醒 | 已修复 | 2026-06-19：ADR-005 已 accepted 为通知式轮播定时器；`start_rotation_timer` 改用 `Condvar` + generation signal 等待 interval/pause/focus/shutdown 变化，不再每秒轮询累计 | Done |
| R-40 | `get_next_wallpapers` per-file `exists()` | 已修复 | 2026-06-19：播放候选改由 SQLite 按权重随机抽样，Rust 只对有限候选做存在性校验，不再全库逐项 `exists()` | Done |
| R-41 | 无虚拟滚动/分页 | 已修复 | 2026-06-19：`WallpaperGrid` 改为分页式增量渲染，初始 120 项，滚动接近底部或按钮操作再加载更多 | Done |
| R-42 | 字体/Bundle 优化空间 | 已修复 | 2026-06-19：改为本地 `fonts.css` 仅加载 Geist Latin variable 字体；生产构建字体资产从多语种/Inter 分片降到单个 29.40 kB woff2 | Done |
| R-43 | `patchWallpaper` 数组全量拷贝 | 已修复 | 2026-06-19：补 ADR-017；store 维护 path-to-index map，单项 patch 改为替换对应数组槽位对象，整体替换/删除/恢复时重建索引 | Done |
| R-44 | 每次 COM 壁纸操作新建 STA 线程 | 已修复 | 2026-06-19：`wallpaper.rs` 增加持久 STA worker 和 request queue；set all/set monitor/display enumeration 复用同一个 COM apartment | Done |
| R-45 | 缩略图缓存源文件变更不失效 | 已修复 | 2026-06-19：缩略图/预览缓存 key 纳入 path、file size、mtime seconds/nanos，源文件替换后会生成新缓存文件 | Done |
| R-46 | Stage 用 CSS background-image + base64 | 已修复 | 2026-06-19：`CurrentWallpaperPanel` 改用真实 `<img>` 渲染 active preview，并复用差异化 alt 文本；CSS 只负责 `object-fit: cover` 布局；同时移除 shell ambient preview CSS background，避免 preview data URL 继续进入 CSS background | Done |
| R-47 | stats 多次 COUNT 查询 | 已修复 | 2026-06-19：`get_stats` 已是单条条件聚合；`get_yearly_stats` 的 total/unique/liked 三个 COUNT 合并为一条聚合查询，月度分布和 Top 5 保持各自粒度查询 | Done |
| R-48 | `setTimeout` 无清理 | 已修复 | 2026-06-18：`WallpaperCard`/`Gallery` 已保存计时器 ID 并在卸载/完成时清理 | Done |
| R-49 | CSP unsafe-inline/unsafe-eval + global Tauri | 已修复 | 2026-06-18：CSP 已移除 script unsafe，`withGlobalTauri` 已关闭；仍建议 Tauri runtime smoke | Done |
| R-50 | Tauri 命令路径输入验证不足 | 已修复 | 2026-06-18：command 层新增目录/图片文件/DB 已登记壁纸校验；导入要求真实目录/图片文件，评分/标签/黑名单/缩略图/预览/元数据/删除要求路径已登记且仍是支持的图片文件；删除先验证再移入回收站，成功后才移除 DB 记录 | Done |
| R-51 | autostart 使用 PowerShell 字符串 | 已修复 | 2026-06-18：`autostart.rs` 已改为 `reg.exe` 直接参数调用；未执行注册表写入 | Done |
| R-52 | `main.rs` 过大 | 已修复 | 2026-06-19：按 ADR-015 抽出 `diagnostics.rs` 和 `widget.rs`，`main.rs` 从 1492 行降至 1388 行并保留 command 注册/播放编排边界 | Done |
| R-53 | 命令错误扁平化为 `String` | 已修复 | 2026-06-19：可失败的 Tauri command 统一返回 `CommandResult<T>`，序列化错误形状为 `{ code, message }`；前端通知兼容读取 typed command error 的 `message` | Done |
| R-54 | `handle_cli_action` 重复播放逻辑 | 已修复 | 2026-06-18：CLI `next` 已改走 `advance_wallpaper_with_db`，与主应用共享 shared/independent 播放路径 | Done |
| R-55 | 自定义 base64 编码器 | 已修复 | 2026-06-19：改用 `base64` crate 标准编码，并删除自写 `base64_encode` | Done |
| R-56 | 缺少若干正式 ADR | 已修复 | 2026-06-18：补 ADR-012 至 ADR-016，覆盖错误反馈、CSP/Tauri 暴露面、PureWall 注册表边界、模块拆分策略、release console 隐藏 | Done |
| R-57 | 硬编码参数较多 | 已修复 | 2026-06-19：缩略图/预览尺寸、JPEG 质量、data URL 前缀、支持扩展名、默认 tag color、widget 尺寸等已集中到对应模块常量或配置边界；剩余窗口尺寸属于 Tauri 产品配置而非隐藏业务参数 | Done |
| R-58 | CLI release 错误反馈不可见 | 已修复 | 2026-06-18：CLI action 失败会写 `purewall-cli.log` 并转发 `operation-failed`；Advanced 面板新增只读 CLI diagnostics，可刷新查看最近日志尾部 | Done |

---

## 3. 修复计划

### P0：先止血（安全、数据一致性、用户可见失败）

1. **收紧 CSP 与 Tauri 暴露面**
   - 移除 `script-src` 的 `unsafe-inline`/`unsafe-eval`。
   - 优先评估关闭 `withGlobalTauri`，改为模块化 API 导入。
   - 验证：`npm run build`、`npx vue-tsc --noEmit`、Tauri 启动 smoke。

2. **SQLite 连接可靠性**
   - 在 `Database::new` 设置 `PRAGMA journal_mode = WAL`、`busy_timeout = 5s`、`foreign_keys = ON`。
   - 对已有 DB 做兼容验证，确认 WAL 文件路径位于 app data。
   - 验证：`cargo check`，新增 DB 初始化/迁移测试或 smoke。

3. **统一 CLI 与主应用状态入口**
   - 首选重新引入单实例机制，将 `--action` 转发给运行中实例。
   - 若单实例接入风险过高，短期先让主计时器读取统一 pause 状态，并让 CLI action 写入诊断日志。
   - 验证：主应用运行时右键 Next/Like/Dislike/Pause 均能同步 UI/DB。

4. **错误通知与诊断日志**
   - 前端新增 toast/notification composable，关键操作失败显示 `aria-live` 通知。
   - 后端后台线程/CLI 错误写入 app data 诊断日志。
   - 将最危险的 `let _ =` 改为显式处理。

5. **批量标签查询**
   - 用 `WHERE wallpaper_id IN (...)` 一次取回标签，按 wallpaper_id 分组。
   - 保持 `get_tags_for_wallpaper` 供单项调用，但列表路径不再 N+1。

### P1：可用性和架构一致性

1. **路径和删除安全**
   - 文件夹命令验证目录，文件命令验证文件和扩展名。
   - 删除/批量删除要求路径来自已登记壁纸记录，并保留前端确认。
   - autostart 改为 `reg.exe` 直接参数调用；只操作 `HKCU...\Run\PureWall`。

2. **播放路径去重**
   - 提取 `playback.rs`：统一 `advance_shared`、`advance_independent`、record/save/emit 语义。
   - CLI、widget、主命令都调用同一路径。
   - `auto-rotated` 事件返回后端更新后的 entry 或前端按 path 重新拉取单条记录。

3. **基础无障碍修复**
   - WallpaperGrid 实现 roving tabindex 和方向键导航。
   - 增加 app 级 h1 / section h2-h3 / skip link。（已完成）
   - alt 文本基于 tag、rating、resolution、序号生成。
   - SidebarItem 区分 filter button 与 page/system navigation。

4. **亮色主题补齐**
   - Stage veil/copy 改用语义 token 或加 light 覆盖。
   - 清理重复 container query。

5. **ADR 补齐**
   - 至少新增 proposed ADR：CSP 策略、错误处理模型、单实例/CLI IPC、Tauri 命令模块拆分、注册表调用方式。
   - ADR-005 已处理：轮播定时器进入通知式调度。

### P2：规模化与长期稳定性

1. watcher 生命周期：AppState 持有 watcher/线程句柄，支持多文件夹，应用退出设置 shutdown flag。
2. 缓存策略：缩略图 key 纳入 mtime/size；前端 Map 加 LRU；后端磁盘缓存设上限。
3. 大库性能：虚拟滚动或分页；`get_next_wallpapers` 避免全库 stat；stats 条件聚合。（P2 部分已完成，R-47 年度 stats 留 P3）
4. 网络路径可靠性：识别 UNC/网络盘，给用户提示；避免在热路径反复 `canonicalize`。
5. COM 执行模型：持久 STA worker 或小型队列，减少频繁线程创建。（已完成）

### P3：体验抛光和维护性

1. Quiet Canvas 过渡、hover overlay 延迟、图片 hover 动画策略。
2. Undo toast：隐藏可即时撤销；删除如果依赖回收站恢复，需要先验证可实现性，否则只做“撤销从列表移除但文件已入回收站”的明确提示。
3. 字体和 release bundle 优化：评估系统字体优先、LTO/size profile。
4. 清理死代码：`widget-like/widget-dislike` 监听、自定义 base64 替换标准 crate。
5. 硬编码参数配置化：只优先暴露用户确实需要调的值，避免设置页膨胀。

---

## 4. 建议执行顺序

1. **安全与状态一致性批次**：R-49、R-04、R-13、R-01、R-02、R-06、R-26。
2. **数据库/性能批次**：R-37、R-38、R-40、R-47。（R-47 年度 stats 留 P3）
3. **播放/CLI 架构批次**：R-54、R-10、R-14、R-07、R-58。
4. **无障碍批次**：R-17、R-18、R-19、R-20、R-21、R-22、R-23、R-24。（已完成）
5. **资源生命周期批次**：R-03、R-05、R-44、R-45。（已完成）
6. **视觉与交互抛光批次**：R-25、R-29、R-31、R-35、R-36。（已完成）

每个批次完成后按项目规范更新 `CHANGELOG_AI.md`；如果遇到新的 Windows/Tauri/注册表/沙箱坑，append 到 `AI_DIARY.md`。

## 5. 2026-06-19 completion status

- 最后一轮关闭 R-27、R-43、R-46、R-57。
- 当前清单中已无开放 P0/P1/P2/P3 项。
- 本轮验证：`npx vue-tsc --noEmit` PASS；`cargo check` PASS（普通沙箱写 `target` 被拒后提权重跑通过）；`npm run build` PASS；`git diff --check` PASS（仅 LF/CRLF 提示）。
- 本轮未执行注册表命令，未修改系统设置。