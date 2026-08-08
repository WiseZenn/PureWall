# Full Resume

```text
[PHASE]
resume

[PREREAD]
- docs/project-docs/ARCHITECTURE.md
- docs/project-docs/FEATURE_PLAYBOOK.md
- docs/project-docs/DECISIONS.md
- docs/project-docs/AI_DIARY.md
- docs/project-docs/CHANGELOG_AI.md

[RULE]
1. 记忆封存 (MemPalace Save)：提取当前会话中的关键新知识、Bug起因、环境变更，并附带时间戳原生追加（Append-Only）到 `AI_DIARY.md` 和 `CHANGELOG_AI.md`，彻底杜绝总结和删减历史知识。
2. 禁止先全盘扫描代码库。基于文档建立上下文后，只做任务相关定向读取。

[OUTPUT]
1. 成功追加到 AI_DIARY 的日志与坑位摘要
2. 历史变更对当前任务的影响与当前骨架摘要
3. 最小实现路径
4. 待确认决策点或 Next Steps
```
