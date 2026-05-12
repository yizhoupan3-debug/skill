# Harness 解绑与共同沉降 — 执行验收清单

**目的**：在 [`host_adapter_contract.md`](../host_adapter_contract.md) 北星契约下，把 **portable core（L2/L3）** 与 **宿主薄适配（L4 + registry）** 的边界落成可勾选验收项；与减法全盘清单 [`harness_subtraction_first_principles_audit_checklist.md`](harness_subtraction_first_principles_audit_checklist.md)、合并门槛 [`EXECUTION_harness_pr_review_checklist.md`](EXECUTION_harness_pr_review_checklist.md) 互补。

**何时用**：接入新宿主、大改 hook/review/PostTool、或季度对齐「主产品 + 多宿主」边界时使用。

**Finding 模板**：`条目 id | 是/否/说不清 | 证据（路径或 grep）| P0/P1/P2`

---

## 1. 解绑门禁（布尔验收）

| # | 检查项 | Done when | Verify |
|---|--------|-----------|--------|
| 1.1 | L4（`hooks.json` / shell）只做 argv、stdin 透传、超时、路径转发 | 无业务分支复制 L3 门控或证据拼写规则 | Cursor/Codex：`rg -n "EVIDENCE_INDEX|reject_reason|review_gate" .cursor/hooks.json .codex/hooks.json` 命中行仅为调用 `router-rs`；Claude：`rg -n "router-rs" .claude/settings.json`（或已安装宿主实际路径）命中为 hook 命令转发，无自建门控脚本 |
| 1.2 | 热路由唯一真源 | 未向 [`skills/SKILL_ROUTING_RUNTIME.json`](../../skills/SKILL_ROUTING_RUNTIME.json) 塞 explain/plugin | 对照 PR 清单 #2 |
| 1.3 | 证据 / closeout 唯一_owner | 无与 L2 schema 平行的第二状态根 | `artifacts/current/` + `framework hook-evidence-append` 契约 |
| 1.4 | 新宿主接入顺序可追溯 | PR 顺序与 [`host_adapter_contract.md`](../host_adapter_contract.md) §3.1 一致 | `RUNTIME_REGISTRY` → `framework_host_targets` → `<host>_hooks` → dispatch → `host_integration` |

---

## 2. 沉降门禁（共用逻辑归宿）

| # | 检查项 | Done when | Verify |
|---|--------|-----------|--------|
| 2.1 | 纯函数门控 / 归一化优先 [`hook_common.rs`](../../scripts/router-rs/src/hook_common.rs) | 双宿主同类启发式不平行粘贴 | `diff`/审查：`codex_hooks` vs `cursor_hooks` 重复块 |
| 2.2 | PostTool → shell evidence 共用入口 | Cursor 异构 stdin 经中性归一化再进 [`try_append_post_tool_shell_evidence`](../../scripts/router-rs/src/framework_runtime/mod.rs) | `rg synthetic_post_tool_evidence_shape scripts/router-rs/src` |
| 2.3 | 宿主专有分支可解释 | 注释或本清单 **§4** 登记「仅 JSON 形状 / 仅 Cursor 字段」 | 代码审查 |
| 2.4 | 长文案不在 L3 const | operator nudge 等来自配置或 L5 | [`HARNESS_OPERATOR_NUDGES.json`](../../configs/framework/HARNESS_OPERATOR_NUDGES.json) |

---

## 3. 形状差异登记（不可盲目合并）

以下路径 **不构成** hook_common 重复缺陷：宿主 stdin / 出站字段不同，合并需共同抽象。

| 主题 | Cursor / Codex / Claude 差异锚点 | 备注 |
|------|----------------------------------|------|
| Review gate 清点 lane | Cursor/Codex：`harness_architecture.md` §5.0；Claude：`claude_reviewer_lane` | 勿把 Claude `reviewer` 字面套到 Cursor/Codex |
| PostTool 原生形状 | Cursor：`hook_posttool_normalize` + terminal 归属；Codex：直连 `try_append_post_tool_shell_evidence` | 见 [`host_adapter_contract.md`](../host_adapter_contract.md) §3.4 |
| `tool_input` 多键合并 | 顶层 map 上经 [`hook_common::tool_input_value_from_map`](../../scripts/router-rs/src/hook_common.rs) 合并；Cursor 仍对嵌套 `HOOK_EVENT_NESTED` 做二次扫描 | **键优先级冻结**与代码一致，见 [`host_adapter_contract.md`](../host_adapter_contract.md) §3.4 段末；非整段 stdin 形状统一 |
| Claude stdin 误接 Cursor | 顶层 `cursor_version` + `workspace_roots` 静默 | [`claude_hooks.rs`](../../scripts/router-rs/src/claude_hooks.rs) `payload_looks_like_cursor_hook_stdin` |

---

## 4. 已知耦合寄存器（grep 锚点）

宿主专有或 install 路径；新宿主接入时逐项对照。

| 能力 | 路径 / 符号 | Verify |
|------|-------------|--------|
| 共用工具输入键合并 | [`hook_common.rs`](../../scripts/router-rs/src/hook_common.rs) `tool_input_value_from_map` | Cursor 嵌套扫描仍在 `cursor_hooks` |
| Cursor review 路由信号（编译嵌入） | [`review_routing_signals.rs`](../../scripts/router-rs/src/review_routing_signals.rs) + `REVIEW_ROUTING_SIGNALS.json` | `rust_contracts.md`「重建 router-rs」说明 |
| 宿主投影适配表 | [`host_integration.rs`](../../scripts/router-rs/src/host_integration.rs) `HOST_PROJECTION_ADAPTERS` | `registry` + `framework install --to` |
| Cursor user MCP（browser-mcp） | [`host_integration.rs`](../../scripts/router-rs/src/host_integration.rs) `install_cursor_mcp_server` 等 | 仅 user scope 行为 |
| Session supervisor | [`session_supervisor.rs`](../../scripts/router-rs/src/session_supervisor.rs) Codex driver only | `driver_id_for_host` → `unknown_driver` |
| Maint 投影刷新 | [`framework_maint.rs`](../../scripts/router-rs/src/framework_maint.rs) `refresh_host_projections` | `installable` / `projection_status` |
| Profile bundle | [`framework_profile.rs`](../../scripts/router-rs/src/framework_profile.rs) `host_projections` + legacy `codex_profile` | 契约节 Host Projection |

**能力矩阵（权威表）**：[`host_adapter_contract.md`](../host_adapter_contract.md) §3.2（`framework_maint` / `session_supervisor` / `framework_profile` / `hook_posttool_normalize`）；闭集宿主 id 以 [`RUNTIME_REGISTRY.json`](../../configs/framework/RUNTIME_REGISTRY.json) `host_targets.supported` 为准。本清单 §4 为 grep 运维面；语义变更须先改 §3.2 再改此处。

---

## 5. 可选演进（非合并门槛）

- 多 crate workspace 独立「portable core」：[`host_adapter_contract.md`](../host_adapter_contract.md) §5。
- PostTool：**当前**已在 crate 级 [`hook_posttool_normalize.rs`](../../scripts/router-rs/src/hook_posttool_normalize.rs) 收敛「stdin → append 形状」（见 [`host_adapter_contract.md`](../host_adapter_contract.md) §3.4）。进一步演进是把 `tool_name_of` / `tool_input_of` / `extract_first_session_string` 等抽取 helper 迁出 [`cursor_hooks/`](../../scripts/router-rs/src/cursor_hooks/mod.rs)，打破 `hook_posttool_normalize` ↔ `cursor_hooks` 的 crate 内依赖环，便于日后拆仓；测试覆盖齐备后再做。

---

## 6. 验证命令速查

```bash
cargo test --manifest-path scripts/router-rs/Cargo.toml
cargo clippy --manifest-path scripts/router-rs/Cargo.toml -- -D warnings
```

若改动 `RUNTIME_REGISTRY` 或根包 policy：`cargo test`（仓库根）。

---

## 7. 计划 vs 实际（execution closeout）

| 计划项 | 实际 |
|--------|------|
| 新建解绑清单 | 已新增本文件；[`host_adapter_contract.md`](../host_adapter_contract.md) 顶部指针 + §1 Portable core / §3.2 / §3.4 同步 |
| PostTool crate 级模块 | [`hook_posttool_normalize.rs`](../../scripts/router-rs/src/hook_posttool_normalize.rs) 取代 `cursor_hook_posttool.rs` |
| hook_common 下沉 | 新增 `tool_input_value_from_map`；[`frag_02_gate_event.rs`](../../scripts/router-rs/src/cursor_hooks/frag_02_gate_event.rs)、[`claude_hooks.rs`](../../scripts/router-rs/src/claude_hooks.rs) 共用 |
| 能力矩阵 | 以 [`host_adapter_contract.md`](../host_adapter_contract.md) §3.2 为权威表；本清单 §4 为运维 grep 面 |
| 顺带修复 | `codex_hooks` 测试中 `CODEX_ADDITIONAL_CONTEXT_MAX_CHARS` → `codex_additional_context_max_chars()`；`read_codex_stdin_limited` UTF-8 错误映射跨平台一致 |
| Review 收口（本轮） | 清单 §5 / §1.1 与契约 §3.4 对齐；`hook_common` 注释澄清；收紧 stdin UTF-8 子串匹配；§3.4 冻结 `tool_input_value_from_map` 键序 |
