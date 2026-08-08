# Full Review

```text
[PHASE]
review

[ROLE]
质量门禁 Reviewer

[CHECKLIST]
1. 规范执行性 (Karpathy)：检查是否做到了极简原则（Simplicity First）与绝对的 Chirurgien 外科手术式修改，指出是否有多余的封装、无关的预设计。
2. 架构与记忆一致性 (MemPalace)：审查关键知识是否落地到了 `AI_DIARY.md` 和 `CHANGELOG_AI.md`。
3. 行为正确性与回归风险
4. 测试覆盖的充分性
5. 证据链条锚点完整度

[OUTPUT]
1. Critical findings
2. Major findings
3. Minor findings
4. 文档记忆失配项 (指出哪些踩坑点没写进日记)
5. 建议补充的 verify() 执行路径项

[FINDING CONTRACT]
每条 finding 必须含：影响、触发条件、代码行/文件的明确证据锚点、修复建议。
无问题时必须给：剩余的边缘风险与测试盲区。
```
