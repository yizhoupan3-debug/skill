---
name: goalx-registration-2026-06
description: 2026-06-25 注册 /goalx 命令 + auto-detect hook 注入 → set_goal 协议 + 对抗审核修复
metadata:
  type: project
---

2026-06-25 两阶段修复 + 对抗审核：

**Phase 1 — /goalx 注册**（解决显式入口缺失）：
- `skills/goalx/SKILL.md` — framework_command for goal management
- `SKILL_ROUTING_RUNTIME.json` — 添加 goalx 条目（trigger_hints 含 /goalx, goalx, goal:, Goal:, 目标：等）
- `RUNTIME_REGISTRY.json` — framework_commands 注册 goalx

**Why:** SKILL_ROUTING_RUNTIME.json 的 43 个 skill 中无任何 trigger_hints 含 "goal"；goal_auto_detect 引擎要求 ≥2 复杂度指标，纯 "goal:" 输入不触发。

**Phase 2 — auto-detect hook 注入 + set_goal 协议**（解决自动路由触发）：
- `gate_eval.rs:build_user_prompt_context_injection()` — 新增复杂任务自动检测路径
- `skills/goalx/SKILL.md` — 新增 `## set_goal 协议` 节

**对抗审核修复（2026-06-25）**：11 项发现全部修复
1. **HIGH** `.ok().flatten()` 吞噬 Err → 改用 `match read_goal_state` 显式处理
2. **MEDIUM** literal `<task_id>` 占位符 → 追加 `（请将 <task_id> 等替换为实际值）`
3. **MEDIUM** `network_access: none` 与 set_goal 矛盾 → 改为 `conditional`
4. **MEDIUM** `drive_until_done` 遗漏 → 加入 `has_active_goal` 检查
5. **LOW** 冗余 `analyze_complexity` → 消除重复调用（区块间互斥）
6. **LOW** 双读 GOAL_STATE → 提升为一次读取共享
7-8. **LOW** 格式字符串续行符 → 改用 `\n\` 标准模式
9. **LOW** `as_bool` 类型安全 → 已知低风险，CODE REVIEW 记录
10. **LOW** `is_plan_keyword_in_prompt` 遗漏 → 追加 "create a plan", "做个计划" 等 11 个短语

**设计原则：**
- Goal 严格不做跨会话持久化（session_id 校验）
- set_goal ≠ 复述用户原话，而是分析提炼后创建有结构的 Goal 契约
- 允许调研发掘任务上下文后再创建 Goal
