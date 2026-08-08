# PureWall 全面审查报告

> **审查日期**：2026-07-02
> **审查范围**：全栈 — Rust 后端 · Vue 前端 · UI/UX · 安全 · 构建 · 开源合规
> **审查方法**：三方并行逐行审查（架构代理 + 前端代理 + 安全代理）+ 架构师综合研判
> **发现总数**：52 项

---

## 评级

| 级别 | 含义 | 数量 |
|------|------|------|
| 🔴 Critical | 影响数据完整性 / 安全 / 功能可用性 | 3 |
| 🟠 High | 用户体验显著受损 / 潜在 bug / 安全风险 | 14 |
| 🟡 Medium | 代码质量 / 可维护性 / 边界情况 | 22 |
| 🟢 Low | 优化建议 / 锦上添花 | 13 |

---

## 一、🔴 Critical (3)

### CRT-01: `md5_hash` 函数名误导 + 哈希值跨版本不稳定

**文件**：`src-tauri/src/scanner.rs:250-263` | **分类**：数据完整性

**问题**：函数名为 `md5_hash` 但实际使用 `std::collections::hash_map::DefaultHasher`，与 MD5 完全无关。且 `DefaultHasher` 输出不保证跨 Rust 版本/编译一致。用户升级 PureWall 后重新扫描 → 同一壁纸产生不同 hash → DB 重复条目。

**修复**：替换为真正的 `md5` crate 或 `blake3`。旧 DB 的 hash 值会失效需一次性重扫。

### CRT-02: `trash::delete` 批量删除弹出确认对话框

**文件**：`src-tauri/src/main.rs:823, 941` | **分类**：用户体验 / 功能可用

**问题**：`trash` crate v5 在 Windows 上使用 `IFileOperation`，默认显示确认对话框。批量删除 50 张壁纸 = 50 个连续弹出框。

**修复**：直接调用 `IFileOperation` API 并设置 `FOF_SILENT | FOF_NOCONFIRMATION` 标志。

### CRT-03: 通知组件使用了 5 个未定义的 CSS 变量 — 透明不可见

**文件**：`src/styles.css:3401-3470` | **分类**：UI 功能

**问题**：`.app-notification` 引用 `--stroke-strong`、`--panel-solid`、`--shadow-elevated`、`--text-tertiary`、`--surface-hover` — 全未定义。通知面板无边框、无背景、无阴影，与背景融为一体，用户完全看不到。

**修复**：映射到已有令牌 — `--stroke-strong→--border-strong`、`--panel-solid→--bg-panel` 等。

---

## 二、🟠 High (14)

### HIG-01: WallpaperCard 单击有 180ms 人为延迟
**文件**：`src/components/WallpaperCard.vue:42-47` | **修复**：用 `MouseEvent.detail`（click=1, dblclick=2）替代 setTimeout debounce

### HIG-02: QuietCanvas CSS background-image 加载 Asset 协议 URL
**文件**：`src/components/QuietCanvas.vue:23` | **修复**：改用 `<img>` + `object-fit: cover` + 绝对定位

### HIG-03: import_wallpaper_folder DB 加锁失败静默丢弃
**文件**：`src-tauri/src/main.rs:361-391` | **修复**：加锁失败时 emit `operation-failed`

### HIG-04: 批量数据库操作没有事务包装
**文件**：`src-tauri/src/db.rs:543-562` | **修复**：包裹在 `conn.transaction()?` 中

### HIG-05: batch_delete_wallpapers 部分失败无回滚
**文件**：`src-tauri/src/main.rs:933-951` | **修复**：先校验所有文件 → 批量 trash → DB 事务回滚

### HIG-06: 用户提供的路径未经规范化
**文件**：`scanner.rs:43-86`, `main.rs:179-214` | **修复**：`ensure_local_path` 后 `.canonicalize()`

### HIG-07: SQLite WAL 文件无自动 checkpoint
**文件**：`db.rs:77-78` | **修复**：`PRAGMA wal_autocheckpoint=1000` + 关闭时 `wal_checkpoint(TRUNCATE)`

### HIG-08: get_next_wallpapers 逐条文件存在检查
**文件**：`db.rs:616` | **修复**：加 `file_exists` 列或批量检查

### HIG-09: 壁纸卡片 aria-selected 指向错误布尔值
**文件**：`WallpaperCard.vue:109` | **修复**：`:aria-selected="selected"` + `role="option"`

### HIG-10: CompactDropdown 键盘焦点从不移动到选项
**文件**：`CompactDropdown.vue:42-62` | **修复**：`aria-activedescendant` 或主动聚焦选项

### HIG-11: 触摸设备上基于悬浮的卡片操作完全不可用
**文件**：`WallpaperCard.vue:121, 148-183` | **修复**：`@touchstart` 长按 + always-visible more 按钮

### HIG-12: TagCombobox 缺少 aria-controls
**文件**：`TagCombobox.vue:108-110` | **修复**：添加 `:aria-controls` + 对应 `id`

### HIG-13: generate_resized_jpeg 全尺寸解码后降采样
**文件**：`thumbnails.rs:168` | **修复**：使用 `image::io::Reader` 解码时设置 resize

### HIG-14: 数据库损坏时无恢复路径
**文件**：`db.rs:73-74` | **修复**：`PRAGMA integrity_check` + WAL 恢复 + 关闭时备份

---

## 三、🟡 Medium (22)

| ID | 问题 | 文件 | 修复方向 |
|----|------|------|----------|
| MED-01 | thumbnail_job_tx 被不必要 Mutex 包裹 | main.rs:31 | 直接存储 Sender |
| MED-02 | idx_wallpapers_blacklisted 索引创建两次 | db.rs:156,172 | 从 migrate() 移除 |
| MED-03 | wallpapers.created_at 缺少索引 | db.rs:306,326 | 添加索引 |
| MED-04 | 壁纸列表总是两次 SQL | db.rs:369-375 | LEFT JOIN 聚合 |
| MED-05 | VACUUM 每次启动都执行 | db.rs:136-148 | 加 7 天间隔 |
| MED-06 | 迁移重建表无事务 | db.rs:214-257 | 包裹 BEGIN/COMMIT |
| MED-07 | 扫描和文件监听器竞态 | main.rs:294-317 | 先 watcher 后 scan |
| MED-08 | CLI 日志文件无限增长 | diagnostics.rs:17-34 | >1MB 截断 |
| MED-09 | app_data_dir() 三处各自定义 | 三个文件 | 提取 paths.rs |
| MED-10 | useTheme 监听器永不清理 | useTheme.ts:68-85 | 导出 cleanupTheme() |
| MED-11 | patch 后滚动位置不必要重置 | WallpaperGrid.vue:77 | 移除 length 依赖 |
| MED-12 | 冷启动 Inspector 自动选中壁纸 | wallpapers.ts:189 | 返回 null |
| MED-13 | 图库空状态仅纯文本 | WallpaperGrid.vue:195 | 加图标+按钮 |
| MED-14 | 每次滚动触发缩略图加载 | WallpaperGrid.vue:91 | 增量加载 |
| MED-15 | 预览生成无并发排重 | thumbnails.rs | 添加 in_flight |
| MED-16 | 前端零测试 | src/ | store+composable 测试 |
| MED-17 | Release 缺少 LTO | Cargo.toml | lto+strip |
| MED-18 | CI 缺少 cargo audit | ci.yml | 添加 audit 步骤 |
| MED-19 | 加权随机大量重复候选 | db.rs:595-603 | Rust 端采样 |
| MED-20 | rotation 计时器每次超时查 DB | main.rs:1231 | 直读 AtomicBool |
| MED-21 | CLI 日志查看器未声明 CSS 变量 | styles.css:2142 | --bg-elevated→--bg-panel-elevated |
| MED-22 | Gallery 窄屏选择栏溢出 | styles.css:3258 | 折叠到 More |

---

## 四、🟢 Low (13)

| ID | 问题 | 文件 |
|----|------|------|
| LOW-01 | shell_metadata 固定 1024 缓冲 | shell_metadata.rs:33 |
| LOW-02 | rotate_interval 无最大上限 | main.rs:1110 |
| LOW-03 | CurrentWallpaperPanel 硬编码 "Main Display" | CurrentWallpaperPanel.vue:58 |
| LOW-04 | StatusBar 自动启开关只读装饰 | StatusBar.vue |
| LOW-05 | reduced-motion 媒体查询缺失 | styles.css |
| LOW-06 | 空 catch 块是死代码 | EmptyState.vue, TitleBar.vue |
| LOW-07 | Sidebar "System" 是 div 非标题 | Sidebar.vue:134 |
| LOW-08 | IconButton 未声明 emits | IconButton.vue:1-32 |
| LOW-09 | windows crate 0.60 → 0.61 | Cargo.toml:25 |
| LOW-10 | CI 不构建 release | ci.yml |
| LOW-11 | 无 CODE_OF_CONDUCT.md | 根目录 |
| LOW-12 | parse_bool_setting "paused" 无注释 | main.rs:564 |
| LOW-13 | get_next_wallpapers(0) 返回 1 | db.rs:588 |

---

## 五、优先修复方案 (Top 10)

| 排名 | ID | 问题 | 预估时间 |
|------|-----|------|----------|
| 1 | CRT-03 | 通知面板不可见 | 10 min |
| 2 | CRT-02 | 批量删除确认对话框 | 30 min |
| 3 | CRT-01 | 哈希函数不稳定 | 1 hour |
| 4 | HIG-01 | 单击 180ms 延迟 | 10 min |
| 5 | HIG-06 | 路径未规范化 | 30 min |
| 6 | HIG-04 | 批量操作无事务 | 1 hour |
| 7 | HIG-03 | 导入失败静默丢弃 | 20 min |
| 8 | HIG-09 | aria-selected 错误 | 5 min |
| 9 | HIG-07 | WAL 无 checkpoint | 15 min |
| 10 | MED-01 | thumbnail_job_tx Mutex | 15 min |

---

## 六、正面评价

| 领域 | 亮点 |
|------|------|
| **CSP** | `script-src 'self'` 严格，无 `unsafe-eval`，`connect-src` 锁定 `ipc:` |
| **注册表安全** | 所有操作限定 PureWall 自有 HKCU 键，有测试验证白名单 |
| **路径安全** | UNC 拒绝、网络驱动检测、符号链接跳过、扫描深度/数量上限 |
| **文件删除** | 进回收站、7 秒撤销窗口 |
| **线程安全** | Condvar 替代轮询、ComGuard RAII、STA worker 单例 |
| **虚拟滚动** | useVirtualList + 行虚拟化 + 5 行 overscan |
| **开源文档** | README 安全承诺、CONTRIBUTING 安全检查、SECURITY.md 完善 |
| **错误模式** | CommandError + CommandResult 统一边界 |
| **Asset 协议** | Scope 精确到 `$APPDATA/thumbnails/**`、零 Base64 残留 |

---

*报告结束。52 项发现已按优先级分类。建议按 Top 10 顺序依次修复。*
