# 深度 review 试跑：`closeout_enforcement` + `rfv_loop` 切片

**范围**：[`scripts/router-rs/src/closeout_enforcement.rs`](../../scripts/router-rs/src/closeout_enforcement.rs)（L1–L120 及 schema 叙事）、[`scripts/router-rs/src/rfv_loop.rs`](../../scripts/router-rs/src/rfv_loop.rs)（验证枚举与 external research strict 入口）。**姿态**：hostile-but-fair，review-only。

## verdict

`ship with caveats`：契约层设计清晰（`deny_unknown_fields`、显式枚举），但缺少针对「恶意/畸形 JSON」与跨字段一致性的专项测试时，仍存在运维误用与回归风险。

## P0–P2（带位置）

- **P0**（correctness）：`evaluate_closeout_record` 依赖 `CloseoutRecord` 反序列化失败时直接返回 Err（见 `evaluate_closeout_record_value` L107–L111）；调用方若未区分「schema 拒识」与「IO/序列化」错误，可能统一重试导致放大负载——建议在响应层文档化错误分类或在测试中固定错误串契约（锚点：`evaluate_closeout_record_value`, `evaluate_closeout_record`）。
- **P1**（correctness）：`ALLOWED_VERIFY_RESULTS` 将 `verify_result` 限制为 PASS/FAIL/SKIPPED/UNKNOWN（`rfv_loop.rs` L22–L24）；若宿主未来引入新状态字符串，**旧二进制**会硬拒——需 semver/迁移说明与兼容测试（锚点：`ALLOWED_VERIFY_RESULTS`）。
- **P2**（security / abuse）：`COMPLETION_KEYWORDS` 与中文完成词用于完成态检测（`closeout_enforcement.rs` L9–L18）；若上游把用户可控长文本直接匹配关键词，存在误判「已完成」风险——应确认仅作用于受信任 closeout 载荷字段而非任意用户输入（锚点：`COMPLETION_KEYWORDS`）。

## test_repro_gap

最小缺口：**构造一份缺字段/未知字段的 closeout JSON** 与 **一份 `verify_result` 为小写 `pass` 的 RFV round**，断言分别得到「parse failed」与「append_round 拒绝」的稳定错误信息；当前 review 未在本地执行测试，需在 `cargo test --manifest-path scripts/router-rs/Cargo.toml` 中确认是否已有等价用例，若无则补集成测试名可检索 `closeout_record` / `ALLOWED_VERIFY_RESULTS`。

---

## Lane_correctness

- `CloseoutRecord` 使用 `#[serde(deny_unknown_fields)]`（L58–L59）与 schema 版本常量（L4），有利于防止静默字段漂移，与 `CLOSEOUT_RECORD_SCHEMA.json` 锁步的注释（L53–L57）一致。
- `rfv_loop` 对 `external_research` strict 路径有长度与可溯源启发式（L71–L95、`EXTERNAL_RESEARCH_STRICT_TRACE_MIN_LEN`），降低「假深度」外研进入账本的概率。

## Lane_security

- closeout 记录含 `commands_run` 与 `stdout_summary`/`stderr_summary`（L26–L36）：若日志回灌到 UI，需防日志注入与秘密泄露（取决于谁写入 `CloseoutCommandRecord`）；建议在 human-facing 渲染路径做脱敏（超出本切片未读，标为 **caveat**）。
- `source_traceable_heuristic` 仅检查前缀形态（L47–L68）：不验证 URL 可达性或 TLS；strict 模式仍可能收录钓鱼域——依赖上游治理与 allowlist 时需在 ops 文档中写明。
