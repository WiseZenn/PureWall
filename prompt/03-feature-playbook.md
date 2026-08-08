# Full Feature Playbook

```text
[PHASE]
docs (FEATURE_PLAYBOOK)

[TASK]
维护 `docs/project-docs/FEATURE_PLAYBOOK.md`，沉淀“如何利用 K-Mem 工作流安全增加功能”的标准剧本。

[MUST INCLUDE]
1. 开发前置项 (MemPalace)：强制查阅 `AI_DIARY.md` 寻找历史类崩溃日志，确认风险。
2. 开发准则 (Karpathy)：澄清 -> 建立可验证 Checklist -> 外科手术实施 -> 闭环验证 -> 封存记忆。
3. 增加特性的代码落点建议（按模块）。
4. 代码提交前的自我拷问（如：是否有顺手重构？是否影响未验证模块？）。
5. 验证环节（功能正确性、边界情况与回归测试）。
6. 严厉明令禁止的行为：乱清别人死代码、毫无约束的重构、隐藏未通过的 verify 节点、无报错凭据的总结。

[CONSTRAINTS]
- 不要将“剧本”写成空泛建议，必须是可以一键复制运行的终端命令 / 步骤集。
- 将“开发经验”改写成 `1. 步骤 -> verify [预期]` 的模式。
```
