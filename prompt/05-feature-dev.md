# Full Feature Dev

```text
[PHASE]
dev

[ROLE]
实现工程师

[PREREAD]
- docs/project-docs/ARCHITECTURE.md
- docs/project-docs/FEATURE_PLAYBOOK.md
- docs/project-docs/DECISIONS.md
- docs/project-docs/AI_DIARY.md (必读以避免重复踩坑)
- docs/project-docs/CHANGELOG_AI.md 最新记录

[PRE-FLIGHT]
1. 运行 `cargo check` 确认 Rust 可编译 → verify: 零 error
2. 运行 `vue-tsc --noEmit` 确认 TypeScript 可编译 → verify: 零 error
3. 确认 AI_DIARY.md 中无与当前任务相关的未解决历史坑位
4. 若有未确认的 proposed ADR 与当前任务相关 → 先进入 docs(DECISIONS) 阶段

[WORKFLOW - STRICT]
0. 决策门禁检查：若任务影响架构/目录/入口，先检查 `DECISIONS.md` 是否已有 `accepted` ADR；若无，则先进入 docs(DECISIONS) 阶段并等待用户确认，禁止写代码。
1. 给出最小改动方案 (Think Before Coding)：梳理逻辑、接口影响、死角与回滚点。不胡乱重构旁边相关的未损坏代码。
2. CheckList (Goal-Driven)：将任务划分为 `1. [实施步骤] -> 执行: [测试/命令行] -> verify: [预期结果(PASS)]` 格式。未验证通过禁止进行下一步。
3. 实施改动 (Surgical Changes)：优先最小变更，每次修改精确到具体行。如果你造了孤儿变量，顺手清理。
4. 验证 (Self-Verify)：实施前置设定的 Checklist，汇报（PASS/FAIL/PARTIAL）。
5. 记忆存档：记录改动到 `CHANGELOG_AI.md`；若产生新知识/坑/Bug原因，需原样 (Verbatim) 追加到 `AI_DIARY.md`。

[ROLLBACK PLAN]
每次修改前声明：
- 如果此改动导致 X 现象，回滚方式是 Y
- 具体哪些文件需要 revert

[OUTPUT]
A. 实施方案与 Checklist (必须包含 verify 项)
B. 改动摘要 (含准确的行号和特征代码块)
C. 回滚计划
D. 风险评估
E. 验证结果（PASS/FAIL/PARTIAL 连带输出的终端证据）
F. 文档更新清单 (AI_DIARY.md / CHANGELOG_AI.md)
G. Review 关注点
```
