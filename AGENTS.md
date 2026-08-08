# AGENTS.md — PureWall 项目记忆（每次会话自动加载）

## 项目信息
- **项目名**: PureWall（桌面壁纸美学管家）
- **仓库**: D:\Desktop\PureWall
- **技术栈**: Tauri 2 + Vue 3 + TypeScript + Tailwind CSS + SQLite
- **目标平台**: Windows

## 文档体系 (docs/project-docs/)

每次会话必须按 prompt 链读取/维护以下文件：

| 文件 | 用途 | 何时读 | 何时写 |
|------|------|--------|--------|
| ARCHITECTURE.md | 系统骨架 | onboard/dev | 架构变更时 |
| FEATURE_PLAYBOOK.md | 开发剧本 | dev 前 | 流程优化时 |
| DECISIONS.md | 架构决策 ADR | dev 前 | 涉及架构变更时 |
| AI_DIARY.md | 踩坑记忆体 | dev 前 | 每次遇到坑时 append |
| CHANGELOG_AI.md | 代码变更日志 | dev 前 | 每次完成改动时 |

## Prompt 链工作流

```
onboard → 01-onboard.md + 02-architecture.md
dev     → 05-feature-dev.md + 08-changelog.md + AI_DIARY.md
debug   → 09-debug.md + AI_DIARY.md
review  → 06-review.md
docs    → 02/03/04/08/AI_DIARY.md
resume  → 07-resume.md (会话恢复)
```

## 关键约束
1. 外科手术式修改 — 最小改动，不碰无关代码
2. Verify 驱动 — 每步 cargo check + vue-tsc
3. 决策门禁 — 架构变更必须先有 accepted ADR
4. Append-Only — AI_DIARY.md 只增不删
5. **注册表操作必须维护文档** — 修改注册表后同步更新 AI_DIARY.md #registry 条目
6. **禁止修改系统设置** — 任何非 PureWall 自身注册表条目的系统级修改（如禁用 Win11 新菜单、修改 HKLM、修改系统策略等），**一律禁止自动执行**。必须先向用户说明风险并获得明确确认后才能操作。PureWall 只允许操作自己创建的注册表条目（`PWNext`/`PWLike`/`PWDislike`/`PWPause`/`PureWall`）。
7. **每次改动必须更新文档** — 代码变更后必须同步更新 `CHANGELOG_AI.md`（记录改动范围和验证结果），如有新坑必须 append 到 `AI_DIARY.md`。禁止改完代码不更新文档。此规则对所有任务生效，无例外。

## 每次任务 Pre-Flight Checklist（严格执行）

会话中每次开始新任务前，必须按序执行以下步骤：

```
1. 读取 AI_DIARY.md 全文        → 确认历史踩坑，避免重复
2. 读取 CHANGELOG_AI.md 最近记录 → 确认当前进度和上次改动
3. 运行 cargo check             → verify: 零 error
4. 运行 vue-tsc --noEmit        → verify: 零 error
5. 确认 DECISIONS.md 无相关 proposed ADR 阻塞
```

- 以上步骤必须在动手写代码之前完成
- 跳过任何一步导致重复踩坑，记录到 AI_DIARY.md
- 任务完成后必须更新 CHANGELOG_AI.md + AI_DIARY.md（如有新坑）

## 每次任务 Post-Flight Checklist（严格执行）

每次任务完成后（或用户说"今天到这"），必须按序执行：

```
1. 更新 CHANGELOG_AI.md   → 记录变更目标、代码范围、验证证据
2. 更新 AI_DIARY.md       → append 新踩坑（如有）
3. 未解决问题标记到 CHANGELOG_AI.md "未解决项"
```
- 任务完成后必须更新 CHANGELOG_AI.md + AI_DIARY.md（如有新坑）

---

## 维护手册：Windows 注册表操作

### 右键菜单（扁平条目结构）

**注册表路径**：`HKCU\Software\Classes\Directory\Background\shell\`

**结构**（4 个独立条目）：
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

**Win11 新菜单禁用**：`CLSID\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}\InprocServer32` → `(default) = ""`

**注册方式**：Rust 写 `.ps1` 脚本 → `powershell -ExecutionPolicy Bypass -File` 执行
**关键踩坑**：
- PowerShell 变量拼接必须用 `'"' + $exe + '" --action xxx'`
- 子菜单需要 COM DLL，纯注册表不可行（见 AI_DIARY.md #registry-001）
- 注册前清理旧条目
**代码位置**：`src-tauri/src/context_menu.rs`

### 开机自启

**注册表路径**：`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
**键名**：`PureWall`
**键值**：`"<exe_path>"`
**代码位置**：`src-tauri/src/autostart.rs`

### 数据存储

**路径**：`APPDATA/com.purewall.app/`
**文件**：
- `purewall.db` — SQLite 元数据库
- `current_wallpaper.txt` — 当前壁纸路径（CLI action 用）
- `register_menu.ps1` — 右键菜单注册脚本
**注意**：CLI action 和 Tauri app 必须用同一路径，用 `app_data_dir()` helper

---

## 启动方式
```bash
cd D:\Desktop\PureWall
npm run tauri dev
```
