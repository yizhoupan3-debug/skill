---
last_verified: "2026-06-02"
depends_on:
  - ../harness_architecture/index.md
  - ../host_adapter_contract.md
  - ../README.md
---

# Runtime Rust Contracts

## Purpose

This document freezes the Rust-owned runtime contracts for this repository.
Historical migration notes live in git history and `MIGRATION.md`; this file describes only
the current runtime truth in `router-rs` and related Rust tools.

Upper-layer control-plane narrative (L1–L5, evidence/resume injection boundaries):
[`harness_architecture/index.md`](../harness_architecture/index.md). Host adapter portability (portable core, per-host Hook event matrices in `docs/hosts/`, new-host checklist):
[`host_adapter_contract.md`](../host_adapter_contract.md). Steady-state doc index:
[`README.md`](../README.md) in this directory.

**Host-neutral entrypoint sync**：`router-rs framework sync-entrypoints --repo-root <repo>` 与 `router-rs codex sync --repo-root <repo>` 调用同一 `sync_host_entrypoints`（Codex provider）；优先在文档/操作序列中使用前者以减少「只有 Codex 才做 sync」的误导。

**Skill `platforms` 缺省**：`router-rs framework skills refresh` 从 `RUNTIME_REGISTRY.json` 读取 `host_targets.supported`；`SKILL.md` 未写 `platforms` 或写 `supported` / `all-hosts` 时，生成产物中的 `host_support.platforms` 展开为闭集全集（与 harness 默认路由对等策略一致）。

It is the contract source of truth for:

- routing and route diagnostics
- profile / explicit host projection compilation
- execution response shape
- runtime control-plane descriptors
- framework runtime snapshot / artifact continuity
- trace transport, checkpointing, compaction, observability, and sandbox policy

## 拆分导航

本文件已拆分为以下聚焦子文档：

| 主题 | 文件 |
|------|------|
| 概述 + 契约规则 + 当前边界（本文件） | [index.md](index.md)（当前） |
| 宿主投影不变量 | [01-host-projection.md](01-host-projection.md) |
| 路由契约 + 插件 ABI | [02-routing-and-plugin.md](02-routing-and-plugin.md) |
| 状态账本 + 可移植性 + 外部基准 | [03-status-and-portability.md](03-status-and-portability.md) |

## Harness architecture (control plane)

Upper-level layering for hooks, continuity artifacts, and evidence flows lives in [`harness_architecture/index.md`](../harness_architecture/index.md) (L1–L5 model, extension rules). **Closed-set host ids** and install/sync alignment with manifests: `configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported` (see [`host_adapter_contract.md`](../host_adapter_contract.md)). Operator nudge strings for RFV / goal-drive **stdio**（`framework_rfv_loop` / `framework_goal_drive`；原 autopilot 模块名）与 skill 对照 loaded from `configs/framework/HARNESS_OPERATOR_NUDGES.json` (`harness_operator_nudges`)；**2026-05 起** Stop/SessionStart **不**经 hook 注入续跑或 digest，长 math/retrieval/strict hints 留在 docs/schema。跨宿主 review-gate env（canonical **`ROUTER_RS_REVIEW_*`** + per-host legacy）真源 [`core-policy/env_flags.rs`](../../core/core-policy/src/env_flags.rs)（`router_rs_review_gate_disabled_for_host`、`router_rs_review_gate_stop_max_nudges_cap`、`router_rs_review_pending_cycle_max` 等）；仅 `1`/`true`/`yes`/`on` 关闭 gate disable 类开关。完整表见 [`harness_architecture/03-hook-and-switches.md`](../harness_architecture/03-hook-and-switches.md) §5 与 [`references/AGENTS_OPERATOR_SURFACE.md`](../references/AGENTS_OPERATOR_SURFACE.md)。**Cursor review-routing regexes** ship as **`include_str!("…/REVIEW_ROUTING_SIGNALS.json")`** in `review_routing_signals.rs` (build-time snapshot; changing the JSON on disk alone does not change hook behavior until `router-rs` is rebuilt). **`router_rs_observation`** (`core/runtime-core/src/router_rs_observation.rs`) labels outbound hook JSON with **`cursor` / `codex` / `claude-code`**; `router-rs claude hook` attach it after shared stdio-agent hook dispatch; MCP 宿主 JSON-RPC **不** attach。各宿主保持独立 state 目录、env 名、gate token 与投影路径。 **`host_projections.*.capabilities`** today mixes product-facing affordances (e.g. MCP, supervisor) with harness expectations; **`host_projections.*.harness_capabilities`** is the explicit harness-semantics slice (routing/continuity/closeout/review-gate observation, etc.) and must not duplicate product-only tokens such as **`mcp_servers`**—see [`host_adapter_contract.md`](../host_adapter_contract.md) **§0** / **§3.2** and `tests/policy_contracts.rs` **`runtime_registry_host_projections_split_harness_capabilities`**. Treat absent product keys as "not claimed for that host" rather than silently assuming cross-host parity. Rust contracts below remain the implementation authority.

## Current Boundary

Rust owns the default runtime and contract path.

- `router-rs route <query>` owns route decisions; route diagnostics use the Rust stdio route policy/report operations.
- `router-rs profile emit` and `router-rs profile artifacts` own the shared framework profile plus explicit Codex projection artifacts.
- Rust stdio `execute` operation owns the live/dry-run execution response contract.
- `router-rs framework doctor`（人读路径/钩子文件/同步提示）、`router-rs framework snapshot`, `contract-summary`, `session-artifact-write`, `hook-evidence-append`, and `prompt-compression` own framework runtime read/write/policy surfaces. Cursor `PostToolUse` normalizes stdin via `hook_posttool_normalize::synthetic_post_tool_evidence_shape` before append and may emit `cursor_post_tool_verification` rows (terminal tools + verification-shaped commands) alongside Codex `codex_post_tool_verification` and `rust-lint`'s `cursor_rust_lint` hook evidence.
- Cursor `review_gate` / Codex **`codex hook`** / **`router-rs claude hook`** 出站 JSON 可含顶层 **`router_rs_observation`**（`core/runtime-core/src/router_rs_observation.rs`；hook 宿主 id：`cursor` / `codex` / `claude-code`）。MCP 宿主 JSON-RPC **不** attach `router_rs_observation`。
- Stdio op `framework_hook_evidence_append` mirrors `router-rs framework hook-evidence-append --input-json …` for scripted callers appending rows to `EVIDENCE_INDEX.json` under continuity (same payload shape as the CLI).
- `router-rs codex sync` and **`router-rs framework sync-entrypoints`** remain compatible CLIs for repo host-entrypoint materialization; internally, `host_entrypoint_sync` is the shared sync engine and `codex_hooks` supplies the Codex provider for `.codex/hooks.json`, **`AGENTS_CODEX.md`**, `.codex/README.md`, and Codex skill surface refresh. Hooks compile-time embed **`AGENTS.md` + `AGENTS_CODEX.md`**; sync does **not** overwrite repo-root `AGENTS.md`. Full sync applies to the current root; matched sibling worktrees receive JSON hook/manifest updates only, so local policy text entrypoints are not overwritten across worktrees.
- `router-rs framework host-integration ...` owns native install/status/remove, bootstrap, projection, and related host integration flows. `router-rs codex host-integration ...` is a thin compatibility alias only.
- **`router-rs schema-drift {contract,baseline,check}`** (`core/runtime-core/src/schema_drift.rs`) captures per-task baselines under `artifacts/current/<task_id>/SCHEMA_DRIFT_BASELINE.json`. Required/forbidden Cursor hook event sets are shared with [`cursor_hooks/subtraction.rs`](../../core/runtime-core/src/hosts/cursor_hooks/subtraction.rs) and [`check-cursor-hooks-parity.sh`](../../scripts/ci/check-cursor-hooks-parity.sh) (parity script loads lists via `schema-drift contract`). Check compares hooks.json vs workspace template (commands/timeouts), gate timeout table, REQUIREMENTS↔ROADMAP `##`/`###` heading fingerprints, and `EVIDENCE_INDEX.artifacts[]` when present.

## Contract Rules

- Contract changes must be explicit and versioned.
- Rust may replace implementations, not silently redefine semantics.
- Host-private fields stay under explicit host projection payloads such as `codex_profile.codex_host_payload`; they must not enter framework core truth.
- Active contracts must describe current owners and outputs, not migration inventory.
- Any alternate runtime, routing, artifact, hook, or host-integration implementation is a regression unless explicitly approved as a host-private edge script.
