---
parent: docs/spec.md
version: unified-v7
---

## 11. 安全守卫

### 11.1 web_fetch_guard.rs — SSRF 防护

**功能**：限制 web_fetch 仅访问公网，阻止 loopback/CGNAT/link-local/私有网段。

- `validate_web_fetch_url()` — URL 校验
- `validate_and_resolve_web_fetch_url()` — DNS 解析（防 TOCTOU rebinding）
- 阻止：`.localhost`、`.local`、`.internal`、loopback、CGNAT（100.64/10）

### 11.2 mcp_pre_guard.rs — MCP 前置守卫

**功能**：MCP `tools/call` 前置安全检查，panic 时降级为 block。

- 工具安全检查 + 受保护路径检测
- 依赖：`hook_common::path_guard`、`hook_policy::dangerous_mcp_tool_reason`

---

## 12. Closeout 与生命周期

### 12.1 Goal State

**真源**：`artifacts/current/<task_id>/GOAL_STATE.json`

操作：`start` · `checkpoint` · `pause` · `resume` · `complete` · `block` · `clear`

契约：`drive_until_done=true` 时强制 non_goals + done_when≥2 + validation_commands

### 12.2 RFV Loop

**真源**：`artifacts/current/<task_id>/RFV_LOOP_STATE.json`

操作：`start`（upsert）· `append_round`

关闭门控：`verify_pass` · `min_depth_score` · `external_research_strict`

### 12.3 closeout_enforcement.rs

**功能**：closeout 门控强制执行与管理。

- `evaluate_closeout_record_value()` — 评估 closeout 记录
- `summary_claims_completion()` — 摘要是否声称完成
- **interactive**: advisory；**非 interactive**: closeout fail-closed
- `closeout_gate` — 门控定义及拦截逻辑，用于验证是否满足 closeout 状态
- `closeout_record_write` — 写入 closeout 记录与断言结果

### 12.4 Goal readiness（原 ship_readiness.rs，已迁移）

**历史**：`ship_readiness.rs` 曾位于 runtime-core，提供 Goal/Stop followup 评估。已删除，逻辑迁移至 `host-projection/src/hooks.rs`。

**当前行为**：
- 生产环境下 `evaluate_goal_readiness_from_disk()` 返回 `GoalReadiness::default()`（全 false）
- 生产环境下 `goal_stop_followup_line()` 返回空字符串
- 仅 `#[cfg(test)]` 下通过 `register_ship_readiness()` 注入可测试实现

### 12.5 连续性锚点

| 锚点 | 路径 |
|------|------|
| EVIDENCE_INDEX | `artifacts/current/EVIDENCE_INDEX.json` |
| NEXT_ACTIONS | `artifacts/current/NEXT_ACTIONS.json` |
| SESSION_SUMMARY | `artifacts/current/SESSION_SUMMARY.md` |
| TRACE_METADATA | `artifacts/current/TRACE_METADATA.json` |
| GOAL_STATE | `artifacts/current/<task_id>/GOAL_STATE.json` |
| RFV_LOOP_STATE | `artifacts/current/<task_id>/RFV_LOOP_STATE.json` |
| TASK_LEDGER | `artifacts/current/<task_id>/TASK_LEDGER.jsonl` |
| STEP_LEDGER | `artifacts/current/<task_id>/STEP_LEDGER.jsonl` |

### 12.6 生命周期 Profile

真源：`configs/framework/RUNTIME_REGISTRY.json` → `lifecycle_profiles`

| Profile | REVIEW_GATE | AG_FOLLOWUP | closeout | loop_capable | 备注 |
|---------|:-----------:|:-----------:|:--------:|:------------:|------|
| interactive | suppressed | advisory | advisory | false | 默认轻量模式（原 my-light） |
| loop-auto | **mandatory** | advisory | **hard-block** | true | 自动循环模式，含 cost_budget + kill_switch |

---

