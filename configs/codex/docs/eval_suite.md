# Eval Suite

本页描述 harness 行为评估面；机器可读 fixture 见
`configs/framework/HARNESS_BEHAVIORAL_EVAL_CASES.json`，失败分类见
`configs/framework/HARNESS_FAILURE_TAXONOMY.json`。评估只消费现有
`router-rs`、`TRACE_EVENTS.jsonl`、`EVIDENCE_INDEX.json`、`STEP_LEDGER.jsonl`
和 closeout，不引入第二套运行框架。

## Required Eval Tracks

### routing_accuracy

- Question: task 是否命中正确 owner / gate / overlay，且 route_context 与预期一致。
- Fixture: `tests/routing_eval_cases.json`
- Verify: `router-rs eval route --cases tests/routing_eval_cases.json`
- Failure class: `route_miss` / `owner_drift`

### token_efficiency

- Question: 新 runtime 能力是否减少上下文噪声，成功输出是否静默，长账本是否只以摘要进入提示。
- Fixture: Codex/Cursor compact context tests；PostTool evidence heuristics。
- Verify: `cargo test --manifest-path scripts/router-rs/Cargo.toml codex_compact_contexts`
- Failure class: `context_rot` / `tool_contract_bad`

### long_task_continuity

- Question: 长任务是否能从 `SESSION_SUMMARY.md`、`EVIDENCE_INDEX.json`、
  `TRACE_METADATA.json`、`STEP_LEDGER.jsonl` 的摘要恢复，而不是重读整段聊天。
- Fixture: `router-rs framework step-ledger` append/summary。
- Verify: `cargo test --manifest-path scripts/router-rs/Cargo.toml step_ledger`
- Failure class: `step_recovery_gap`

### trajectory_health

- Question: harness 关键动作是否能用 `TRACE_EVENTS.jsonl` 复盘 owner、phase、lane/tool、
  status、failure_class 与 evidence_ref。
- Fixture: `trace_runtime record-event`。
- Verify: `cargo test --manifest-path scripts/router-rs/Cargo.toml trace`
- Failure class: `trace_gap`

### closeout_integrity

- Question: 完成/通过声明是否必须有 verifier、artifact 或 `EVIDENCE_INDEX` 成功行支撑。
- Fixture: closeout CLI policy tests。
- Verify: `cargo test --test policy_contracts closeout`
- Failure class: `verification_missing`

### skill_contract_quality

- Question: 高影响 skill/tool 是否具备清晰触发、负边界、验证面与失败语义。
- Fixture: `router-rs eval skill-contract-lint --input-json '{"skills_root":"skills"}'`
- Verify: `cargo test --test policy_contracts harness_skill_contract_lint_cli_reports_protocol_shape`
- Failure class: `tool_contract_bad` / `route_miss`

### subagent_lane_integrity

- Question: sidecar lane 是否有 bounded scope、forbidden scope、verification_required、
  final_digest/evidence_ref 位置，并且父线程只消费 digest/ref。
- Fixture: `session_supervisor` lane contract tests。
- Verify: `cargo test --manifest-path scripts/router-rs/Cargo.toml session_supervisor`
- Failure class: `subagent_misuse`

## Initial Success Criteria

- Routing eval 无 owner/overlay 回归。
- `TRACE_EVENTS` / `STEP_LEDGER` 只落盘或摘要投影，不进入 SessionStart 长提示。
- Closeout 仍以 `EVIDENCE_INDEX`、命令、artifact 或 blocker/risk 为准。
- Skill contract lint 输出 `findings`、`execution_items`、`verification_results` 三段，沿用共享协议。
