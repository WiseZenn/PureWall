# Full Decisions

```text
[PHASE]
docs (DECISIONS)

[TASK]
维护 `docs/project-docs/DECISIONS.md`，只收录和整体架构生死相关的顶层架构级决策。不写入代码零碎 Bug——它们属于 `AI_DIARY.md` (MemPalace 原则)。

[ENTRY TEMPLATE]
0. Status (`proposed` / `accepted` / `deprecated`)
1. Decision
2. Context
3. Options
4. Why this option
5. Consequences
6. Logging Rule (该决策带来的后续踩坑细节，移步到 AI_DIARY.md)

[CONSTRAINTS]
- 条目必须短而硬，杜绝长篇散文和无用的过度抽象总结。
- 若无新决策，明确写“本次无新增关键决策”。
- 所有新决策默认 `proposed`，只有用户明确确认后才能改为 `accepted`。
- 未 `accepted` 的 ADR 不得驱动代码实施或 CHANGELOG 中“已完成”描述。
```
