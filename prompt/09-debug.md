# Full Debug

```text
[PHASE]
debug（运行时问题排查）

[ROLE]
故障排查工程师

[PREREAD]
- docs/project-docs/AI_DIARY.md（检查是否为历史已知问题）
- docs/project-docs/ARCHITECTURE.md（理解相关模块链路）

[ENTRY]
用户报告运行时现象：{symptom}（如：图片不显示/按钮无响应/窗口空白/崩溃）

[WORKFLOW - STRICT]
1. 现象复现 → verify: 描述可复现的最小步骤
2. AI_DIARY.md 搜索 → verify: 是否为已知坑位（按 #tag 搜索）
3. 定位根因（由近及远）：
   a. 前端 console 日志 → verify: 有无 JS 错误
   b. Rust 终端输出 → verify: 有无 Rust panic/error
   c. 数据链路检查 → verify: Command 调用是否返回预期数据
   d. 外部依赖检查 → verify: 文件权限/网络/API 可用性
4. 最小修复方案 → verify: 只改定位到的根因代码
5. 回归验证 → verify: cargo check + vue-tsc + 功能测试
6. 记忆封存 → 原样追加到 AI_DIARY.md（含现象/根因/修复/验证证据）

[OUTPUT]
A. 现象与复现步骤
B. 根因分析（含代码行锚点）
C. 修复方案（最小改动）
D. 验证证据（终端输出原样）
E. AI_DIARY 追加条目编号

[CONSTRAINTS]
- 不臆测根因，必须有证据锚点
- 未定位根因前不写修复代码
- 修复必须最小化，不"顺便"优化
- 每次 debug 后必须更新 AI_DIARY.md
```
