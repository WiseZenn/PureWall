# FEATURE_PLAYBOOK.md

> "如何安全增加功能"的标准剧本

## 开发前置项 (MemPalace)

1. 启动开发前**必须**读取 `docs/project-docs/AI_DIARY.md` 最新条目，确认历史踩坑
2. 检查 `docs/project-docs/DECISIONS.md` 是否有 `proposed` 状态的 ADR 与当前任务相关
3. 若影响架构/目录/入口，必须先形成 ADR 并等用户确认 `accepted`

## 开发准则 (Karpathy)

### 1. 澄清需求 → verify: 需求明确无歧义
- 确认功能边界（做什么、不做什么）
- 确认输入输出（Rust Command 参数/返回值、前端 UI 变化）

### 2. 建立 Checklist → verify: 每步有可验证预期
```
1. [步骤] → 执行: [命令] → verify: [预期结果(PASS)]
2. [步骤] → 执行: [命令] → verify: [预期结果(PASS)]
...
```

### 3. 外科手术实施 → verify: 最小改动，不碰无关代码
- Rust 新功能：新建 module → main.rs 注册 Command → Cargo.toml 加依赖
- 前端新功能：新建 component → store 加方法 → App.vue 引入
- 禁止"顺手"重构已有功能

### 4. 闭环验证 → verify: 每个 Checklist 项 PASS
- `cargo check` — Rust 编译
- `vue-tsc --noEmit` — TypeScript 检查
- `npm run tauri dev` — 功能验证

### 5. 封存记忆
- 改动记入 `docs/project-docs/CHANGELOG_AI.md`
- 新坑/踩坑原样追加到 `docs/project-docs/AI_DIARY.md`

## 增加特性的代码落点

| 类型 | 文件 | 模式 |
|------|------|------|
| 新 Rust Command | `src-tauri/src/main.rs` | 添加 `#[tauri::command]` + 注册到 `generate_handler!` |
| 新 Rust 模块 | `src-tauri/src/xxx.rs` | 在 `main.rs` 添加 `mod xxx;` |
| 新前端组件 | `src/components/Xxx.vue` | `<script setup>` + Tailwind + CSS 变量 |
| 新 store 方法 | `src/stores/wallpapers.ts` | 在 store 内添加 async function，return 中暴露 |
| 新 UI 入口 | `src/components/Sidebar.vue` | filter 数组 / 按钮区域 |

## 提交前自拷问

- [ ] 是否有"顺手"重构了未损坏的代码？
- [ ] 是否影响了未验证的模块？
- [ ] cargo check 是否零 error？
- [ ] vue-tsc --noEmit 是否零 error？
- [ ] 功能是否在 dev 模式下实际验证过？

## 禁止行为

- 乱清理他人标记为保留的代码
- 无约束的跨模块重构
- 隐藏未通过的 verify 节点
- 无终端证据的"已验证"总结
- 未 `accepted` 的 ADR 驱动代码实施
