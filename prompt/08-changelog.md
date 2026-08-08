# Full Changelog

```text
[PHASE]
docs (CHANGELOG_AI)

[TASK]
基于本次改动更新 `docs/project-docs/CHANGELOG_AI.md`，并在遇到关键卡点时同步追加写入 `docs/project-docs/AI_DIARY.md`。

[PRE-FLIGHT]
1. 读取 AI_DIARY.md 最新 5 条，确认本次改动不会重复历史踩坑
2. 读取 CHANGELOG_AI.md 最近记录，确认变更连续性

[MUST INCLUDE]
1. 变更目标与真实代码行范围 (Surgical 范围)
2. 架构与依赖的影响范围
3. 关键实现点 (拒绝废话和过度总结)
4. 踩坑记录与原样 Log (MemPalace 原则，录入 AI_DIARY)
5. 验证证据：测试步骤、命令、终端成功/失败的原样结果
6. 未解决项、风险与下一步

[CONSTRAINTS]
- 坚辞隐藏失败的验证环节。
- 绝不把明天的计划写成已完成的代码。
- PARTIAL 状态必须说明缺失了哪种环境和具体的补测建议。
- CHANGELOG 仅记录已执行改动；提案阶段内容必须留在 DECISIONS（Status=proposed）。
- AI_DIARY 追加必须包含 #tag 编号（如 #thumbnail-002），方便后续搜索。
```
