# PureWall Phase 3 本地图库管理设计

- 日期：2026-07-28
- 状态：对话与书面设计已确认；3A、3B、3C 已完成，3D 待执行
- 目标版本：PureWall Phase 3
- 实施分支：`codex/purewall-phase-3`
- 基线：`origin/main@199c7dc`

## 1. 背景与目标

PureWall 的核心仍是普通、可靠的本地壁纸管理体验：像音乐播放器一样随机播放壁纸，并允许用户执行喜欢、不喜欢、下一张和暂停。Phase 3 不引入推荐系统或 PureWall-X 能力，而是在现有播放闭环之上补齐本地图库管理。

本阶段解决三类问题：

1. 用户可以看见、维护和恢复已添加的图库来源。
2. 用户可以对多张壁纸执行完整且可预测的批量整理。
3. 用户可以导出、预览并合并导入 PureWall 元数据备份。

完成标准不是“界面上出现按钮”，而是来源、元数据和运行时 watcher 在失败、离线、重定位、重启及部分恢复场景下仍保持一致。

## 2. 范围

### 2.1 本阶段包含

- `Settings → Library Sources` 图库来源管理。
- 来源列表、状态、重新扫描、重试、重定位和移除。
- 移除来源时由用户选择“保留元数据”或“清除专属元数据”。
- 批量喜欢、不喜欢、清除评分、隐藏、恢复、添加/移除标签、添加/移除合集。
- 保留现有回收站删除与撤销逻辑。
- 版本化 JSON 元数据备份导出、预览和合并导入。
- 来源离线后的原路径重试和新目录重定位。
- 对当前 GitHub Windows CI 路径别名测试缺陷的首要修复。
- 相应的数据库迁移、Tauri 命令、前端状态、测试、ADR 和项目过程文档。

### 2.2 明确不包含

- PureWall-X、推荐算法、云同步、账号、远程图库或壁纸下载。
- 复制、移动或删除用户的原始壁纸文件。
- 备份壁纸二进制文件、缩略图、预览图或其他派生缓存。
- 改动 Windows 系统设置、HKLM、系统策略、右键菜单模式或注册表。
- 自动修复依赖审计结果或进行无关依赖升级。
- 复杂批量规则编辑器、智能标签或多层来源分组。

## 3. 已确认的产品决策

### 3.1 移除来源

移除来源时显示明确二选一：

- **保留元数据**：停止 watcher 并移除来源记录；只将不再被其他来源覆盖的壁纸标记为不可用，保留评分、标签、合集和自定义标题。
- **清除专属元数据**：先显示影响数量，再删除只属于该来源且不被其他保留来源覆盖的壁纸元数据。

两种模式都不得删除、移动或回收原始图片。

### 3.2 备份导入冲突与文件安全

以规范化后的壁纸路径作为合并键：

- 同路径存在冲突时，备份中的评分、隐藏状态、自定义标题、标签关联和合集关联优先。
- “备份优先”表示 v1 中的完整字段值覆盖本地值：备份中的 `null` 可清空本地可空字段，标签/合集关联集合替换本地集合，而不是取并集。
- 本地存在但备份中没有的壁纸与元数据保留。
- 标签和合集按名称复用现有实体，避免重复定义。
- 导出命令本身强制 `.json`，并按统一路径身份拒绝覆盖已登记的壁纸文件；PureWall app-data 根目录及其子路径也不得作为导出目标。原生调用者不能绕过文件选择器 filter。
- 预览结果携带所读内容的 SHA-256 摘要；确认导入必须回传并在解析/事务前校验该摘要。文件在预览后发生任何变化都返回稳定错误并要求重新预览。
- 上述后端门禁与前端提示共同维护“元数据备份不修改原壁纸或应用数据”的安全承诺。

### 3.3 离线来源恢复

提供两个动作：

- **Retry**：重试原路径。
- **Relocate**：选择新目录，按相对路径迁移元数据，保留壁纸 ID，从而保留评分、标签、合集和自定义标题。

### 3.4 批量操作范围

- Like、Dislike、Clear rating。
- Hide、Restore。
- Add tag、Remove tag。
- Add collection、Remove collection。
- 现有 Recycle Bin delete + Undo。

### 3.5 UI 位置

图库来源的权威管理入口位于 `Settings → Library Sources`。Gallery 中已有的 Folder 按钮继续作为“添加图库来源”的快捷入口。

## 4. 总体架构

PureWall 继续保持单个 Tauri 应用，不拆分独立服务。Phase 3 在现有代码上建立清晰的“图库来源”边界：

- **Rust 数据库层**：拥有来源持久化、影响范围计算、来源删除/重定位、备份合并和批量事务。
- **Rust 应用协调层**：协调 SQLite、文件扫描和运行时 watcher。所有目录扫描都在数据库锁外执行。
- **Tauri 命令层**：提供稳定、窄职责、可返回结构化错误的命令。
- **Pinia store**：作为前端调用门面，统一 busy/error、刷新和选择状态。
- **Vue 界面**：Settings 展示来源生命周期；Gallery 承担选择与批量整理。

来源生命周期会改变数据库与运行时 watcher 的协调边界，因此生产代码实施前必须先新增并接受一条 ADR。ADR 至少要定义：

- 数据库提交与 watcher 启停/替换的顺序。
- watcher 创建失败时数据库状态与用户可见错误的处理。
- 任何会 `Drop` 并 join 线程的 watcher 句柄不得在持有 SQLite 锁时替换。
- 数据库事务只能保证元数据原子性，不能把文件系统扫描或 watcher 启停伪装成同一事务。

## 5. 数据模型

### 5.1 `watched_folders` 增量迁移

在现有 `watched_folders(path, source)` 基础上增量增加：

- `last_scan_at`：最近一次成功完成扫描的时间，可空。
- `last_error`：最近一次来源级错误摘要，可空。

迁移必须先增加列，再创建或使用依赖新列的语句/索引，遵守现有 SQLite 迁移约束。`online` 和 `scanning` 不持久化：

- `online` 由当前文件系统可达性推导。
- `scanning` 是进程内短期状态。

### 5.2 来源 DTO

前端来源行至少接收：

```ts
interface LibrarySource {
  path: string;
  source: string;
  status: "online" | "offline" | "scanning" | "error";
  availableCount: number;
  unavailableCount: number;
  lastScanAt: string | null;
  lastError: string | null;
}
```

数量按规范化路径的“相同或后代”关系计算。若来源重叠，展示数量可以反映该来源覆盖的记录；但清除操作的实际影响数量必须只计算不被其他保留来源覆盖的记录。

### 5.3 路径身份

所有新来源、重定位目标、备份路径和影响范围计算都使用统一 Windows 路径身份规则：

- 接受当前存在的磁盘路径后先验证并 canonicalize。
- 比较时统一分隔符、ASCII 大小写、尾部分隔符、`\\?\` 驱动器前缀和 `\\?\UNC\` 前缀。
- 相同/后代判断不得直接依赖原始字符串。
- 导出沿用数据库中已经持久化的 canonical 绝对路径；导入时，存在的路径重新 canonicalize，不存在的绝对路径只做确定性的 Windows 词法归一化并保持 unavailable，不能因为路径离线而拒绝元数据。
- 若一个当前存在路径与一个离线备份路径无法证明为同一身份，则不得猜测合并；保留两条记录并通过导入 warning 提示用户以后 Relocate。
- 测试夹具必须走与生产相同的 canonical root 边界，避免系统临时目录短名/长名别名。

## 6. 来源生命周期

### 6.1 列出来源

`list_library_sources` 从持久化来源读取记录，在数据库锁外检查目录可达性，并附加可用/不可用数量、扫描时间和错误。

读取来源不得隐式启动大扫描。启动恢复仍由应用生命周期负责；用户触发的重试和重新扫描走显式命令。

### 6.2 添加与重新扫描

Gallery Folder 快捷入口与 Settings 添加入口复用现有文件夹验证/导入路径：

1. 验证目录并得到 canonical root。
2. 在数据库锁外扫描。
3. 以 bounded batch 写入数据库。
4. 持久化来源并清除成功后的 `last_error`。
5. 在不持有 SQLite 锁时安装或保留 watcher。
6. 刷新来源状态和 Gallery 页面。

`rescan_library_source` 只扫描指定来源。失败保留来源记录，写入 `last_error`，并向 UI 返回可见错误；不得让全局 loading 无限持续。

### 6.3 移除并保留元数据

执行顺序由新 ADR 最终确认，但行为合同固定：

1. 计算被移除来源覆盖且不被其他来源覆盖的路径集合。
2. 移除来源持久化记录。
3. 将上述独占路径标记为 `file_available = 0`。
4. 在无 SQLite guard 时停止并移除运行时 watcher。
5. 刷新来源列表和当前 Gallery 页面。

评分、隐藏状态、自定义标题、标签、合集和播放历史不删除。

### 6.4 移除并清除专属元数据

确认对话框必须展示由后端计算的影响数量。确认后在一个 SQLite 事务中：

1. 再次计算独占于该来源的记录，防止确认后来源状态变化造成误删。
2. 删除这些壁纸记录及其依赖关联。
3. 删除来源记录。

被其他保留来源覆盖的记录不得删除。原图文件始终不修改。

### 6.5 Retry

Retry 使用原 canonical path：

- 路径仍不可达：保持来源和离线元数据，更新 `last_error`。
- 路径恢复：扫描、恢复匹配记录的可用性、补充新增文件、更新时间并恢复 watcher。

### 6.6 Relocate

Relocate 的目标是“同一图库搬到了新位置”，不是导入第二个无关来源：

1. 验证并 canonicalize 新根目录。
2. 拒绝与现有来源完全相同的目标。
3. 若目标与另一个保留来源形成相同、祖先或后代关系，导致来源归属不明确或重复 watcher，则中止并返回稳定错误。
4. 在数据库锁外扫描新目录。
5. 使用旧根目录下的相对路径与新根目录文件匹配。
6. 在写入前检查每个目标壁纸路径；若已属于另一个 wallpaper ID，则以 `SOURCE_RELOCATE_COLLISION` 中止，不静默合并。
7. 在单一 SQLite 事务中更新来源根路径和匹配壁纸路径；保持 wallpaper ID 不变。
8. 未匹配的旧壁纸保留元数据并保持不可用。
9. 提交后在无 SQLite guard 时用新 watcher 替换旧 watcher。

如果数据库事务失败，来源和壁纸路径全部回滚。如果提交后的 watcher 恢复失败，数据库迁移结果保留，来源显示 error/offline 并允许 Retry；不能把已提交的数据迁移谎报成完全失败。

## 7. 批量整理

### 7.1 后端命令

保留窄命令而不是一个包含任意操作的通用解释器：

```text
batch_set_rating(paths, 1 | -1 | 0)
batch_blacklist(paths, hidden)
batch_assign_tag(paths, tag_id)
batch_unassign_tag(paths, tag_id)
batch_assign_collection(paths, collection_id)
batch_unassign_collection(paths, collection_id)
batch_delete_wallpapers(paths)
undo_last_delete()
```

每个命令都必须：

- 去重路径。
- 限制最大批量大小。
- 验证参数和实体存在性。
- 在单一 SQLite 事务中完成对应元数据修改。
- 返回受影响数量和必要的刷新提示。

### 7.2 前端交互

- 选择状态沿用现有 Gallery 多选模式。
- 评分区增加 Clear rating。
- 标签与合集采用紧凑的 Add/Remove 菜单，不增加复杂批量编辑器。
- 操作成功后清空选择并刷新受影响页面/统计。
- 操作失败时保留选择，使用户可以重试。
- 删除继续使用现有回收站和 Undo 合同，不与“移除来源”混淆。

## 8. 备份与恢复

### 8.1 文件格式

导出文件采用明确版本，例如 `purewall-backup-v1.json`：

```json
{
  "app": "PureWall",
  "schemaVersion": 1,
  "exportedAt": "2026-07-28T00:00:00Z",
  "sources": [],
  "wallpapers": [],
  "tags": [],
  "collections": [],
  "settings": {}
}
```

包含：

- 用户选择的图库来源路径。
- 壁纸路径、评分、隐藏状态、自定义标题和恢复关联所需的稳定元数据。
- 标签定义和壁纸关联。
- 合集定义和壁纸关联。
- SQLite allowlist 设置：`rotationSecs`、`displayMode`、`focusModeEnabled`、`paused`。
- 客户端 allowlist 设置：`theme`（`system | light | dark`）与 `workspaceMode`（`workbench | quiet`）。

不包含：

- 原始图片、缩略图、预览图和派生缓存。
- 播放历史、临时统计或运行日志。
- 应用数据目录、可执行文件路径等内部机器路径；用户选择的图库路径属于恢复数据，例外保留。
- 注册表、自启、右键菜单或系统设置。
- 凭据、令牌或秘密。

### 8.2 导出

- 从数据库读取一致的元数据快照。
- 对输出大小设置合理上限。
- 写入同目录临时文件，flush 成功后原子替换最终文件。
- 失败不得留下看似成功的半文件；可清理确认属于本次操作的临时文件。

### 8.3 导入预览

导入分两步：

1. **Preview**：限制文件大小，验证 JSON、应用标识和支持的 schema 版本；返回来源、壁纸、标签、合集、新增、覆盖、缺失路径和警告摘要。
2. **Confirm import**：用户确认后执行合并。

不支持的版本返回稳定错误，不尝试猜测字段含义。

### 8.4 合并事务

数据库元数据合并在一个 SQLite 事务中完成：

- 标签/合集按名称复用，否则创建。
- 备份来源按路径身份新增或复用；本地独有来源保留。
- 存在的壁纸路径按 canonical identity 合并；离线路径按备份中已归一化的绝对身份合并。
- 同路径冲突时备份元数据优先；v1 的可空字段和标签/合集关联集合按备份完整值替换。
- 本地独有记录保留。
- 文件存在时检查并恢复可用性。
- 文件不存在时仍创建或更新不可用元数据行，以便来源恢复后重新激活。
- 只有备份中实际出现、类型合法且属于 allowlist 的设置键才覆盖本地值；缺失键和未列入 allowlist 的设置不变。

事务失败时全部回滚。事务提交后再恢复来源 watcher：

- 可达来源进入 online 并安装 watcher。
- 缺失来源保持 offline。
- 单个 watcher 恢复失败作为来源级 warning 返回，不回滚已经原子提交的元数据。

该“数据库原子提交 + 提交后 watcher 警告”边界必须在导入结果和 UI 文案中明确，避免把部分运行时恢复误报为数据库部分导入。

## 9. UI 设计

### 9.1 Library Sources 卡片

每个来源行展示：

- 完整路径或可展开路径。
- Online / Offline / Scanning / Error。
- 可用与不可用壁纸数量。
- 最近扫描时间。
- 最近错误摘要。
- Rescan 或 Retry。
- Relocate。
- Remove。

行级操作有独立 busy/error，不能锁死整个设置页。

### 9.2 移除确认

对话框展示：

- 保留元数据：说明记录会离线，可在以后重新添加覆盖目录时恢复。
- 清除专属元数据：显示影响数量。
- 固定安全提示：两种方式都不会删除原始图片。

### 9.3 备份入口

`Settings → Library Sources` 同一区域提供：

- Export Backup。
- Import Backup。
- 导入预览摘要。
- 明确确认按钮和不支持版本/路径缺失/来源恢复 warning。

## 10. 错误合同

继续使用现有 `CommandResult`/结构化结果和用户可见通知，不引入第二套全局错误系统。来源操作至少区分稳定错误码：

- `SOURCE_OFFLINE`
- `SOURCE_PATH_CONFLICT`
- `SOURCE_PERMISSION_DENIED`
- `SOURCE_INVALID_PATH`
- `SOURCE_RELOCATE_COLLISION`
- `BACKUP_TOO_LARGE`
- `BACKUP_INVALID_APP`
- `BACKUP_UNSUPPORTED_SCHEMA`
- `BACKUP_INVALID_DATA`
- `BACKUP_WRITE_FAILED`

错误信息必须可操作：说明用户可以 Retry、Relocate、选择其他文件或升级应用。日志保留技术上下文；UI 除用户已经选择的图库/备份路径外，不暴露应用内部敏感路径或实现细节。

## 11. 实施切片

### 3A — 来源生命周期（已完成）

1. 首先修复 GitHub Windows CI 的 canonical temp-root 测试缺陷。
2. 新增并接受来源生命周期 ADR。
3. 增量迁移来源扫描元数据。
4. 实现列表、重新扫描、移除两模式、Retry、Relocate。
5. 实现 Settings 来源卡片和确认流程。

### 3B — 批量闭环（已完成）

1. 补齐 Clear rating。
2. 新增 tag/collection unassign 命令。
3. 完成紧凑 Add/Remove UI、失败保留选择和成功刷新。
4. 保持现有回收站删除/撤销。

### 3C — 备份与恢复（已完成）

1. 定义版本化 schema 和上限。
2. 实现原子导出。
3. 实现校验与导入预览。
4. 实现数据库合并事务。
5. 实现提交后来源 watcher 恢复与 warning。
6. 实现 Settings 导出/预览/确认、客户端设置恢复与成功/警告并列反馈。

### 3D — QA、文档与评审（待执行）

1. 完整静态、单元和构建门禁。
2. Windows 真实目录生命周期手工 smoke。
3. 800×600、缩放和键盘可达性检查。
4. 更新架构、开发剧本、ADR、CHANGELOG 和 AI_DIARY。
5. 完成独立全分支 review，关闭 Critical/Important 后再合并。

每个切片形成可验证闭环，前一切片通过后再推进下一切片。

## 12. 测试策略

### 12.1 Rust

- CI 回归：临时来源根与保存路径走同一 canonical identity。
- 移除保留元数据。
- 移除清除专属元数据。
- 父/子重叠来源保护。
- Retry 恢复离线记录。
- Relocate 相对路径迁移并保持 ID。
- Relocate 目标冲突和事务回滚。
- Relocate 部分匹配，未匹配项保持 unavailable。
- 批量命令去重、上限、事务回滚和关联移除。
- 备份 schema 校验、过大文件拒绝。
- 备份合并冲突优先级、本地独有保留和幂等导入。
- watcher 恢复 warning 不破坏已提交元数据。

### 12.2 Frontend

- 来源状态和行级 busy/error。
- Remove 两模式、影响数量和无原图删除文案。
- Retry/Relocate/Rescan 命令路由。
- 批量成功清空选择，失败保留选择。
- tag/collection Add/Remove 菜单。
- 导入预览摘要、确认和不支持 schema。
- post-commit watcher warning 与数据库导入成功状态同时可见。

### 12.3 完整门禁

- `cargo fmt -- --check`
- `cargo check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `npx vue-tsc --noEmit`
- `npm run test:unit`
- `npm run build`
- `git diff --check`

### 12.4 手工 Windows QA

在用户批准的真实 Tauri 会话中验证：

- 添加临时真实目录并触发 watcher。
- 临时断开、Retry 和 Relocate。
- 两种 Remove 模式不删除原图。
- 导出、预览、合并导入。
- 800×600、常见缩放比例、键盘导航和焦点。

手工 QA 使用专用测试目录和测试数据库，不自动修改注册表、系统设置或用户真实图库。

## 13. 安全与不变量

- 原始壁纸文件只读；只有现有、明确的回收站删除操作可以在用户主动选择时处理图片。
- 备份导出在任何写入前拒绝非 JSON、已登记壁纸路径和 PureWall app-data 内路径；导入确认必须与同一次预览的 SHA-256 内容摘要一致。
- 来源移除、重定位、导入和重新扫描都不能静默删除原图。
- 不执行任何注册表命令，不改 PureWall 自有条目，也不改系统条目。
- 扫描在数据库锁外；数据库锁不得跨 watcher `Drop`/join 或文件系统 IO。
- 数据库事务失败不留下半迁移元数据。
- watcher 是运行时资源，其失败通过来源状态/warning 表达，不伪装成 SQLite 事务的一部分。
- 所有路径集合 bounded、去重并按统一 Windows 路径身份比较。
- PureWall-X 继续保持后续阶段，不进入 Phase 3 代码、UI 或文档合同。

## 14. 已知风险与处理

### Windows 路径别名

GitHub Windows runner 上，原测试直接使用 `std::env::temp_dir()` 根，而保存的图片路径经过 `canonicalize()`；短名/长名或扩展前缀差异使字符串归一化仍可能不等价。本机用例通过不能证明 CI 已修复。3A 的第一项必须让测试根走生产 canonical 边界，并在 GitHub Windows CI 复验。

### 跨资源“原子性”

SQLite、文件扫描和 watcher 无法构成真实分布式事务。本设计只承诺：

- 扫描结果在进入数据库事务前准备。
- 数据库元数据原子提交或回滚。
- watcher 在提交后协调；失败形成可重试的来源级状态。

### 重叠来源

展示覆盖数量与“清除专属元数据”的影响数量不是同一概念。后者必须在确认时和提交时都按其他保留来源重新计算，不能复用缓存的展示数字执行删除。

### 备份路径可移植性

备份保留用户选择的绝对图库路径，因此跨机器导入可能全部离线。该情况是预期行为：元数据仍导入，用户随后通过 Relocate 恢复，不自动猜测或扫描其他磁盘。

## 15. 验收标准

Phase 3 完成时应满足：

1. 用户能在 Settings 看见所有图库来源及其真实状态。
2. 离线来源可 Retry 或 Relocate，且相对路径匹配的元数据不丢失。
3. 移除来源必须选择保留或清除专属元数据，且原图不受影响。
4. 重叠来源不会因移除一个来源而错误删除仍被另一个来源覆盖的数据。
5. Gallery 的批量评分、隐藏、标签和合集均支持添加与撤销语义。
6. 备份可原子导出、预览并按既定冲突规则合并导入。
7. 缺失路径以 unavailable/offline 形式保留，可在未来恢复。
8. 当前 GitHub Windows CI 路径测试修复并通过。
9. 完整自动化门禁通过；手工 Windows QA 的未执行项不得冒充 PASS。
10. ADR、架构、变更日志和踩坑记录与代码同步。

## 16. Phase 3D 本地执行状态（2026-07-30）

当前结论：**Phase 3D / Phase 3 未完成**。

- **PASS — 本地自动化与发布构建**
  - Rust：`cargo fmt/check/test/clippy/release` 通过；121 passed / 0 failed / 1 ignored 手工性能基准。
  - 前端：`vue-tsc`、14 files / 49 tests、169-module production build 通过。
  - 依赖：完整与 production-only `npm audit` 均为 0 vulnerabilities。
  - 安装包：MSI/NSIS 均成功生成并记录 SHA-256。
- **PASS — 独立代码审查**
  - 初审的 1 个 Critical 与 1 个 Important 代码问题已按 RED/GREEN 修复。
  - 同一只读审查者复审后，代码层 Critical/Important 为 0；3 个 Minor 明确 deferred。
- **PASS（mocked shell）/ INCONCLUSIVE（视觉基线与真实 DPI）**
  - 空库 Settings、1200×800 / 800×600 / 375×667、明暗主题、Enter/Tab/Shift+Tab/Escape 行为通过。
  - 浏览器桥接为只读 Tauri mock；无 committed screenshot baseline，CSS zoom clipping 不能等同真实 Windows DPI。
- **BLOCKED / NOT RUN — 真实 Tauri 验收**
  - 进程级 `APPDATA` 未覆盖 Windows Known Folder/Tauri 数据路径；启动时出现真实库缩略图，进程在任何 UI 交互前停止。
  - 添加来源、watcher、Retry、Relocate、两种 Remove、备份往返、原生对话框键盘与源文件 hash 不变量均未运行。
- **NOT RUN — 远程与集成**
  - GitHub Windows CI 未因本地镜像通过而冒充 PASS。
  - 未合并、未推送、未创建 PR。

满足第 8、9 项中的远程 CI 与手工 Windows QA 要求之前，不得把 Phase 3 标为完成或发布就绪。
