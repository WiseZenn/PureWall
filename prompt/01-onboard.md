# Full Onboard

```text
[PHASE]
onboard

[ROLE]
架构建模助手

[INPUT]
- 仓库根目录
- 主要技术栈（可空）
- 业务目标（可空）

[PRE-FLIGHT - 环境检查]
在正式 onboard 前，验证开发环境就绪：
1. Rust: `rustc --version` + `cargo --version`（未安装则运行 rustup）
2. MSVC: 检查 cl.exe 是否存在（Tauri Windows 必需）
3. Node.js: `node --version` + `npm --version`
4. Git: `git --version`
记录环境状态到 AI_DIARY.md（如安装了新工具、版本号等）

[TASK]
1. 记忆唤醒 (MemPalace Wake-up)：强制优先读取 `docs/project-docs/AI_DIARY.md` 和 `docs/project-docs/ARCHITECTURE.md`，获取真实踩坑记录与架构约束。禁止未读记忆就直接扫描代码。
2. 全量扫描：目录结构、入口、主链路、依赖边界、部署路径。
3. 构建系统地图：模块职责、输入输出、耦合关系、共享基础设施。
4. 风险分层：单点故障、隐性耦合、低可测区域、高变更成本区域。
5. 初始化或更新文档：保证 `docs/project-docs/ARCHITECTURE.md`, `docs/project-docs/FEATURE_PLAYBOOK.md`, `docs/project-docs/DECISIONS.md`, `docs/project-docs/AI_DIARY.md`, `docs/project-docs/CHANGELOG_AI.md` 存在。

[OUTPUT]
A. 记忆恢复摘要（来自 AI_DIARY 的核心上下文）
B. 扫描覆盖面（看了什么）
C. 系统骨架结论（10-20 条）
D. 风险清单（Critical/Major/Minor）
E. 待确认项（必须给获取方式，禁止臆测）

[STOP]
- 五大核心记忆文档 (ARCHITECTURE, PLAYBOOK, DECISIONS, AI_DIARY, CHANGELOG_AI) 已确认创建或读取完毕。
- 待排查项已明确挂起，不再做无脑瞎猜。
```
