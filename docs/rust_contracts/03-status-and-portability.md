---
last_verified: "2026-06-02"
depends_on:
  - ../harness_architecture/index.md
---

# 状态账本与可移植性 (Status Ledger & Portability)

[返回索引](index.md)

## Current Status Ledger

### 当前真源

- Routing authority is Rust.
- **Hook observation gate 分类**真源为 [`configs/framework/ROUTER_RS_HOOK_OBSERVATION_RULES.json`](../../configs/framework/ROUTER_RS_HOOK_OBSERVATION_RULES.json)（`schema_version`: **`router-rs-hook-observation-rules-v1`**）；由 `core/router-rs/src/hook_observation_rules.rs` **`include_str!`** 编译期嵌入 `router-rs`，驱动 `router_rs_observation` 对 `followup_message` / `additional_context` 的匹配顺序与 `router-rs <token>` → `gate.code` 映射。仅改工作区 JSON 不重建二进制则 hook 行为不变。
- **路由启发式切片（第一批）**真源为 [`configs/framework/ROUTING_SIGNAL_MARKERS.json`](../../configs/framework/ROUTING_SIGNAL_MARKERS.json)（`schema_version`: **`routing-signal-markers-v1`**）；`core/router-rs/src/route/signals.rs` 以 **`include_str!`** 嵌入，供给 **`is_meta_routing_task`**、`build_route_context` 使用的 completion / supervisor marker 字符串表。契约去重见根 `tests/policy_contracts.rs` **`routing_signal_markers_json_unique_nonempty_lists`**。
- **NL 热路由 per-record suppress/boost** 真源为 [`configs/framework/NL_ROUTE_ADJUSTMENTS.json`](../../configs/framework/NL_ROUTE_ADJUSTMENTS.json)（`schema_version`: **`nl-route-adjustments-v1`**）；由 `core/router-rs/src/route/nl_route_adjustments.rs` **`include_str!`** 嵌入，在 `score_route_candidate` 中与 `ROUTING_SIGNAL_MARKERS` 分层使用（前者：按 skill 记录的条件动作；后者：跨查询短语 marker）。
- **`skills/SKILL_PLUGIN_CATALOG.json`** 中 `skills.<slug>.host_support.platforms` 由 **`router-rs framework skills refresh`** 从各 **`skills/<slug>/SKILL.md`** 的 `platforms` / `metadata.platforms` 生成并归一到闭集宿主 id；**不要**手改 JSON 作为宿主列表真源。契约测试：`tests/policy_contracts.rs` 的 **`runtime_host_support_platforms_are_registry_closed_and_match_skill_md`**。
- Live execution and dry-run preview use Rust stdio.
- Runtime control plane publishes Rust-owned authority for `router`, `state`, `trace`, storage, and `background`.
- Framework snapshot, contract summary, session artifact writing, hook evidence append (CLI + stdio), and prompt policy use direct `router-rs` surfaces.
- Host entrypoint sync and native integration are Rust-owned through `router-rs`; the **closed-set supported hosts** are defined by **`configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported`** (install-skills/tool spellings derive from `framework_host_targets` in router-rs). `host_projections` is the profile/projection payload set, `framework_commands.*.host_entrypoints` is the explicit command-entrypoint set, and `SKILL_PLUGIN_CATALOG.json` `skills.<slug>.host_support.platforms` is the skill-body support set. Older docs and onboarding examples sometimes mention `codex-cli`（已退役，canonical **`codex`**）; **that is not an alternate host-id enumeration**—the authoritative closed-set ids are **only** whatever appears under `host_targets.supported` in the checked-in registry JSON.
- `HostProjectionAdapter` remains the thin Rust adapter table for projection install/status/remove side effects; the registry still owns the closed host ids and install-tool spellings.
- Runtime traces expose resumable `seq` / `cursor` metadata, transport binding artifacts, handoff descriptors, and process-external attach resolution.
- Harness trajectory diagnostics reuse `TRACE_EVENTS.jsonl`; `router-rs eval harness-contract` defines the required payload convention and failure taxonomy, while `router-rs framework step-ledger` owns task-scoped `STEP_LEDGER.jsonl` append/summary for long-task recovery.
- Runtime storage exposes backend-family capability discovery, digest verification, and fail-closed alignment between store/checkpointer/trace/state families.
- SQLite is the strongest local backend for WAL, consistent append, compaction, and snapshot-delta support; filesystem remains the safe default storage.
- Session supervisor and background state expose Rust-owned PID/session/rate-limit/resume control-plane records without external runtime dependency (P8 de-tmux).
- Observability vocabulary, exporter descriptor, metric catalog, dashboard schema, and metric record payloads are Rust-owned.
- Sandbox lifecycle contract is frozen and has a minimal Rust-owned control-plane surface.

### 默认面边界

- Do not add a second route authority, default artifact emitter, host-specific generated layer, or parallel runtime state root.
- Generated host entrypoints are limited sync outputs, not hand-authored truth.
- Historical migration inventory belongs in `MIGRATION.md` / git history, not in steady-state contracts.

### 下一 safe slice

- Harden remote-capable attach/handoff/binding/replay semantics so every consumer uses the same descriptor contract.
- Deepen backend-family compaction and snapshot-delta behavior without changing logical state meaning.
- Expand sandbox lifecycle enforcement without claiming a remote sandbox backend before it exists.
- Keep host integration Rust-only and fail if generated entrypoints drift.
- Refresh docs and generated routing outputs only when contract changes require it.

## Portability and environment (`router-rs`)

- **Non-Unix**: Hook helpers that depend on POSIX process semantics (for example lock staleness or `kill(pid, 0)`) use conservative defaults under `cfg(not(unix))` so builds stay green; behavior may differ from Linux/macOS until those paths are specialized.
- **`libc` and `unsafe`**: Codex/Cursor hooks use narrow `unsafe` blocks for `flock`, `kill`, and related syscalls. Call sites are responsible for invariants; errors surface as structured hook outcomes, not panics, except where tests explicitly exercise failure injection.
- **`ROUTER_RS_*` flags**: Parsing, default-on/default-off policy, and naming for environment toggles should stay in [`core/router-rs/src/router_env_flags.rs`](../../core/router-rs/src/router_env_flags.rs) so new flags do not sprawl across the crate.
- **Browser MCP**: Browser MCP stdio in this repo is the Rust implementation (`core/router-rs/src/browser_mcp/` and CLI wiring). The TypeScript package `tools/browser-mcp/` has been retired; Rust is the sole product path.

## External Benchmark

DeerFlow 2.0 remains a useful benchmark for decomposition ideas:

- harness/app split
- explicit run-manager conflict semantics
- resumable stream bridge
- unified store/checkpointer seams
- sandbox lifecycle boundaries

It is not a template to copy directly. This repo keeps its own Rust-owned state
machine and avoids LangGraph-shaped or reflection-heavy runtime assumptions.
