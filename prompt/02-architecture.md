# Full Architecture

```text
[PHASE]
docs (ARCHITECTURE)

[TASK]
维护 `docs/project-docs/ARCHITECTURE.md` 与架构有关的决策同步。目标是让新成员 10 分钟理解项目骨架。

[MUST INCLUDE]
1. 系统目标与边界
2. 目录与模块职责图（文字版）
3. 核心执行流程（输入 -> 处理 -> 输出）
4. 数据流与状态管理
5. 外部依赖与集成边界
6. 扩展点（新增功能应落在哪）
7. 架构级记忆关联 (指出具体的架构坑点已记录在 AI_DIARY.md 的哪一部分)
8. 当前隐患与重构建议

[CONSTRAINTS]
- 杜绝长篇大论的“历史流水账”，长篇排错日志请放入 `AI_DIARY.md` (MemPalace 原则)。
- 高层抽象优先，不做逐函数注释。
- 每条结论都要可映射到模块、文件或链路锚点。
```
